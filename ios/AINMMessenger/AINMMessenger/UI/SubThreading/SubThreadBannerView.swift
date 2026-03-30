import SwiftUI

/// Fixed banner at top of sub-thread showing parent context.
/// Per UI-SPEC Component 9: "Thread from {sender}" with back navigation.
struct SubThreadBannerView: View {
    let parentSenderName: String
    var onBackToParent: () -> Void

    var body: some View {
        HStack {
            Button(action: onBackToParent) {
                Image(systemName: "arrow.uturn.backward")
                    .font(.system(size: 14))
                    .foregroundStyle(Color.accentColor)
            }
            .accessibilityLabel("Return to parent thread")

            Spacer()

            Text("Thread from \(parentSenderName)")
                .font(.subheadline.bold())
                .lineLimit(1)

            Spacer()

            Button(action: onBackToParent) {
                Text("View Parent")
                    .font(.caption.bold())
                    .foregroundStyle(Color.accentColor)
            }
        }
        .padding(.horizontal, 16)
        .frame(height: 44)
        .background(Color(.secondarySystemBackground))
        .overlay(alignment: .bottom) {
            Rectangle()
                .fill(Color(.separator))
                .frame(height: 0.5)
        }
    }
}
