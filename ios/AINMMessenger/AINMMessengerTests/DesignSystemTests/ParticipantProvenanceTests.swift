import XCTest
@testable import AINMMessenger

final class ParticipantProvenanceTests: XCTestCase {

    func testHomeHostExtractsHostPart() {
        XCTAssertEqual(ParticipantProvenance.homeHost(from: "alice@localhost"), "localhost")
        XCTAssertEqual(ParticipantProvenance.homeHost(from: "bob@acme.test"), "acme.test")
    }

    func testHomeHostNilWhenNoAtOrEmptyHost() {
        XCTAssertNil(ParticipantProvenance.homeHost(from: "noatsign"))
        XCTAssertNil(ParticipantProvenance.homeHost(from: "trailing@"))
    }

    func testHostSuffixMatchesHomeHostLogic() {
        XCTAssertEqual(ParticipantProvenance.hostSuffix(of: "x@acme.test"), "acme.test")
        XCTAssertNil(ParticipantProvenance.hostSuffix(of: "x"))
    }

    func testIsForeignHostTrueWhenHostsDiffer() {
        XCTAssertTrue(ParticipantProvenance.isForeignHost("bob@acme.test", homeHost: "localhost"))
    }

    func testIsForeignHostFalseWhenSameHost() {
        XCTAssertFalse(ParticipantProvenance.isForeignHost("bob@localhost", homeHost: "localhost"))
    }

    func testIsForeignHostFalseWhenHomeHostNil() {
        XCTAssertFalse(ParticipantProvenance.isForeignHost("bob@acme.test", homeHost: nil))
    }

    func testIsForeignHostFalseWhenAddressHasNoHost() {
        XCTAssertFalse(ParticipantProvenance.isForeignHost("bob", homeHost: "localhost"))
    }
}
