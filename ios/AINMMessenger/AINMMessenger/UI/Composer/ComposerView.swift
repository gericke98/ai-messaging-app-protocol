import SwiftUI

/// Message composer bar per UI-SPEC Screen 2a (D-12).
/// Auto-growing text field with send button, disabled when offline.
/// Supports @-mention autocomplete for thread participants.
struct ComposerView: View {
    var isOffline: Bool = false
    var participants: [MentionCandidate] = []
    var onSend: (String) -> Void
    var onSendWithMentions: ((String, [ContentBlock.Mention]) -> Void)?

    @State private var text: String = ""
    @State private var showMentionPopup = false
    @State private var mentionFilter = ""
    @State private var mentions: [ContentBlock.Mention] = []

    private var canSend: Bool {
        !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && !isOffline
    }

    var body: some View {
        HStack(alignment: .bottom, spacing: 8) {
            TextField(
                isOffline ? "Offline -- messages will send when reconnected" : "Message...",
                text: $text,
                axis: .vertical
            )
            .lineLimit(1...4)
            .font(.body)
            .disabled(isOffline)
            .onChange(of: text) { _, newValue in
                detectMentionTrigger(in: newValue)
            }

            Button {
                let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
                guard !trimmed.isEmpty else { return }
                if let onSendWithMentions, !mentions.isEmpty {
                    onSendWithMentions(trimmed, mentions)
                } else {
                    onSend(trimmed)
                }
                text = ""
                mentions = []
                showMentionPopup = false
            } label: {
                Image(systemName: "arrow.up.circle.fill")
                    .font(.system(size: 32))
                    .foregroundStyle(canSend ? Color.accentColor : .secondary)
            }
            .disabled(!canSend)
            .accessibilityLabel("Send message")
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 8)
        .background(Color(.systemBackground))
        .overlay(alignment: .top) {
            if showMentionPopup {
                MentionAutocompleteView(
                    candidates: participants,
                    filter: mentionFilter,
                    onSelect: { candidate in
                        insertMention(candidate)
                    }
                )
                .padding(.horizontal, 16)
                .offset(y: -216)
            }
        }
        .overlay(alignment: .top) {
            Divider()
        }
    }

    // MARK: - Mention Detection

    /// Scan backwards from end of text to find an @ trigger.
    /// The @ must be at position 0 or preceded by a space/newline.
    private func detectMentionTrigger(in value: String) {
        guard !value.isEmpty else {
            showMentionPopup = false
            mentionFilter = ""
            return
        }

        // Find last @ that is at start or preceded by whitespace
        var searchIndex = value.endIndex
        while searchIndex > value.startIndex {
            let prev = value.index(before: searchIndex)
            if value[prev] == "@" {
                let isAtStart = prev == value.startIndex
                let precededBySpace: Bool
                if isAtStart {
                    precededBySpace = true
                } else {
                    let beforeAt = value.index(before: prev)
                    let charBefore = value[beforeAt]
                    precededBySpace = charBefore.isWhitespace || charBefore.isNewline
                }

                if isAtStart || precededBySpace {
                    let afterAt = value.index(after: prev)
                    let filterText = String(value[afterAt...])
                    // Only show popup if filter doesn't contain spaces (single word)
                    if !filterText.contains(" ") {
                        showMentionPopup = true
                        mentionFilter = filterText
                        return
                    }
                }
            }
            searchIndex = prev
        }

        showMentionPopup = false
        mentionFilter = ""
    }

    // MARK: - Mention Insertion

    /// Replace the @filter text with @DisplayName and track mention metadata.
    func insertMention(_ candidate: MentionCandidate) {
        // Find the @ trigger position
        guard let atRange = findLastMentionTrigger(in: text) else { return }

        let atPosition = text.distance(from: text.startIndex, to: atRange.lowerBound)
        let mentionText = "@\(candidate.displayName) "

        // Replace @filter with @DisplayName + trailing space
        text.replaceSubrange(atRange, with: mentionText)

        // Track mention metadata (offset is the @ position, length includes @)
        let mention = ContentBlock.Mention(
            address: candidate.id,
            offset: atPosition,
            length: candidate.displayName.count + 1  // +1 for @ character
        )
        mentions.append(mention)
        showMentionPopup = false
        mentionFilter = ""
    }

    /// Find the range of the last @trigger text in the string.
    private func findLastMentionTrigger(in value: String) -> Range<String.Index>? {
        var searchIndex = value.endIndex
        while searchIndex > value.startIndex {
            let prev = value.index(before: searchIndex)
            if value[prev] == "@" {
                let isAtStart = prev == value.startIndex
                let precededBySpace: Bool
                if isAtStart {
                    precededBySpace = true
                } else {
                    let beforeAt = value.index(before: prev)
                    precededBySpace = value[beforeAt].isWhitespace || value[beforeAt].isNewline
                }
                if isAtStart || precededBySpace {
                    return prev ..< value.endIndex
                }
            }
            searchIndex = prev
        }
        return nil
    }
}
