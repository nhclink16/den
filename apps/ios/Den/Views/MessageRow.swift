import DenAPI
import SwiftUI
import UIKit

struct MessageRow: View {
    let message: API.Message
    let grouped: Bool
    let store: AppStore
    let onReply: () -> Void
    var thread: API.ThreadSummary?

    init(message: API.Message, grouped: Bool, store: AppStore,
         thread: API.ThreadSummary? = nil, onReply: @escaping () -> Void) {
        self.message = message; self.grouped = grouped; self.store = store
        self.thread = thread; self.onReply = onReply
    }
    @State private var parent: API.Message?
    @State private var parentUnavailable = false
    @State private var showReactions = false
    @State private var showEdit = false
    @State private var showDelete = false
    @State private var editDraft = ""
    @State private var savingEdit = false
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize
    private var theme: DenTheme { store.theme.resolve(colorScheme) }
    private var own: Bool { message.authorId == store.user?.id }
    private let quickReactions = ["👍", "😂", "❤️", "🔥", "👀", "💀"]

    var body: some View {
        VStack(alignment: .leading, spacing: 5) {
            if message.replyTo != nil { replyReference }
            HStack(alignment: .top, spacing: 10) {
                if grouped { Color.clear.frame(width: 36, height: 1).accessibilityHidden(true) }
                else { PersonAvatar(userId: message.authorId, store: store) }
                VStack(alignment: .leading, spacing: 5) {
                    if !grouped { authorLine }
                    if !message.content.isEmpty { MarkdownMessageText(content: message.content, store: store) }
                    if message.editedAt != nil {
                        Text("edited").font(theme.monoFont(.caption2)).foregroundStyle(theme.ink3)
                    }
                    ForEach(message.objects ?? [], id: \.id) { object in
                        HStack(alignment: .top, spacing: 10) {
                            Image(systemName: object.kind == "terminal" ? "terminal" : "scribble.variable")
                                .foregroundStyle(theme.accent).padding(.top, 2)
                            VStack(alignment: .leading, spacing: 4) {
                                Text(object.name).fontWeight(.medium)
                                Text("Open this \(object.kind == "terminal" ? "terminal" : "canvas") on desktop.")
                                    .font(theme.bodyFont(.caption)).foregroundStyle(theme.ink2)
                            }
                            Spacer(minLength: 0)
                        }.padding(12).background(theme.bg2, in: RoundedRectangle(cornerRadius: theme.radius))
                    }
                    ForEach(message.attachments, id: \.id) { upload in
                        AuthenticatedAttachmentView(upload: upload, store: store)
                    }
                    if let thread { threadButton(thread) }
                    if !(message.reactions ?? []).isEmpty { reactions }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .padding(.horizontal, 16).padding(.top, grouped ? 3 : 12 * theme.density).padding(.bottom, 3)
        .background((message.mentionIds ?? []).contains(store.user?.id ?? "") ? theme.accentGlow : Color.clear)
        .contentShape(Rectangle())
        .contextMenu {
            Button("Reply", systemImage: "arrowshape.turn.up.left", action: onReply)
            Button("React", systemImage: "face.smiling") { showReactions = true }
            Button("Copy", systemImage: "doc.on.doc") { UIPasteboard.general.string = message.content }
            if own { Button("Edit", systemImage: "pencil") { editDraft = message.content; showEdit = true } }
            if own || store.user?.role == .admin {
                Button("Delete", systemImage: "trash", role: .destructive) { showDelete = true }
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Message from \(store.userName(message.authorId))")
        .accessibilityIdentifier("message-\(message.id)")
        .accessibilityAction(named: "Reply", onReply)
        .accessibilityAction(named: "React") { showReactions = true }
        .confirmationDialog("React to message", isPresented: $showReactions, titleVisibility: .visible) {
            ForEach(quickReactions, id: \.self) { emoji in Button(emoji) { react(emoji) } }
            Button("Cancel", role: .cancel) { }
        }
        .confirmationDialog("Delete this message?", isPresented: $showDelete, titleVisibility: .visible) {
            Button("Delete message", role: .destructive) {
                Task { do { try await store.delete(message: message) } catch { store.report(error) } }
            }
        } message: { Text("This removes it for everyone.") }
        .sheet(isPresented: $showEdit) { editSheet }
        .task(id: message.replyTo) {
            guard let id = message.replyTo else { return }
            parentUnavailable = false
            if let cached = store.messages[message.channelId]?.first(where: { $0.id == id }) { parent = cached }
            else {
                do { parent = try await store.fetchMessage(id: id, channelId: message.channelId) }
                catch { parentUnavailable = true }
            }
        }
    }

    private var authorLine: some View {
        Group {
            if dynamicTypeSize.isAccessibilitySize {
                VStack(alignment: .leading, spacing: 2) { authorName; timestamp }
            } else {
                ViewThatFits(in: .horizontal) {
                    HStack(alignment: .firstTextBaseline, spacing: 8) { authorName; timestamp }
                    VStack(alignment: .leading, spacing: 2) { authorName; timestamp }
                }
            }
        }
    }

    private var authorName: some View {
        HStack(spacing: 5) {
            Text(store.userName(message.authorId)).font(theme.bodyFont().weight(.semibold)).foregroundStyle(theme.ink)
            if store.users.first(where: { $0.id == message.authorId })?.bot == true {
                Image(systemName: "sparkles").font(.caption).foregroundStyle(theme.accent).accessibilityLabel("Agent")
            }
        }
    }

    private var timestamp: some View {
        Group {
            if let date = MessagePresentation.date(message.createdAt) {
                Text(date, format: .dateTime.hour().minute()).font(theme.monoFont(.caption2)).foregroundStyle(theme.ink3)
            }
        }
    }

    private var replyReference: some View {
        Button {
            if let id = message.replyTo { store.selectChannel(message.channelId, messageId: id) }
        } label: {
            HStack(spacing: 5) {
                Image(systemName: "arrowshape.turn.up.left")
                if let parent {
                    Text(store.userName(parent.authorId)).fontWeight(.semibold)
                    Text(parent.content.isEmpty ? "Sent an attachment" : parent.content).lineLimit(1)
                } else {
                    Text(parentUnavailable ? "Original message unavailable" : "Loading reply…")
                }
            }
            .font(theme.bodyFont(.caption)).foregroundStyle(theme.ink2)
            .frame(minHeight: 44, alignment: .leading)
            .padding(.leading, 46)
        }
        .buttonStyle(.plain).disabled(parentUnavailable)
        .accessibilityLabel("Open original message")
    }

    private var reactions: some View {
        ScrollView(.horizontal) { HStack(spacing: 6) { reactionButtons } }
            .scrollIndicators(.hidden)
    }

    private func threadButton(_ thread: API.ThreadSummary) -> some View {
        Button(action: onReply) {
            HStack(spacing: 7) {
                Image(systemName: "bubble.left.and.bubble.right")
                Text(thread.title).lineLimit(1)
                Spacer(minLength: 4)
                Text("\(thread.replyCount)").font(theme.monoFont(.caption))
                if let state = store.threadReadStates[thread.id], state.unreadCount > 0 {
                    Text(state.mentionCount > 0 ? "@" : "\(state.unreadCount)")
                        .font(theme.monoFont(.caption).weight(.semibold))
                        .foregroundStyle(theme.accent)
                }
            }
            .font(theme.bodyFont(.caption)).foregroundStyle(theme.ink2)
            .frame(minHeight: 44).padding(.horizontal, 10)
            .background(theme.bg2, in: RoundedRectangle(cornerRadius: theme.radius))
        }
        .buttonStyle(.plain)
        .accessibilityLabel("Open conversation \(thread.title), \(thread.replyCount) replies")
        .accessibilityIdentifier("thread-root-\(thread.id)")
    }

    private var reactionButtons: some View {
        ForEach(message.reactions ?? [], id: \.emoji) { reaction in
            let selected = reaction.userIds.contains(store.user?.id ?? "")
            Button { react(reaction.emoji) } label: {
                HStack(spacing: 5) {
                    Text(reaction.emoji)
                    Text("\(reaction.userIds.count)").font(theme.monoFont(.caption))
                }
                .padding(.horizontal, 10).frame(minHeight: 44)
                .background(selected ? theme.accentGlow : theme.bg2, in: Capsule())
                .overlay(Capsule().stroke(selected ? theme.accentDim : theme.line, lineWidth: 1))
            }
            .buttonStyle(.plain)
            .padding(.vertical, 4)
            .accessibilityLabel("\(reaction.emoji), \(reaction.userIds.count) reactions, \(reaction.userIds.map(store.userName).joined(separator: ", "))")
            .accessibilityHint(selected ? "Remove your reaction" : "Add your reaction")
            .accessibilityIdentifier("reaction-\(message.id)-\(reaction.emoji)")
            .accessibilityAddTraits(selected ? [.isSelected] : [])
        }
    }

    private func react(_ emoji: String) {
        Task {
            do {
                try await store.react(message: message, emoji: emoji)
                UIImpactFeedbackGenerator(style: .light).impactOccurred()
            } catch { store.report(error) }
        }
    }

    private var editSheet: some View {
        NavigationStack {
            TextEditor(text: $editDraft).font(theme.bodyFont()).padding(12).disabled(savingEdit)
                .scrollContentBackground(.hidden).background(theme.bg)
                .accessibilityLabel("Edit message").accessibilityIdentifier("edit-message-field")
                .navigationTitle("Edit message").navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .cancellationAction) { Button("Cancel") { showEdit = false } }
                    ToolbarItem(placement: .confirmationAction) {
                        Button(savingEdit ? "Saving…" : "Save") {
                            savingEdit = true
                            Task {
                                defer { savingEdit = false }
                                do { try await store.edit(message: message, content: editDraft); showEdit = false }
                                catch { store.report(error) }
                            }
                        }.disabled(savingEdit || editDraft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    }
                }
        }
    }
}
