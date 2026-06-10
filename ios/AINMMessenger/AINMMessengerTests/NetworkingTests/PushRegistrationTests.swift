import Foundation
import Testing

@testable import AINMMessenger

/// APNs registration plumbing (M4): the device token arrives as raw `Data` and
/// is uploaded hex-encoded to `PUT /accounts/push-token` with bearer auth.
@Suite("Push registration")
struct PushRegistrationTests {

    @Test("device token Data encodes as lowercase hex")
    func tokenHexEncoding() {
        let data = Data([0x0A, 0xFF, 0x00, 0x42])
        #expect(PushRegistration.hexToken(data) == "0aff0042")
        #expect(PushRegistration.hexToken(Data()) == "")
    }

    @Test("register request: PUT /accounts/push-token, bearer, ios platform body")
    func registerRequestShape() throws {
        let req = try PushRegistration.makeRegisterRequest(
            baseURL: #require(URL(string: "https://server.example")),
            bearer: "sess_tok_3",
            deviceTokenHex: "0aff0042"
        )
        #expect(req.httpMethod == "PUT")
        #expect(try #require(req.url).path() == "/accounts/push-token")
        #expect(req.value(forHTTPHeaderField: "Authorization") == "Bearer sess_tok_3")
        let body = try JSONSerialization.jsonObject(
            with: #require(req.httpBody)) as? [String: String]
        #expect(body?["device_token"] == "0aff0042")
        #expect(body?["platform"] == "ios")
    }

    @Test("notification payload thread_id extraction for deep link")
    func payloadThreadId() {
        let userInfo: [AnyHashable: Any] = [
            "aps": ["alert": ["title": "alice", "body": "hi"]],
            "thread_id": "t-99",
        ]
        #expect(PushRegistration.threadId(fromUserInfo: userInfo) == "t-99")
        #expect(PushRegistration.threadId(fromUserInfo: ["aps": [:]]) == nil)
    }
}
