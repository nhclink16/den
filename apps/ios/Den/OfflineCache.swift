import Foundation
import CryptoKit

struct CachedSession: Codable {
    var user: API.User
    var channels: [API.Channel]
    var categories: [API.Category]
    var users: [API.User]
    var readStates: [API.ChannelReadState]
    var messages: [String: [API.Message]]
    var instanceName: String
}

enum OfflineCache {
    static func directory(origin: URL) -> URL {
        let digest = SHA256.hash(data: Data(origin.absoluteString.utf8)).map { String(format: "%02x", $0) }.joined()
        return URL.applicationSupportDirectory.appending(path: "Den/\(digest)", directoryHint: .isDirectory)
    }
    static func read(origin: URL) -> CachedSession? {
        guard let data = try? Data(contentsOf: directory(origin: origin).appending(path: "text.json")) else { return nil }
        return try? JSONDecoder().decode(CachedSession.self, from: data)
    }
    static func save(_ value: CachedSession, origin: URL) throws {
        let folder = directory(origin: origin)
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        var excluded = folder
        var values = URLResourceValues(); values.isExcludedFromBackup = true
        try excluded.setResourceValues(values)
        let data = try JSONEncoder().encode(value)
        try data.write(to: folder.appending(path: "text.json"), options: [.atomic, .completeFileProtectionUntilFirstUserAuthentication])
    }
    static func remove(origin: URL) { try? FileManager.default.removeItem(at: directory(origin: origin)) }
}
