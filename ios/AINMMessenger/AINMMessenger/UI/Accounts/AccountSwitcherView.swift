import SwiftUI

/// Account switcher sheet reachable from the Conversations toolbar. Lists the device's
/// accounts (active marked), switches on tap, adds a new account via onboarding, and
/// removes accounts via swipe-to-delete with confirmation.
struct AccountSwitcherView: View {
    @Bindable var appState: AppState
    @Environment(\.dismiss) private var dismiss
    @State private var pendingRemoval: AccountSummary?

    var body: some View {
        NavigationStack {
            List {
                Section("Accounts") {
                    ForEach(appState.accounts) { account in
                        accountRow(account)
                    }
                }
                Section {
                    Button {
                        appState.addAccount()
                        dismiss()
                    } label: {
                        Label("Add Account", systemImage: "plus.circle")
                    }
                }
            }
            .navigationTitle("Accounts")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Done") { dismiss() }
                }
            }
            .confirmationDialog(
                "Remove this account from this device?",
                isPresented: Binding(
                    get: { pendingRemoval != nil },
                    set: { if !$0 { pendingRemoval = nil } }
                ),
                titleVisibility: .visible
            ) {
                if let account = pendingRemoval {
                    Button("Remove \(account.username)", role: .destructive) {
                        Task {
                            await appState.removeAccount(account.address)
                            pendingRemoval = nil
                            if appState.sessionPhase == .onboarding { dismiss() }
                        }
                    }
                }
                Button("Cancel", role: .cancel) { pendingRemoval = nil }
            } message: {
                Text("This deletes its key and local conversations on this device. The account itself is not deleted from the server.")
            }
            .onAppear { appState.refreshAccounts() }
        }
    }

    @ViewBuilder
    private func accountRow(_ account: AccountSummary) -> some View {
        Button {
            guard account.address != appState.activeAccount?.address else {
                dismiss(); return
            }
            Task {
                await appState.switchAccount(to: account.address)
                dismiss()
            }
        } label: {
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text(account.username).font(.body)
                    Text(account.address).font(.caption).foregroundStyle(.secondary)
                }
                Spacer()
                if account.address == appState.activeAccount?.address {
                    Image(systemName: "checkmark")
                        .foregroundStyle(Color.accentColor)
                        .accessibilityLabel("Active account")
                }
            }
        }
        .buttonStyle(.plain)
        .swipeActions {
            Button(role: .destructive) {
                pendingRemoval = account
            } label: {
                Label("Remove", systemImage: "trash")
            }
        }
    }
}
