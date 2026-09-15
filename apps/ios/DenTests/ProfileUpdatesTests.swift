import DenAPI
import Foundation
import Testing
@testable import Den

@Suite(.serialized) struct ProfileUpdatesTests {
    @Test @MainActor func peerRenameReachesEveryNameLabelAndTheOfflineCacheWithoutRefetching() async throws {
        let store = try loaded()
        defer { OfflineCache.remove(origin: store.origin) }

        try await store.receive(event(user(id: "user-ann", username: "ann", name: "Annabel")))

        #expect(store.userName("user-ann") == "Annabel")
        #expect(store.channelTitle(dm) == "Annabel", "A DM title is built from its other members' current names.")
        #expect(store.channelTitle(general) == "general", "A text channel keeps its own name.")
        #expect(MessagePresentation.mentions(in: "hey @ann", users: store.users).map(\.displayName) == ["Annabel"])
        #expect(store.callTitle(channelId: dm.id, incomingFrom: nil) == "Annabel")
        #expect(store.callTitle(channelId: dm.id, incomingFrom: "user-ann") == "Annabel")

        #expect(store.users.map(\.id) == ["user-me", "user-ann", "user-bo"], "A rename is an in-place upsert, not a reorder.")
        #expect(store.user?.displayName == "Me", "Another person's profile never replaces the signed-in identity.")
        #expect(store.messages["room-general"]?.map(\.id) == ["message-1"], "A rename must not discard loaded history.")
        #expect(store.readStates.first?.unreadCount == 3, "A rename must not clear unread counts.")
        #expect(store.syncProblem == .networkOffline, "A rename must not silently claim the session came back online.")
        #expect(store.selectedChannelId == "room-general")

        let cached = try #require(OfflineCache.read(origin: store.origin))
        #expect(cached.users.first { $0.id == "user-ann" }?.displayName == "Annabel")
        #expect(cached.messages["room-general"]?.map(\.id) == ["message-1"])
    }

    @Test @MainActor func ownProfileUpdateReplacesTheSignedInIdentityAndItsPeopleEntry() async throws {
        let store = try loaded()
        defer { OfflineCache.remove(origin: store.origin) }

        try await store.receive(event(user(id: "user-me", username: "me", name: "Nicholas")))

        #expect(store.user?.displayName == "Nicholas", "Settings reads the signed-in user, not the people list.")
        #expect(store.users.first { $0.id == "user-me" }?.displayName == "Nicholas")
        #expect(store.channelTitle(dm) == "Ann", "Renaming yourself does not relabel a DM with someone else.")
        #expect(OfflineCache.read(origin: store.origin)?.user.displayName == "Nicholas")
    }

    @Test @MainActor func anUnknownUserIsAddedSoTheirMessagesAreLabelledWithoutAFullRefetch() async throws {
        let store = try loaded()
        defer { OfflineCache.remove(origin: store.origin) }

        #expect(store.userName("user-cy") == "Someone")
        try await store.receive(event(user(id: "user-cy", username: "cy", name: "Cy")))

        #expect(store.userName("user-cy") == "Cy")
        #expect(store.users.map(\.id) == ["user-me", "user-ann", "user-bo", "user-cy"])
        #expect(store.messages["room-general"]?.count == 1, "Adding a user must not trigger the refresh that a missing channel does.")
    }

    @Test @MainActor func repeatingAUserUpdateChangesNothingAndKeepsUnrelatedStateIntact() async throws {
        let store = try loaded()
        defer { OfflineCache.remove(origin: store.origin) }
        let renamed = user(id: "user-ann", username: "ann", name: "Annabel")

        try await store.receive(event(renamed))
        let afterFirst = store.users
        store.typing["room-general"] = ["user-bo": Date(timeIntervalSince1970: 10)]
        try await store.receive(event(renamed))
        try await store.receive(event(renamed))

        #expect(store.users == afterFirst)
        #expect(store.users.count == 3)
        #expect(store.typing["room-general"]?["user-bo"] == Date(timeIntervalSince1970: 10))
        #expect(store.presence == ["user-ann"])
    }

    @Test @MainActor func anUnsupportedEventIsIgnoredAndTheKnownEventAfterItStillArrives() async throws {
        let store = try loaded()
        defer { OfflineCache.remove(origin: store.origin) }
        let unsupported: [[String: Any]] = [
            ["type": "sounds_updated", "user_id": "user-ann"],
            ["type": "voice_preferences_updated", "user_id": "user-ann", "preferences": ["camera": ["resolution": "hd"]]],
            ["type": "music_queue_updated", "queue": ["room_id": "room-voice", "tracks": []]],
            ["type": "a_tag_that_does_not_exist_yet", "payload": ["anything": 1]],
        ]
        for value in unsupported {
            try await store.receive(try JSONSerialization.data(withJSONObject: value))
        }
        #expect(store.userName("user-ann") == "Ann", "An ignored event must not have changed anything.")

        try await store.receive(event(user(id: "user-ann", username: "ann", name: "Annabel")))

        #expect(store.userName("user-ann") == "Annabel", "Delivery continues after an event this client does not route.")
        #expect(store.presence == ["user-ann"])
        #expect(store.syncProblem == .networkOffline)
    }

    @Test @MainActor func theEventSocketSendsOnlyItsOneUseTicketAndNeverOptsIntoMusicOrSounds() throws {
        let secure = try AppStore.socketURL(origin: try ServerOrigin.canonical("https://denchat.app"),
                                            ticket: "01JTESTTICKET0000000000001")
        #expect(secure.absoluteString == "wss://denchat.app/ws?ticket=01JTESTTICKET0000000000001",
                "Den gates music and sound events behind query opt-ins; iOS consumes neither.")
        let local = try AppStore.socketURL(origin: try ServerOrigin.canonical("http://127.0.0.1:7000"), ticket: "t")
        #expect(local.absoluteString == "ws://127.0.0.1:7000/ws?ticket=t")

        // A ticket is opaque server text. It must stay one value, never become a second parameter.
        let awkward = try AppStore.socketURL(origin: try ServerOrigin.canonical("https://denchat.app"),
                                             ticket: "a b&music=true")
        let items = try #require(URLComponents(url: awkward, resolvingAgainstBaseURL: false)?.queryItems)
        #expect(items.map(\.name) == ["ticket"])
        #expect(items.first?.value == "a b&music=true")
    }

    @Test @MainActor func theOnlyEmittedClientEventIsPinnedToItsWireTagNotItsOneOfPosition() throws {
        let frame = try #require(AppStore.typingFrame(channelId: "room-general"))
        let value = try #require(try JSONSerialization.jsonObject(with: Data(frame.utf8)) as? [String: String])
        // ClientEvent variants are named by oneOf position, not by tag. Compilation already
        // rejects a plain reorder; this pins the bytes against a regeneration, a generator
        // change, or a later adaptation of the call that still builds but emits something else.
        #expect(value == ["type": "typing", "channel_id": "room-general"])
    }

    @Test @MainActor func aRenameBeforeTheJoinSurvivesTheCredentialSnapshotAndALaterOneStillRetitles() async throws {
        let store = try loaded()
        defer { OfflineCache.remove(origin: store.origin) }
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [HeldCallTokenStub.self]
        let service = DenService(origin: try ServerOrigin.canonical("https://held-call.test"),
                                 token: "test-only-token", sessionConfiguration: configuration)
        defer { service.close() }
        let session = CallSession()
        let controller = CallController(session: session, api: UnusedCallAPI())
        defer { controller.provider.invalidate() }
        store.calls = controller
        let context = CallControllerContext(id: UUID(), channelID: "room-dm", title: "Ann", mode: .outgoingDM)
        controller.contexts[context.id] = context
        // Already in hand, as restoreCallSession would have returned them a moment ago.
        let stale = CallControllerCredentials(generation: store.generation, service: service,
            user: try #require(store.user), names: ["user-me": "Me", "user-ann": "Ann"])

        // The rename lands after those credentials were captured and before the join starts.
        try await store.receive(event(user(id: "user-ann", username: "ann", name: "Annabel")))
        #expect(context.title == "Annabel", "A call that has not joined yet still gets relabelled.")

        // The token request is never answered, so this parks at .connecting with names installed.
        let joining = Task { try? await controller.connectMedia(context, credentials: stale) }
        defer { joining.cancel() }
        try await waitFor { session.callID == context.id }
        #expect(session.phase == .connecting)

        #expect(session.names["user-ann"] == "Annabel",
                "The join must not install the credential snapshot taken before the rename.")
        #expect(session.names["user-me"] == "Me", "The rest of the snapshot is not dropped.")
        #expect(session.names["user-bo"] == "Bo",
                "A peer absent from the credential snapshot shows which map was installed.")
        #expect(session.title == "Annabel", "The join is handed context.title, already corrected.")

        // A second rename, this time while the call is connecting, must still reach the dock.
        try await store.receive(event(user(id: "user-ann", username: "ann", name: "Ann B.")))

        #expect(context.title == "Ann B.")
        #expect(session.title == "Ann B.", "The dock and call sheet read CallSession.title.")
        #expect(session.names["user-ann"] == "Ann B.")
        #expect(controller.names["user-ann"] == "Ann B.")

        joining.cancel()
        _ = await joining.value
    }

    @Test @MainActor func aFinishedCallAnUnloadedRoomAndAnUnjoinedSessionAreAllLeftAlone() async throws {
        let store = try loaded()
        defer { OfflineCache.remove(origin: store.origin) }
        let session = CallSession()
        let controller = CallController(session: session, api: UnusedCallAPI())
        defer { controller.provider.invalidate() }
        store.calls = controller
        let live = CallControllerContext(id: UUID(), channelID: "room-dm", title: "Ann", mode: .outgoingDM)
        let finished = CallControllerContext(id: UUID(), channelID: "room-dm", title: "Ann", mode: .outgoingDM)
        finished.ended = true
        let unloaded = CallControllerContext(id: UUID(), channelID: "room-not-loaded", title: "Reported name", mode: .outgoingDM)
        for context in [live, finished, unloaded] { controller.contexts[context.id] = context }

        try await store.receive(event(user(id: "user-ann", username: "ann", name: "Annabel")))

        #expect(live.title == "Annabel")
        #expect(finished.title == "Ann", "A finished call has nothing left to relabel.")
        #expect(unloaded.title == "Reported name", "An unloaded channel keeps the title it was reported with.")
        #expect(session.title == "Call", "A session that never joined a call must not be relabelled.")
        #expect(controller.names["user-ann"] == "Annabel")
    }

    @Test @MainActor func signingOutDropsTheNamesSoTheNextAccountCannotInheritThem() async throws {
        let store = try loaded()
        defer { OfflineCache.remove(origin: store.origin) }
        let session = CallSession()
        let controller = CallController(session: session, api: UnusedCallAPI())
        defer { controller.provider.invalidate() }
        store.calls = controller
        try await store.receive(event(user(id: "user-ann", username: "ann", name: "Annabel")))
        #expect(controller.names["user-ann"] == "Annabel")

        store.clearSession()

        #expect(controller.names.isEmpty, "One account's names must not label the next account's calls.")
        let service = DenService(origin: try ServerOrigin.canonical("https://unused.test"), token: "test-only-token")
        defer { service.close() }
        let credentials = CallControllerCredentials(generation: store.generation, service: service,
            user: user(id: "user-me", username: "me", name: "Me"), names: ["user-ann": "Ann"])
        #expect(controller.joinNames(credentials)["user-ann"] == "Ann",
                "A cold answer that joins before any refresh still has the credential snapshot to fall back on.")
    }

    // MARK: - Fixtures

    private var dm: API.Channel {
        .init(id: "room-dm", kind: .dm, memberIds: ["user-me", "user-ann"], name: "", position: 1)
    }
    private var general: API.Channel {
        .init(id: "room-general", kind: .text, memberIds: [], name: "general", position: 0)
    }
    private func user(id: String, username: String, name: String) -> API.User {
        .init(bot: false, displayName: name, id: id, role: .member, username: username)
    }
    private func event(_ value: API.User) throws -> Data {
        try JSONSerialization.data(withJSONObject: [
            "type": "user_updated",
            "user": try JSONSerialization.jsonObject(with: JSONEncoder().encode(value)),
        ])
    }
    /// A restored, offline session: cached text, an unread room and a peer already online.
    /// It deliberately has no `service`, so any refetch this path attempts throws instead.
    @MainActor private func loaded() throws -> AppStore {
        let store = AppStore()
        store.origin = try ServerOrigin.canonical("https://profile-updates-\(UUID().uuidString).test")
        OfflineCache.remove(origin: store.origin)
        let me = user(id: "user-me", username: "me", name: "Me")
        store.user = me
        store.users = [me, user(id: "user-ann", username: "ann", name: "Ann"),
                       user(id: "user-bo", username: "bo", name: "Bo")]
        store.channels = [general, dm]
        store.messages = ["room-general": [.init(attachments: [], authorId: "user-ann", channelId: "room-general",
            content: "hey @ann", createdAt: "2026-09-15T12:00:00Z", id: "message-1")]]
        store.readStates = [.init(channelId: "room-general", mentionCount: 1, notificationCount: 1, unreadCount: 3)]
        store.selectedChannelId = "room-general"
        store.presence = ["user-ann"]
        store.syncProblem = .networkOffline
        return store
    }
    @MainActor private func waitFor(_ condition: @MainActor () -> Bool) async throws {
        for _ in 0..<200 {
            if condition() { return }
            try await Task.sleep(for: .milliseconds(10))
        }
        try #require(condition(), "The call never reached the expected connecting state.")
    }
}

/// Holds the call token request open so `CallSession.join` stays suspended at `.connecting`
/// with its callID and title assigned, without a LiveKit room or any CallKit transaction.
private final class HeldCallTokenStub: URLProtocol, @unchecked Sendable {
    override class func canInit(with request: URLRequest) -> Bool { request.url?.host == "held-call.test" }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func startLoading() {}
    override func stopLoading() {}
}

/// These tests drive titles and names only. Any signaling call here is a test defect.
@MainActor private final class UnusedCallAPI: CallControllerAPI {
    enum Failure: Error { case unexpected }
    func restoreCallSession() async throws -> CallControllerCredentials { throw Failure.unexpected }
    func isCurrent(_ credentials: CallControllerCredentials) -> Bool { true }
    func invitation(id: String, fetchTicket: String?) async throws -> API.CallInvitationState { throw Failure.unexpected }
    func invitations() async throws -> [API.CallInvitationState] { throw Failure.unexpected }
    func invite(channelID: String) async throws -> API.CallInvitation { throw Failure.unexpected }
    func accept(_ invitation: API.CallInvitation, answerID: UUID) async throws -> API.CallInvitationState { throw Failure.unexpected }
    func decline(_ invitation: API.CallInvitation) async throws { throw Failure.unexpected }
    func cancel(_ invitation: API.CallInvitation) async throws { throw Failure.unexpected }
    func end(_ invitation: API.CallInvitation) async throws { throw Failure.unexpected }
}
