import SwiftUI

/// Renders a FileBlock with filename, size, and download/share action.
/// Per UI-SPEC: bordered container with doc icon, file info, and download arrow.
/// Tap initiates download via URLSession and presents UIActivityViewController.
struct FileBlockView: View {
    let content: ContentBlock.FileContent
    @State private var downloadState: DownloadState = .idle
    @State private var downloadedFileURL: URL?
    @State private var showShareSheet = false

    enum DownloadState {
        case idle
        case downloading
        case completed
        case failed
    }

    var body: some View {
        Button {
            handleTap()
        } label: {
            HStack(spacing: 12) {
                Image(systemName: "doc.fill")
                    .font(.system(size: 28))
                    .foregroundStyle(.secondary)
                    .accessibilityHidden(true)

                VStack(alignment: .leading, spacing: 2) {
                    Text(content.filename)
                        .font(.subheadline)
                        .fontWeight(.bold)
                        .foregroundStyle(Color(.label))
                        .lineLimit(1)

                    Text(formattedFileSize)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                }

                Spacer()

                Group {
                    switch downloadState {
                    case .idle:
                        Image(systemName: "arrow.down.circle")
                            .font(.system(size: 24))
                            .foregroundStyle(Color.accentColor)
                    case .downloading:
                        ProgressView()
                    case .completed:
                        Image(systemName: "checkmark.circle.fill")
                            .font(.system(size: 24))
                            .foregroundStyle(.green)
                    case .failed:
                        Image(systemName: "exclamationmark.circle")
                            .font(.system(size: 24))
                            .foregroundStyle(.red)
                    }
                }
                .accessibilityLabel("Download file")
            }
            .padding(12)
            .background(
                RoundedRectangle(cornerRadius: 8)
                    .stroke(Color(.separator), lineWidth: 1)
            )
        }
        .buttonStyle(.plain)
        .sheet(isPresented: $showShareSheet) {
            if let fileURL = downloadedFileURL {
                ShareSheet(items: [fileURL])
            }
        }
    }

    private var formattedFileSize: String {
        let formatter = ByteCountFormatter()
        formatter.countStyle = .file
        return formatter.string(fromByteCount: Int64(content.sizeBytes))
    }

    private func handleTap() {
        if downloadedFileURL != nil {
            showShareSheet = true
            return
        }

        guard downloadState != .downloading else { return }
        downloadState = .downloading

        Task {
            do {
                guard let url = URL(string: content.url) else {
                    downloadState = .failed
                    return
                }
                let (tempURL, _) = try await URLSession.shared.download(from: url)
                let destinationURL = FileManager.default.temporaryDirectory
                    .appendingPathComponent(content.filename)
                try? FileManager.default.removeItem(at: destinationURL)
                try FileManager.default.moveItem(at: tempURL, to: destinationURL)
                downloadedFileURL = destinationURL
                downloadState = .completed
                showShareSheet = true
            } catch {
                downloadState = .failed
            }
        }
    }
}

/// UIKit wrapper for UIActivityViewController share sheet.
struct ShareSheet: UIViewControllerRepresentable {
    let items: [Any]

    func makeUIViewController(context: Context) -> UIActivityViewController {
        UIActivityViewController(activityItems: items, applicationActivities: nil)
    }

    func updateUIViewController(_ vc: UIActivityViewController, context: Context) {}
}
