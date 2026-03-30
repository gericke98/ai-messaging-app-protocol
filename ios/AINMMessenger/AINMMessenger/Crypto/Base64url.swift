import CryptoKit
import Foundation

/// Base64url encoding/decoding per RFC 4648 Section 5.
/// URL-safe alphabet (- instead of +, _ instead of /) with no padding.
extension Data {

    /// Encode data as base64url string (no padding).
    func base64urlEncodedString() -> String {
        base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }

    /// Decode a base64url string (handles missing padding).
    init?(base64urlEncoded string: String) {
        var base64 = string
            .replacingOccurrences(of: "-", with: "+")
            .replacingOccurrences(of: "_", with: "/")
        let remainder = base64.count % 4
        if remainder > 0 {
            base64 += String(repeating: "=", count: 4 - remainder)
        }
        self.init(base64Encoded: base64)
    }
}

/// Convenience extension for Ed25519 public key base64url representation.
extension Curve25519.Signing.PublicKey {
    var base64urlString: String {
        rawRepresentation.base64urlEncodedString()
    }
}
