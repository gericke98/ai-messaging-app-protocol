import SwiftUI

/// Reusable avatar circle for conversation list rows and message thread headers.
/// Per UI-SPEC D-02, D-05: human shows person.fill, agent shows cpu.fill.
/// Default 36pt diameter per UI-SPEC conversation row specification.
struct AvatarView: View {
    let participantType: String  // "human" or "agent"
    var size: CGFloat = 36       // 36pt per UI-SPEC

    var body: some View {
        Circle()
            .fill(participantType == "agent"
                ? Color.orange.opacity(0.15)
                : Color.blue.opacity(0.15))
            .frame(width: size, height: size)
            .overlay {
                Image(systemName: participantType == "agent" ? "cpu.fill" : "person.fill")
                    .font(.system(size: size * 0.45))
                    .foregroundStyle(participantType == "agent" ? .orange : .blue)
            }
            .accessibilityLabel(participantType == "agent" ? "Agent avatar" : "Human avatar")
    }
}
