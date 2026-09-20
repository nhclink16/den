import Foundation
import DenAPI
import Synchronization
import Testing
import UserNotifications
@testable import Den

@Suite(.serialized) struct NotificationRecoveryTests {
    @Test @MainActor func foregroundRechecksPermissionRetriesFailedRegistrationAndDeduplicatesSuccess() async throws {
        NotificationRecoveryStub.devicePosts.withLock { $0 = 0 }
        NotificationRecoveryStub.deviceRequests.withLock { $0.removeAll() }
        let installationID = VoIPPushInstallation.clientID().uuidString
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [NotificationRecoveryStub.self]
        let store = AppStore()
        store.origin = URL(string: "https://notifications.test")!
        store.user = user()
        let service = DenService(origin: store.origin, token: "test-only-token", sessionConfiguration: configuration)
        store.service = service
        let deviceKey = "den.device:https://notifications.test:notification-user"
        UserDefaults.standard.removeObject(forKey: deviceKey)
        defer {
            store.stopNetwork()
            UserDefaults.standard.removeObject(forKey: deviceKey)
        }
        let permission = Mutex(UNAuthorizationStatus.denied)
        var permissionChecks = 0
        var systemRegistrations = 0
        let controller = NotificationController(store: store,
            authorizationStatus: { permissionChecks += 1; return permission.withLock { $0 } },
            requestAuthorization: { false },
            registerRemote: { systemRegistrations += 1 })
        store.notifications = controller

        await controller.registered(token: "01020304")
        #expect(NotificationRecoveryStub.devicePosts.withLock { $0 } == 1)
        #expect(controller.status != nil)
        #expect(UserDefaults.standard.string(forKey: deviceKey) == nil)
        // Reconnect callbacks during an outage must not post the same token in a tight loop.
        await controller.registered(token: "01020304")
        #expect(NotificationRecoveryStub.devicePosts.withLock { $0 } == 1)
        await controller.registerIfAuthorized()
        #expect(systemRegistrations == 0)

        // Simulate returning from Settings after granting permission. The actual foreground
        // entry point must recheck it and retry the cached token without another OS callback.
        permission.withLock { $0 = .authorized }
        store.foreground()
        for _ in 0..<100 {
            if controller.status == nil, NotificationRecoveryStub.devicePosts.withLock({ $0 }) == 2 { break }
            try await Task.sleep(for: .milliseconds(20))
        }
        #expect(permissionChecks >= 2)
        #expect(systemRegistrations >= 1)
        #expect(NotificationRecoveryStub.devicePosts.withLock { $0 } == 2)
        let requests = NotificationRecoveryStub.deviceRequests.withLock { $0 }
        #expect(requests.count == 2)
        for request in requests {
            let payload = try #require(try JSONSerialization.jsonObject(with: request.body) as? [String: Any])
            #expect(payload["purpose"] as? String == "alert")
            #expect(payload["environment"] as? String == "sandbox")
            #expect(payload["client_id"] as? String == installationID)
            #expect(payload["token"] as? String == "01020304")
            #expect(payload["platform"] as? String == "ios")
            #expect(payload["app_version"] as? String == Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String)
            #expect(request.authorization == "Bearer test-only-token")
        }
        #expect(VoIPPushInstallation.clientID().uuidString == installationID)
        #expect(controller.status == nil)
        #expect(UserDefaults.standard.string(forKey: deviceKey) == "device-1")
        await controller.registerIfAuthorized()
        await controller.registered(token: "01020304")
        #expect(NotificationRecoveryStub.devicePosts.withLock { $0 } == 2)
    }

    @Test @MainActor func pendingNotificationTapOpensAfterLoginEvenWhenPermissionIsDenied() async {
        let store = AppStore()
        var permissionRequests = 0
        let controller = NotificationController(store: store,
            authorizationStatus: { .denied },
            requestAuthorization: { permissionRequests += 1; return false },
            registerRemote: { Issue.record("Denied permission must not register with APNs.") })
        // This is the same app-level handler used by the notification-center delegate.
        await controller.handleTap(channel: "requested-room", message: "requested-message")
        #expect(store.selectedChannelId == nil)
        store.user = user()
        await controller.requestAfterLogin()
        #expect(permissionRequests == 1)
        #expect(store.selectedChannelId == "requested-room")
        #expect(store.targetMessageId == "requested-message")
        store.selectChannel("another-room")
        await controller.requestAfterLogin()
        #expect(store.selectedChannelId == "another-room", "A queued tap must be consumed only once.")
    }

    private func user() -> Components.Schemas.User {
        .init(bot: false, displayName: "Notification test", id: "notification-user", role: .member, username: "notification-test")
    }
}

private final class NotificationRecoveryStub: URLProtocol, @unchecked Sendable {
    struct DeviceRequest: Sendable { let body: Data; let authorization: String? }
    static let devicePosts = Mutex(0)
    static let deviceRequests = Mutex([DeviceRequest]())
    override class func canInit(with request: URLRequest) -> Bool { request.url?.host == "notifications.test" }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func startLoading() {
        let isDevice = request.httpMethod == "POST" && request.url?.path == "/devices"
        if isDevice {
            Self.deviceRequests.withLock { $0.append(.init(body: Self.body(request), authorization: request.value(forHTTPHeaderField: "Authorization"))) }
        }
        let attempt = isDevice ? Self.devicePosts.withLock { $0 += 1; return $0 } : 0
        let status = isDevice && attempt > 1 ? 200 : 503
        let body = status == 200
            ? Data(#"{"id":"device-1","platform":"ios","app_version":"0","purpose":"alert","environment":"sandbox"}"#.utf8)
            : Data(#"{"error":"unavailable","message":"Temporary fixture outage"}"#.utf8)
        let response = HTTPURLResponse(url: request.url!, statusCode: status, httpVersion: "HTTP/1.1", headerFields: ["Content-Type": "application/json"])!
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: body)
        client?.urlProtocolDidFinishLoading(self)
    }
    override func stopLoading() {}
    private static func body(_ request: URLRequest) -> Data {
        if let body = request.httpBody { return body }
        guard let stream = request.httpBodyStream else { return Data() }
        stream.open()
        defer { stream.close() }
        var body = Data(), buffer = [UInt8](repeating: 0, count: 4096)
        // A bound stream may temporarily have no available bytes before its writer runs.
        // Only read() returning zero is EOF; availability is not an end condition.
        while true {
            let count = stream.read(&buffer, maxLength: buffer.count)
            guard count > 0 else { break }
            body.append(buffer, count: count)
        }
        return body
    }
}
