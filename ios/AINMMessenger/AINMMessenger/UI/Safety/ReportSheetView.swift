import SwiftUI

/// Report-content sheet (App Store guideline 1.2). Presented from a message's
/// context menu; files a report against the sender via `POST /accounts/reports`
/// and offers to block them in the same gesture.
struct ReportSheetView: View {
    @Bindable var appState: AppState
    /// The reported user's AINM address.
    let subject: String
    let threadId: String?
    let messageId: String?

    @Environment(\.dismiss) private var dismiss
    @State private var reason: String = ""
    @State private var alsoBlock: Bool = false
    @State private var isSubmitting = false
    @State private var errorMessage: String?

    private var trimmedReason: String? {
        BlocksService.validReason(reason)
    }

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    LabeledContent("Reporting", value: subject)
                } footer: {
                    Text("Your report goes to the server operators for review.")
                }

                Section("What's wrong?") {
                    TextEditor(text: $reason)
                        .frame(minHeight: 120)
                        .accessibilityLabel("Report reason")
                    HStack {
                        Spacer()
                        Text("\(reason.count)/2000")
                            .font(.caption)
                            .foregroundStyle(reason.count > 2000 ? .red : .secondary)
                    }
                }

                Section {
                    Toggle("Also block \(displayName)", isOn: $alsoBlock)
                } footer: {
                    Text("Blocking stops their messages from reaching you.")
                }

                if let errorMessage {
                    Section {
                        Text(errorMessage)
                            .foregroundStyle(.red)
                            .font(.footnote)
                    }
                }
            }
            .navigationTitle("Report")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Submit") {
                        Task { await submit() }
                    }
                    .disabled(trimmedReason == nil || isSubmitting)
                }
            }
            .interactiveDismissDisabled(isSubmitting)
        }
    }

    private var displayName: String {
        subject.components(separatedBy: "@").first ?? subject
    }

    private func submit() async {
        guard let reason = trimmedReason else { return }
        isSubmitting = true
        errorMessage = nil
        do {
            try await appState.reportContent(
                subject: subject, threadId: threadId, messageId: messageId, reason: reason)
            if alsoBlock {
                try await appState.blockUser(subject)
            }
            dismiss()
        } catch {
            errorMessage = (error as? LocalizedError)?.errorDescription ?? "\(error)"
            isSubmitting = false
        }
    }
}
