import XCTest
@testable import AINMMessenger

final class AgentCardDecodeTests: XCTestCase {
    /// Decodes a canonical (camelCase) Agent Card exactly as the server publishes it.
    func testDecodesCanonicalCamelCaseCard() throws {
        let json = """
        {
          "name": "Assistant",
          "description": "Your home server's built-in AI assistant.",
          "url": "https://example.test",
          "version": "0.1.0",
          "protocolVersion": "0.1.0",
          "capabilities": { "streaming": true, "pushNotifications": false },
          "skills": [],
          "ainm": {
            "protocolVersion": "0.1.0",
            "address": "assistant@example.test",
            "type": "agent",
            "contentTypes": ["text", "code"],
            "streaming": { "supported": true, "parallelBlocks": false, "preferredChunkSize": "token" },
            "threading": { "subThreads": true, "maxConcurrentThreads": null },
            "memory": { "archivalSupported": true, "contextFramePreferred": true },
            "tools": []
          }
        }
        """
        let card = try JSONDecoder().decode(AgentCard.self, from: Data(json.utf8))

        XCTAssertEqual(card.name, "Assistant")
        XCTAssertEqual(card.protocolVersion, "0.1.0")
        XCTAssertTrue(card.capabilities.streaming)
        XCTAssertFalse(card.capabilities.pushNotifications)

        let ainm = try XCTUnwrap(card.ainm)
        XCTAssertEqual(ainm.protocolVersion, "0.1.0")
        XCTAssertEqual(ainm.contentTypes, ["text", "code"])
        XCTAssertFalse(ainm.streaming.parallelBlocks)
        XCTAssertEqual(ainm.streaming.preferredChunkSize, "token")
        XCTAssertTrue(ainm.threading.subThreads)
        XCTAssertNil(ainm.threading.maxConcurrentThreads)
        XCTAssertTrue(ainm.memory.archivalSupported)
        XCTAssertTrue(ainm.memory.contextFramePreferred)
        XCTAssertTrue(ainm.tools.isEmpty)
    }

    /// A card round-trips through encode→decode with camelCase keys (saveAgent stores cardJson
    /// by encoding an AgentCard; the stored snapshot must decode back).
    func testRoundTripsThroughEncoding() throws {
        let json = """
        {"name":"Assistant","description":"d","url":"","version":"1.0","protocolVersion":"0.1.0",
         "capabilities":{"streaming":true,"pushNotifications":false},"skills":[],
         "ainm":{"protocolVersion":"0.1.0","address":"assistant@h","type":"agent",
          "contentTypes":["text"],"streaming":{"supported":true,"parallelBlocks":false,"preferredChunkSize":"token"},
          "threading":{"subThreads":true,"maxConcurrentThreads":null},
          "memory":{"archivalSupported":true,"contextFramePreferred":false},"tools":[]}}
        """
        let card = try JSONDecoder().decode(AgentCard.self, from: Data(json.utf8))
        let reencoded = try JSONEncoder().encode(card)
        let again = try JSONDecoder().decode(AgentCard.self, from: reencoded)
        XCTAssertEqual(again.ainm?.memory.archivalSupported, true)
        XCTAssertEqual(again.ainm?.streaming.preferredChunkSize, "token")
        // Encoded form must be camelCase (no snake_case keys).
        let encodedString = String(decoding: reencoded, as: UTF8.self)
        XCTAssertFalse(encodedString.contains("push_notifications"))
        XCTAssertFalse(encodedString.contains("protocol_version"))
    }
}
