import SwiftUI

/// Typing/processing indicator with bouncing dots animation.
/// Per UI-SPEC Component 4: shows avatar, animated dots, and descriptive label.
/// Respects `accessibilityReduceMotion` to disable animation when requested.
struct TypingIndicatorView: View {
    let name: String
    let isAgent: Bool
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var dotOffsets: [CGFloat] = [0, 0, 0]

    var body: some View {
        HStack(alignment: .center, spacing: 8) {
            AvatarView(participantType: isAgent ? "agent" : "human", size: 24)

            HStack(spacing: 4) {
                ForEach(0..<3, id: \.self) { index in
                    Circle()
                        .fill(Color(.tertiaryLabel))
                        .frame(width: 6, height: 6)
                        .offset(y: dotOffsets[index])
                }
            }

            Text("\(name) is \(isAgent ? "processing" : "typing")...")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .padding(.vertical, 8)
        .padding(.horizontal, 16)
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
