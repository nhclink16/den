import DenAPI
import Foundation
import Testing

struct ResponseCompatibilityTests {
    @Test func additiveResponseFieldsDoNotPreventReadingOrLeakIntoUpdates() async throws {
        let transport = AppearanceTransport(response: Self.appearance)
        let client = Client(serverURL: URL(string: "https://compatibility.test")!, transport: transport)
        var appearance = try await client.getUsersMeAppearance().ok.body.json
        #expect(appearance.background?.blur == 12)
        #expect(appearance.contrast == 110)
        appearance.lightTheme = "tide"
        let saved = try await client.putUsersMeAppearance(body: .json(appearance)).ok.body.json
        #expect(saved.lightTheme == "tide")
        let body = try #require(await transport.savedBody)
        let sent = try #require(JSONSerialization.jsonObject(with: body) as? [String: Any])
        #expect(sent["future_root"] == nil)
        let background = try #require(sent["background"] as? [String: Any])
        #expect(background["future_background"] == nil)
        #expect(background["blur"] as? Int == 12)
        #expect(sent["contrast"] as? Int == 110)
        let source = try #require(background["source"] as? [String: Any])
        #expect(source["type"] as? String == "builtin")
        #expect(source["name"] as? String == "aurora")
    }

    @Test func removingRequiredFieldsOrChangingKnownValuesStillFails() async throws {
        let original = try #require(JSONSerialization.jsonObject(with: Self.appearance) as? [String: Any])
        var missing = original
        missing.removeValue(forKey: "light_theme")
        var changed = original
        changed["mode"] = "unknown-future-mode"
        for wire in [missing, changed] {
            let transport = AppearanceTransport(response: try JSONSerialization.data(withJSONObject: wire))
            let client = Client(serverURL: URL(string: "https://compatibility.test")!, transport: transport)
            do {
                _ = try await client.getUsersMeAppearance().ok.body.json
                Issue.record("An incompatible response must still be rejected.")
            } catch let error as ClientError {
                #expect(error.underlyingError is DecodingError)
            }
        }
    }

    private static let appearance = Data(#"""
    {"mode":"system","light_theme":"paper","dark_theme":"tide","custom_themes":[],
     "background":{"blur":12,"dim":20,"saturate":110,"scope":"chat","fit":"cover",
       "source":{"type":"builtin","name":"aurora"},"future_background":{"enabled":true}},
     "contrast":110,"future_root":["new server setting"]}
    """#.utf8)
}

private actor AppearanceTransport: ClientTransport {
    let response: Data
    private(set) var savedBody: Data?
    init(response: Data) { self.response = response }
    func send(_ request: HTTPRequest, body: HTTPBody?, baseURL: URL, operationID: String) async throws -> (HTTPResponse, HTTPBody?) {
        #expect(request.path == "/users/me/appearance")
        if request.method == .put {
            let body = try #require(body)
            savedBody = try await Data(collecting: body, upTo: 1_048_576)
            return (.init(status: .ok, headerFields: [.contentType: "application/json"]), HTTPBody(savedBody!))
        }
        #expect(request.method == .get)
        return (.init(status: .ok, headerFields: [.contentType: "application/json"]), HTTPBody(response))
    }
}
