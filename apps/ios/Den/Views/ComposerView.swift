import CoreTransferable
import DenAPI
import PhotosUI
import SwiftUI
import UniformTypeIdentifiers
import UIKit

/// Photos may vend a movie far larger than memory. Copy the provided file while its lifetime is valid.
private struct PickedMedia: Transferable, Sendable {
    let url: URL

    static var transferRepresentation: some TransferRepresentation {
        FileRepresentation(importedContentType: .movie) { received in try copy(received.file) }
        FileRepresentation(importedContentType: .image) { received in try copy(received.file) }
    }

    private static func copy(_ source: URL) throws -> PickedMedia {
        let folder = FileManager.default.temporaryDirectory.appending(path: "den-photo-\(UUID().uuidString)", directoryHint: .isDirectory)
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        let destination = folder.appending(path: source.lastPathComponent)
        try FileManager.default.copyItem(at: source, to: destination)
        return PickedMedia(url: destination)
    }
}

struct ComposerView: View {
    let channel: API.Channel
    let store: AppStore
    @Binding var reply: API.Message?
    let onSent: () -> Void
    @State private var draft = ""
    @State private var sending = false
    @State private var importing = false
    @State private var showPhotos = false
    @State private var showFiles = false
    @State private var photos: [PhotosPickerItem] = []
    @State private var focused = false
    @State private var selection = NSRange(location: 0, length: 0)
    @State private var dictationInsertion: DictationInsertion?
    @State private var dictationID: UUID?
    @State private var dictationStart: Task<Void, Never>?
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    private var theme: DenTheme { store.theme.resolve(colorScheme) }
    private var uploads: [PendingUpload] { store.pendingUploads.filter { $0.channelId == channel.id } }
    private var canSend: Bool {
        !sending && !importing && !store.offline && uploads.allSatisfy { $0.upload?.complete == true && $0.error == nil }
            && (!draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || !uploads.isEmpty)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if let reply {
                HStack(spacing: 8) {
                    Image(systemName: "arrowshape.turn.up.left").foregroundStyle(theme.accent)
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Replying to \(store.userName(reply.authorId))").fontWeight(.semibold)
                        Text(reply.content.isEmpty ? "Attachment" : reply.content).lineLimit(1).foregroundStyle(theme.ink2)
                    }.font(theme.bodyFont(.caption))
                    Spacer()
                    Button { stopDictation(); self.reply = nil } label: { Image(systemName: "xmark").frame(width: 44, height: 44) }
                        .accessibilityLabel("Cancel reply")
                }
            }
            if !uploads.isEmpty {
                ScrollView(.horizontal) {
                    HStack(spacing: 8) { ForEach(uploads, id: \.id) { upload in uploadChip(upload) } }
                }.scrollIndicators(.hidden)
            }
            if importing {
                HStack { ProgressView(); Text("Preparing attachment…").font(theme.bodyFont(.caption)) }
                    .foregroundStyle(theme.ink2)
            }
            HStack(alignment: .bottom, spacing: 4) {
                Menu {
                    Button("Photos and videos", systemImage: "photo.on.rectangle") { stopDictation(); showPhotos = true }
                    Button("Choose a file", systemImage: "folder") { stopDictation(); showFiles = true }
                } label: {
                    Image(systemName: "plus").font(.body.weight(.medium)).frame(width: 44, height: 44)
                }
                .buttonStyle(.plain).foregroundStyle(theme.ink2)
                .accessibilityLabel("Add an attachment").accessibilityIdentifier("composer-attach")
                .disabled(importing || store.offline)
                .simultaneousGesture(TapGesture().onEnded { stopDictation() })
                MessageInput(text: $draft, focused: $focused, selection: $selection, theme: theme,
                             onSend: send, onManualChange: cancelDictation, onEscape: stopDictation)
                    .overlay(alignment: .topLeading) {
                        if draft.isEmpty {
                            Text("Message \(store.channelTitle(channel))")
                                .font(theme.bodyFont()).foregroundStyle(theme.ink3).lineLimit(1)
                                .padding(.top, 11).allowsHitTesting(false).accessibilityHidden(true)
                        }
                    }
                    .onChange(of: draft) { _, _ in store.sendTyping(channelId: channel.id) }
                if let dictation = store.dictation, dictation.isAvailable {
                    dictationButton(dictation)
                }
                Button(action: send) {
                    Group {
                        if sending { ProgressView().tint(theme.bg) }
                        else { Image(systemName: "arrow.up").font(.system(size: 16, weight: .semibold)) }
                    }
                    .frame(width: 32, height: 32)
                    .foregroundStyle(canSend ? theme.bg : theme.ink3)
                    .background(canSend ? theme.accent : theme.bg3, in: RoundedRectangle(cornerRadius: 9))
                    .frame(width: 44, height: 44).contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .disabled(!canSend).accessibilityLabel("Send message").accessibilityIdentifier("composer-send")
            }
            .padding(4)
            .background(theme.bg2, in: RoundedRectangle(cornerRadius: theme.radius))
            .overlay { RoundedRectangle(cornerRadius: theme.radius).stroke(theme.line, lineWidth: 1).allowsHitTesting(false) }
        }
        .padding(.horizontal, 12).padding(.vertical, 10 * theme.density)
        .task { await store.dictation?.refresh() }
        .onDisappear { cancelDictation(); focused = false }
        .onChange(of: channel.id) { _, _ in cancelDictation() }
        .onChange(of: focused) { _, value in
            // Permission sheets can end editing before scenePhase becomes inactive.
            // Explicit Stop, outside taps, navigation and real background still cancel.
            if !value, store.dictation?.state != .preparing,
               UIApplication.shared.applicationState == .active { stopDictation() }
        }
        .onChange(of: store.dictation?.state) { _, value in
            if value == .idle { releaseDictation() }
        }
        .alert("Dictation", isPresented: Binding(get: { store.dictation?.error != nil }, set: { visible in
            if !visible { store.dictation?.clearError() }
        })) {
            Button("OK") { store.dictation?.clearError() }
        } message: { Text(store.dictation?.error ?? "") }
        .photosPicker(isPresented: $showPhotos, selection: $photos, maxSelectionCount: 10, matching: .any(of: [.images, .videos]), preferredItemEncoding: .current)
        .onChange(of: photos) { _, value in
            guard !value.isEmpty else { return }
            Task { await importPhotos(value) }
        }
        .fileImporter(isPresented: $showFiles, allowedContentTypes: [.item], allowsMultipleSelection: true) { result in
            Task {
                do {
                    let urls = try result.get()
                    importing = true
                    defer { importing = false }
                    for url in urls {
                        let access = url.startAccessingSecurityScopedResource()
                        defer { if access { url.stopAccessingSecurityScopedResource() } }
                        try await store.attach(url: url, channelId: channel.id)
                    }
                } catch { store.report(error) }
            }
        }
        .onChange(of: reply?.id) { _, value in if value != nil { focused = true } }
    }

    private func dictationButton(_ dictation: DictationController) -> some View {
        let listening = dictation.state == .listening
        let active = dictationID != nil || dictation.isActive
        return Button {
            if active { stopDictation() }
            else { startDictation(dictation) }
        } label: {
            ZStack {
                if listening {
                    DictationPulse(reduceMotion: reduceMotion, color: theme.accent)
                        .frame(width: 30, height: 30).accessibilityHidden(true)
                }
                if dictation.state == .preparing || dictation.state == .finishing || (active && dictation.state == .idle) {
                    ProgressView().tint(theme.accent)
                } else {
                    Image(systemName: "mic").font(.body.weight(.medium))
                }
            }.frame(width: 44, height: 44).contentShape(Rectangle())
        }
        .buttonStyle(.plain).foregroundStyle(active ? theme.accent : theme.ink2)
        .disabled(sending || importing)
        .accessibilityLabel(active ? "Stop dictating" : "Dictate")
        .accessibilityValue(dictation.state == .preparing ? "Preparing" :
                            dictation.state == .finishing ? "Finishing" : listening ? "Listening" : "")
        .accessibilityIdentifier("composer-dictate")
    }

    private func startDictation(_ controller: DictationController) {
        guard !sending, !importing, controller.isAvailable,
              let insertion = DictationInsertion(text: draft, selection: selection) else { return }
        let id = UUID()
        let contextRevision = controller.contextRevision
        dictationInsertion = insertion; dictationID = id
        store.dictationChannelId = channel.id
        focused = true
        dictationStart = Task {
            guard !Task.isCancelled, dictationID == id, controller.contextRevision == contextRevision else {
                if dictationID == id { releaseDictation() }
                return
            }
            await controller.start { transcript in
                guard dictationID == id, store.dictationChannelId == channel.id,
                      var insertion = dictationInsertion else { return }
                guard let update = insertion.update(transcript: transcript, currentText: draft) else {
                    cancelDictation(); return
                }
                dictationInsertion = insertion
                draft = update.text; selection = update.selection
            }
            if controller.state == .idle, dictationID == id { releaseDictation() }
        }
    }

    private func stopDictation() {
        guard dictationID != nil, store.dictationChannelId == channel.id else { return }
        if store.dictation?.state == .idle || store.dictation?.state == .preparing {
            cancelDictation(); return
        }
        store.dictation?.stop()
    }

    private func cancelDictation() {
        guard dictationID != nil else { return }
        if store.dictationChannelId == channel.id { store.dictation?.invalidateContext() }
        releaseDictation()
    }

    private func releaseDictation() {
        guard dictationID != nil else { return }
        dictationStart?.cancel(); dictationStart = nil
        dictationID = nil; dictationInsertion = nil
        if store.dictationChannelId == channel.id { store.dictationChannelId = nil }
    }

    private func uploadChip(_ upload: PendingUpload) -> some View {
        HStack(spacing: 8) {
            ZStack {
                Circle().stroke(theme.line, lineWidth: 3)
                if upload.upload?.complete == true { Image(systemName: "checkmark").font(.caption.weight(.semibold)) }
                else if upload.error != nil { Image(systemName: "exclamationmark").font(.caption.weight(.semibold)) }
                else {
                    Circle().trim(from: 0, to: max(0, min(1, upload.progress)))
                        .stroke(theme.accent, style: StrokeStyle(lineWidth: 3, lineCap: .round)).rotationEffect(.degrees(-90))
                }
            }.frame(width: 24, height: 24)
            VStack(alignment: .leading, spacing: 3) {
                Text(upload.filename).lineLimit(1)
                if let error = upload.error {
                    Text(error).foregroundStyle(theme.danger).lineLimit(2)
                    Button("Retry") {
                        Task { do { try await store.retryUpload(id: upload.id) } catch { store.report(error) } }
                    }.frame(minHeight: 44).accessibilityLabel("Retry uploading \(upload.filename)")
                }
                else if upload.upload?.complete != true { Text("\(Int(upload.progress * 100))% uploaded").foregroundStyle(theme.ink3) }
            }.font(theme.bodyFont(.caption)).frame(maxWidth: 180, alignment: .leading)
            Button {
                Task { do { try await store.removeUpload(id: upload.id) } catch { store.report(error) } }
            } label: { Image(systemName: "xmark").frame(width: 44, height: 44) }
                .accessibilityLabel("Remove \(upload.filename)")
        }
        .padding(.leading, 10).background(theme.bg2, in: RoundedRectangle(cornerRadius: theme.radius))
        .accessibilityElement(children: .contain)
    }

    private func importPhotos(_ items: [PhotosPickerItem]) async {
        importing = true
        defer { importing = false; photos = [] }
        for item in items {
            do {
                guard let media = try await item.loadTransferable(type: PickedMedia.self) else {
                    throw CocoaError(.fileReadUnknown)
                }
                defer { try? FileManager.default.removeItem(at: media.url.deletingLastPathComponent()) }
                try await store.attach(url: media.url, channelId: channel.id)
            } catch { store.report(error) }
        }
    }

    private func send() {
        cancelDictation()
        guard canSend else { return }
        sending = true
        let content = draft
        let replyId = reply?.id
        Task {
            defer { sending = false }
            do {
                try await store.send(channelId: channel.id, content: content, replyTo: replyId)
                if draft == content { draft = ""; selection = NSRange(location: 0, length: 0) }
                if reply?.id == replyId { reply = nil }
                UIImpactFeedbackGenerator(style: .light).impactOccurred()
                onSent()
            } catch { store.report(error) }
        }
    }
}

private struct DictationPulse: View {
    let reduceMotion: Bool
    let color: Color
    @State private var expanded = false

    var body: some View {
        Circle().stroke(color.opacity(expanded && !reduceMotion ? 0.15 : 0.5), lineWidth: 1.5)
            .scaleEffect(expanded && !reduceMotion ? 1.12 : 0.9)
            .onAppear { expanded = !reduceMotion }
            .onChange(of: reduceMotion) { _, value in expanded = !value }
            .animation(reduceMotion ? nil : .easeInOut(duration: 1.2).repeatForever(autoreverses: true), value: expanded)
    }
}
