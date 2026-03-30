import CryptoKit
import Foundation
import Testing

@testable import AINMMessenger

// MARK: - Helper for locating test vectors

/// Resolves the repo root from the test source file path.
/// Source: ios/AINMMessenger/AINMMessengerTests/CryptoTests/MessageSignerTests.swift
/// Repo root: 4 directories up from the file's directory.
private func repoRoot() -> URL {
    let thisFile = URL(fileURLWithPath: #filePath)
    // File: ios/AINMMessenger/AINMMessengerTests/CryptoTests/MessageSignerTests.swift
    // Go up: MessageSignerTests.swift -> CryptoTests -> AINMMessengerTests -> AINMMessenger -> ios -> repo root
    return thisFile
        .deletingLastPathComponent() // CryptoTests/
        .deletingLastPathComponent() // AINMMessengerTests/
        .deletingLastPathComponent() // AINMMessenger/
        .deletingLastPathComponent() // ios/
        .deletingLastPathComponent() // repo root
}

private func loadJSON(_ relativePath: String) throws -> Any {
    let url = repoRoot().appendingPathComponent(relativePath)
    let data = try Data(contentsOf: url)
    return try JSONSerialization.jsonObject(with: data, options: [])
}

// MARK: - Test Envelope Factory

/// Create a simple test envelope dictionary (no signature).
private func makeTestEnvelope(text: String = "Hello, world!") -> [String: Any] {
    [
        "id": "01935801-0000-7000-8000-000000000001",
        "from": "test@example.com",
        "thread_id": "01935800-0000-7000-8000-000000000001",
        "timestamp": "2026-01-01T00:00:00.000Z",
        "type": "message",
        "content": [
            [
                "type": "text",
                "version": "1.0",
                "source": "user",
                "text": text,
                "fallback": text,
            ] as [String: Any]
        ],
    ]
}

// MARK: - Tests

@Suite("MessageSigner Tests")
struct MessageSignerTests {

    // MARK: - Round-trip Sign + Verify

    @Test("Sign then verify round-trip succeeds")
    func testSignVerifyRoundTrip() throws {
        let privateKey = Curve25519.Signing.PrivateKey()
        let envelope = makeTestEnvelope()

        let signature = try MessageSigner.sign(envelope: envelope, privateKey: privateKey)

        var signedEnvelope = envelope
        signedEnvelope["signature"] = signature

        let isValid = try MessageSigner.verify(
            envelope: signedEnvelope, publicKey: privateKey.publicKey)
        #expect(isValid)
    }

    // MARK: - Tampered Content Fails

    @Test("Verify fails with tampered content")
    func testTamperedContentFails() throws {
        let privateKey = Curve25519.Signing.PrivateKey()
        let envelope = makeTestEnvelope(text: "Original message")

        let signature = try MessageSigner.sign(envelope: envelope, privateKey: privateKey)

        // Tamper: change the text after signing
        var tampered = makeTestEnvelope(text: "Tampered message")
        tampered["signature"] = signature

        let isValid = try MessageSigner.verify(
            envelope: tampered, publicKey: privateKey.publicKey)
        #expect(!isValid)
    }

    // MARK: - Wrong Public Key Fails

    @Test("Verify fails with wrong public key")
    func testWrongPublicKeyFails() throws {
        let signingKey = Curve25519.Signing.PrivateKey()
        let wrongKey = Curve25519.Signing.PrivateKey()
        let envelope = makeTestEnvelope()

        let signature = try MessageSigner.sign(envelope: envelope, privateKey: signingKey)

        var signedEnvelope = envelope
        signedEnvelope["signature"] = signature

        let isValid = try MessageSigner.verify(
            envelope: signedEnvelope, publicKey: wrongKey.publicKey)
        #expect(!isValid)
    }

    // MARK: - Signature Format

    @Test("Signature is 86-character base64url string (64 bytes Ed25519)")
    func testSignatureFormat() throws {
        let privateKey = Curve25519.Signing.PrivateKey()
        let envelope = makeTestEnvelope()

        let signature = try MessageSigner.sign(envelope: envelope, privateKey: privateKey)

        // 64 bytes -> 86 base64url chars (no padding)
        #expect(signature.count == 86)
        #expect(!signature.contains("+"))
        #expect(!signature.contains("/"))
        #expect(!signature.contains("="))
    }

    // MARK: - Missing Signature Throws

    @Test("Verify throws when signature is missing")
    func testMissingSignatureThrows() throws {
        let privateKey = Curve25519.Signing.PrivateKey()
        let envelope = makeTestEnvelope()  // no signature field

        #expect(throws: MessageSigner.SigningError.self) {
            _ = try MessageSigner.verify(envelope: envelope, publicKey: privateKey.publicKey)
        }
    }

    // MARK: - signInit Format

    @Test("signInit produces base64url signature for identity|timestamp payload")
    func testSignInit() throws {
        let privateKey = Curve25519.Signing.PrivateKey()
        let identity = "alice@example.com"
        let timestamp = "2026-01-01T00:00:00.000Z"

        let signature = try MessageSigner.signInit(
            identity: identity, timestamp: timestamp, privateKey: privateKey)

        // 64 bytes -> 86 base64url chars
        #expect(signature.count == 86)

        // Verify by manually signing the same payload
        let payload = "\(identity)|\(timestamp)"
        let payloadData = payload.data(using: .utf8)!
        let sigData = Data(base64urlEncoded: signature)!
        let isValid = privateKey.publicKey.isValidSignature(sigData, for: payloadData)
        #expect(isValid)
    }

    // MARK: - Cross-Platform Signing Vectors

    @Test("Cross-platform signing vectors: JCS match + Rust signatures verify in Swift")
    func testCrossPlatformSigningVectors() throws {
        let root = try #require(loadJSON("tests/vectors/signing_vectors.json") as? [String: Any])
        let keyPair = try #require(root["key_pair"] as? [String: Any])
        let privateKeyB64 = try #require(keyPair["private_key_base64url"] as? String)
        let publicKeyB64 = try #require(keyPair["public_key_base64url"] as? String)

        let privateKeyData = try #require(Data(base64urlEncoded: privateKeyB64))
        let publicKeyData = try #require(Data(base64urlEncoded: publicKeyB64))

        let privateKey = try Curve25519.Signing.PrivateKey(rawRepresentation: privateKeyData)
        let publicKey = try Curve25519.Signing.PublicKey(rawRepresentation: publicKeyData)

        // Verify that CryptoKit derives the same public key from the seed
        #expect(privateKey.publicKey.rawRepresentation == publicKey.rawRepresentation,
                "CryptoKit public key must match the expected public key from vectors")

        let vectors = try #require(root["vectors"] as? [[String: Any]])
        for vector in vectors {
            let name = try #require(vector["name"] as? String)
            let envelope = try #require(vector["envelope_without_signature"] as? [String: Any])
            let expectedSig = try #require(vector["expected_signature_base64url"] as? String)
            let expectedCanonical = try #require(vector["expected_canonical"] as? String)

            // 1. Verify JCS canonical form matches Rust output exactly
            var signingInput = envelope
            signingInput.removeValue(forKey: "signature")
            let canonical = try JCSCanonicalizer.canonicalize(signingInput)
            #expect(canonical == expectedCanonical,
                    "JCS canonical mismatch for vector '\(name)'")

            // 2. Verify Rust-generated signature is accepted by Swift verify
            //    (Cross-platform interop: Rust sign -> Swift verify)
            var signedEnvelope = envelope
            signedEnvelope["signature"] = expectedSig
            let isValid = try MessageSigner.verify(envelope: signedEnvelope, publicKey: publicKey)
            #expect(isValid, "Rust-generated signature verification failed for vector '\(name)'")

            // 3. Verify Swift-generated signature also verifies
            //    (CryptoKit uses randomized Ed25519, so signature differs from Rust,
            //     but must still verify against the same public key)
            let swiftSig = try MessageSigner.sign(envelope: envelope, privateKey: privateKey)
            var swiftSignedEnvelope = envelope
            swiftSignedEnvelope["signature"] = swiftSig
            let swiftValid = try MessageSigner.verify(
                envelope: swiftSignedEnvelope, publicKey: publicKey)
            #expect(swiftValid,
                    "Swift-generated signature must verify for vector '\(name)'")
        }
    }

    // MARK: - Cross-Platform JCS Vectors

    @Test("JCS vectors match Rust serde_json_canonicalizer output")
    func testCrossPlatformJCSVectors() throws {
        let root = try #require(loadJSON("tests/vectors/jcs_vectors.json") as? [String: Any])
        let vectors = try #require(root["vectors"] as? [[String: Any]])

        for vector in vectors {
            let name = try #require(vector["name"] as? String)
            let input = try #require(vector["input"])
            let expectedCanonical = try #require(vector["expected_canonical"] as? String)

            let result = try JCSCanonicalizer.canonicalize(input)
            #expect(result == expectedCanonical,
                    "JCS mismatch for vector '\(name)': got \(result), expected \(expectedCanonical)")
        }
    }

    // MARK: - Signature Field Stripping

    @Test("Sign strips existing signature field before canonicalization")
    func testSignStripsExistingSignature() throws {
        let privateKey = Curve25519.Signing.PrivateKey()
        var envelope = makeTestEnvelope()

        // Sign without signature field
        let sig1 = try MessageSigner.sign(envelope: envelope, privateKey: privateKey)

        // Sign with a stale signature field present
        envelope["signature"] = "stale-signature-value"
        let sig2 = try MessageSigner.sign(envelope: envelope, privateKey: privateKey)

        // CryptoKit Ed25519 uses randomized signing, so signatures differ.
        // Both must verify against the same canonical data, proving the
        // "signature" field was correctly stripped in both cases.
        var env1 = makeTestEnvelope()
        env1["signature"] = sig1
        var env2 = makeTestEnvelope()
        env2["signature"] = sig2

        let valid1 = try MessageSigner.verify(envelope: env1, publicKey: privateKey.publicKey)
        let valid2 = try MessageSigner.verify(envelope: env2, publicKey: privateKey.publicKey)

        #expect(valid1, "First signature must verify")
        #expect(valid2, "Second signature (with stale sig stripped) must verify")
    }
}
