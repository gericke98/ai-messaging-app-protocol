import SwiftUI

/// New Thread Composer modal per UI-SPEC Screen 3 (D-04, MSG-08).
/// Allows creating a new thread with participant selection and first message.
struct NewThreadComposerView: View {
    @Bindable var appState: AppState
    @Environment(\.dismiss) private var dismiss
    @State private var toField: String = ""
    @State private var messageText: String = ""
    @State private var isSending: Bool = false
    @State private var errorMessage: String?

    var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                // To field
                HStack {
                    Text("To:")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                    TextField("Add participants...", text: $toField)
                        .font(.subheadline)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 12)

                Divider()

                // Message area
                TextEditor(text: $messageText)
                    .font(.body)
                    .padding(.horizontal, 12)
                    .scrollContentBackground(.hidden)

                Spacer(minLength: 0)
            }
            .navigationTitle("New Conversation")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Discard") {
                        dismiss()
                    }
                }
                ToolbarItem(placement: .primaryAction) {
                    Button {
                        Task { await createAndNavigate() }
                    } label: {
                        if isSending {
                            ProgressView()
                        } else {
                            Image(systemName: "arrow.up.circle.fill")
                                .font(.title3)
                        }
                    }
                    .disabled(messageText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || isSending)
                }
            }
            .alert("Error", isPresented: Binding(
                get: { errorMessage != nil },
                set: { if !$0 { errorMessage = nil } }
            )) {
                Button("OK") { errorMessage = nil }
            } message: {
                Text(errorMessage ?? "")
            }
        }
    }

    private func createAndNavigate() async {
        isSending = true
        defer { isSending = false }

        let title = toField.isEmpty ? nil : toField
        let text = messageText.trimmingCharacters(in: .whitespacesAndNewlines)

        do {
            let threadId = try await appState.createThread(title: title, firstMessageText: text)
            dismiss()
            // After dismiss, navigate to the new thread
            appState.selectedThreadId = threadId
        } catch {
            errorMessage = "Message not sent. Tap to retry."
        }
    }
}
