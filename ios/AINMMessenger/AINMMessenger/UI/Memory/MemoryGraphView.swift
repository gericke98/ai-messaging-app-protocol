import SwiftUI

/// Main compound memory screen with tabbed navigation.
/// Provides access to the knowledge graph, latest briefing, and memory settings.
struct MemoryGraphView: View {
    let appState: AppState

    @State private var selectedTab = 0
    @State private var viewModel: MemoryGraphViewModel?

    var body: some View {
        NavigationStack {
            VStack(spacing: 0) {
                // Tab picker
                Picker("View", selection: $selectedTab) {
                    Text("Nodes").tag(0)
                    Text("Briefing").tag(1)
                    Text("Settings").tag(2)
                }
                .pickerStyle(.segmented)
                .padding(.horizontal, 16)
                .padding(.vertical, 8)

                // Tab content
                TabView(selection: $selectedTab) {
                    if let viewModel {
                        NodeListView(viewModel: viewModel)
                            .tag(0)
                    } else {
                        ProgressView()
                            .frame(maxWidth: .infinity, maxHeight: .infinity)
                            .tag(0)
                    }

                    BriefingView(appState: appState)
                        .tag(1)

                    MemorySettingsView(appState: appState)
                        .tag(2)
                }
                .tabViewStyle(.page(indexDisplayMode: .never))
            }
            .navigationTitle("Memory Graph")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    statsLabel
                }
                ToolbarItem(placement: .topBarTrailing) {
                    Button {
                        Task { await appState.fetchMemoryGraph() }
                    } label: {
                        Image(systemName: "arrow.clockwise")
                    }
                }
            }
            .task {
                setupViewModel()
                await appState.fetchMemoryGraph()
            }
        }
    }

    @ViewBuilder
    private var statsLabel: some View {
        if let stats = appState.graphStats {
            Text("\(stats.nodes)N / \(stats.edges)E")
                .font(.caption.monospacedDigit())
                .foregroundStyle(Color(.secondaryLabel))
        }
    }

    private func setupViewModel() {
        guard viewModel == nil, let db = appState.appDatabase else { return }
        let repo = MemoryRepository(dbPool: db.dbPool)
        let vm = MemoryGraphViewModel(repository: repo)
        vm.startObserving()
        viewModel = vm
    }
}
