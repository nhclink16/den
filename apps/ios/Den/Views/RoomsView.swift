import DenAPI
import SwiftUI

struct RoomsView: View {
    @Bindable var store: AppStore
    @State private var newDM = false
    @Environment(\.horizontalSizeClass) private var sizeClass
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize
    @Environment(\.colorScheme) private var colorScheme
    private var theme: DenTheme { store.theme.resolve(colorScheme) }
    private var selected: API.Channel? { store.channels.first { $0.id == store.selectedChannelId } }
    private enum Route: Hashable {
        case channel(String)
        case thread(ThreadSelection)
    }
    private var compactPath: Binding<[Route]> {
        Binding {
            guard let channelId = store.selectedChannelId else { return [] }
            var value: [Route] = [.channel(channelId)]
            if let thread = store.selectedThread { value.append(.thread(thread)) }
            return value
        } set: { value in
            switch value.last {
            case let .thread(thread):
                store.selectedChannelId = thread.channelId
                store.targetMessageId = nil
                store.selectedThread = thread
            case let .channel(channelId):
                store.selectedChannelId = channelId
                store.targetMessageId = nil
                store.selectedThread = nil
            case nil:
                store.selectedChannelId = nil
                store.targetMessageId = nil
                store.selectedThread = nil
            }
        }
    }
    private var regularThreadPath: Binding<[ThreadSelection]> {
        Binding(get: { store.selectedThread.map { [$0] } ?? [] }, set: { value in
            store.selectedThread = value.last
            if let thread = value.last { store.selectedChannelId = thread.channelId }
        })
    }

    var body: some View {
        Group {
            if sizeClass == .regular && !dynamicTypeSize.isAccessibilitySize {
                NavigationSplitView {
                    roomList
                } detail: {
                    if let selected {
                        NavigationStack(path: regularThreadPath) {
                            ConversationView(channel: selected, store: store).id(selected.id)
                                .navigationDestination(for: ThreadSelection.self) { thread in
                                    ThreadConversationView(route: thread, channel: selected, store: store)
                                }
                        }
                    }
                    else { ContentUnavailableView("Make yourself at home", systemImage: "number", description: Text("Choose a room or a direct message.")) }
                }
            } else {
                NavigationStack(path: compactPath) {
                    roomList.navigationDestination(for: Route.self) { route in
                        switch route {
                        case let .channel(id):
                            if let channel = store.channels.first(where: { $0.id == id }) {
                                ConversationView(channel: channel, store: store).id(id)
                            }
                        case let .thread(thread):
                            if let channel = store.channels.first(where: { $0.id == thread.channelId }) {
                                ThreadConversationView(route: thread, channel: channel, store: store)
                            }
                        }
                    }
                }
            }
        }
        .sheet(isPresented: $newDM) { NewDMView(store: store) }
    }

    private var roomList: some View {
        List {
            if store.offline { SyncStatusView(store: store).listRowBackground(Color.clear) }
            let ungrouped = store.channels.filter { $0.categoryId == nil && $0.kind != .dm }
            if !ungrouped.isEmpty { Section("Rooms") { channelRows(ungrouped) } }
            ForEach(store.categories.sorted { $0.position < $1.position }, id: \.id) { category in
                let channels = store.channels.filter { $0.categoryId == category.id && $0.kind != .dm }
                if !channels.isEmpty { Section(category.name) { channelRows(channels) } }
            }
            Section {
                channelRows(store.channels.filter { $0.kind == .dm })
                Button { newDM = true } label: { Label("New message", systemImage: "square.and.pencil").frame(minHeight: 44) }
                    .disabled(store.offline)
                    .accessibilityIdentifier("new-dm")
            } header: { Text("Direct messages") }
            if store.channels.isEmpty && !store.busy && !store.offline {
                ContentUnavailableView("No rooms yet", systemImage: "number", description: Text("Rooms will appear here when your server adds them."))
                    .listRowBackground(Color.clear)
            }
        }
        .listStyle(.insetGrouped).scrollContentBackground(.hidden).background(theme.bg)
        .navigationTitle(store.instanceName.isEmpty ? "Den" : store.instanceName)
        .refreshable { await store.retrySync() }
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button { newDM = true } label: { Image(systemName: "square.and.pencil") }
                    .disabled(store.offline)
                    .accessibilityLabel("New direct message")
            }
        }
    }

    @ViewBuilder private func channelRows(_ channels: [API.Channel]) -> some View {
        ForEach(channels.sorted { $0.position == $1.position ? $0.id < $1.id : $0.position < $1.position }, id: \.id) { channel in
            if sizeClass == .regular && !dynamicTypeSize.isAccessibilitySize {
                Button { store.selectChannel(channel.id) } label: { RoomLabel(channel: channel, store: store) }
                    .listRowBackground(store.selectedChannelId == channel.id ? theme.accentGlow : theme.bg2)
                    .accessibilityIdentifier("room-\(channel.id)")
            } else {
                NavigationLink(value: Route.channel(channel.id)) {
                    RoomLabel(channel: channel, store: store)
                }
                .listRowBackground(theme.bg2)
                .accessibilityIdentifier("room-\(channel.id)")
            }
        }
    }
}

private struct NewDMView: View {
    let store: AppStore
    @State private var selected: Set<String> = []
    @State private var query = ""
    @State private var opening = false
    @Environment(\.dismiss) private var dismiss
    @Environment(\.colorScheme) private var colorScheme
    private var theme: DenTheme { store.theme.resolve(colorScheme) }

    var body: some View {
        NavigationStack {
            List {
                Section(selected.count > 1 ? "Group message · \(selected.count) people" : "Choose who to message") {
                    ForEach(store.users.filter {
                        $0.id != store.user?.id && (query.isEmpty || $0.username.localizedCaseInsensitiveContains(query) || $0.displayName.localizedCaseInsensitiveContains(query))
                    }, id: \.id) { user in
                        Button {
                            if selected.contains(user.id) { selected.remove(user.id) } else { selected.insert(user.id) }
                        } label: {
                            HStack {
                                PersonAvatar(userId: user.id, store: store)
                                VStack(alignment: .leading) {
                                    Text(store.userName(user.id)).foregroundStyle(theme.ink)
                                    Text("@" + user.username).font(theme.bodyFont(.caption)).foregroundStyle(theme.ink2)
                                }
                                Spacer()
                                Image(systemName: selected.contains(user.id) ? "checkmark.circle.fill" : "circle")
                                    .foregroundStyle(selected.contains(user.id) ? theme.accent : theme.ink3)
                            }.frame(minHeight: 44)
                        }
                        .accessibilityAddTraits(selected.contains(user.id) ? [.isSelected] : [])
                        .accessibilityIdentifier("dm-person-\(user.id)")
                        .listRowBackground(theme.bg2)
                    }
                }
            }
            .searchable(text: $query, prompt: "Find a person")
            .navigationTitle("New message").navigationBarTitleDisplayMode(.inline)
            .scrollContentBackground(.hidden).background(theme.bg)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button(opening ? "Opening…" : "Message") {
                        opening = true
                        Task {
                            defer { opening = false }
                            do {
                                let channel = try await store.openDM(userIds: selected.sorted())
                                store.selectChannel(channel.id)
                                dismiss()
                            } catch { store.report(error) }
                        }
                    }.disabled(selected.isEmpty || opening || store.offline).accessibilityIdentifier("open-dm")
                }
            }
        }
    }
}
