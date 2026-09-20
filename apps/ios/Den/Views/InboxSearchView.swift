import DenAPI
import SwiftUI

struct InboxView: View {
    let store: AppStore
    var onNavigate: () -> Void = {}
    @State private var markingAll = false
    @State private var unreadThreadIds: [String] = []
    @Environment(\.colorScheme) private var colorScheme
    private var theme: DenTheme { store.theme.resolve(colorScheme) }
    private var unread: [API.Channel] {
        store.channels.filter { (store.readState($0.id)?.unreadCount ?? 0) > 0 }.sorted { left, right in
            let lm = (store.readState(left.id)?.mentionCount ?? 0) > 0
            let rm = (store.readState(right.id)?.mentionCount ?? 0) > 0
            if lm != rm { return lm }
            return (store.messages[left.id]?.last?.id ?? "") > (store.messages[right.id]?.last?.id ?? "")
        }
    }

    var body: some View {
        List {
            if store.offline { SyncStatusView(store: store).listRowBackground(Color.clear) }
            if unread.isEmpty {
                ContentUnavailableView("You're caught up", systemImage: "tray", description: Text("Mentions, direct messages, and unread rooms appear here."))
                    .listRowBackground(Color.clear)
            }
            ForEach(unread, id: \.id) { channel in
                let previews = previewMessages(channel)
                let threads = unreadThreads(channel)
                Section {
                    Button { open(channel) } label: { RoomLabel(channel: channel, store: store) }
                        .accessibilityIdentifier("inbox-room-\(channel.id)")
                    ForEach(threads, id: \.id) { thread in
                        let state = store.threadReadStates[thread.id]
                        Button { open(thread) } label: {
                            HStack(alignment: .top, spacing: 10) {
                                Image(systemName: thread.resolvedAt == nil ? "bubble.left.and.bubble.right" : "checkmark.circle")
                                    .foregroundStyle(theme.accent).frame(width: 28)
                                VStack(alignment: .leading, spacing: 4) {
                                    Text(thread.title).fontWeight(.semibold).foregroundStyle(theme.ink)
                                    Text(threadDescription(thread, state: state))
                                        .font(theme.bodyFont(.caption)).foregroundStyle(theme.ink2)
                                }
                                Spacer()
                                if (state?.mentionCount ?? 0) > 0 {
                                    Image(systemName: "at").foregroundStyle(theme.accent)
                                }
                            }.padding(.vertical, 5)
                        }
                        .accessibilityIdentifier("inbox-thread-\(thread.id)")
                    }
                    ForEach(Array(previews.suffix(4)), id: \.id) { message in
                        Button { open(channel) } label: {
                            HStack(alignment: .top, spacing: 10) {
                                PersonAvatar(userId: message.authorId, store: store, size: 28)
                                VStack(alignment: .leading, spacing: 4) {
                                    HStack(alignment: .firstTextBaseline) {
                                        Text(store.userName(message.authorId)).fontWeight(.semibold)
                                        Spacer(minLength: 4)
                                        if let date = MessagePresentation.date(message.createdAt) {
                                            Text(date, format: .dateTime.hour().minute()).font(theme.monoFont(.caption2)).foregroundStyle(theme.ink3)
                                        }
                                    }
                                    Text(message.content.isEmpty ? "Sent an attachment" : message.content)
                                        .font(theme.bodyFont(.subheadline)).foregroundStyle(theme.ink2).lineLimit(2)
                                }
                            }.padding(.vertical, 5).foregroundStyle(theme.ink)
                        }
                    }
                    let threadUnread = threads.reduce(Int64(0)) {
                        $0 + (store.threadReadStates[$1.id]?.unreadCount ?? 0)
                    }
                    let roomUnread = max(0, (store.readState(channel.id)?.unreadCount ?? 0) - threadUnread)
                    let earlier = Int(roomUnread) - min(previews.count, 4)
                    if earlier > 0 {
                        Button("\(earlier) earlier \(earlier == 1 ? "message" : "messages")") { open(channel) }
                            .font(theme.bodyFont(.caption))
                    }
                }.listRowBackground(theme.bg2)
            }
        }
        .listStyle(.insetGrouped).scrollContentBackground(.hidden).background(theme.bg)
        .navigationTitle("Inbox")
        .toolbar {
            if !unread.isEmpty {
                ToolbarItem(placement: .topBarTrailing) {
                    Button(markingAll ? "Marking…" : "Mark all read") { markAll() }
                        .disabled(markingAll || store.offline)
                }
            }
        }
        .refreshable { await store.retrySync(); await loadPreviews() }
        .task(id: unread.map { "\($0.id):\(store.readState($0.id)?.unreadCount ?? 0)" }) {
            await loadPreviews()
        }
    }

    private func previewMessages(_ channel: API.Channel) -> [API.Message] {
        let lastRead = store.readState(channel.id)?.lastReadId ?? ""
        return (store.messages[channel.id] ?? []).filter { $0.id > lastRead && $0.authorId != store.user?.id }
    }

    private func open(_ channel: API.Channel) {
        onNavigate()
        // loadConversation fetches the unread window before the timeline picks its first unread row.
        store.selectChannel(channel.id)
    }

    private func open(_ thread: API.ThreadSummary) {
        onNavigate()
        store.selectThread(channelId: thread.channelId, rootId: thread.rootMessageId,
                           threadId: thread.id)
    }

    private func unreadThreads(_ channel: API.Channel) -> [API.ThreadSummary] {
        unreadThreadIds.compactMap { store.threadMetadata[$0] }
            .filter { $0.channelId == channel.id && (store.threadReadStates[$0.id]?.unreadCount ?? 0) > 0 }
            .sorted { $0.lastActivityAt > $1.lastActivityAt }
    }

    private func threadDescription(_ thread: API.ThreadSummary, state: API.ThreadReadState?) -> String {
        let unread = state?.unreadCount ?? 0
        let noun = unread == 1 ? "reply" : "replies"
        let status = thread.resolvedAt == nil ? "" : " · Resolved"
        return "\(unread) unread \(noun)\(status)"
    }

    private func loadPreviews() async {
        guard !store.offline else { return }
        for channel in unread {
            do {
                if store.messages[channel.id] == nil { try await store.loadMessages(channelId: channel.id) }
                let views = try await store.loadAllThreads(channelId: channel.id, unreadOnly: true)
                for id in views.map(\.thread.id) where !unreadThreadIds.contains(id) {
                    unreadThreadIds.append(id)
                }
            }
            catch { store.report(error); return }
        }
    }

    private func markAll() {
        let channels = unread
        markingAll = true
        Task {
            defer { markingAll = false }
            do {
                for channel in channels {
                    try await store.markAllRead(channelId: channel.id)
                }
                unreadThreadIds = []
            } catch { store.report(error) }
        }
    }
}

struct SearchView: View {
    let store: AppStore
    var onNavigate: () -> Void = {}
    @State private var query = ""
    @State private var scope = ""
    @State private var results: [API.Message] = []
    @State private var searching = false
    @State private var submitted = false
    @State private var searchTask: Task<Void, Never>?
    @FocusState private var queryFocused: Bool
    @Environment(\.colorScheme) private var colorScheme
    private var theme: DenTheme { store.theme.resolve(colorScheme) }

    var body: some View {
        List {
            Section {
                HStack(spacing: 10) {
                    Image(systemName: "magnifyingglass").foregroundStyle(theme.ink3).accessibilityHidden(true)
                    TextField("Search messages", text: $query)
                        .textInputAutocapitalization(.never).autocorrectionDisabled()
                        .submitLabel(.search).focused($queryFocused).onSubmit {
                            queryFocused = false
                            search()
                        }
                        .accessibilityLabel("Search messages").accessibilityIdentifier("search-query")
                    if !query.isEmpty {
                        Button { query = ""; queryFocused = true } label: {
                            Image(systemName: "xmark.circle.fill").frame(width: 44, height: 44)
                        }
                        .buttonStyle(.plain).foregroundStyle(theme.ink3).accessibilityLabel("Clear search")
                    }
                }
                .frame(minHeight: 44).listRowBackground(theme.bg2)
                Picker("Search in", selection: $scope) {
                    Text("All rooms and messages").tag("")
                    ForEach(store.channels.filter { $0.kind != .voice }, id: \.id) { channel in
                        Text(store.channelTitle(channel)).tag(channel.id)
                    }
                }.listRowBackground(theme.bg2)
            }
            if searching { ProgressView("Searching…").listRowBackground(Color.clear) }
            else if results.isEmpty {
                ContentUnavailableView(submitted ? "No messages found" : "Find a conversation", systemImage: "magnifyingglass",
                    description: Text(submitted ? "Try a different word or search all rooms." : "Search messages across your Den."))
                    .listRowBackground(Color.clear)
            }
            ForEach(results, id: \.id) { message in
                Button {
                    queryFocused = false
                    onNavigate()
                    if let threadId = message.threadId {
                        store.selectThread(channelId: message.channelId, threadId: threadId,
                                           messageId: message.id)
                    } else {
                        store.selectChannel(message.channelId, messageId: message.id)
                    }
                } label: {
                    VStack(alignment: .leading, spacing: 8) {
                        HStack {
                            Text(store.channels.first(where: { $0.id == message.channelId }).map(store.channelTitle) ?? "Room")
                                .font(theme.bodyFont(.caption)).foregroundStyle(theme.accent)
                            Spacer()
                            if let date = MessagePresentation.date(message.createdAt) {
                                Text(date, format: .dateTime.month(.abbreviated).day())
                                    .font(theme.monoFont(.caption2)).foregroundStyle(theme.ink3)
                            }
                        }
                        HStack(alignment: .top, spacing: 10) {
                            PersonAvatar(userId: message.authorId, store: store, size: 28)
                            VStack(alignment: .leading, spacing: 4) {
                                Text(store.userName(message.authorId)).fontWeight(.semibold)
                                Text(message.content.isEmpty ? "Attachment" : message.content)
                                    .font(theme.bodyFont(.subheadline)).lineLimit(4)
                            }
                        }.foregroundStyle(theme.ink)
                    }.padding(.vertical, 6)
                }.listRowBackground(theme.bg2).accessibilityIdentifier("search-result-\(message.id)")
            }
        }
        .accessibilityIdentifier("search-list")
        .listStyle(.insetGrouped).scrollContentBackground(.hidden).background(theme.bg)
        .navigationTitle("Search")
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button("Search") {
                    queryFocused = false
                    search()
                }
                .disabled(searching || query.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                .accessibilityIdentifier("search-submit")
            }
        }
        .onChange(of: scope) { _, _ in if submitted { search() } }
        .onChange(of: query) { _, value in
            if value.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                searchTask?.cancel(); searching = false; submitted = false; results = []
            }
        }
        .onDisappear { queryFocused = false; searchTask?.cancel(); searching = false }
    }

    private func search() {
        let query = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !query.isEmpty else { return }
        searchTask?.cancel()
        searching = true
        submitted = true
        let channelId = scope.isEmpty ? nil : scope
        searchTask = Task {
            do {
                let found = try await store.search(query: query, channelId: channelId)
                try Task.checkCancellation()
                results = found
                searching = false
            } catch {
                guard !Task.isCancelled else { return }
                searching = false; store.report(error)
            }
        }
    }
}
