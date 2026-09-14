import DenAPI
import Foundation
import Synchronization
import Testing
@testable import Den

@Suite(.serialized) struct NativeCallAPITests {
    @Test @MainActor func ticketRedemptionUsesSelectedOriginWithoutBearerOrAccountRestoration() async throws {
        let expected = state()
        let bytes = try JSONEncoder().encode(expected)
        NativeCallTransportStub.reset { _ in .response(200, bytes) }
        let (store, api, service) = context()
        defer { service.close(); NativeCallTransportStub.clear() }
        let originalUser = store.user
        let ticket = "https://untrusted-push.test/not-a-server?opaque=state-only-test-ticket"

        let result = try await api.invitation(id: expected.invitation.id, fetchTicket: ticket)

        #expect(result == expected)
        #expect(store.user == originalUser)
        #expect(store.service === service)
        #expect(NativeCallTransportStub.anonymousOrigins.withLock { $0 } == [store.origin])
        let requests = NativeCallTransportStub.requests.withLock { $0 }
        #expect(requests.count == 1, "Redeeming a state-only ticket must not restore identity or join media.")
        let request = try #require(requests.first)
        #expect(request.method == "POST")
        #expect(request.url.absoluteString == "https://trusted-calls.test/calls/invitations/redeem")
        #expect(request.url.query == nil)
        #expect(request.authorization == nil, "A fetch ticket must not be accompanied by the account bearer.")
        let body = try JSONDecoder().decode(Components.Schemas.RedeemCallInvitation.self, from: request.body)
        #expect(body.ticket == ticket)
    }

    @Test @MainActor func mismatchedRedeemedInvitationIsRejectedWithoutCredentialedFallback() async throws {
        let wrong = try JSONEncoder().encode(state(id: "unrelated-invitation"))
        let correct = try JSONEncoder().encode(state())
        let identity = try JSONEncoder().encode(user())
        NativeCallTransportStub.reset { request in
            switch request.url?.path {
            case "/calls/invitations/redeem": .response(200, wrong)
            case "/users/me": .response(200, identity)
            default: .response(200, correct)
            }
        }
        let (store, api, service) = context()
        defer { service.close(); NativeCallTransportStub.clear(); _ = store }
        var result: Components.Schemas.CallInvitationState?
        var failure: Error?
        do { result = try await api.invitation(id: "wanted-invitation", fetchTicket: "test-ticket") }
        catch { failure = error }

        #expect(result == nil)
        #expect(failure != nil)
        #expect(NativeCallTransportStub.requests.withLock { $0.map(\.url.path) } == ["/calls/invitations/redeem"],
                "A mismatched success is a protocol violation, not an expired ticket to retry with credentials.")
    }

    @Test @MainActor func expiredTicketReconcilesThroughAuthenticatedIdentityAndInvitationRead() async throws {
        let expired = try JSONEncoder().encode(Components.Schemas.ApiError(error: "gone", message: "Ticket expired"))
        let expected = state()
        let bytes = try JSONEncoder().encode(expected)
        let identity = try JSONEncoder().encode(user())
        NativeCallTransportStub.reset { request in
            switch request.url?.path {
            case "/calls/invitations/redeem": .response(410, expired)
            case "/users/me": .response(200, identity)
            default: .response(200, bytes)
            }
        }
        let (store, api, service) = context()
        defer { service.close(); NativeCallTransportStub.clear(); _ = store }

        let result = try await api.invitation(id: expected.invitation.id, fetchTicket: "expired-test-ticket")

        #expect(result == expected)
        let requests = NativeCallTransportStub.requests.withLock { $0 }
        #expect(requests.map(\.url.path) == ["/calls/invitations/redeem", "/users/me", "/calls/invitations/wanted-invitation"])
        #expect(requests.map(\.method) == ["POST", "GET", "GET"])
        #expect(requests.map(\.authorization) == [nil, "Bearer test-account-token", "Bearer test-account-token"])
        #expect(requests.allSatisfy { $0.url.host == "trusted-calls.test" && $0.url.query == nil })
        #expect(store.user?.id == "call-user")
    }

    @Test @MainActor func changedGenerationRejectsDelayedTicketAndDoesNotRestoreOldIdentity() async throws {
        for stage in ["ticket", "identity", "authenticated-invitation"] {
            let response: Data
            if stage == "ticket" { response = try JSONEncoder().encode(state()) }
            else if stage == "identity" { response = try JSONEncoder().encode(user()) }
            else { response = try JSONEncoder().encode(state()) }
            let identity = try JSONEncoder().encode(user())
            NativeCallTransportStub.reset { request in
                if stage == "authenticated-invitation", request.url?.path == "/users/me" {
                    return .response(200, identity)
                }
                return .held(200, response)
            }
            let (store, api, service) = context()
            // Identity restoration starts with a retained service and an unresolved user, as in
            // an interrupted cold-start restore. A generation change invalidates that work.
            if stage == "identity" { store.user = nil }
            let pending = Task { @MainActor () throws -> Bool in
                if stage == "ticket" {
                    _ = try await api.invitation(id: "wanted-invitation", fetchTicket: "delayed-test-ticket")
                } else if stage == "identity" {
                    _ = try await api.restoreCallSession()
                } else {
                    _ = try await api.invitation(id: "wanted-invitation", fetchTicket: nil)
                }
                return true
            }
            defer { pending.cancel(); service.close(); NativeCallTransportStub.clear() }
            for _ in 0..<200 {
                if NativeCallTransportStub.pending.withLock({ $0 != nil }) { break }
                try await Task.sleep(for: .milliseconds(5))
            }
            let responseToRelease = try #require(NativeCallTransportStub.pending.withLock { $0 })
            store.generation = UUID()
            if stage != "authenticated-invitation" { store.user = nil }
            responseToRelease.finish()
            var applied = false
            var failure: Error?
            do { applied = try await pending.value } catch { failure = error }

            #expect(!applied, "An old generation must not return a usable \(stage) result.")
            #expect(failure != nil)
            if stage != "authenticated-invitation" { #expect(failure is CancellationError) }
            if stage != "authenticated-invitation" {
                #expect(store.user == nil, "A delayed identity response must not sign the previous account back in.")
            } else {
                #expect(store.user?.id == "call-user")
            }
            #expect(NativeCallTransportStub.requests.withLock { $0.count } == (stage == "authenticated-invitation" ? 2 : 1),
                    "Generation cancellation must stop follow-up requests.")
        }
    }

    @MainActor private func context() -> (AppStore, NativeCallAPI, DenService) {
        let store = AppStore()
        store.origin = URL(string: "https://trusted-calls.test")!
        store.user = user()
        let service = makeService(origin: store.origin, token: "test-account-token")
        store.service = service
        let api = NativeCallAPI(store: store, anonymousService: { origin in
            NativeCallTransportStub.anonymousOrigins.withLock { $0.append(origin) }
            return makeService(origin: origin)
        })
        return (store, api, service)
    }

    private func state(id: String = "wanted-invitation") -> Components.Schemas.CallInvitationState {
        .init(accepted: [], declinedUserIds: [], invitation: .init(channelId: "dm-room",
            expiresAt: Int64(Date().addingTimeInterval(45).timeIntervalSince1970), fromUserId: "caller", id: id), state: .ringing)
    }
    private func user() -> Components.Schemas.User {
        .init(bot: false, displayName: "Call test user", id: "call-user", role: .member, username: "call-test")
    }
}

private func makeService(origin: URL, token: String? = nil) -> DenService {
    let configuration = URLSessionConfiguration.ephemeral
    configuration.protocolClasses = [NativeCallTransportStub.self]
    return DenService(origin: origin, token: token, sessionConfiguration: configuration)
}

private final class NativeCallTransportStub: URLProtocol, @unchecked Sendable {
    enum Reply: Sendable { case response(Int, Data), held(Int, Data) }
    struct Request: Sendable { let method: String; let url: URL; let authorization: String?; let body: Data }
    struct Pending: @unchecked Sendable {
        let transport: NativeCallTransportStub
        let status: Int
        let data: Data
        func finish() { transport.respond(status: status, data: data) }
    }
    static let route = Mutex<(@Sendable (URLRequest) -> Reply)?>(nil)
    static let requests = Mutex([Request]())
    static let anonymousOrigins = Mutex([URL]())
    static let pending = Mutex<Pending?>(nil)
    static func reset(_ handler: @escaping @Sendable (URLRequest) -> Reply) {
        clear(); route.withLock { $0 = handler }
    }
    static func clear() {
        route.withLock { $0 = nil }; requests.withLock { $0.removeAll() }
        anonymousOrigins.withLock { $0.removeAll() }; pending.withLock { $0 = nil }
    }
    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func startLoading() {
        guard let url = request.url, let route = Self.route.withLock({ $0 }) else {
            client?.urlProtocol(self, didFailWithError: URLError(.badURL)); return
        }
        Self.requests.withLock { $0.append(.init(method: request.httpMethod ?? "", url: url,
            authorization: request.value(forHTTPHeaderField: "Authorization"), body: Self.body(request))) }
        switch route(request) {
        case let .response(status, data): respond(status: status, data: data)
        case let .held(status, data): Self.pending.withLock { $0 = .init(transport: self, status: status, data: data) }
        }
    }
    func respond(status: Int, data: Data) {
        let response = HTTPURLResponse(url: request.url!, statusCode: status, httpVersion: "HTTP/1.1",
            headerFields: ["Content-Type": "application/json"])!
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: data)
        client?.urlProtocolDidFinishLoading(self)
    }
    override func stopLoading() {}
    private static func body(_ request: URLRequest) -> Data {
        if let body = request.httpBody { return body }
        guard let stream = request.httpBodyStream else { return Data() }
        stream.open(); defer { stream.close() }
        var result = Data(), buffer = [UInt8](repeating: 0, count: 4096)
        // A bound stream may temporarily have no available bytes before its writer runs.
        // Only read() returning zero is EOF; availability is not an end condition.
        while true {
            let count = stream.read(&buffer, maxLength: buffer.count)
            guard count > 0 else { break }
            result.append(buffer, count: count)
        }
        return result
    }
}
