import DenAPI
import SwiftUI

extension AppStore {
    func channelTitle(_ channel: API.Channel) -> String {
        guard channel.kind == .dm else { return channel.name }
        let others = channel.memberIds.filter { $0 != user?.id }
        return others.isEmpty ? "Notes to yourself" : others.map(userName).joined(separator: ", ")
    }

    func readState(_ channelId: String) -> API.ChannelReadState? {
        readStates.first { $0.channelId == channelId }
    }
}

struct PersonAvatar: View {
    let userId: String
    let store: AppStore
    var size: CGFloat = 36
    @Environment(\.colorScheme) private var colorScheme
    private var theme: DenTheme { store.theme.resolve(colorScheme) }

    var body: some View {
        ZStack(alignment: .bottomTrailing) {
            Text(String(store.userName(userId).prefix(1)).uppercased())
                .font(theme.bodyFont(.subheadline).weight(.semibold))
                .dynamicTypeSize(...DynamicTypeSize.xxxLarge)
                .lineLimit(1).minimumScaleFactor(0.7)
                .foregroundStyle(theme.ink2)
                .frame(width: size, height: size)
                .background(theme.bg3, in: RoundedRectangle(cornerRadius: theme.radius))
            if store.presence.contains(userId) {
                Circle().fill(theme.accent).frame(width: 9, height: 9)
                    .overlay(Circle().stroke(theme.bg, lineWidth: 2)).offset(x: 2, y: 2)
            }
        }
        .accessibilityLabel("\(store.userName(userId)), \(store.presence.contains(userId) ? "online" : "offline")")
    }
}

struct QuietOfflineChip: View {
    var body: some View {
        Label("Offline", systemImage: "wifi.slash")
            .font(.caption).padding(.horizontal, 10).padding(.vertical, 6)
            .glassEffect(.regular, in: Capsule())
            .accessibilityIdentifier("offline-status")
    }
}

struct SyncStatusView: View {
    let store: AppStore
    @State private var retrying = false

    var body: some View {
        if store.syncProblem == .incompatibleResponse {
            VStack(alignment: .leading, spacing: 8) {
                Label("Update Den", systemImage: "arrow.down.app").font(.headline)
                Text(DenFailure.updateRequired.localizedDescription).font(.subheadline)
                Button(retrying ? "Checking…" : "Try again") {
                    retrying = true
                    Task { await store.retrySync(); retrying = false }
                }
                .disabled(retrying)
                .accessibilityIdentifier("compatibility-retry")
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .accessibilityIdentifier("compatibility-status")
        } else if store.syncProblem == .networkOffline {
            QuietOfflineChip()
        }
    }
}

struct MarkdownMessageText: View {
    let content: String
    let store: AppStore
    @Environment(\.colorScheme) private var colorScheme
    private var theme: DenTheme { store.theme.resolve(colorScheme) }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            ForEach(MessagePresentation.blocks(content)) { block in
                if block.isCode {
                    ScrollView(.horizontal) {
                        Text(block.text).font(theme.monoFont(.body))
                            .textSelection(.enabled).padding(10)
                    }
                    .background(theme.bg3, in: RoundedRectangle(cornerRadius: theme.smallRadius))
                } else {
                    Text(MessagePresentation.markdown(block.text, users: store.users, theme: theme))
                        .font(theme.bodyFont()).tint(theme.accent)
                        .textSelection(.enabled).frame(maxWidth: .infinity, alignment: .leading)
                }
            }
        }
        .foregroundStyle(theme.ink)
    }
}

struct RoomLabel: View {
    let channel: API.Channel
    let store: AppStore
    @Environment(\.colorScheme) private var colorScheme
    private var theme: DenTheme { store.theme.resolve(colorScheme) }
    private var participants: [String] {
        store.callStates.first { $0.channelId == channel.id }?.participantIds ?? []
    }

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: channel.kind == .voice ? "headphones" : channel.kind == .dm ? "bubble.left.and.bubble.right" : "number")
                .font(.system(size: 20))
                .accessibilityHidden(true)
                .foregroundStyle(channel.kind == .voice && !participants.isEmpty ? theme.accent : theme.ink3)
                .frame(width: 22)
            VStack(alignment: .leading, spacing: 3) {
                Text(store.channelTitle(channel)).font(theme.bodyFont()).foregroundStyle(theme.ink)
                    .fontWeight((store.readState(channel.id)?.unreadCount ?? 0) > 0 ? .semibold : .regular)
                if channel.kind == .voice {
                    Text(participants.isEmpty ? "No one here yet" : participants.map(store.userName).joined(separator: ", "))
                        .font(theme.bodyFont(.caption)).foregroundStyle(theme.ink2).lineLimit(2)
                }
            }
            Spacer(minLength: 4)
            if let read = store.readState(channel.id), read.unreadCount > 0 {
                if read.mentionCount > 0 {
                    Image(systemName: "at.circle.fill").foregroundStyle(theme.accent)
                        .accessibilityLabel("Mentions you")
                }
                Text(read.unreadCount > 99 ? "99+" : "\(read.unreadCount)")
                    .font(theme.monoFont(.caption)).foregroundStyle(theme.accent)
                    .padding(.horizontal, 8).padding(.vertical, 3)
                    .background(theme.accentGlow, in: Capsule())
                    .accessibilityLabel("\(read.unreadCount) unread messages")
            }
        }
        .padding(.vertical, 5 * theme.density)
        .frame(minHeight: 44)
        .contentShape(Rectangle())
        .accessibilityElement(children: .combine)
    }
}
