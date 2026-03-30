import Foundation
import Testing

@testable import AINMMessenger

@Suite("Base64url Encoding/Decoding Tests (RFC 4648 Section 5)")
struct Base64urlTests {

    @Test("Encode known byte sequence")
    func testEncode() {
        // "Hello" in base64 = "SGVsbG8=", base64url = "SGVsbG8"
        let data = Data("Hello".utf8)
        let encoded = data.base64urlEncodedString()
        #expect(encoded == "SGVsbG8")
    }

    @Test("Decode known base64url string")
    func testDecode() {
        let decoded = Data(base64urlEncoded: "SGVsbG8")
        #expect(decoded != nil)
        #expect(String(data: decoded!, encoding: .utf8) == "Hello")
    }

    @Test("Round-trip empty data")
    func testRoundTripEmpty() {
        let original = Data()
        let encoded = original.base64urlEncodedString()
        let decoded = Data(base64urlEncoded: encoded)
        #expect(decoded == original)
    }

    @Test("Round-trip 1 byte (padding edge case)")
    func testRoundTrip1Byte() {
        let original = Data([0xFF])
        let encoded = original.base64urlEncodedString()
        let decoded = Data(base64urlEncoded: encoded)
        #expect(decoded == original)
    }

    @Test("Round-trip 2 bytes (padding edge case)")
    func testRoundTrip2Bytes() {
        let original = Data([0xFF, 0xFE])
        let encoded = original.base64urlEncodedString()
        let decoded = Data(base64urlEncoded: encoded)
        #expect(decoded == original)
    }

    @Test("Round-trip 3 bytes (no padding needed)")
    func testRoundTrip3Bytes() {
        let original = Data([0xFF, 0xFE, 0xFD])
        let encoded = original.base64urlEncodedString()
        let decoded = Data(base64urlEncoded: encoded)
        #expect(decoded == original)
    }

    @Test("Output never contains +, /, or =")
    func testNoForbiddenChars() {
        // Use bytes that would produce +, /, and = in standard base64
        let data = Data([0x3E, 0xFF, 0x3F, 0xFB, 0xEF, 0xBF])
        let encoded = data.base64urlEncodedString()
        #expect(!encoded.contains("+"), "base64url must not contain '+'")
        #expect(!encoded.contains("/"), "base64url must not contain '/'")
        #expect(!encoded.contains("="), "base64url must not contain '='")
    }

    @Test("Encode single null byte")
    func testEncodeNullByte() {
        let data = Data([0x00])
        let encoded = data.base64urlEncodedString()
        #expect(encoded == "AA")
    }
}
