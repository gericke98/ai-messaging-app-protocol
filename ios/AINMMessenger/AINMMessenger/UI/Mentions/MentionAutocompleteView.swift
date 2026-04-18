import SwiftUI

/// Participant data for mention autocomplete.
struct MentionCandidate: Identifiable, Sendable {
    let id: String          // identity (AINM address)
    let displayName: String
    let type: String        // "human" or "agent"
}

/// Overlay popup for @-mention autocomplete per UI-SPEC Component 2.
/// Shows filtered list of thread participants when typing @ in the composer.
struct MentionAutocompleteView: View {
    let candidates: [MentionCandidate]
    let filter: String  // text after @
    var onSelect: (MentionCandidate) -> Void

    private var filtered: [MentionCandidate] {
        if filter.isEmpty { return candidates }
        return candidates.filter {
            $0.displayName.localizedCaseInsensitiveContains(filter)
        }
    }

    var body: some View {
        VStack(spacing: 0) {
            if filtered.isEmpty {
                Text("No matches")
                    .font(.subheadline)
                    .foregroundStyle(Color(.secondaryLabel))
                    .frame(height: 48)
                    .frame(maxWidth: .infinity)
                    .accessibilityIdentifier(UITestID.Mention.autocompleteEmpty)
            } else {
                ScrollView {
                    LazyVStack(spacing: 0) {
                        ForEach(filtered) { candidate in
                            Button { onSelect(candidate) } label: {
                                autocompleteRow(candidate)
                            }
                            .buttonStyle(.plain)
                            .accessibilityIdentifier(UITestID.Mention.autocompleteRow(identity: candidate.id))
                            if candidate.id != filtered.last?.id {
                                Divider()
                                    .foregroundStyle(Color(.separator))
                            }
                        }
                    }
                }
                .frame(maxHeight: 200)
                .accessibilityIdentifier(UITestID.Mention.autocompleteList)
            }
        }
        .background(Color(.systemBackground))
        .clipShape(RoundedRectangle(cornerRadius: 12))
        .shadow(color: .black.opacity(0.15), radius: 8, y: -4)
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(Color(.separator), lineWidth: 0.5)
        )
        .accessibilityLabel("Mention suggestions")
    }

    private func autocompleteRow(_ candidate: MentionCandidate) -> some View {
        HStack(spacing: 8) {
            AvatarView(participantType: candidate.type, size: 24)
            VStack(alignment: .leading, spacing: 0) {
                Text(candidate.displayName)
                    .font(.body)
                    .foregroundStyle(Color(.label))
                    .lineLimit(1)
                Text(candidate.id)
                    .font(.caption)
                    .foregroundStyle(Color(.secondaryLabel))
                    .lineLimit(1)
            }
            Spacer()
        }
        .padding(.horizontal, 12)
        .frame(height: 48)
        .contentShape(Rectangle())
        .accessibilityLabel("\(candidate.displayName), \(candidate.type)")
    }
}
