import Foundation
import Testing

@testable import AINMMessenger

/// Unit coverage for the JSON re-encode step that bridges the FFI's `kdfParamsJson`
/// (a JSON object *string*) to/from ``AccountService``'s typed ``KdfParams``.
///
/// The full enroll/recover loop exercises live FFI + a running server (Wave 4 E2E);
/// here we lock down the lossless JSON round-trip ``EscrowClient`` relies on:
///   enroll: `result.kdfParamsJson` (String) → decode → `KdfParams` (upload)
///   recover: `KdfParams` → encode → String → FFI `begin(kdfParamsJson:)`
@Suite("EscrowClient KdfParams JSON Round-Trip")
struct EscrowClientTests {

    @Test("Decode FFI kdfParamsJson string into typed KdfParams")
    func testDecodeKdfParamsJson() throws {
        // Shape the FFI emits: a JSON object string with snake_case keys.
        let json = #"{"salt":"YWJjZGVm","m_cost":19456,"t_cost":2,"p_cost":1}"#
        let kdf = try JSONDecoder().decode(AccountService.KdfParams.self, from: Data(json.utf8))
        #expect(kdf.salt == "YWJjZGVm")
        #expect(kdf.m_cost == 19456)
        #expect(kdf.t_cost == 2)
        #expect(kdf.p_cost == 1)
    }

    @Test("Encode KdfParams to a JSON string for the recovery FFI begin() call")
    func testEncodeKdfParamsToJsonString() throws {
        let kdf = AccountService.KdfParams(salt: "YWJjZGVm", m_cost: 19456, t_cost: 2, p_cost: 1)
        let kdfJson = String(decoding: try JSONEncoder().encode(kdf), as: UTF8.self)
        // Re-decode to assert the string is well-formed and lossless (key order is not guaranteed).
        let again = try JSONDecoder().decode(AccountService.KdfParams.self, from: Data(kdfJson.utf8))
        #expect(again.salt == kdf.salt)
        #expect(again.m_cost == kdf.m_cost)
        #expect(again.t_cost == kdf.t_cost)
        #expect(again.p_cost == kdf.p_cost)
    }

    @Test("Full string→typed→string→typed round-trip is lossless")
    func testFullRoundTrip() throws {
        let original = #"{"salt":"c2FsdHkr","m_cost":65536,"t_cost":3,"p_cost":4}"#
        let decoded = try JSONDecoder().decode(AccountService.KdfParams.self, from: Data(original.utf8))
        let reencoded = String(decoding: try JSONEncoder().encode(decoded), as: UTF8.self)
        let redecoded = try JSONDecoder().decode(AccountService.KdfParams.self, from: Data(reencoded.utf8))
        #expect(redecoded.salt == decoded.salt)
        #expect(redecoded.m_cost == decoded.m_cost)
        #expect(redecoded.t_cost == decoded.t_cost)
        #expect(redecoded.p_cost == decoded.p_cost)
    }
}
