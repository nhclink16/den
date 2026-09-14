#if DEBUG && targetEnvironment(simulator)
import Foundation

/// Only the simulator Debug build can prefill a disposable test-server login.
/// The request still goes through the visible sign-in button and real API client.
enum DebugFixture {
    static var credentials: (origin: String, username: String, password: String)? {
        guard ProcessInfo.processInfo.arguments.contains("--den-ui-test"),
              let data = try? Data(contentsOf: URL(fileURLWithPath:
                ProcessInfo.processInfo.environment["DEN_UI_FIXTURE_PATH"] ?? "/tmp/den-ios-qa.json")),
              let fixture = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let address = fixture["origin"] as? String,
              let origin = try? ServerOrigin.canonical(address),
              ["127.0.0.1", "localhost", "[::1]"].contains(origin.host() ?? ""),
              let users = fixture["users"] as? [[String: Any]],
              let index = Int(ProcessInfo.processInfo.environment["DEN_UI_USER"] ?? "0"),
              users.indices.contains(index), let username = users[index]["username"] as? String,
              let password = users[index]["password"] as? String else { return nil }
        return (origin.absoluteString, username, password)
    }
}
#endif
