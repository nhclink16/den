import Foundation
import DenAPI

extension AppStore {
    func connectSocket() {
        socketLoop?.cancel(); socket?.cancel(with: .goingAway, reason: nil)
        guard let service else { return }
        let expected = generation
        socketLoop = Task { [weak self] in
            var retry: UInt64 = 1
            while !Task.isCancelled {
                guard let self, self.generation == expected else { return }
                do {
                    let ticket = try await service.ticket()
                    try self.check(expected)
                    let url = try AppStore.socketURL(origin: service.origin, ticket: ticket.ticket)
                    // The websocket uses a one-use ticket, not an Authorization default header.
                    let socket = service.session.webSocketTask(with: url)
                    self.socket = socket; socket.resume()
                    let heartbeat = Task {
                        while !Task.isCancelled {
                            try await Task.sleep(for: .seconds(20))
                            try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
                                socket.sendPing { error in
                                    if let error { continuation.resume(throwing: error) }
                                    else { continuation.resume() }
                                }
                            }
                        }
                    }
                    defer { heartbeat.cancel(); socket.cancel(with: .goingAway, reason: nil) }
                    // Every new connection refetches. Den does not promise event replay.
                    try await self.refresh(); try self.check(expected); retry = 1
                    Task { await self.notifications?.registerIfAuthorized() }
                    Task { await self.voip?.sessionRestored(); await self.calls?.reconcileInvitations() }
                    while !Task.isCancelled {
                        let incoming = try await socket.receive()
                        try self.check(expected)
                        let data: Data
                        switch incoming {
                        case .data(let bytes): data = bytes
                        case .string(let text): data = Data(text.utf8)
                        @unknown default: continue
                        }
                        try await self.receive(data)
                    }
                } catch {
                    guard !Task.isCancelled, self.generation == expected else { return }
                    if DenFailure.unauthorized(error) { self.report(error); return }
                    self.recordSyncFailure(error)
                    if self.syncProblem == .incompatibleResponse { return }
                    try? await Task.sleep(for: .seconds(retry))
                    retry = min(retry * 2, 30)
                }
            }
        }
    }
    /// The only query item is the one-use ticket. Den gates music and sound events behind
    /// `?music=`/`?sounds=` opt-ins for clients that consume them; iOS has neither consumer,
    /// so it stays opted out and ignores the tags it does not route instead.
    static func socketURL(origin: URL, ticket: String) throws -> URL {
        guard var components = URLComponents(url: origin, resolvingAgainstBaseURL: false) else { throw DenFailure.invalidOrigin }
        components.scheme = origin.scheme == "https" ? "wss" : "ws"
        components.path = "/ws"; components.queryItems = [.init(name: "ticket", value: ticket)]
        guard let url = components.url else { throw DenFailure.invalidOrigin }
        return url
    }
    func foreground() {
        guard service != nil else { return }
        connectSocket()
        Task { await notifications?.registerIfAuthorized(retryImmediately: true) }
        Task { await voip?.sessionRestored(); await calls?.reconcileInvitations() }
    }
    func sendTyping(channelId: String) {
        guard Date().timeIntervalSince(lastTyping) >= 3, let socket else { return }
        lastTyping = Date()
        guard let frame = AppStore.typingFrame(channelId: channelId) else { return }
        Task { try? await socket.send(.string(frame)) }
    }
    /// ClientEvent is generated from the shared schema; typing is the only event iOS emits.
    /// Its variants are inline, so every generated payload type is named by oneOf position,
    /// and `object_open`/`object_close` are shape-identical. Regeneration could therefore
    /// renumber them and still compile. A test pins the bytes this produces to the wire tag.
    static func typingFrame(channelId: String) -> String? {
        let event = API.ClientEvent.case4(.init(channelId: channelId, _type: .typing))
        guard let data = try? JSONEncoder().encode(event) else { return nil }
        return String(decoding: data, as: UTF8.self)
    }
    func receive(_ data: Data) async throws {
        // Route by the wire tag rather than generator-assigned oneOf case numbers.
        // Decodable payloads remain the shared generated schema models.
        guard let value = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              let type = value["type"] as? String else { return }
        func field<T: Decodable>(_ key: String, as: T.Type) throws -> T {
            guard let raw = value[key] else { throw DenFailure.invalidResponse }
            return try JSONDecoder().decode(T.self, from: JSONSerialization.data(withJSONObject: raw))
        }
        switch type {
        case "message_created", "message_edited":
            let message = try JSONDecoder().decode(API.Message.self, from: data)
            if !channels.contains(where: { $0.id == message.channelId }) { try await refresh() }
            let loaded = messages[message.channelId]?.contains(where: { $0.id == message.id }) == true
            if loaded || (type == "message_created" && messageHasNewer[message.channelId] != true) {
                merge([message], channelId: message.channelId)
            }
            typing[message.channelId]?[message.authorId] = nil
            saveCache()
        case "message_deleted":
            if let channel = value["channel_id"] as? String, let id = value["id"] as? String {
                messages[channel]?.removeAll { $0.id == id }; saveCache()
            }
        case "reactions_updated":
            if let channel = value["channel_id"] as? String, let id = value["message_id"] as? String,
               let index = messages[channel]?.firstIndex(where: { $0.id == id }) {
                messages[channel]?[index].reactions = try field("reactions", as: [API.Reaction].self)
            }
        case "typing":
            if let channel = value["channel_id"] as? String, let id = value["user_id"] as? String, id != user?.id {
                typing[channel, default: [:]][id] = Date()
            }
        case "presence":
            if let id = value["user_id"] as? String, let online = value["online"] as? Bool {
                if online { presence.insert(id) } else { presence.remove(id) }
            }
        case "call_state":
            if let channel = value["channel_id"] as? String, let ids = value["participant_ids"] as? [String] {
                callStates.removeAll { $0.channelId == channel }
                callStates.append(.init(channelId: channel, participantIds: ids))
            }
        case "call_invitation_state":
            let call = try field("call", as: API.CallInvitationState.self)
            await calls?.receiveAuthenticated(call)
        case "user_updated": apply(try field("user", as: API.User.self))
        case "appearance_updated": theme.receive(try field("appearance", as: API.Appearance.self))
        case "notification_preferences_updated": preferences = try field("preferences", as: API.NotificationPreferences.self)
        case "read_state_updated":
            let state = try field("state", as: API.ChannelReadState.self)
            readStates.removeAll { $0.channelId == state.channelId }; readStates.append(state)
            saveCache(); await notifications?.updateBadge()
        case "notification":
            if let service {
                let expected = generation
                let reads = try await service.readStates(); try check(expected); readStates = reads
                await notifications?.updateBadge()
            }
        case "settings_updated", "resync": try await refresh()
        default: break // M5c live objects stay as native placeholder cards.
        }
    }
}
