import CryptoKit
import Foundation
import Testing

@testable import AINMMessenger

/// Session-token mint client for `POST /accounts/auth/token` (server
/// `auth_token_mint_handler`): the request proves possession of the account's
/// Ed25519 key via the domain-separated challenge; the response carries the
/// bearer token used by every authed REST endpoint (blocks, reports, sessions).
@Suite("SessionAuthService")
struct SessionAuthServiceTests {

    @Test("mint request body: snake_case keys, RFC3339 timestamp, verifying signature")
    func mintRequestBody() throws {
        let key = Curve25519.Signing.PrivateKey()
        let now = Date(timeIntervalSince1970: 1_780_000_000)

        let req = try SessionAuthService.makeMintRequest(
            identity: "alice@test.local",
            privateKey: key,
            deviceLabel: "iPhone",
            now: now
        )

        #expect(req.identity == "alice@test.local")
        #expect(req.device_label == "iPhone")
        // Timestamp parses back as RFC3339 and equals `now` (second precision).
        let parsed = try #require(ISO8601DateFormatter().date(from: req.timestamp))
        #expect(abs(parsed.timeIntervalSince(now)) < 1)
        // The signature verifies over the server's challenge payload.
        let sig = try #require(Data(base64urlEncoded: req.signature))
        let payload = Data("ainm-auth-token|alice@test.local|\(req.timestamp)".utf8)
        #expect(key.publicKey.isValidSignature(sig, for: payload))

        // JSON keys are exactly what the server's MintTokenReq expects.
        let json = try #require(String(data: JSONEncoder().encode(req), encoding: .utf8))
        #expect(json.contains("\"identity\""))
        #expect(json.contains("\"timestamp\""))
        #expect(json.contains("\"signature\""))
        #expect(json.contains("\"device_label\""))
    }

    @Test("mint request omits nil device_label")
    func mintRequestOmitsNilLabel() throws {
        let req = try SessionAuthService.makeMintRequest(
            identity: "alice@test.local",
            privateKey: Curve25519.Signing.PrivateKey(),
            deviceLabel: nil,
            now: Date(timeIntervalSince1970: 1_780_000_000)
        )
        let json = try #require(String(data: JSONEncoder().encode(req), encoding: .utf8))
        #expect(!json.contains("device_label"))
    }

    @Test("cached token is valid until 60s before expiry, then re-mints")
    func tokenValidityMargin() {
        let expiry = Date(timeIntervalSince1970: 2_000_000_000)
        let token = SessionAuthService.SessionToken(token: "t", expiresAt: expiry)
        // Comfortably before expiry: still valid.
        #expect(token.isValid(now: expiry.addingTimeInterval(-3600)))
        // Inside the 60s safety margin: treat as expired (mint a fresh one).
        #expect(!token.isValid(now: expiry.addingTimeInterval(-30)))
        // Past expiry: invalid.
        #expect(!token.isValid(now: expiry.addingTimeInterval(10)))
    }

    @Test("MintResp decodes the server's token/session_id/expires_at")
    func mintRespDecodes() throws {
        let serverJSON = """
        {
          "token": "ainm_sess_abc123",
          "session_id": "0197a3a0-1111-7000-8000-000000000001",
          "expires_at": "2026-07-10T12:00:00Z"
        }
        """
        let resp = try JSONDecoder().decode(
            SessionAuthService.MintResp.self, from: Data(serverJSON.utf8))
        #expect(resp.token == "ainm_sess_abc123")
        #expect(resp.session_id == "0197a3a0-1111-7000-8000-000000000001")
        let expiry = try #require(ISO8601DateFormatter().date(from: resp.expires_at))
        #expect(expiry > Date(timeIntervalSince1970: 0))
    }
}
