import XCTest
@testable import AINMMessenger

/// Decode-only regression tests locking in three client/server schema mismatches:
/// 1. `ServerCapabilities` must decode when the reference server omits
///    `maxContentBlocks`/`maxMessageSize` (previously required Int → keyNotFound
///    aborted the handshake after the WebSocket upgrade).
/// 2. `handleMessageDelivery` must unwrap the server's `{ "message": …, "verified": … }`
///    envelope wrapper before decoding `MessageEnvelope`.
/// 3. The canonical agent envelope (snake_case `thread_id`, text content block) must
///    decode into a `.text` `ContentBlock`.
///
/// All cases are pure decode tests — no network, no database.
final class EnvelopeDecodeTests: XCTestCase {

    /// Canonical envelope JSON shared by the wrapper and direct-decode tests.
    private let canonicalEnvelopeJSON = """
    {
      "id": "m1",
      "from": "assistant@localhost",
      "thread_id": "t1",
      "type": "message",
      "timestamp": "2026-06-04T05:44:49Z",
      "content": [
        {
          "type": "text",
          "version": "1.0",
          "source": "agent",
          "text": "hi",
          "fallback": "hi"
        }
      ],
      "signature": "abc"
    }
    """

    // MARK: - 1. InitializeResult without the optional max fields

    func test_initializeResult_decodesWithoutMaxFields() throws {
        let json = """
        {
          "protocolVersion": "1.0",
          "serverInfo": { "name": "ainm-reference", "version": "0.1.0" },
          "capabilities": {
            "streaming": true,
            "subThreads": true,
            "storeAndForward": false
          },
          "nonce": "deadbeef"
        }
        """
        let data = Data(json.utf8)

        let result = try JSONDecoder().decode(InitializeResult.self, from: data)

        XCTAssertEqual(result.capabilities.streaming, true)
        XCTAssertNil(result.capabilities.maxContentBlocks)
        XCTAssertNil(result.capabilities.maxMessageSize)
    }

    // MARK: - 2. Delivery wrapper unwrap (mirrors AINMClient.handleMessageDelivery)

    func test_deliverWrapper_decodesInnerEnvelope() throws {
        let wrapperJSON = """
        {
          "message": \(canonicalEnvelopeJSON),
          "verified": true
        }
        """
        let wrapperData = Data(wrapperJSON.utf8)

        // Replicate handleMessageDelivery: decode wrapper as [String: Any],
        // pull "message", re-serialize, decode MessageEnvelope.
        let params = try XCTUnwrap(
            JSONSerialization.jsonObject(with: wrapperData) as? [String: Any]
        )
        let envelopeObj = try XCTUnwrap((params["message"] as? [String: Any]))
        let paramsData = try JSONSerialization.data(withJSONObject: envelopeObj)
        let envelope = try JSONDecoder().decode(MessageEnvelope.self, from: paramsData)

        XCTAssertEqual(envelope.from, "assistant@localhost")
        XCTAssertEqual(envelope.threadId, "t1")
    }

    // MARK: - 3. Canonical agent envelope decodes a text content block

    func test_agentCanonicalEnvelope_decodesTextBlock() throws {
        let data = Data(canonicalEnvelopeJSON.utf8)

        let envelope = try JSONDecoder().decode(MessageEnvelope.self, from: data)

        XCTAssertEqual(envelope.type, .message)
        XCTAssertEqual(envelope.content.count, 1)

        switch envelope.content[0] {
        case .text(let text):
            XCTAssertEqual(text.text, "hi")
        default:
            XCTFail("Expected first content block to be .text, got \(envelope.content[0])")
        }
    }
}
