import SwiftUI

/// Typing/processing indicator with bouncing dots (UI-SPEC Component 4).
/// Uses the participant's identity-colored avatar (agents get a halo) and the
/// Editorial Warm tokens. Respects `accessibilityReduceMotion`.
struct TypingIndicatorView: View {
    let name: String
    let isAgent: Bool
    var address: String? = nil
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var dotOffsets: [CGFloat] = [0, 0, 0]

    var body: some View {
        HStack(alignment: .center, spacing: Theme.Spacing.sm) {
            AvatarView(
                participantType: isAgent ? "agent" : "human",
                size: 24,
                address: address,
                displayName: name,
                showAgentHalo: isAgent
            )

            HStack(spacing: Theme.Spacing.xs) {
                ForEach(0..<3, id: \.self) { index in
                    Circle()
                        .fill(Theme.Color.inkSecondary)
                        .frame(width: 6, height: 6)
                        .offset(y: dotOffsets[index])
                }
            }

            Text("\(name) is \(isAgent ? "processing" : "typing")...")
                .font(Theme.Typography.caption())
                .foregroundStyle(Theme.Color.inkSecondary)
        }
        .padding(.vertical, Theme.Spacing.sm)
        .padding(.horizontal, Theme.Spacing.lg)
        .accessibilityElement(children: .combine)
        .accessibilityLabel("\(name) is \(isAgent ? "processing" : "typing")")
        .onAppear {
            guard !reduceMotion else { return }
            for i in 0..<3 {
                withAnimation(
                    .easeInOut(duration: 0.4)
                    .repeatForever(autoreverses: true)
                    .delay(Double(i) * 0.15)
                ) {
                    dotOffsets[i] = -4
                }
            }
        }
    }
}
