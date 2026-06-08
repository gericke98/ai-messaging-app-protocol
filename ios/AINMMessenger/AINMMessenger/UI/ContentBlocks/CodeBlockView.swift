import SwiftUI

/// Renders a CodeBlock with syntax highlighting, language label, and copy button.
/// Per D-08: regex-based highlighting via SyntaxHighlighter, monospaced font.
/// Background: Color(.tertiarySystemBackground), 8pt corner radius.
struct CodeBlockView: View {
    let content: ContentBlock.CodeContent
    @State private var showCopied = false

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Header row: language label + copy button
            HStack {
                Text(content.language.uppercased())
                    .font(.caption)
                    .fontWeight(.bold)
                    .foregroundStyle(Theme.Color.inkSecondary)

                Spacer()

                Button {
                    UIPasteboard.general.string = content.code
                    showCopied = true
                    Task {
                        try? await Task.sleep(for: .seconds(1.5))
                        showCopied = false
                    }
                } label: {
                    Text(showCopied ? "Copied!" : "Copy")
                        .font(.caption)
                        .foregroundStyle(Theme.Color.accent)
                }
                .accessibilityLabel("Copy code")
            }
            .padding(.horizontal, 16)
            .padding(.top, 12)
            .padding(.bottom, 8)

            // Code content with horizontal scroll for long lines
            ScrollView(.horizontal, showsIndicators: false) {
                Text(SyntaxHighlighter.highlight(content.code, language: content.language))
                    .textSelection(.enabled)
                    .padding(.horizontal, 16)
                    .padding(.bottom, 12)
            }
        }
        .background(Theme.Color.surface)
        .clipShape(RoundedRectangle(cornerRadius: Theme.Radius.chip))
        .overlay { RoundedRectangle(cornerRadius: Theme.Radius.chip).stroke(Theme.Color.surfaceBorder, lineWidth: 1) }
    }
}
