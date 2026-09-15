import Foundation
import Security
import DenAPI

typealias API = Components.Schemas

enum DenFailure: Error, LocalizedError, Equatable {
    case invalidOrigin, keychain(OSStatus), signedOut, offline, server(Int, String), invalidResponse, updateRequired
    var errorDescription: String? {
        switch self {
        case .invalidOrigin: "Enter an HTTPS server address without a path, username or password."
        case .keychain: "Den could not access this device's saved login. Try unlocking the device."
        case .signedOut: "Your session expired. Log in again."
        case .offline: "Cannot reach your server. Your cached conversations are still available."
        case .server(_, let message): message
        case .invalidResponse: "The server returned a response Den could not read."
        case .updateRequired: "This version of Den cannot read your server's response. Update Den, then try again. Your saved conversations and login are still on this device."
        }
    }
    static func present(_ error: Error) -> String {
        if incompatible(error) { return DenFailure.updateRequired.localizedDescription }
        if let failure = error as? DenFailure { return failure.localizedDescription }
        if let client = error as? ClientError { return present(client.underlyingError) }
        if error is URLError { return DenFailure.offline.localizedDescription }
        return "That did not finish. Please try again."
    }
    static func unauthorized(_ error: Error) -> Bool {
        if let client = error as? ClientError { return unauthorized(client.underlyingError) }
        if let value = error as? DenFailure, case .server(401, _) = value { return true }
        return false
    }
    static func cancelled(_ error: Error) -> Bool {
        if let client = error as? ClientError { return cancelled(client.underlyingError) }
        return error is CancellationError || (error as? URLError)?.code == .cancelled
    }
    static func incompatible(_ error: Error) -> Bool {
        if let client = error as? ClientError { return incompatible(client.underlyingError) }
        return error is DecodingError || (error as? DenFailure) == .invalidResponse || (error as? DenFailure) == .updateRequired
    }
}

enum SyncProblem: Equatable {
    case networkOffline, incompatibleResponse
}

enum ServerOrigin {
    static func canonical(_ value: String) throws -> URL {
        guard var parts = URLComponents(string: value.trimmingCharacters(in: .whitespacesAndNewlines)),
              let scheme = parts.scheme?.lowercased(), let host = parts.host?.lowercased(),
              !host.isEmpty, parts.user == nil, parts.password == nil,
              parts.query == nil, parts.fragment == nil,
              parts.path.isEmpty || parts.path == "/",
              scheme == "https" || (scheme == "http" && ["localhost", "127.0.0.1", "::1", "[::1]"].contains(host))
        else { throw DenFailure.invalidOrigin }
        parts.scheme = scheme; parts.host = host; parts.path = ""
        if (scheme == "https" && parts.port == 443) || (scheme == "http" && parts.port == 80) { parts.port = nil }
        guard let url = parts.url else { throw DenFailure.invalidOrigin }
        return url
    }
}

enum SessionVault {
    private static let service = "app.denchat.ios.session"
    private static func query(_ origin: URL) -> [String: Any] {
        [kSecClass as String: kSecClassGenericPassword,
         kSecAttrService as String: service, kSecAttrAccount as String: origin.absoluteString]
    }
    static func read(_ origin: URL) throws -> String? {
        var q = query(origin)
        q[kSecReturnData as String] = true
        q[kSecMatchLimit as String] = kSecMatchLimitOne
        var item: CFTypeRef?
        let status = SecItemCopyMatching(q as CFDictionary, &item)
        if status == errSecItemNotFound { return nil }
        guard status == errSecSuccess, let data = item as? Data,
              let value = String(data: data, encoding: .utf8) else { throw DenFailure.keychain(status) }
        return value
    }
    static func save(_ token: String, origin: URL) throws {
        let q = query(origin)
        let attributes: [String: Any] = [kSecValueData as String: Data(token.utf8),
          kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly]
        var status = SecItemUpdate(q as CFDictionary, attributes as CFDictionary)
        if status == errSecItemNotFound {
            status = SecItemAdd(q.merging(attributes) { _, value in value } as CFDictionary, nil)
        }
        guard status == errSecSuccess else { throw DenFailure.keychain(status) }
    }
    static func delete(_ origin: URL) throws {
        let status = SecItemDelete(query(origin) as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else { throw DenFailure.keychain(status) }
    }
}

final class NoRedirects: NSObject, URLSessionTaskDelegate, Sendable {
    func urlSession(_ session: URLSession, task: URLSessionTask,
                    willPerformHTTPRedirection response: HTTPURLResponse, newRequest request: URLRequest,
                    completionHandler: @escaping @Sendable (URLRequest?) -> Void) {
        // Den endpoints do not redirect. Never forward a bearer to a Location target.
        completionHandler(nil)
    }
}
