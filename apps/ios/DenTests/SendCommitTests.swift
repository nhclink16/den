import Foundation
import DenAPI
import Synchronization
import Testing
@testable import Den

struct SendCommitTests {
    @Test @MainActor func successfulSendIsCommittedEvenWhenTailRefreshFails() async throws {
        let uploadJSON: [String: Any] = ["id": "upload-1", "channel_id": "room", "filename": "clip.mp4",
            "content_type": "video/mp4", "size": 4, "offset": 4, "complete": true]
        let upload = try JSONDecoder().decode(Components.Schemas.Upload.self,
            from: JSONSerialization.data(withJSONObject: uploadJSON))
        let messageJSON: [String: Any] = ["id": "z-committed", "channel_id": "room", "author_id": "sender",
            "content": "The committed message", "created_at": "2026-09-14T12:00:00Z", "attachments": [uploadJSON]]
        let response = try JSONSerialization.data(withJSONObject: messageJSON)
        SendCommitStub.response.withLock { $0 = response }
        SendCommitStub.counts.withLock { $0 = (posts: 0, reads: 0) }
        defer {
            SendCommitStub.response.withLock { $0 = Data() }
        }
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [SendCommitStub.self]
        let service = DenService(origin: URL(string: "https://send-commit.test")!, token: "test-only-token", sessionConfiguration: configuration)
        defer { service.close() }
        let store = AppStore()
        store.service = service
        store.messageHasNewer["room"] = true
        store.messages["room"] = [.init(attachments: [], authorId: "sender", channelId: "room", content: "Older window",
            createdAt: "2026-09-13T12:00:00Z", id: "a-older")]
        let local = try AttachmentFiles.directory().appendingPathComponent("clip.mp4")
        try Data([1, 2, 3, 4]).write(to: local)
        defer { AttachmentFiles.remove(local) }
        store.pendingUploads = [.init(id: UUID(), channelId: "room", filename: "clip.mp4", localURL: local,
            progress: 1, upload: upload)]

        var failure: Error?
        do { try await store.send(channelId: "room", content: "The committed message", replyTo: nil) }
        catch { failure = error }

        #expect(failure == nil, "A failed refresh must not turn the committed POST into a failed send.")
        #expect(store.pendingUploads.isEmpty)
        #expect(!FileManager.default.fileExists(atPath: local.path))
        #expect(store.messages["room"]?.map(\.id) == ["z-committed"])
        #expect(store.messages["room"]?.last?.content == "The committed message")
        #expect(store.messages["room"]?.last?.attachments.first?.id == "upload-1")
        #expect(store.messageHasNewer["room"] == false)
        #expect(SendCommitStub.counts.withLock { $0.posts } == 1)
        #expect(SendCommitStub.counts.withLock { $0.reads } == 1)
    }
}

private final class SendCommitStub: URLProtocol, @unchecked Sendable {
    static let response = Mutex(Data())
    static let counts = Mutex((posts: 0, reads: 0))
    override class func canInit(with request: URLRequest) -> Bool { request.url?.host == "send-commit.test" }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func startLoading() {
        if request.httpMethod == "POST", request.url?.path == "/channels/room/messages" {
            Self.counts.withLock { $0.posts += 1 }
            let response = HTTPURLResponse(url: request.url!, statusCode: 200, httpVersion: "HTTP/1.1", headerFields: ["Content-Type": "application/json"])!
            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
            client?.urlProtocol(self, didLoad: Self.response.withLock { $0 })
            client?.urlProtocolDidFinishLoading(self)
        } else {
            Self.counts.withLock { $0.reads += 1 }
            client?.urlProtocol(self, didFailWithError: URLError(.networkConnectionLost))
        }
    }
    override func stopLoading() {}
}
