import SwiftUI

/// Central design-token namespace for the Editorial Warm system.
/// Colors are backed by Assets.xcassets (light + dark); typography uses the
/// system serif (New York) for the editorial voice and SF for body text.
enum Theme {

    // MARK: - Color
    enum Color {
        private static func ds(_ name: String) -> SwiftUI.Color {
            SwiftUI.Color(name, bundle: .designSystem)
        }
        static var paper: SwiftUI.Color           { ds("Paper") }
        static var surface: SwiftUI.Color         { ds("Surface") }
        static var surfaceBorder: SwiftUI.Color   { ds("SurfaceBorder") }
        static var inkPrimary: SwiftUI.Color      { ds("InkPrimary") }
        static var inkSecondary: SwiftUI.Color    { ds("InkSecondary") }
        static var humanBubble: SwiftUI.Color     { ds("HumanBubble") }
        static var humanBubbleText: SwiftUI.Color { ds("HumanBubbleText") }
        static var accent: SwiftUI.Color          { ds("Accent") }
        static var accentSoft: SwiftUI.Color      { ds("AccentSoft") }
        static var separator: SwiftUI.Color       { ds("Separator") }

        /// Per-identity palette. Index order MUST match Identity0…9 in the asset catalog.
        static var identityPalette: [SwiftUI.Color] {
            (0..<10).map { ds("Identity\($0)") }
        }
    }

    // MARK: - Typography (text styles only — Dynamic Type preserved)
    enum Typography {
        static func displayLarge() -> Font { .system(.largeTitle, design: .serif).weight(.semibold) }
        static func displayTitle() -> Font { .system(.title2, design: .serif).weight(.semibold) }
        static func senderName()  -> Font  { .system(.subheadline, design: .serif).weight(.semibold) }
        static func agentProse()  -> Font  { .system(.body, design: .serif) }
        static func body()        -> Font  { .system(.body) }
        static func callout()     -> Font  { .system(.callout) }
        static func label()       -> Font  { .system(.caption, design: .default).weight(.semibold) }
        static func caption()     -> Font  { .system(.caption2) }
    }

    // MARK: - Spacing
    enum Spacing {
        static let xs: CGFloat = 4
        static let sm: CGFloat = 8
        static let md: CGFloat = 12
        static let lg: CGFloat = 16
        static let xl: CGFloat = 20
        static let xxl: CGFloat = 24
    }

    // MARK: - Radius
    enum Radius {
        static let chip: CGFloat = 8
        static let card: CGFloat = 12
        static let bubble: CGFloat = 16
    }

    // MARK: - Motion
    enum Motion {
        static let messageAppear = Animation.spring(response: 0.34, dampingFraction: 0.82)
        static let subtle = Animation.easeInOut(duration: 0.2)
    }
}
