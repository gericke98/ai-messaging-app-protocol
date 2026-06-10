import CryptoKit
import Foundation
import Testing

@testable import AINMMessenger

/// Session-token mint challenge signing (server signing.rs `verify_auth_challenge`):
/// payload is `"ainm-auth-token|{identity}|{timestamp}"` — domain-separated from the
/// WebSocket handshake payload (`"{identity}|{timestamp}"`) so a captured handshake
/// signature can never mint a REST session.
@Suite("Auth challenge signing")
struct AuthChallengeSignTests {

    @Test("signature verifies against the domain-separated payload")
    func signatureVerifiesAgainstDomainPayload() throws {
        let key = Curve25519.Signing.PrivateKey()
        let identity = "alice@test.local"
        let timestamp = "2026-06-10T12:00:00Z"

        let sigB64 = try MessageSigner.signAuthChallenge(
            identity: identity, timestamp: timestamp, privateKey: key)

        let sig = try #require(Data(base64urlEncoded: sigB64))
        let payload = Data("ainm-auth-token|\(identity)|\(timestamp)".utf8)
        #expect(key.publicKey.isValidSignature(sig, for: payload))
    }

    @Test("signature is NOT valid for the handshake payload (domain separation)")
    func domainSeparationFromHandshake() throws {
        let key = Curve25519.Signing.PrivateKey()
        let identity = "alice@test.local"
        let timestamp = "2026-06-10T12:00:00Z"

        let sigB64 = try MessageSigner.signAuthChallenge(
            identity: identity, timestamp: timestamp, privateKey: key)

        let sig = try #require(Data(base64urlEncoded: sigB64))
        let handshakePayload = Data("\(identity)|\(timestamp)".utf8)
        #expect(!key.publicKey.isValidSignature(sig, for: handshakePayload))
    }

    @Test("signature is base64url without padding")
    func base64urlNoPadding() throws {
        let key = Curve25519.Signing.PrivateKey()
        let sigB64 = try MessageSigner.signAuthChallenge(
            identity: "a@b", timestamp: "2026-06-10T12:00:00Z", privateKey: key)
        #expect(!sigB64.contains("="))
        #expect(!sigB64.contains("+"))
        #expect(!sigB64.contains("/"))
        #expect(sigB64.count == 86)  // 64-byte Ed25519 sig → 86 base64url chars
    }
}
