import SwiftUI

/// Blocked-users management (App Store guideline 1.2): lists the addresses this
/// account has blocked (`GET /accounts/blocks`) with swipe-to-unblock. Reached
/// from the account switcher's Safety section.
struct BlockedUsersView: View {
    @Bindable var appState: AppState
    @State private var isLoading = true
    @State private var errorMessage: String?

    var body: some View {
        List {
            if appState.blockedAddresses.isEmpty, !isLoading {
                ContentUnavailableView(
                    "No Blocked Users",
                    systemImage: "hand.raised",
                    description: Text("People you block from a conversation will appear here.")
                )
            } else {
                ForEach(appState.blockedAddresses, id: \.self) { address in
                    VStack(alignment: .leading, spacing: 2) {
                        Text(address.components(separatedBy: "@").first ?? address)
                            .font(.body)
                        Text(address)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    .swipeActions {
                        Button {
                            Task { await unblock(address) }
                        } label: {
                            Label("Unblock", systemImage: "hand.raised.slash")
                        }
                        .tint(.green)
                    }
                }
            }

            if let errorMessage {
                Section {
                    Text(errorMessage)
                        .foregroundStyle(.red)
                        .font(.footnote)
                }
            }
        }
        .navigationTitle("Blocked Users")
        .navigationBarTitleDisplayMode(.inline)
        .overlay {
            if isLoading { ProgressView() }
        }
        .task {
            await appState.refreshBlockedUsers()
            isLoading = false
        }
        .refreshable {
            await appState.refreshBlockedUsers()
        }
    }

    private func unblock(_ address: String) async {
        errorMessage = nil
        do {
            try await appState.unblockUser(address)
        } catch {
            errorMessage = (error as? LocalizedError)?.errorDescription ?? "\(error)"
        }
    }
}
