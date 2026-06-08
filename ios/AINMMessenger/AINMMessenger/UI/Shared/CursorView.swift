import SwiftUI

/// Blinking caret for streaming text (UI-SPEC Component 2).
/// 2pt-wide rounded rectangle that blinks at a 0.5s cycle in the supplied color
/// (the message's identity color, or Accent by default). Stays solid under
/// `accessibilityReduceMotion`.
struct CursorView: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var opacity: Double = 1.0
    var height: CGFloat = 12  // matches body cap height
    var color: Color = Theme.Color.accent

    var body: some View {
        RoundedRectangle(cornerRadius: 1)
            .fill(color)
            .frame(width: 2, height: height)
            .opacity(opacity)
            .onAppear {
                if !reduceMotion {
                    withAnimation(.easeInOut(duration: 0.5).repeatForever(autoreverses: true)) {
                        opacity = 0.0
                    }
                }
            }
            .accessibilityHidden(true)
    }
}
