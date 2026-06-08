import SwiftUI

/// Deterministic per-identity color + initials. Pure and stable across processes/devices
/// (uses an explicit FNV-1a hash, NOT Swift's seed-randomized Hasher).
enum ParticipantColor {

    /// Stable palette index for an address in 0..<count.
    static func paletteIndex(for address: String, count: Int) -> Int {
        precondition(count > 0, "palette count must be positive")
        var hash: UInt32 = 2166136261          // FNV-1a 32-bit offset basis
        for byte in address.utf8 {
            hash ^= UInt32(byte)
            hash = hash &* 16777619            // FNV prime, wrapping multiply
        }
        return Int(hash % UInt32(count))
    }

    /// The SwiftUI Color for an address, drawn from Theme.Color.identityPalette.
    static func color(for address: String) -> Color {
        let palette = Theme.Color.identityPalette
        return palette[paletteIndex(for: address, count: palette.count)]
    }

    /// Up to two uppercase initials. For an email-like address with no spaces,
    /// derives from the local-part (before "@"). Never returns empty ("?" fallback).
    static func initials(for displayName: String) -> String {
        let isBareAddress = displayName.contains("@") && !displayName.contains(" ")
        let source = isBareAddress
            ? String(displayName.split(separator: "@").first ?? "")
            : displayName

        let words = source
            .split(whereSeparator: { $0 == " " || $0 == "." || $0 == "_" || $0 == "-" })
            .filter { $0.contains(where: \.isLetter) }

        let letters = words.prefix(2).compactMap { $0.first(where: \.isLetter) }
        if !letters.isEmpty {
            return letters.map { String($0).uppercased() }.joined()
        }
        if let c = source.first(where: \.isLetter) {
            return String(c).uppercased()
        }
        return "?"
    }
}
