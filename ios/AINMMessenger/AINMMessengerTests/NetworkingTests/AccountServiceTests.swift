import Foundation
import Testing

@testable import AINMMessenger

/// Verifies that AccountService DTOs encode/decode with the exact snake_case
/// JSON keys the server expects, using the default (verbatim) key strategy.
@Suite("AccountService DTO Codable Tests")
struct AccountServiceTests {

    @Test("StoreEscrowReq encodes exact snake_case keys")
    func testStoreEscrowReqEncoding() throws {
        let req = AccountService.StoreEscrowReq(
            enrollment_token: "tok",
            ciphertext_b64: "Y2lwaGVy",
            kdf_params: AccountService.KdfParams(salt: "c2FsdA", m_cost: 19_456, t_cost: 2, p_cost: 1),
            oprf_key_b64: "b3ByZg"
        )

        let data = try JSONEncoder().encode(req)
        let json = try #require(String(data: data, encoding: .utf8))

        #expect(json.contains("\"enrollment_token\""))
        #expect(json.contains("\"ciphertext_b64\""))
        #expect(json.contains("\"kdf_params\""))
        #expect(json.contains("\"oprf_key_b64\""))
        // KdfParams nested keys
        #expect(json.contains("\"salt\""))
        #expect(json.contains("\"m_cost\""))
        #expect(json.contains("\"t_cost\""))
        #expect(json.contains("\"p_cost\""))
        // No camelCase leakage from default encoder strategy
        #expect(!json.contains("enrollmentToken"))
        #expect(!json.contains("ciphertextB64"))
    }

    @Test("EscrowFetchResp decodes server snake_case keys")
    func testEscrowFetchRespDecoding() throws {
        let serverJSON = """
        {
          "evaluated_point": "ZXZhbA",
          "ciphertext_b64": "Y2lwaGVy",
          "kdf_params": { "salt": "c2FsdA", "m_cost": 19456, "t_cost": 2, "p_cost": 1 }
        }
        """
        let data = Data(serverJSON.utf8)
        let resp = try JSONDecoder().decode(AccountService.EscrowFetchResp.self, from: data)

        #expect(resp.evaluated_point == "ZXZhbA")
        #expect(resp.ciphertext_b64 == "Y2lwaGVy")
        #expect(resp.kdf_params.salt == "c2FsdA")
        #expect(resp.kdf_params.m_cost == 19_456)
        #expect(resp.kdf_params.t_cost == 2)
        #expect(resp.kdf_params.p_cost == 1)
    }

    @Test("BindKeyReq encodes snake_case keys and omits nil device_label")
    func testBindKeyReqEncoding() throws {
        let req = AccountService.BindKeyReq(
            enrollment_token: "tok",
            public_key: "cHVia2V5",
            device_label: nil
        )
        let json = try #require(String(data: try JSONEncoder().encode(req), encoding: .utf8))
        #expect(json.contains("\"enrollment_token\""))
        #expect(json.contains("\"public_key\""))
        // nil optional is omitted by default encoder
        #expect(!json.contains("device_label"))
    }

    @Test("EscrowFetchReq and VerifyResp use exact server keys")
    func testTokenKeys() throws {
        let fetchJSON = try #require(
            String(data: try JSONEncoder().encode(
                AccountService.EscrowFetchReq(recovery_token: "rtok", blinded_point: "YmxpbmQ")
            ), encoding: .utf8)
        )
        #expect(fetchJSON.contains("\"recovery_token\""))
        #expect(fetchJSON.contains("\"blinded_point\""))

        let verifyResp = try JSONDecoder().decode(
            AccountService.VerifyResp.self,
            from: Data("{\"enrollment_token\":\"tok\"}".utf8)
        )
        #expect(verifyResp.enrollment_token == "tok")
    }
}
