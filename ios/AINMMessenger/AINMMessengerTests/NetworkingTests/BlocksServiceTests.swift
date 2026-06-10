import Foundation
import Testing

@testable import AINMMessenger

/// Bearer-authed UGC safety client (App Store guideline 1.2): block users
/// (`PUT/DELETE/GET /accounts/blocks`) and report content (`POST /accounts/reports`).
@Suite("BlocksService")
struct BlocksServiceTests {

    @Test("block request targets accounts/blocks/{address} with bearer auth")
    func blockRequestShape() throws {
        let req = try BlocksService.makeRequest(
            baseURL: #require(URL(string: "https://server.example")),
            path: ["accounts", "blocks", "spammer@evil.example"],
            method: "PUT",
            token: "sess_tok_1"
        )
        #expect(req.httpMethod == "PUT")
        // The AINM address survives as one path segment (axum decodes {address}).
        let url = try #require(req.url)
        #expect(url.lastPathComponent == "spammer@evil.example")
        #expect(url.path(percentEncoded: false) == "/accounts/blocks/spammer@evil.example")
        #expect(req.value(forHTTPHeaderField: "Authorization") == "Bearer sess_tok_1")
    }

    @Test("ReportReq encodes snake_case keys and omits nil optionals")
    func reportReqEncoding() throws {
        let full = BlocksService.ReportReq(
            subject: "spammer@evil.example",
            thread_id: "t-1",
            message_id: "m-1",
            reason: "harassment"
        )
        let fullJSON = try #require(String(data: JSONEncoder().encode(full), encoding: .utf8))
        #expect(fullJSON.contains("\"subject\""))
        #expect(fullJSON.contains("\"thread_id\""))
        #expect(fullJSON.contains("\"message_id\""))
        #expect(fullJSON.contains("\"reason\""))

        let minimal = BlocksService.ReportReq(
            subject: "spammer@evil.example", thread_id: nil, message_id: nil, reason: "spam")
        let minimalJSON = try #require(
            String(data: JSONEncoder().encode(minimal), encoding: .utf8))
        #expect(!minimalJSON.contains("thread_id"))
        #expect(!minimalJSON.contains("message_id"))
    }

    @Test("report reason: trimmed, 1-2000 chars (server rule), else nil")
    func reasonValidation() {
        #expect(BlocksService.validReason("  harassment  ") == "harassment")
        #expect(BlocksService.validReason("") == nil)
        #expect(BlocksService.validReason("   \n ") == nil)
        #expect(BlocksService.validReason(String(repeating: "x", count: 2000)) != nil)
        #expect(BlocksService.validReason(String(repeating: "x", count: 2001)) == nil)
    }

    @Test("blocked list decodes the server's bare string array")
    func blockedListDecodes() throws {
        let list = try JSONDecoder().decode(
            [String].self, from: Data(#"["a@x.example","b@y.example"]"#.utf8))
        #expect(list == ["a@x.example", "b@y.example"])
    }
}
