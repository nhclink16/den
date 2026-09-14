import DenAPI
import SwiftUI

struct RoomsView: View {
    @Bindable var store: AppStore
    @State private var newDM = false
    @Environment(\.horizontalSizeClass) private var sizeClass
    @Environment(\.colorScheme) private var colorScheme
    private var theme: DenTheme { store.theme.resolve(colorScheme) }
    private var selected: API.Channel? { store.channels.first { $0.id == store.selectedChannelId } }
    private var path: Binding<[String]> {
        Binding(get: { store.selectedChannelId.map { [$0] } ?? [] }, set: { store.selectedChannelId = $0.last })
    }

    var body: some View {
        Group {
            if sizeClass == .regular {
                NavigationSplitView {
                    roomList
                } detail: {
                    if let selected { ConversationView(channel: selected, store: store).id(selected.id) }
                    else { ContentUnavailableView("Make yourself at home", systemImage: "number", description: Text("Choose a room or a direct message.")) }
                }
            } else {
                NavigationStack(path: path) {
                    roomList.navigationDestination(for: String.self) { id in
                        if let channel = store.channels.first(where: { $0.id == id }) {
                            ConversationView(channel: channel, store: store).id(id)
                        }
                    }
                }
            }
        }
        .sheet(isPresented: $newDM) { NewDMView(store: store) }
    }

    private var roomList: some View {
        List {
            if store.offline { QuietOfflineChip().listRowBackground(Color.clear) }
            let ungrouped = store.channels.filter { $0.categoryId == nil && $0.kind != .dm }
            if !ungrouped.isEmpty { Section("Rooms") { channelRows(ungrouped) } }
            ForEach(store.categories.sorted { $0.position < $1.position }, id: \.id) { category in
                let channels = store.channels.filter { $0.categoryId == category.id && $0.kind != .dm }
                if !channels.isEmpty { Section(category.name) { channelRows(channels) } }
            }
            Section {
                channelRows(store.channels.filter { $0.kind == .dm })
                Button { newDM = true } label: { Label("New message", systemImage: "square.and.pencil").frame(minHeight: 44) }
                    .accessibilityIdentifier("new-dm")
            } header: { Text("Direct messages") }
            if store.channels.isEmpty && !store.busy {
                ContentUnavailableView("No rooms yet", systemImage: "number", description: Text("Rooms will appear here when your server adds them."))
                    .listRowBackground(Color.clear)
            }
        }
        .listStyle(.insetGrouped).scrollContentBackground(.hidden).background(theme.bg)
        .navigationTitle(store.instanceName.isEmpty ? "Den" : store.instanceName)
        .refreshable { do { try await store.refresh() } catch { store.report(error) } }
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Button { newDM = true } label: { Image(systemName: "square.and.pencil") }
                    .accessibilityLabel("New direct message")
            }
        }
    }

    private func channelRows(_ channels: [API.Channel]) -> some View {
        ForEach(channels.sorted { $0.position == $1.position ? $0.id < $1.id : $0.position < $1.position }, id: \.id) { channel in
            Button { store.selectChannel(channel.id) } label: { RoomLabel(channel: channel, store: store) }
                .listRowBackground(store.selectedChannelId == channel.id ? theme.accentGlow : theme.bg2)
                .accessibilityIdentifier("room-\(channel.id)")
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
                    }.disabled(selected.isEmpty || opening).accessibilityIdentifier("open-dm")
                }
            }
        }
    }
}
