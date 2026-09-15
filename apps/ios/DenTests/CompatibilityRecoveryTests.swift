import Foundation
import DenAPI
import Synchronization
import Testing
@testable import Den

@Suite(.serialized) struct CompatibilityRecoveryTests {
    @Test @MainActor func unreadableBootstrapPreservesSessionAndCacheUntilFullRefreshRecovers() async throws {
        CompatibilityStub.reset()
        let store = makeStore()
        try SessionVault.save("test-only-token", origin: store.origin)
        store.user = user()
        store.channels = [.init(id: "cached-room", kind: .text, memberIds: [], name: "Cached room", position: 0)]
        store.messages["cached-room"] = [.init(attachments: [], authorId: "compatibility-user", channelId: "cached-room",
            content: "Available while updating", createdAt: "2026-09-15T12:00:00Z", id: "cached-message")]
        store.saveCache()
        let generation = store.generation
        let service = store.service
        defer { cleanUp(store) }
        // Exercise the same generated ClientError and restore catch as a cold launch.
        CompatibilityStub.overrides.withLock { $0["/users/me/appearance"] = #"{"mode":"system","light_theme":"den","custom_themes":[]}"# }
        store.user = nil; store.channels = []; store.messages = [:]
        await store.restore()

        #expect(store.syncProblem == .incompatibleResponse)
        #expect(store.offline)
        #expect(store.user?.id == "compatibility-user")
        #expect(store.channels.map(\.id) == ["cached-room"])
        #expect(store.messages["cached-room"]?.first?.content == "Available while updating")
        #expect(OfflineCache.read(origin: store.origin)?.channels.map(\.id) == ["cached-room"])
        #expect(try SessionVault.read(store.origin) == "test-only-token")
        #expect(store.service === service)
        #expect(store.generation == generation)
        #expect(CompatibilityStub.ticketRequests.withLock { $0 } == 0, "A schema failure must not start an automatic reconnect loop.")

        try await store.loadMessages(channelId: "cached-room")
        #expect(store.syncProblem == .incompatibleResponse, "One readable endpoint does not prove the bootstrap contract is compatible.")
        CompatibilityStub.overrides.withLock { $0.removeValue(forKey: "/users/me/appearance") }
        await store.retrySync()
        // Recovery has to restore realtime too; the schema failure cancelled that loop.
        #expect(store.socketLoop != nil)
        store.stopNetwork()
        #expect(store.syncProblem == nil)
        #expect(store.error == nil)
        #expect(!store.offline)
        #expect(store.channels.map(\.id) == ["current-room"])
        #expect(store.instanceName == "Compatible server")
        #expect(try SessionVault.read(store.origin) == "test-only-token")
    }

    @Test @MainActor func networkBootstrapFailureRemainsOfflineAndCanRecover() async throws {
        CompatibilityStub.reset()
        let store = makeStore()
        defer { cleanUp(store) }
        CompatibilityStub.networkFailures.withLock { $0.insert("/users/me/appearance") }
        do {
            try await store.refresh()
            Issue.record("The simulated connection loss must reach the refresh catch.")
        } catch {
            #expect(error is ClientError)
            #expect(!DenFailure.incompatible(error))
            #expect(DenFailure.present(error) == DenFailure.offline.localizedDescription)
        }
        #expect(store.syncProblem == .networkOffline)
        CompatibilityStub.networkFailures.withLock { $0.insert("/auth/ws-ticket") }
        store.connectSocket()
        for _ in 0..<150 {
            if CompatibilityStub.ticketRequests.withLock({ $0 }) >= 2 { break }
            try await Task.sleep(for: .milliseconds(10))
        }
        #expect(CompatibilityStub.ticketRequests.withLock { $0 } >= 2, "Network failures retain automatic reconnection.")
        #expect(store.syncProblem == .networkOffline)
        store.socketLoop?.cancel(); store.socketLoop = nil
        CompatibilityStub.networkFailures.withLock { $0.removeAll() }
        try await store.refresh()
        #expect(store.syncProblem == nil)
        #expect(store.user?.id == "compatibility-user")
    }

    @Test @MainActor func invalidKnownResponseStopsSocketRetriesUntilExplicitRetry() async throws {
        CompatibilityStub.reset()
        let store = makeStore()
        defer { cleanUp(store) }
        CompatibilityStub.overrides.withLock { $0["/auth/ws-ticket"] = #"{"ticket":5,"expires_in":30}"# }
        store.connectSocket()
        for _ in 0..<100 {
            if store.syncProblem == .incompatibleResponse { break }
            try await Task.sleep(for: .milliseconds(10))
        }
        #expect(store.syncProblem == .incompatibleResponse)
        #expect(store.offline)
        try await Task.sleep(for: .milliseconds(1_150))
        #expect(CompatibilityStub.ticketRequests.withLock { $0 } == 1, "Do not retry the same unreadable response every second.")

        store.foreground()
        for _ in 0..<100 {
            if CompatibilityStub.ticketRequests.withLock({ $0 }) >= 2 { break }
            try await Task.sleep(for: .milliseconds(10))
        }
        #expect(CompatibilityStub.ticketRequests.withLock { $0 } == 2, "Foreground is an explicit opportunity to recheck compatibility.")
        #expect(store.syncProblem == .incompatibleResponse)
        store.report(DenFailure.invalidResponse)
        #expect(store.error == DenFailure.updateRequired.localizedDescription)
    }

    @MainActor private func makeStore() -> AppStore {
        let store = AppStore()
        store.origin = URL(string: "https://compatibility-recovery.test")!
        store.theme.reset(origin: store.origin)
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [CompatibilityStub.self]
        store.service = DenService(origin: store.origin, token: "test-only-token", sessionConfiguration: configuration)
        return store
    }
    @MainActor private func cleanUp(_ store: AppStore) {
        store.stopNetwork()
        try? SessionVault.delete(store.origin)
        OfflineCache.remove(origin: store.origin)
    }
    private func user() -> Components.Schemas.User {
        .init(bot: false, displayName: "Compatibility test", id: "compatibility-user", role: .member, username: "compatibility")
    }
}

private final class CompatibilityStub: URLProtocol, @unchecked Sendable {
    static let overrides = Mutex([String: String]())
    static let networkFailures = Mutex(Set<String>())
    static let ticketRequests = Mutex(0)
    static func reset() {
        overrides.withLock { $0.removeAll() }
        networkFailures.withLock { $0.removeAll() }
        ticketRequests.withLock { $0 = 0 }
    }
    override class func canInit(with request: URLRequest) -> Bool { request.url?.host == "compatibility-recovery.test" }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func startLoading() {
        let path = request.url!.path
        if path == "/auth/ws-ticket" { Self.ticketRequests.withLock { $0 += 1 } }
        if Self.networkFailures.withLock({ $0.contains(path) }) {
            client?.urlProtocol(self, didFailWithError: URLError(.networkConnectionLost)); return
        }
        let body = Self.overrides.withLock { $0[path] } ?? Self.body(path)
        let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: "HTTP/1.1", headerFields: ["Content-Type": "application/json"])!
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: Data(body.utf8))
        client?.urlProtocolDidFinishLoading(self)
    }
    override func stopLoading() {}
    private static func body(_ path: String) -> String {
        switch path {
        case "/users/me": #"{"id":"compatibility-user","username":"compatibility","display_name":"Compatibility test","bot":false,"role":"member"}"#
        case "/channels": #"[{"id":"current-room","kind":"text","name":"Current room","position":0,"member_ids":[]}]"#
        case "/users/me/appearance": #"{"mode":"system","light_theme":"den","dark_theme":"den","custom_themes":[]}"#
        case "/users/me/notification-preferences": #"{"dms":true,"mentions":true,"subscribed_channel_ids":[]}"#
        case "/presence": #"{"online_user_ids":[],"objects":[]}"#
        case "/instance": #"{"instance_name":"Compatible server","version":"0.3.0"}"#
        default: "[]"
        }
    }
}
