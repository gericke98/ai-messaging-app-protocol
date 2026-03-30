import Foundation
import Testing

@testable import AINMMessenger

@Suite("RFC 8785 JCS Canonicalization Tests")
struct JCSCanonicalizerTests {

    @Test("Sort object keys alphabetically")
    func testSortedKeys() throws {
        let input = Data(#"{"b":2,"a":1}"#.utf8)
        let result = try JCSCanonicalizer.canonicalize(input)
        #expect(result == #"{"a":1,"b":2}"#)
    }

    @Test("Remove whitespace between tokens")
    func testNoWhitespace() throws {
        let input = Data(#"{ "a" : 1 }"#.utf8)
        let result = try JCSCanonicalizer.canonicalize(input)
        #expect(result == #"{"a":1}"#)
    }

    @Test("Sort nested object keys recursively")
    func testNestedSorting() throws {
        let input = Data(#"{"z":{"b":2,"a":1},"a":0}"#.utf8)
        let result = try JCSCanonicalizer.canonicalize(input)
        #expect(result == #"{"a":0,"z":{"a":1,"b":2}}"#)
    }

    @Test("Preserve array element order")
    func testArrayPreservation() throws {
        let input = Data(#"{"a":[3,1,2]}"#.utf8)
        let result = try JCSCanonicalizer.canonicalize(input)
        #expect(result == #"{"a":[3,1,2]}"#)
    }

    @Test("Handle null, true, false literals")
    func testLiterals() throws {
        let input = Data(#"{"a":true,"b":false,"c":null}"#.utf8)
        let result = try JCSCanonicalizer.canonicalize(input)
        #expect(result == #"{"a":true,"b":false,"c":null}"#)
    }

    @Test("Escape control characters in strings")
    func testStringEscaping() throws {
        // Tab and newline should be escaped
        let input = Data("{\"a\":\"hello\\tworld\\n\"}".utf8)
        let result = try JCSCanonicalizer.canonicalize(input)
        #expect(result == "{\"a\":\"hello\\tworld\\n\"}")
    }

    @Test("Integer serializes without decimal point")
    func testIntegerSerialization() throws {
        // JSONSerialization may turn 1 into 1 or 1.0; JCS must output 1
        let input = Data(#"{"n":1}"#.utf8)
        let result = try JCSCanonicalizer.canonicalize(input)
        #expect(result == #"{"n":1}"#)
        #expect(!result.contains("1.0"))
    }

    @Test("Fractional number preserves decimal")
    func testFractionalNumber() throws {
        let input = Data(#"{"n":1.5}"#.utf8)
        let result = try JCSCanonicalizer.canonicalize(input)
        #expect(result == #"{"n":1.5}"#)
    }

    @Test("MessageEnvelope-shaped dict produces deterministic output")
    func testMessageEnvelopeShape() throws {
        let input = Data("""
        {
            "type": "message",
            "from": "alice@example.com",
            "id": "019580a1-7b3e-7000-8000-abcdef123456",
            "thread_id": "019580a1-7b3e-7000-8000-000000000001",
            "timestamp": "2026-03-28T12:34:56.789Z",
            "content": [{"type": "text", "version": "1.0", "source": "user", "text": "Hello", "fallback": "Hello"}]
        }
        """.utf8)
        let result = try JCSCanonicalizer.canonicalize(input)

        // Keys must be sorted at all levels
        let keys = ["content", "from", "id", "thread_id", "timestamp", "type"]
        for i in 0..<(keys.count - 1) {
            let posA = result.range(of: "\"\(keys[i])\"")
            let posB = result.range(of: "\"\(keys[i + 1])\"")
            #expect(posA != nil && posB != nil, "Missing key: \(keys[i]) or \(keys[i + 1])")
            if let posA, let posB {
                #expect(posA.lowerBound < posB.lowerBound, "\(keys[i]) should come before \(keys[i + 1])")
            }
        }

        // No extra whitespace
        #expect(!result.contains(" :"))
        #expect(!result.contains(": "))
    }

    @Test("Top-level array")
    func testTopLevelArray() throws {
        let input = Data(#"[3,1,2]"#.utf8)
        let result = try JCSCanonicalizer.canonicalize(input)
        #expect(result == "[3,1,2]")
    }

    @Test("Empty object")
    func testEmptyObject() throws {
        let input = Data(#"{}"#.utf8)
        let result = try JCSCanonicalizer.canonicalize(input)
        #expect(result == "{}")
    }

    @Test("Empty array")
    func testEmptyArray() throws {
        let input = Data(#"[]"#.utf8)
        let result = try JCSCanonicalizer.canonicalize(input)
        #expect(result == "[]")
    }
}
