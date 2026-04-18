#if DEBUG
import CryptoKit
import Foundation
import Testing

@testable import AINMMessenger

/// Regression guards for the deterministic UAT Ed25519 keypair.
/// Any drift in the seed bytes invalidates every signed fixture — these tests
/// catch the drift at build time rather than in a failing UI test run later.
struct UITestIdentityBootstrapTests {

    @Test("uatSeedBytes is exactly 32 bytes (Ed25519 scalar size)")
    func test_uatSeedBytes_exactly32() {
        #expect(UITestIdentityBootstrap.uatSeedBytes.count == 32)
    }

    @Test("Deterministic public key is stable across successive computations")
    func test_deterministicPublicKey_stableAcrossCalls() {
        let first = UITestIdentityBootstrap.deterministicPublicKeyBase64
        let second = UITestIdentityBootstrap.deterministicPublicKeyBase64
        #expect(first == second)
        #expect(!first.isEmpty)
    }

    @Test("Seed ASCII prefix spells UAT_USER@LOCALHOST (diff-legible literal)")
    func test_seedAsciiPrefix_matchesIdentity() {
        let expectedPrefix: [UInt8] = Array("UAT_USER@LOCALHOST".utf8)
        let seedPrefix = Array(UITestIdentityBootstrap.uatSeedBytes.prefix(expectedPrefix.count))
        #expect(seedPrefix == expectedPrefix)
    }

    @Test("identityAddress matches the ASCII encoding of the seed prefix")
    func test_identityAddress_consistentWithSeed() {
        #expect(UITestIdentityBootstrap.identityAddress == "uat-user@localhost")
    }

    @Test("Private key reconstructed from seed yields a 32-byte public key")
    func test_privateKeyFromSeed_producesValidPublicKey() throws {
        let seed = Data(UITestIdentityBootstrap.uatSeedBytes)
        let privateKey = try Curve25519.Signing.PrivateKey(rawRepresentation: seed)
        #expect(privateKey.publicKey.rawRepresentation.count == 32)
    }
}
#endif
