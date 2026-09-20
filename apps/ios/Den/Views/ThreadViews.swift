import DenAPI
import SwiftUI

struct ThreadStripView: View {
    let channel: API.Channel
    let store: AppStore
    @State private var mode = Mode.open
    @State private var lists: [Mode: ListState] = [:]
    @Environment(\.colorScheme) private var colorScheme
    private var theme: DenTheme { store.theme.resolve(colorScheme) }

    private enum Mode: String, CaseIterable, Hashable {
        case open = "Open"
        case unread = "Unread"
        case resolved = "Resolved"
    }
    private struct ListState {
        var ids: [String] = []
        var cursor: String?
        var more = false
        var loading = false
        var error: String?
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Picker("Conversations", selection: $mode) {
                ForEach(Mode.allCases, id: \.self) { Text($0.rawValue).tag($0) }
            }
            .pickerStyle(.segmented)
            .accessibilityIdentifier("thread-strip-mode")
            ScrollView(.horizontal) {
                HStack(spacing: 7) {
                    let state = lists[mode] ?? ListState()
                    if state.loading && rows.isEmpty { ProgressView().controlSize(.small) }
                    if let error = state.error {
                        Text(error).foregroundStyle(theme.danger).lineLimit(1)
                        Button("Retry") { load(mode) }.buttonStyle(.bordered)
                    } else if rows.isEmpty && !state.loading {
                        Text(emptyText).font(theme.bodyFont(.caption)).foregroundStyle(theme.ink3)
                    }
                    ForEach(rows, id: \.id) { thread in
                        let read = store.threadReadStates[thread.id]
                        Button {
                            store.selectThread(channelId: channel.id, rootId: thread.rootMessageId,
                                               threadId: thread.id)
                        } label: {
                            HStack(spacing: 6) {
                                Text(thread.title).lineLimit(1)
                                Text("\(thread.replyCount)").font(theme.monoFont(.caption2))
                                if let read, read.unreadCount > 0 {
                                    Text(read.mentionCount > 0 ? "@" : "\(read.unreadCount)")
                                        .font(theme.monoFont(.caption2).weight(.semibold))
                                        .foregroundStyle(theme.accent)
                                }
                                if thread.resolvedAt != nil { Image(systemName: "checkmark.circle") }
                            }
                            .padding(.horizontal, 11).frame(minHeight: 36)
                            .background(theme.bg2, in: Capsule())
                            .overlay(Capsule().stroke(theme.line, lineWidth: 1))
                        }
                        .buttonStyle(.plain)
                        .accessibilityIdentifier("thread-chip-\(thread.id)")
                    }
                    if state.more {
                        Button(state.loading ? "Loading…" : "More") { load(mode, append: true) }
                            .disabled(state.loading).buttonStyle(.bordered)
                    }
                }
            }
            .scrollIndicators(.hidden)
        }
        .padding(.horizontal, 12).padding(.vertical, 7)
        .background(theme.bg)
        .overlay(alignment: .bottom) { Rectangle().fill(theme.line).frame(height: 1) }
        .accessibilityElement(children: .contain)
        .accessibilityLabel("Conversations in this room")
        .task(id: channel.id) { await load(mode: .open) }
        .onChange(of: mode) { _, value in load(value) }
    }

    private var rows: [API.ThreadSummary] {
        var ids = lists[mode]?.ids ?? []
        for thread in store.threadMetadata.values where thread.channelId == channel.id && !ids.contains(thread.id) {
            ids.append(thread.id)
        }
        return ids.compactMap { store.threadMetadata[$0] }.filter { thread in
            switch mode {
            case .open: thread.resolvedAt == nil
            case .unread: (store.threadReadStates[thread.id]?.unreadCount ?? 0) > 0
            case .resolved: thread.resolvedAt != nil
            }
        }.sorted {
            if $0.lastActivityAt == $1.lastActivityAt { return $0.id > $1.id }
            return $0.lastActivityAt > $1.lastActivityAt
        }
    }

    private var emptyText: String {
        switch mode {
        case .open: "No open conversations. Reply to a message to start one."
        case .unread: "No unread conversations."
        case .resolved: "No resolved conversations."
        }
    }

    private func load(_ mode: Mode, append: Bool = false) {
        Task { await load(mode: mode, append: append) }
    }

    private func load(mode: Mode, append: Bool = false) async {
        var state = lists[mode] ?? ListState()
        guard !state.loading, !store.offline else { return }
        state.loading = true; state.error = nil; lists[mode] = state
        do {
            let page = try await store.loadThreads(channelId: channel.id,
                resolved: mode == .open ? false : mode == .resolved ? true : nil,
                unreadOnly: mode == .unread ? true : nil,
                before: append ? state.cursor : nil, limit: 20)
            let ids = page.map(\.thread.id)
            if append { state.ids.append(contentsOf: ids.filter { !state.ids.contains($0) }) }
            else { state.ids = ids }
            state.cursor = ids.last ?? state.cursor
            state.more = page.count == 20
            state.loading = false; state.error = nil; lists[mode] = state
        } catch {
            state.loading = false; state.error = DenFailure.present(error); lists[mode] = state
        }
    }
}

struct ThreadConversationView: View {
    let route: ThreadSelection
    let channel: API.Channel
    let store: AppStore
    @State private var reply: API.Message?
    @State private var loading = false
    @State private var loadingOlder = false
    @State private var loadingNewer = false
    @State private var exhausted = false
    @State private var nearBottom = false
    @State private var visible = false
    @State private var initialPositionSet = false
    @State private var openedAt: String?
    @State private var capturedReadMarker = false
    @State private var showRename = false
    @State private var renameDraft = ""
    @State private var acting = false
    @Environment(\.scenePhase) private var scenePhase
    @Environment(\.colorScheme) private var colorScheme
    private var theme: DenTheme { store.theme.resolve(colorScheme) }
    private var selection: ThreadSelection {
        guard let current = store.selectedThread, current.channelId == route.channelId,
              current.threadId == route.threadId || current.rootId == route.rootId else { return route }
        return current
    }
    private var threadId: String? { selection.threadId ?? selection.rootId.flatMap { store.threadForRoot($0)?.id } }
    private var summary: API.ThreadSummary? { threadId.flatMap { store.threadMetadata[$0] } }
    private var rootId: String? { selection.rootId ?? summary?.rootMessageId }
    private var root: API.Message? { rootId.flatMap { store.rootMessage($0) } }
    private var replies: [API.Message] { threadId.flatMap { store.threadMessages[$0] } ?? [] }
    private var conversation: Conversation? { rootId.map { .thread(channelId: channel.id, rootId: $0) } }
    private var resolved: Bool { summary?.resolvedAt != nil }
    private var firstUnread: String? {
        guard capturedReadMarker else { return nil }
        return MessagePresentation.firstUnread(in: replies, after: openedAt, excluding: store.user?.id)
    }

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 0) {
                    if store.offline { SyncStatusView(store: store).padding(.horizontal, 16) }
                    if let root {
                        Text("Started from").font(theme.monoFont(.caption)).foregroundStyle(theme.ink3)
                            .padding(.horizontal, 16).padding(.top, 12)
                        MessageRow(message: root, grouped: false, store: store) { reply = root }
                            .id(root.id)
                        Rectangle().fill(theme.line).frame(height: 1).padding(.horizontal, 16).padding(.vertical, 8)
                    } else if loading {
                        ProgressView().frame(maxWidth: .infinity).padding(40)
                    }
                    if let id = threadId, !replies.isEmpty && !exhausted {
                        Button(loadingOlder ? "Loading…" : "Load earlier replies") {
                            guard !loadingOlder, let first = replies.first else { return }
                            loadingOlder = true
                            let count = replies.count
                            Task {
                                defer { loadingOlder = false }
                                do {
                                    try await store.loadThreadMessages(id: id, before: first.id)
                                    exhausted = replies.count == count
                                    proxy.scrollTo(first.id, anchor: .top)
                                } catch { store.report(error) }
                            }
                        }
                        .disabled(loadingOlder || store.offline).frame(maxWidth: .infinity, minHeight: 44)
                    }
                    ForEach(Array(replies.enumerated()), id: \.element.id) { index, message in
                        let prior = index == 0 ? nil : replies[index - 1]
                        VStack(alignment: .leading, spacing: 0) {
                            if message.id == firstUnread { unreadSeparator }
                            MessageRow(message: message,
                                grouped: MessagePresentation.groups(message, after: prior), store: store,
                                onOpenReference: { target in
                                    guard let id = threadId else { return }
                                    Task {
                                        do {
                                            guard try await store.prepareThreadReference(
                                                threadId: id, parentId: target) else { return }
                                            await Task.yield()
                                            proxy.scrollTo(target, anchor: .center)
                                        } catch { store.report(error) }
                                    }
                                }) {
                                reply = message
                            }
                        }.id(message.id)
                    }
                    if let id = threadId, store.threadHasNewer[id] == true {
                        Button(loadingNewer ? "Loading…" : "Load newer replies") {
                            loadingNewer = true; nearBottom = false
                            Task {
                                defer { loadingNewer = false }
                                do { try await store.loadNewerThreadMessages(id: id) }
                                catch { store.report(error) }
                            }
                        }
                        .disabled(loadingNewer || store.offline).frame(maxWidth: .infinity, minHeight: 44)
                    }
                    Color.clear.frame(height: 1).id("thread-bottom")
                }.padding(.bottom, 8)
            }
            .accessibilityIdentifier("thread-timeline")
            .defaultScrollAnchor(.bottom).scrollDismissesKeyboard(.interactively)
            .onScrollGeometryChange(for: Bool.self) { geometry in
                geometry.contentSize.height - geometry.visibleRect.maxY < 100
            } action: { _, value in nearBottom = value; if value { markTailRead() } }
            .safeAreaInset(edge: .bottom, spacing: 0) {
                VStack(spacing: 0) {
                    typingIndicator
                    if resolved {
                        HStack {
                            Text("This conversation is resolved.").foregroundStyle(theme.ink2)
                            Spacer()
                            Button("Reopen") { setResolved(false) }.disabled(acting || store.offline)
                                .accessibilityIdentifier("thread-reopen")
                        }.padding(12)
                    } else if let conversation {
                        ComposerView(channel: channel, store: store, reply: $reply, onSent: {
                            proxy.scrollTo("thread-bottom", anchor: .bottom); markTailRead()
                        }, conversation: conversation)
                    }
                }.background(theme.bg)
            }
            .task(id: "\(threadId ?? ""):\(selection.rootId ?? ""):\(selection.targetMessageId ?? "")") {
                await load(proxy: proxy)
            }
            .onChange(of: replies.last?.id) { _, _ in
                guard initialPositionSet, nearBottom, !loadingNewer else { return }
                proxy.scrollTo("thread-bottom", anchor: .bottom); markTailRead()
            }
            .onChange(of: scenePhase) { _, phase in if phase == .active && nearBottom { markTailRead() } }
        }
        .background(theme.bg).navigationBarTitleDisplayMode(.inline)
        .navigationBarBackButtonHidden(true)
        .navigationTitle(summary?.title ?? "Conversation")
        .toolbar {
            ToolbarItem(placement: .topBarLeading) {
                Button { store.selectedThread = nil } label: { Image(systemName: "chevron.left") }
                    .accessibilityLabel("Back to room")
                    .accessibilityIdentifier("thread-back")
            }
            ToolbarItem(placement: .topBarTrailing) {
                Menu {
                    if let summary {
                        Button("Rename", systemImage: "pencil") {
                            renameDraft = summary.title; showRename = true
                        }
                        .accessibilityIdentifier("thread-action-rename")
                        let following = store.threadReadStates[summary.id]?.following == true
                        Button(following ? "Unfollow" : "Follow",
                               systemImage: following ? "bell.slash" : "bell") { setFollowing(!following) }
                            .accessibilityIdentifier("thread-action-follow")
                        Button(resolved ? "Reopen" : "Resolve",
                               systemImage: resolved ? "arrow.uturn.backward" : "checkmark.circle") {
                            setResolved(!resolved)
                        }
                        .accessibilityIdentifier("thread-action-resolve")
                    }
                } label: { Image(systemName: "ellipsis.circle") }
                    .disabled(summary == nil || acting || store.offline)
                    .accessibilityLabel("Conversation actions")
                    .accessibilityIdentifier("thread-actions")
            }
        }
        .alert("Rename conversation", isPresented: $showRename) {
            TextField("Conversation title", text: $renameDraft)
                .accessibilityIdentifier("thread-rename-field")
            Button("Cancel", role: .cancel) {}
            Button("Save") { rename() }
                .disabled(renameDraft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
        }
        .onAppear { visible = true; if nearBottom { markTailRead() } }
        .onDisappear { visible = false }
    }

    private func load(proxy: ScrollViewProxy) async {
        loading = true; initialPositionSet = false
        defer { loading = false }
        do {
            var id = threadId
            if id == nil, let rootId, let existing = store.threadForRoot(rootId) {
                store.selectThread(channelId: channel.id, rootId: rootId, threadId: existing.id)
                id = existing.id
            }
            if let id {
                let target = selection.targetMessageId == rootId ? nil : selection.targetMessageId
                try await store.hydrateThread(id: id, target: target)
                openedAt = store.threadReadStates[id]?.lastReadId
                capturedReadMarker = true
            }
            let destination = selection.targetMessageId ?? firstUnread
            if let destination { proxy.scrollTo(destination, anchor: .top) }
            else { proxy.scrollTo("thread-bottom", anchor: .bottom) }
            initialPositionSet = true
            if destination == nil || destination == replies.last?.id { markTailRead() }
        } catch { store.report(error) }
    }

    private func markTailRead() {
        guard visible, !store.offline, scenePhase == .active, initialPositionSet,
              let id = threadId, let marker = replies.last?.id ?? rootId,
              marker != store.threadReadStates[id]?.lastReadId else { return }
        Task { do { try await store.markThreadRead(id: id, messageId: marker) } catch { store.report(error) } }
    }

    @ViewBuilder private var typingIndicator: some View {
        if let conversation {
            TimelineView(.periodic(from: .now, by: 1)) { context in
                let names = (store.typing[conversation] ?? [:]).filter {
                    $0.key != store.user?.id && context.date.timeIntervalSince($0.value) < 6
                }.keys.sorted().map(store.userName)
                if !names.isEmpty {
                    Text(names.joined(separator: ", ") + (names.count == 1 ? " is typing…" : " are typing…"))
                        .font(theme.bodyFont(.caption)).foregroundStyle(theme.ink2)
                        .frame(maxWidth: .infinity, alignment: .leading).padding(.horizontal, 16).padding(.vertical, 4)
                }
            }
        }
    }

    private var unreadSeparator: some View {
        HStack { Rectangle().fill(theme.accent).frame(height: 1); Text("New").foregroundStyle(theme.accent) }
            .font(theme.monoFont(.caption)).padding(.horizontal, 16).padding(.vertical, 8)
            .accessibilityLabel("Unread replies start here")
    }

    private func setResolved(_ value: Bool) {
        guard let id = threadId else { return }
        acting = true
        Task { defer { acting = false }; do { _ = try await store.updateThread(id: id, resolved: value) } catch { store.report(error) } }
    }

    private func setFollowing(_ value: Bool) {
        guard let id = threadId else { return }
        acting = true
        Task { defer { acting = false }; do { try await store.followThread(id: id, following: value) } catch { store.report(error) } }
    }

    private func rename() {
        guard let id = threadId else { return }
        let value = renameDraft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !value.isEmpty else { return }
        acting = true
        Task { defer { acting = false }; do { _ = try await store.updateThread(id: id, title: value) } catch { store.report(error) } }
    }
}
