import Foundation
import Testing
@testable import DenShell

// The page asks for photos and shares links through the shell. Only well-formed asks pass, and a picked file read back
// piece by piece must come out whole, with the last piece saying so.
struct MediaBridgeTests {
    @Test func readsWhatThePageSends() throws {
        #expect(try MediaCommand(["type": "pick"]) == .pick)
        #expect(try MediaCommand(["type": "read", "id": "A1", "offset": 2_097_152]) == .read(id: "A1", offset: 2_097_152))
        #expect(try MediaCommand(["type": "share", "url": "https://den.example/settings/desktop", "text": "Get Den"])
            == .share(url: URL(string: "https://den.example/settings/desktop")!, text: "Get Den", from: nil))
        #expect(try MediaCommand(["type": "share", "url": "https://den.example/login?invite=x", "text": NSNull(),
                                  "from": ["x": 10, "y": 20.5, "width": 100, "height": 44]])
            == .share(url: URL(string: "https://den.example/login?invite=x")!, text: nil, from: CGRect(x: 10, y: 20.5, width: 100, height: 44)))
    }

    @Test func refusesTheRest() {
        #expect(throws: CallError.self) { try MediaCommand(["type": "read", "id": "A1", "offset": -1]) }
        #expect(throws: CallError.self) { try MediaCommand(["type": "read", "id": "A1", "offset": 1.5]) }
        #expect(throws: CallError.self) { try MediaCommand(["type": "read", "id": "A1", "offset": true]) }
        #expect(throws: CallError.self) { try MediaCommand(["type": "read", "offset": 0]) }
        #expect(throws: CallError.self, "only web links") { try MediaCommand(["type": "share", "url": "file:///etc/passwd"]) }
        #expect(throws: CallError.self) { try MediaCommand(["type": "share", "url": "javascript:alert(1)"]) }
        #expect(throws: CallError.self) { try MediaCommand(["type": "share", "url": "https://den.example", "text": 3]) }
        #expect(throws: CallError.self) { try MediaCommand(["type": "delete"]) }
        #expect(throws: CallError.self) { try MediaCommand("pick") }
    }

    @Test func readsAFileBackWholeInPieces() throws {
        let size = MediaBridge.pieceSize * 2 + 123
        let bytes = Data((0 ..< size).map { UInt8(truncatingIfNeeded: $0 &* 31) })
        let url = FileManager.default.temporaryDirectory.appending(path: "media-test-\(UUID().uuidString)")
        try bytes.write(to: url)
        defer { try? FileManager.default.removeItem(at: url) }

        var whole = Data(), offset = 0, lasts: [Bool] = []
        while true {
            let (piece, last) = try MediaBridge.piece(of: url, at: offset)
            let data = try #require(Data(base64Encoded: piece))
            whole.append(data)
            offset += data.count
            lasts.append(last)
            if last { break }
        }
        #expect(whole == bytes)
        #expect(lasts == [false, false, true])
        #expect(throws: CallError.self, "past the end") { try MediaBridge.piece(of: url, at: size + 1) }
    }

    @Test func anEmptyFileIsOnePiece() throws {
        let url = FileManager.default.temporaryDirectory.appending(path: "media-test-\(UUID().uuidString)")
        try Data().write(to: url)
        defer { try? FileManager.default.removeItem(at: url) }
        let (piece, last) = try MediaBridge.piece(of: url, at: 0)
        #expect(piece.isEmpty && last)
    }
}
