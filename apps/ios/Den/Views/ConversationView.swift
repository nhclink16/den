import DenAPI
import LiveKit
import SwiftUI

struct ConversationView: View {
    let channel: API.Channel
    let store: AppStore
    @State private var reply: API.Message?
    @State private var openedAt: String?
    @State private var capturedReadMarker = false
    @State private var initialPositionSet = false
    @State private var loading = false
    @State private var loadingOlder = false
    @State private var loadingNewer = false
    @State private var exhausted = false
    @State private var nearBottom = true
    @State private var showPeople = false
    @State private var visible = false
    @State private var startingCall = false
    @Environment(\.colorScheme) private var colorScheme
    @Environment(\.scenePhase) private var scenePhase
    private var theme: DenTheme { store.theme.resolve(colorScheme) }
    private var messages: [API.Message] { store.messages[channel.id] ?? [] }
    private var firstUnread: String? {
        guard capturedReadMarker else { return nil }
        return messages.first { $0.id > (openedAt ?? "") && $0.authorId != store.user?.id }?.id
    }

    var body: some View {
        Group {
            if channel.kind == .voice { voiceRoom }
            else { timeline }
        }
        .background(theme.bg)
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .principal) {
                Text(store.channelTitle(channel)).font(theme.displayFont(.headline)).lineLimit(1)
                    .accessibilityAddTraits(.isHeader)
            }
            ToolbarItem(placement: .topBarTrailing) {
                Button { showPeople = true } label: { Image(systemName: "person.2") }
                    .accessibilityLabel("People in this room")
            }
            if channel.kind == .dm {
                ToolbarItem(placement: .topBarTrailing) {
                    Button(action: joinCall) { Image(systemName: "phone") }
                        .accessibilityLabel(hasParticipants ? "Join call" : "Call this conversation")
                        .accessibilityIdentifier("conversation-call")
                        .disabled(startingCall || store.offline || store.calls?.currentCallID != nil)
                }
            }
        }
        .sheet(isPresented: $showPeople) { peopleSheet }
        .onAppear { visible = true; if nearBottom { markTailRead() } }
        .onDisappear { visible = false }
    }

    private var timeline: some View {
        // Lazy rows may outlive a store update or a channel transition. Their
        // neighboring message must come from the same immutable render snapshot.
        let messages = self.messages
        return ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    if store.offline { QuietOfflineChip().padding(.horizontal, 16).padding(.bottom, 8) }
                    if !messages.isEmpty && !exhausted {
                        Button {
                            guard !loadingOlder, let first = messages.first else { return }
                            loadingOlder = true
                            let count = messages.count
                            Task {
                                defer { loadingOlder = false }
                                do {
                                    try await store.loadMessages(channelId: channel.id, before: first.id)
                                    exhausted = messages.count == count
                                    proxy.scrollTo(first.id, anchor: .top)
                                } catch { store.report(error) }
                            }
                        } label: {
                            HStack {
                                Spacer()
                                if loadingOlder { ProgressView() }
                                Text(loadingOlder ? "Loading earlier messages…" : "Load earlier messages")
                                Spacer()
                            }.frame(minHeight: 44)
                        }.disabled(loadingOlder || store.offline).font(theme.bodyFont(.footnote))
                    }
                    if loading && messages.isEmpty { ProgressView().frame(maxWidth: .infinity).padding(40) }
                    if !loading && messages.isEmpty {
                        ContentUnavailableView("This is where it starts", systemImage: "bubble.left", description: Text("Say the first thing in \(store.channelTitle(channel))."))
                    }
                    ForEach(Array(messages.enumerated()), id: \.element.id) { index, message in
                        let prior = index == 0 ? nil : messages[index - 1]
                        VStack(alignment: .leading, spacing: 0) {
                            if MessagePresentation.startsDay(message, after: prior) { daySeparator(message.createdAt) }
                            if message.id == firstUnread { newSeparator }
                            MessageRow(message: message, grouped: MessagePresentation.groups(message, after: prior), store: store) { reply = message }
                        }
                        .id(message.id)
                        .background(message.id == store.targetMessageId ? theme.accentGlow : Color.clear)
                    }
                    if store.messageHasNewer[channel.id] == true {
                        Button {
                            guard !loadingNewer, let last = messages.last else { return }
                            loadingNewer = true
                            nearBottom = false
                            Task {
                                defer { loadingNewer = false }
                                do {
                                    try await store.loadNewerMessages(channelId: channel.id)
                                    proxy.scrollTo(last.id, anchor: .top)
                                } catch { store.report(error) }
                            }
                        } label: {
                            HStack {
                                Spacer()
                                if loadingNewer { ProgressView() }
                                Text(loadingNewer ? "Loading newer messages…" : "Load newer messages")
                                Spacer()
                            }.frame(minHeight: 44)
                        }
                        .disabled(loadingNewer || store.offline)
                        .font(theme.bodyFont(.footnote))
                        .accessibilityIdentifier("load-newer-messages")
                    }
                    Color.clear.frame(height: 1).id("timeline-bottom")
                }
                .padding(.vertical, 12)
            }
            .defaultScrollAnchor(.bottom)
            .scrollDismissesKeyboard(.interactively)
            .onScrollGeometryChange(for: Bool.self) { geometry in
                geometry.contentSize.height - geometry.visibleRect.maxY < 100
            } action: { _, value in
                nearBottom = value
                if value { markTailRead() }
            }
            .refreshable {
                do { try await store.loadMessages(channelId: channel.id) } catch { store.report(error) }
            }
            .safeAreaInset(edge: .bottom, spacing: 0) {
                VStack(spacing: 0) {
                    typingIndicator
                    ComposerView(channel: channel, store: store, reply: $reply) {
                        proxy.scrollTo("timeline-bottom", anchor: .bottom)
                    }
                }.background(theme.bg)
            }
            .task(id: channel.id + ":" + (store.targetMessageId ?? "")) {
                if !capturedReadMarker {
                    openedAt = store.readState(channel.id)?.lastReadId
                    capturedReadMarker = true
                }
                loading = true
                initialPositionSet = false
                defer { loading = false }
                do {
                    try await store.loadConversation(channelId: channel.id)
                    let destination = store.targetMessageId ?? firstUnread
                    if let destination { proxy.scrollTo(destination, anchor: .top) }
                    else { proxy.scrollTo("timeline-bottom", anchor: .bottom) }
                    initialPositionSet = true
                    if destination == nil { markTailRead() }
                } catch { store.report(error) }
            }
            .onChange(of: messages.last?.id) { _, _ in
                guard initialPositionSet, nearBottom, !loadingNewer else { return }
                proxy.scrollTo("timeline-bottom", anchor: .bottom)
                markTailRead()
            }
            .onChange(of: scenePhase) { _, value in if value == .active && nearBottom { markTailRead() } }
        }
    }

    private func markTailRead() {
        guard visible, !store.offline, scenePhase == .active, initialPositionSet, let message = messages.last,
              message.id != store.readState(channel.id)?.lastReadId else { return }
        Task { do { try await store.markRead(channelId: channel.id, messageId: message.id) } catch { store.report(error) } }
    }

    private func daySeparator(_ timestamp: String) -> some View {
        HStack(spacing: 12) {
            Rectangle().fill(theme.line).frame(height: 1)
            if let date = MessagePresentation.date(timestamp) {
                Text(date.formatted(.dateTime.month(.abbreviated).day().year()))
                    .font(theme.monoFont(.caption)).foregroundStyle(theme.ink3).fixedSize()
            }
            Rectangle().fill(theme.line).frame(height: 1)
        }.padding(.horizontal, 16).padding(.top, 18).padding(.bottom, 10)
    }

    private var newSeparator: some View {
        HStack {
            Rectangle().fill(theme.accent).frame(height: 1)
            Text("New").font(theme.monoFont(.caption)).foregroundStyle(theme.accent)
        }.padding(.horizontal, 16).padding(.vertical, 8).accessibilityLabel("Unread messages start here")
    }

    private var typingIndicator: some View {
        TimelineView(.periodic(from: .now, by: 1)) { context in
            let names = (store.typing[channel.id] ?? [:]).filter {
                $0.key != store.user?.id && context.date.timeIntervalSince($0.value) < 6
            }.keys.sorted().map(store.userName)
            if !names.isEmpty {
                Text(names.joined(separator: ", ") + (names.count == 1 ? " is typing…" : " are typing…"))
                    .font(theme.bodyFont(.caption)).foregroundStyle(theme.ink2)
                    .frame(maxWidth: .infinity, alignment: .leading).padding(.horizontal, 16).padding(.vertical, 4)
                    .accessibilityIdentifier("typing-indicator")
            }
        }
    }

    private var voiceRoom: some View {
        VStack(spacing: 24) {
            Image(systemName: "headphones").font(.system(size: 48, weight: .light)).foregroundStyle(theme.accent)
            Text(store.channelTitle(channel)).font(theme.displayFont(.largeTitle))
            let participants = store.callStates.first { $0.channelId == channel.id }?.participantIds ?? []
            if participants.isEmpty { Text("No one is in the hangout.").foregroundStyle(theme.ink2) }
            else {
                ForEach(participants, id: \.self) { id in
                    HStack { PersonAvatar(userId: id, store: store); Text(store.userName(id)) }
                }
            }
            if store.calls?.session.channelID == channel.id {
                Text("You're in this call. Use the call bar to open its controls or leave.")
                    .font(theme.bodyFont(.footnote)).foregroundStyle(theme.ink2)
            } else {
                Button(action: joinCall) {
                    Label(startingCall ? "Joining…" : "Join hangout", systemImage: "phone.fill").frame(minHeight: 44)
                }.buttonStyle(.borderedProminent)
                    .disabled(startingCall || store.offline || store.calls?.currentCallID != nil)
                    .accessibilityIdentifier("join-hangout")
            }
        }.frame(maxWidth: .infinity, maxHeight: .infinity).padding(24)
    }

    private var hasParticipants: Bool {
        !(store.callStates.first { $0.channelId == channel.id }?.participantIds ?? []).isEmpty
    }

    private func joinCall() {
        guard !startingCall, let calls = store.calls else { return }
        startingCall = true
        Task {
            defer { startingCall = false }
            // Foreground consent happens before CallKit's time-limited start action.
            // Denied microphone access still permits listening and receiving video.
            _ = await LiveKitSDK.ensureDeviceAccess(for: [.audio])
            do {
                try await calls.requestOutgoing(channelID: channel.id, title: store.channelTitle(channel),
                    inviteDM: channel.kind == .dm && !hasParticipants)
            } catch { store.report(error) }
        }
    }

    private var peopleSheet: some View {
        NavigationStack {
            List {
                ForEach(store.users.filter { channel.kind != .dm || channel.memberIds.contains($0.id) }, id: \.id) { person in
                    HStack {
                        PersonAvatar(userId: person.id, store: store)
                        Text(store.userName(person.id))
                        if person.bot { Image(systemName: "sparkles").accessibilityLabel("Agent") }
                        Spacer()
                        Text(store.presence.contains(person.id) ? "Online" : "Offline")
                            .foregroundStyle(store.presence.contains(person.id) ? theme.accent : theme.ink3)
                            .font(theme.bodyFont(.caption))
                    }.listRowBackground(theme.bg2)
                }
            }.scrollContentBackground(.hidden).background(theme.bg)
                .navigationTitle("People").navigationBarTitleDisplayMode(.inline)
                .toolbar { ToolbarItem(placement: .confirmationAction) { Button("Done") { showPeople = false } } }
        }
    }
}
