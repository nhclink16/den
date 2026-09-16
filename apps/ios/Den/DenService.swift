import Foundation
import DenAPI

struct DenTransport: ClientTransport {
    let token: String?
    let underlying: URLSessionTransport
    func send(_ request: HTTPRequest, body: HTTPBody?, baseURL: URL, operationID: String) async throws -> (HTTPResponse, HTTPBody?) {
        var request = request
        if let token { request.headerFields[.authorization] = "Bearer \(token)" }
        let (response, responseBody) = try await underlying.send(request, body: body, baseURL: baseURL, operationID: operationID)
        guard (200..<300).contains(response.status.code) else {
            var message = "The server could not complete that request."
            if let responseBody, let data = try? await Data(collecting: responseBody, upTo: 64 * 1024),
               let error = try? JSONDecoder().decode(API.ApiError.self, from: data) { message = error.message }
            throw DenFailure.server(response.status.code, message)
        }
        return (response, responseBody)
    }
}

final class DenService: Sendable {
    let origin: URL
    let client: Client
    let session: URLSession
    private let token: String?
    init(origin: URL, token: String? = nil, sessionConfiguration: URLSessionConfiguration? = nil) {
        self.origin = origin; self.token = token
        let configuration = sessionConfiguration ?? URLSessionConfiguration.ephemeral
        configuration.httpShouldSetCookies = false
        configuration.httpCookieStorage = nil
        configuration.urlCache = nil
        configuration.timeoutIntervalForRequest = 30
        configuration.timeoutIntervalForResource = 120
        session = URLSession(configuration: configuration, delegate: NoRedirects(), delegateQueue: nil)
        client = Client(serverURL: origin, transport: DenTransport(token: token,
            underlying: URLSessionTransport(configuration: .init(session: session))))
    }
    func close() { session.invalidateAndCancel() }
    func login(username: String, password: String) async throws -> API.Session {
        try await client.postAuthLogin(body: .json(.init(password: password, username: username))).ok.body.json
    }
    func me() async throws -> API.User { try await client.getUsersMe().ok.body.json }
    func channels() async throws -> [API.Channel] { try await client.getChannels().ok.body.json }
    func categories() async throws -> [API.Category] { try await client.getCategories().ok.body.json }
    func users() async throws -> [API.User] { try await client.getUsers().ok.body.json }
    func appearance() async throws -> API.Appearance { try await client.getUsersMeAppearance().ok.body.json }
    func preferences() async throws -> API.NotificationPreferences { try await client.getUsersMeNotificationPreferences().ok.body.json }
    func readStates() async throws -> [API.ChannelReadState] { try await client.getUsersMeReadState().ok.body.json }
    func presence() async throws -> API.PresenceState { try await client.getPresence().ok.body.json }
    func calls() async throws -> [API.CallState] { try await client.getCalls().ok.body.json }
    func instance() async throws -> API.Instance { try await client.getInstance().ok.body.json }
    func hosts() async throws -> [API.Host] { try await client.getHosts().ok.body.json }
    func grants() async throws -> [API.Grant] { try await client.getGrants().ok.body.json }
    func messages(channelId: String, before: String? = nil, after: String? = nil,
                  rootsOnly: Bool? = nil) async throws -> [API.Message] {
        try await client.getChannelsIdMessages(path: .init(id: channelId),
            query: .init(before: before, after: after, limit: 100, rootsOnly: rootsOnly)).ok.body.json
    }
    func message(id: String) async throws -> API.Message { try await client.getMessagesId(path: .init(id: id)).ok.body.json }
    func search(query: String, channelId: String?) async throws -> [API.Message] {
        try await client.getSearchMessages(query: .init(q: query, channelId: channelId, limit: 100)).ok.body.json
    }
    func send(channelId: String, content: String, replyTo: String?, uploads: [String]) async throws -> API.Message {
        try await client.postChannelsIdMessages(path: .init(id: channelId),
            body: .json(.init(content: content, replyTo: replyTo, uploadIds: uploads))).ok.body.json
    }
    func edit(id: String, content: String) async throws -> API.Message {
        try await client.patchMessagesId(path: .init(id: id), body: .json(.init(content: content))).ok.body.json
    }
    func delete(id: String) async throws { _ = try await client.deleteMessagesId(path: .init(id: id)).noContent }
    func react(id: String, emoji: String, remove: Bool) async throws {
        if remove { _ = try await client.deleteMessagesIdReactions(path: .init(id: id), body: .json(.init(emoji: emoji))).ok.body.json }
        else { _ = try await client.putMessagesIdReactions(path: .init(id: id), body: .json(.init(emoji: emoji))).ok.body.json }
    }
    func openDM(userIds: [String]) async throws -> API.Channel {
        try await client.postDms(body: .json(.init(memberIds: userIds))).ok.body.json
    }
    func markRead(channelId: String, messageId: String, rootsOnly: Bool? = nil) async throws -> API.ChannelReadState {
        try await client.putChannelsIdRead(path: .init(id: channelId),
            body: .json(.init(messageId: messageId, rootsOnly: rootsOnly))).ok.body.json
    }

    // MARK: - Conversations
    //
    // The whole thread surface, wrapped here so the store never reaches into the
    // generated client. Nothing in this PR calls these: the UI stays flat until
    // the activation PR. A page from `threads` is FILTERED and PAGINATED and
    // never proves a thread is absent.
    func threads(channelId: String, resolved: Bool? = nil, unreadOnly: Bool? = nil,
                 before: String? = nil, limit: Int32 = 50) async throws -> [API.ThreadView] {
        try await client.getChannelsIdThreads(path: .init(id: channelId),
            query: .init(resolved: resolved, unreadOnly: unreadOnly, before: before, limit: limit)).ok.body.json
    }
    /// Metadata plus this account's own read state. A REPLY carries only
    /// `threadId`, so resolving one to its conversation comes through here; a
    /// ROOT already carries its summary inline and needs no lookup.
    func thread(id: String) async throws -> API.ThreadView {
        try await client.getThreadsId(path: .init(id: id)).ok.body.json
    }
    func threadMessages(id: String, before: String? = nil, after: String? = nil) async throws -> [API.Message] {
        try await client.getThreadsIdMessages(path: .init(id: id),
            query: .init(before: before, after: after, limit: 100)).ok.body.json
    }
    /// Rename, resolve or reopen. An omitted field is left alone by the server.
    func updateThread(id: String, title: String? = nil, resolved: Bool? = nil) async throws -> API.ThreadSummary {
        try await client.patchThreadsId(path: .init(id: id),
            body: .json(.init(title: title, resolved: resolved))).ok.body.json
    }
    /// Advances only this thread. Never touches the room position, never follows.
    func markThreadRead(id: String, messageId: String) async throws -> API.ThreadReadState {
        try await client.putThreadsIdRead(path: .init(id: id), body: .json(.init(messageId: messageId))).ok.body.json
    }
    /// Follow advances to the current tail; unfollow keeps the position.
    func followThread(id: String, following: Bool) async throws -> API.ThreadReadState {
        try await client.putThreadsIdFollow(path: .init(id: id), body: .json(.init(following: following))).ok.body.json
    }
    func saveAppearance(_ value: API.Appearance) async throws -> API.Appearance {
        try await client.putUsersMeAppearance(body: .json(value)).ok.body.json
    }
    func savePreferences(_ value: API.NotificationPreferences) async throws -> API.NotificationPreferences {
        try await client.putUsersMeNotificationPreferences(body: .json(value)).ok.body.json
    }
    func logout() async throws { _ = try await client.postAuthLogout().noContent }
    func ticket() async throws -> API.WsTicket { try await client.postAuthWsTicket().ok.body.json }
    func beginUpload(channelId: String, filename: String, contentType: String, size: Int64) async throws -> API.Upload {
        try await client.postUploads(body: .json(.init(channelId: channelId, contentType: contentType, filename: filename, size: size))).ok.body.json
    }
    func uploadStatus(id: String) async throws -> API.Upload { try await client.getUploadsId(path: .init(id: id)).ok.body.json }
    func uploadChunk(id: String, offset: Int64, data: Data) async throws -> API.Upload {
        try await client.patchUploadsId(path: .init(id: id), headers: .init(uploadOffset: offset), body: .binary(HTTPBody(data))).ok.body.json
    }
    func completeUpload(id: String) async throws -> API.Upload { try await client.postUploadsIdComplete(path: .init(id: id)).ok.body.json }
    func deleteUpload(id: String) async throws { _ = try await client.deleteUploadsId(path: .init(id: id)).noContent }
    func authenticatedRequest(path: String, range: String? = nil) -> URLRequest {
        var request = URLRequest(url: origin.appending(path: path))
        if let token { request.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization") }
        if let range { request.setValue(range, forHTTPHeaderField: "Range") }
        return request
    }
}
