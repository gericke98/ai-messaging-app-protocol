import Foundation
import Testing

@testable import AINMMessenger

@Suite("JSON-RPC 2.0 Encode/Decode Tests")
struct JsonRpcTests {

    let encoder: JSONEncoder = {
        let e = JSONEncoder()
        e.outputFormatting = [.sortedKeys]
        return e
    }()

    let decoder = JSONDecoder()

    // MARK: - Request Tests

    @Test("Encode ainm.initialize request")
    func testEncodeRequest() throws {
        let request = JsonRpcRequest(
            jsonrpc: "2.0",
            id: .int(1),
            method: "ainm.initialize",
            params: .object([
                "protocolVersion": .string("0.1.0"),
                "identity": .string("alice@example.com")
            ])
        )
        let data = try encoder.encode(request)
        let json = try #require(String(data: data, encoding: .utf8))

        #expect(json.contains("\"jsonrpc\":\"2.0\""))
        #expect(json.contains("\"id\":1"))
        #expect(json.contains("\"method\":\"ainm.initialize\""))
        #expect(json.contains("\"protocolVersion\":\"0.1.0\""))
    }

    @Test("Encode request with nil params omits params key")
    func testEncodeRequestNilParams() throws {
        let request = JsonRpcRequest(
            jsonrpc: "2.0",
            id: .int(1),
            method: "ainm.ping",
            params: nil
        )
        let data = try encoder.encode(request)
        let json = try #require(String(data: data, encoding: .utf8))

        #expect(!json.contains("\"params\""))
    }

    // MARK: - Response Tests

    @Test("Decode success response with result field")
    func testDecodeResponse() throws {
        let json = """
        {
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "protocolVersion": "0.1.0",
                "serverInfo": {"name": "ainm-demo", "version": "0.1.0"},
                "capabilities": {"streaming": true},
                "nonce": "abc123"
            }
        }
        """
        let response = try decoder.decode(JsonRpcResponse.self, from: Data(json.utf8))

        #expect(response.jsonrpc == "2.0")
        #expect(response.id == .int(1))
        #expect(response.error == nil)

        if case .object(let result) = response.result {
            #expect(result["protocolVersion"] == .string("0.1.0"))
            #expect(result["nonce"] == .string("abc123"))
        } else {
            Issue.record("Expected object result")
        }
    }

    @Test("Decode error response with code -32010")
    func testDecodeError() throws {
        let json = """
        {
            "jsonrpc": "2.0",
            "id": 1,
            "error": {
                "code": -32010,
                "message": "Invalid signature"
            }
        }
        """
        let response = try decoder.decode(JsonRpcResponse.self, from: Data(json.utf8))

        #expect(response.jsonrpc == "2.0")
        #expect(response.result == nil)
        #expect(response.error?.code == -32010)
        #expect(response.error?.message == "Invalid signature")
    }

    // MARK: - Notification Tests

    @Test("Encode ainm.initialized notification")
    func testInitializedNotification() throws {
        let notification = JsonRpcNotification.initialized()
        let data = try encoder.encode(notification)
        let json = try #require(String(data: data, encoding: .utf8))

        #expect(json.contains("\"jsonrpc\":\"2.0\""))
        #expect(json.contains("\"method\":\"ainm.initialized\""))
        #expect(!json.contains("\"params\""))
    }

    // MARK: - MessageEnvelope Tests

    @Test("MessageEnvelope encodes threadId as thread_id")
    func testMessageEnvelopeCodingKeys() throws {
        let envelope = MessageEnvelope(
            id: "019580a1-7b3e-7000-8000-abcdef123456",
            from: "alice@example.com",
            threadId: "019580a1-7b3e-7000-8000-000000000001",
            timestamp: "2026-03-28T12:34:56.789Z",
            type: .message,
            content: [
                ContentBlock(
                    type: "text",
                    version: "1.0",
                    source: "user",
                    text: "Hello",
                    fallback: "Hello"
                )
            ],
            signature: nil,
            parentId: nil
        )
        let data = try encoder.encode(envelope)
        let json = try #require(String(data: data, encoding: .utf8))

        // Verify snake_case encoding
        #expect(json.contains("\"thread_id\""))
        #expect(!json.contains("\"threadId\""))
        // Verify signature is omitted when nil
        #expect(!json.contains("\"signature\""))
        // Verify parent_id is omitted when nil
        #expect(!json.contains("\"parent_id\""))
    }

    @Test("MessageEnvelope round-trips through JSON")
    func testMessageEnvelopeRoundTrip() throws {
        let original = MessageEnvelope(
            id: "019580a1-7b3e-7000-8000-abcdef123456",
            from: "alice@example.com",
            threadId: "019580a1-7b3e-7000-8000-000000000001",
            timestamp: "2026-03-28T12:34:56.789Z",
            type: .message,
            content: [
                ContentBlock(
                    type: "text",
                    version: "1.0",
                    source: "user",
                    text: "Hello world",
                    fallback: "Hello world"
                )
            ],
            signature: "dGhpcyBpcyBhIHNpZ25hdHVyZQ",
            parentId: nil
        )
        let data = try encoder.encode(original)
        let decoded = try decoder.decode(MessageEnvelope.self, from: data)

        #expect(decoded.id == original.id)
        #expect(decoded.from == original.from)
        #expect(decoded.threadId == original.threadId)
        #expect(decoded.type == original.type)
        #expect(decoded.signature == original.signature)
    }

    // MARK: - AnyCodableValue Tests

    @Test("AnyCodableValue decodes all JSON types")
    func testAnyCodableValueDecode() throws {
        let json = """
        {
            "null": null,
            "bool": true,
            "int": 42,
            "double": 3.14,
            "string": "hello",
            "array": [1, 2, 3],
            "object": {"key": "value"}
        }
        """
        let value = try decoder.decode(AnyCodableValue.self, from: Data(json.utf8))
        if case .object(let dict) = value {
            #expect(dict["null"] == .null)
            #expect(dict["bool"] == .bool(true))
            #expect(dict["int"] == .int(42))
            #expect(dict["string"] == .string("hello"))
        } else {
            Issue.record("Expected object")
        }
    }
}
