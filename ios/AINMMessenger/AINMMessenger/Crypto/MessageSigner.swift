import CryptoKit
import Foundation

/// Signs and verifies message envelopes per security.md Sections 4 and 5.
///
/// Signing procedure (Section 4):
/// 1. Remove "signature" field from envelope dictionary
/// 2. Canonicalize with RFC 8785 JCS via `JCSCanonicalizer`
/// 3. Encode canonical string as UTF-8 bytes
/// 4. Sign bytes with Ed25519 private key
/// 5. Return base64url-encoded signature (no padding, 86 chars)
///
/// Verification procedure (Section 5):
/// 1. Extract "signature" field
/// 2. Remove "signature" from envelope copy
/// 3. Canonicalize and encode as UTF-8
/// 4. Decode signature from base64url
/// 5. Verify Ed25519 signature against public key
struct MessageSigner: Sendable {

    enum SigningError: Error, Sendable {
        case encodingFailed
        case missingSignature
        case invalidSignature
        case canonicalizationFailed
    }

    // MARK: - Message Envelope Signing (security.md Section 4)

    /// Sign a message envelope dictionary.
    ///
    /// - Parameters:
    ///   - envelope: Dictionary representation of envelope. The "signature" key, if present, is removed before signing.
    ///   - privateKey: Ed25519 private key for signing.
    /// - Returns: Base64url-encoded Ed25519 signature (86 characters, no padding).
    static func sign(
        envelope: [String: Any],
        privateKey: Curve25519.Signing.PrivateKey
    ) throws -> String {
        // Step 1: Remove signature field (defensive)
        var signingInput = envelope
        signingInput.removeValue(forKey: "signature")

        // Step 2: Canonicalize with RFC 8785 JCS
        let canonical = try JCSCanonicalizer.canonicalize(signingInput)

        // Step 3: Encode canonical string as UTF-8 bytes
        guard let data = canonical.data(using: .utf8) else {
            throw SigningError.encodingFailed
        }

        // Step 4: Sign with Ed25519
        let signature = try privateKey.signature(for: data)

        // Step 5: Encode as base64url without padding
        return signature.withUnsafeBytes { Data($0) }.base64urlEncodedString()
    }

    // MARK: - Message Envelope Verification (security.md Section 5)

    /// Verify a message envelope's signature.
    ///
    /// - Parameters:
    ///   - envelope: Dictionary with "signature" key present.
    ///   - publicKey: Ed25519 public key of the claimed sender.
    /// - Returns: `true` if signature is valid, `false` otherwise.
    /// - Throws: `SigningError.missingSignature` if no signature field,
    ///           `SigningError.invalidSignature` if signature can't be decoded.
    static func verify(
        envelope: [String: Any],
        publicKey: Curve25519.Signing.PublicKey
    ) throws -> Bool {
        // Step 1: Extract signature
        guard let sigString = envelope["signature"] as? String else {
            throw SigningError.missingSignature
        }

        // Step 2: Remove signature field
        var signingInput = envelope
        signingInput.removeValue(forKey: "signature")

        // Step 3-4: Canonicalize and encode as UTF-8
        let canonical = try JCSCanonicalizer.canonicalize(signingInput)
        guard let data = canonical.data(using: .utf8) else {
            throw SigningError.encodingFailed
        }

        // Step 5: Decode signature from base64url
        guard let sigData = Data(base64urlEncoded: sigString) else {
            throw SigningError.invalidSignature
        }

        // Step 6: Verify Ed25519 signature
        return publicKey.isValidSignature(sigData, for: data)
    }

    // MARK: - Init Token Signing (transport.md Section 4.3)

    /// Sign an initialization request.
    ///
    /// The init token format is: `identity_address + "|" + timestamp`
    ///
    /// - Parameters:
    ///   - identity: AINM address (e.g., "alice@server.com")
    ///   - timestamp: RFC 3339 timestamp string
    ///   - privateKey: Ed25519 private key
    /// - Returns: Base64url-encoded Ed25519 signature (86 characters)
    static func signInit(
        identity: String,
        timestamp: String,
        privateKey: Curve25519.Signing.PrivateKey
    ) throws -> String {
        let payload = "\(identity)|\(timestamp)"
        guard let data = payload.data(using: .utf8) else {
            throw SigningError.encodingFailed
        }
        let signature = try privateKey.signature(for: data)
        return signature.withUnsafeBytes { Data($0) }.base64urlEncodedString()
    }
}
