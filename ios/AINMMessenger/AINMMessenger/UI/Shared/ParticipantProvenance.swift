import Foundation

/// Pure helpers for federation provenance: extracting and comparing the host
/// part of an AINM address (`name@host`). No host/UI dependencies — unit-testable.
enum ParticipantProvenance {

    /// Host part (after `@`) of an address, or nil if there's no `@` or the host is empty.
    static func homeHost(from address: String) -> String? {
        guard let at = address.firstIndex(of: "@") else { return nil }
        let host = String(address[address.index(after: at)...])
        return host.isEmpty ? nil : host
    }

    /// Host substring of an address for display. Same rule as `homeHost(from:)`.
    static func hostSuffix(of address: String) -> String? {
        homeHost(from: address)
    }

    /// True when `address`'s host differs from `homeHost`. False when `homeHost`
    /// is nil (not connected → treat all as local) or the address has no host.
    static func isForeignHost(_ address: String, homeHost: String?) -> Bool {
        guard let homeHost else { return false }
        guard let host = hostSuffix(of: address) else { return false }
        return host != homeHost
    }
}
