import Foundation
import AVFoundation
import DenAPI
import Synchronization
import Testing
@testable import Den

@Suite(.serialized) struct UploadMediaTests {
    @Test @MainActor func lostChunkResponseResumesFromServerOffsetAndRemovalDeletesOwnedCopy() async throws {
        let source = FileManager.default.temporaryDirectory.appendingPathComponent("upload-source-\(UUID()).mp4")
        let contents = Data((0..<(2 * 1_048_576 + 37)).map { UInt8($0 % 251) })
        try contents.write(to: source)
        defer { try? FileManager.default.removeItem(at: source) }
        let state = Mutex(UploadServer(size: contents.count))
        MediaStub.route.withLock { route in
            route = { request in
                try state.withLock { server in
                    #expect(request.value(forHTTPHeaderField: "Authorization") == "Bearer test-only-token")
                    switch (request.httpMethod, request.url?.path) {
                    case ("POST", "/uploads"):
                        return .response(200, ["Content-Type": "application/json"], try server.json())
                    case ("GET", "/uploads/upload-1"):
                        server.statusReads += 1
                        return .response(200, ["Content-Type": "application/json"], try server.json())
                    case ("PATCH", "/uploads/upload-1"):
                        let offset = Int(request.value(forHTTPHeaderField: "Upload-Offset") ?? "") ?? -1
                        let data = try MediaStub.body(request)
                        server.offsets.append(offset)
                        #expect(!data.isEmpty)
                        #expect(data.count <= 1_048_576)
                        guard offset == server.stored.count else {
                            return .response(409, ["Content-Type": "application/json"], Data(#"{"error":"conflict","message":"offset mismatch"}"#.utf8))
                        }
                        server.stored.append(data)
                        if !server.lostResponse {
                            server.lostResponse = true
                            return .failure(URLError(.networkConnectionLost))
                        }
                        return .response(200, ["Content-Type": "application/json"], try server.json())
                    case ("POST", "/uploads/upload-1/complete"):
                        #expect(server.stored.count == server.size)
                        server.complete = true
                        return .response(200, ["Content-Type": "application/json"], try server.json())
                    case ("DELETE", "/uploads/upload-1"):
                        server.deleted = true
                        return .response(204, [:], Data())
                    default:
                        Issue.record("Unexpected request \(request.httpMethod ?? "") \(request.url?.path ?? "")")
                        return .response(404, [:], Data())
                    }
                }
            }
        }
        defer { MediaStub.route.withLock { $0 = nil } }
        let store = AppStore(), service = makeService()
        store.service = service
        defer { service.close() }
        do { try await store.attach(url: source, channelId: "room"); Issue.record("The lost response should leave a retryable upload") }
        catch { #expect(!(error is CancellationError)) }
        let pending = try #require(store.pendingUploads.first)
        #expect(pending.error != nil)
        #expect(pending.localURL != source)
        #expect(FileManager.default.fileExists(atPath: pending.localURL.path))
        try FileManager.default.removeItem(at: source)
        do { try await store.retryUpload(id: pending.id) }
        catch {
            _ = state.withLock { server in
                Issue.record("Retry failed with \(String(describing: error)); server offset \(server.stored.count), attempted offsets \(server.offsets), status reads \(server.statusReads)")
            }
            throw error
        }
        #expect(store.pendingUploads.first?.upload?.complete == true)
        #expect(store.pendingUploads.first?.error == nil)
        #expect(store.pendingUploads.first?.progress == 1)
        state.withLock { server in
            #expect(server.statusReads == 1)
            #expect(server.offsets == [0, 1_048_576, 2 * 1_048_576])
            #expect(server.stored == contents)
        }
        try await store.removeUpload(id: pending.id)
        #expect(store.pendingUploads.isEmpty)
        #expect(!FileManager.default.fileExists(atPath: pending.localURL.path))
        #expect(state.withLock { $0.deleted })
    }

    @Test func protectedPlaybackReadsTheRequestedOffsetWithBearerAndRejectsInvalidResponses() async throws {
        let expected = Data((0..<73).map { UInt8($0) })
        let upload = try uploadModel(size: 8_000)
        let state = Mutex((requests: 0, mode: 0, observed: [URLRequest]()))
        MediaStub.route.withLock { route in
            route = { request in
                state.withLock { state in
                    state.requests += 1
                    state.observed.append(request)
                    #expect(request.url?.absoluteString == "https://media.test/uploads/upload-1/file")
                    #expect(request.url?.query == nil)
                    #expect(request.value(forHTTPHeaderField: "Authorization") == "Bearer test-only-token")
                    #expect(request.value(forHTTPHeaderField: "Range") == "bytes=4096-4168")
                    var headers = ["Content-Type": "video/mp4", "Content-Length": "73", "Content-Range": "bytes 4096-4168/8000"]
                    switch state.mode {
                    case 1: return .response(200, headers, expected) // A server ignoring Range is not a valid seek.
                    case 2: headers["Content-Range"] = "bytes 0-72/8000"
                    case 3: headers["Content-Type"] = "text/html"
                    case 4: headers["Content-Length"] = "72"
                    case 5: return .response(302, ["Location": "https://other.test/stolen"], Data())
                    default: break
                    }
                    return .response(206, headers, expected)
                }
            }
        }
        defer { MediaStub.route.withLock { $0 = nil } }
        let service = makeService()
        defer { service.close() }
        #expect(try await ProtectedMedia.range(upload: upload, service: service, start: 4_096, end: 4_168) == expected)
        for mode in 1...5 {
            state.withLock { $0.mode = mode }
            do {
                _ = try await ProtectedMedia.range(upload: upload, service: service, start: 4_096, end: 4_168)
                Issue.record("Rejected response mode \(mode) was accepted")
            } catch { #expect(error is DenFailure) }
        }
        #expect(state.withLock { $0.requests } == 6)
        // URLProtocol callbacks do not inherit a Swift Testing task. Assert captured
        // requests here as well so a missing bearer/range fails this named test.
        let observed = state.withLock { $0.observed }
        #expect(observed.map { $0.value(forHTTPHeaderField: "Range") } == Array(repeating: "bytes=4096-4168", count: 6))
        #expect(observed.allSatisfy { $0.value(forHTTPHeaderField: "Authorization") == "Bearer test-only-token" })
        #expect(observed.allSatisfy { $0.url?.absoluteString == "https://media.test/uploads/upload-1/file" && $0.url?.query == nil })
    }

    @Test @MainActor func authenticatedAVPlayerLoadsAndSeeksSyntheticVideo() async throws {
        let clip = try #require(Data(base64Encoded: Self.syntheticClip))
        let upload = try uploadModel(size: clip.count)
        let requests = Mutex<[String]>([])
        MediaStub.route.withLock { route in
            route = { request in
                #expect(request.value(forHTTPHeaderField: "Authorization") == "Bearer test-only-token")
                let range = request.value(forHTTPHeaderField: "Range") ?? ""
                requests.withLock { $0.append(range) }
                let parts = range.replacingOccurrences(of: "bytes=", with: "").split(separator: "-")
                guard parts.count == 2, let start = Int(parts[0]), let end = Int(parts[1]),
                      start >= 0, end >= start, end < clip.count else {
                    return .response(416, [:], Data())
                }
                return .response(206, ["Content-Type": "video/mp4", "Content-Length": "\(end - start + 1)",
                    "Content-Range": "bytes \(start)-\(end)/\(clip.count)"], clip.subdata(in: start..<(end + 1)))
            }
        }
        defer { MediaStub.route.withLock { $0 = nil } }
        let service = makeService()
        let playback = ProtectedPlayback(upload: upload, service: service) { Issue.record(Comment(rawValue: $0)) }
        defer { playback.stop(); service.close() }
        let item = try #require(playback.player.currentItem)
        for _ in 0..<200 where item.status == AVPlayerItem.Status.unknown { try await Task.sleep(for: .milliseconds(50)) }
        #expect(item.status == AVPlayerItem.Status.readyToPlay, "AVPlayer error: \(String(describing: item.error))")
        guard item.status == AVPlayerItem.Status.readyToPlay else { return }
        let sought = await playback.player.seek(to: CMTime(seconds: 2, preferredTimescale: 600),
            toleranceBefore: CMTime.zero, toleranceAfter: CMTime.zero)
        #expect(sought)
        #expect(abs(playback.player.currentTime().seconds - 2) < 0.1)
        #expect(!requests.withLock { $0 }.isEmpty)
    }

    private func makeService() -> DenService {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [MediaStub.self]
        return DenService(origin: URL(string: "https://media.test")!, token: "test-only-token", sessionConfiguration: configuration)
    }
    // Synthetic 16x16 blue clip, four seconds, H.264, no copyrighted media.
    // ffmpeg -f lavfi -i color=c=blue:s=16x16:r=1 -t 4 -c:v libx264 -pix_fmt yuv420p -g 1 -movflags +faststart clip.mp4
    private static let syntheticClip = "AAAAIGZ0eXBpc29tAAACAGlzb21pc28yYXZjMW1wNDEAAAMgbW9vdgAAAGxtdmhkAAAAAAAAAAAAAAAAAAAD6AAAD6AAAQAAAQAAAAAAAAAAAAAAAAEAAAAAAAAAAAAAAAAAAAABAAAAAAAAAAAAAAAAAABAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAgAAAkp0cmFrAAAAXHRraGQAAAADAAAAAAAAAAAAAAABAAAAAAAAD6AAAAAAAAAAAAAAAAAAAAAAAAEAAAAAAAAAAAAAAAAAAAABAAAAAAAAAAAAAAAAAABAAAAAABAAAAAQAAAAAAAkZWR0cwAAABxlbHN0AAAAAAAAAAEAAA+gAAAAAAABAAAAAAHCbWRpYQAAACBtZGhkAAAAAAAAAAAAAAAAAABAAAABAABVxAAAAAAALWhkbHIAAAAAAAAAAHZpZGUAAAAAAAAAAAAAAABWaWRlb0hhbmRsZXIAAAABbW1pbmYAAAAUdm1oZAAAAAEAAAAAAAAAAAAAACRkaW5mAAAAHGRyZWYAAAAAAAAAAQAAAAx1cmwgAAAAAQAAAS1zdGJsAAAAuXN0c2QAAAAAAAAAAQAAAKlhdmMxAAAAAAAAAAEAAAAAAAAAAAAAAAAAAAAAABAAEABIAAAASAAAAAAAAAABFUxhdmM2Mi4yOC4xMDIgbGlieDI2NAAAAAAAAAAAAAAAGP//AAAAL2F2Y0MBZBAK/+EAE2dkEAqsu9gIgAAAAwCAAAADAQIBAAVo7g8si/34+AAAAAAQcGFzcAAAAAEAAAABAAAAFGJ0cnQAAAAAAAAFigAAAAAAAAAYc3R0cwAAAAAAAAABAAAABAAAQAAAAAAcc3RzYwAAAAAAAAABAAAAAQAAAAQAAAABAAAAJHN0c3oAAAAAAAAAAAAAAAQAAAJ6AAAAGQAAABkAAAAZAAAAFHN0Y28AAAAAAAAAAQAAA1AAAABidWR0YQAAAFptZXRhAAAAAAAAACFoZGxyAAAAAAAAAABtZGlyYXBwbAAAAAAAAAAAAAAAAC1pbHN0AAAAJal0b28AAAAdZGF0YQAAAAEAAAAATGF2ZjYyLjEyLjEwMgAAAAhmcmVlAAACzW1kYXQAAAJeBgX//1rcRem95tlIt5Ys2CDZI+7veDI2NCAtIGNvcmUgMTY1IHIzMjIyIGIzNTYwNWEgLSBILjI2NC9NUEVHLTQgQVZDIGNvZGVjIC0gQ29weWxlZnQgMjAwMy0yMDI1IC0gaHR0cDovL3d3dy52aWRlb2xhbi5vcmcveDI2NC5odG1sIC0gb3B0aW9uczogY2FiYWM9MSByZWY9MSBkZWJsb2NrPTE6MDowIGFuYWx5c2U9MHgzOjB4MTEzIG1lPWhleCBzdWJtZT03IHBzeT0xIHBzeV9yZD0xLjAwOjAuMDAgbWl4ZWRfcmVmPTAgbWVfcmFuZ2U9MTYgY2hyb21hX21lPTEgdHJlbGxpcz0xIDh4OGRjdD0xIGNxbT0wIGRlYWR6b25lPTIxLDExIGZhc3RfcHNraXA9MSBjaHJvbWFfcXBfb2Zmc2V0PS0yIHRocmVhZHM9MSBsb29rYWhlYWRfdGhyZWFkcz0xIHNsaWNlZF90aHJlYWRzPTAgbnI9MCBkZWNpbWF0ZT0xIGludGVybGFjZWQ9MCBibHVyYXlfY29tcGF0PTAgY29uc3RyYWluZWRfaW50cmE9MCBiZnJhbWVzPTAgd2VpZ2h0cD0wIGtleWludD0xIGtleWludF9taW49MSBzY2VuZWN1dD00MCBpbnRyYV9yZWZyZXNoPTAgcmM9Y3JmIG1idHJlZT0wIGNyZj0yMy4wIHFjb21wPTAuNjAgcXBtaW49MCBxcG1heD02OSBxcHN0ZXA9NCBpcF9yYXRpbz0xLjQwIGFxPTE6MS4wMACAAAAAFGWIhAS//ujJ/MsteeZ1GomWWg+dAAAAFWWIggF//u6CvgU3X8QPwzzx+tAhwAAAABVliIQF//7ugr4FN1/ED8M88frQIcEAAAAVZYiCAX/+7oK+BTdfxA/DPPH60CHA"

    private func uploadModel(size: Int) throws -> Components.Schemas.Upload {
        try JSONDecoder().decode(Components.Schemas.Upload.self, from: UploadServer(size: size).json())
    }
}

private struct UploadServer: Sendable {
    let size: Int
    var stored = Data()
    var offsets: [Int] = []
    var statusReads = 0
    var lostResponse = false
    var complete = false
    var deleted = false
    func json() throws -> Data {
        try JSONSerialization.data(withJSONObject: ["id": "upload-1", "channel_id": "room", "filename": "clip.mp4",
            "content_type": "video/mp4", "size": size, "offset": stored.count, "complete": complete])
    }
}

private final class MediaStub: URLProtocol, @unchecked Sendable {
    enum Reply: Sendable {
        case response(Int, [String: String], Data)
        case failure(URLError)
    }
    static let route = Mutex<(@Sendable (URLRequest) throws -> Reply)?>(nil)
    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
    override func startLoading() {
        do {
            let route = Self.route.withLock { $0 }
            guard let route else { throw URLError(.unsupportedURL) }
            switch try route(request) {
            case .response(let status, let headers, let data):
                let response = HTTPURLResponse(url: request.url!, statusCode: status, httpVersion: "HTTP/1.1", headerFields: headers)!
                client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
                client?.urlProtocol(self, didLoad: data)
                client?.urlProtocolDidFinishLoading(self)
            case .failure(let error): client?.urlProtocol(self, didFailWithError: error)
            }
        } catch { client?.urlProtocol(self, didFailWithError: error) }
    }
    override func stopLoading() {}
    static func body(_ request: URLRequest) throws -> Data {
        if let data = request.httpBody { return data }
        guard let stream = request.httpBodyStream else { return Data() }
        stream.open(); defer { stream.close() }
        var output = Data(), buffer = [UInt8](repeating: 0, count: 65_536)
        while true {
            let count = stream.read(&buffer, maxLength: buffer.count)
            if count < 0 { throw stream.streamError ?? URLError(.cannotDecodeRawData) }
            if count == 0 { break }
            output.append(contentsOf: buffer.prefix(count))
        }
        return output
    }
}
