import UIKit

/// Fire-and-forget haptic feedback. No-ops gracefully where unavailable (e.g. Simulator).
@MainActor
enum Haptics {
    static func send() {
        let g = UIImpactFeedbackGenerator(style: .light)
        g.prepare(); g.impactOccurred()
    }
    static func receive() {
        let g = UIImpactFeedbackGenerator(style: .soft)
        g.prepare(); g.impactOccurred()
    }
    static func select() {
        let g = UISelectionFeedbackGenerator()
        g.prepare(); g.selectionChanged()
    }
    static func success() {
        let g = UINotificationFeedbackGenerator()
        g.prepare(); g.notificationOccurred(.success)
    }
}
