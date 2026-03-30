import SwiftUI

/// Renders an ImageBlock with aspect-fit scaling, max 300pt height, and tap-to-fullscreen.
/// Per D-09: pre-set frame from server-provided dimensions to prevent layout jumps.
/// Placeholder: tertiarySystemFill rectangle with SF Symbol photo.
struct ImageBlockView: View {
    let content: ContentBlock.ImageContent
    @State private var showFullscreen = false

    var body: some View {
        Button {
            showFullscreen = true
        } label: {
            AsyncImage(url: URL(string: content.url)) { phase in
                switch phase {
                case .success(let image):
                    image
                        .resizable()
                        .aspectRatio(contentMode: .fit)
                case .failure:
                    placeholderView
                        .overlay {
                            Image(systemName: "exclamationmark.triangle")
                                .font(.title2)
                                .foregroundStyle(.secondary)
                        }
                case .empty:
                    placeholderView
                        .overlay {
                            ProgressView()
                        }
                @unknown default:
                    placeholderView
                }
            }
            .frame(maxWidth: .infinity)
            .frame(maxHeight: maxImageHeight)
            .clipShape(RoundedRectangle(cornerRadius: 8))
        }
        .buttonStyle(.plain)
        .accessibilityLabel(content.alt)
        .fullScreenCover(isPresented: $showFullscreen) {
            FullscreenImageView(url: content.url, alt: content.alt)
        }
    }

    private var maxImageHeight: CGFloat {
        if let width = content.width, let height = content.height, width > 0 {
            let aspectRatio = CGFloat(height) / CGFloat(width)
            let calculatedHeight = UIScreen.main.bounds.width * aspectRatio
            return min(calculatedHeight, 300)
        }
        return 300
    }

    private var placeholderView: some View {
        Rectangle()
            .fill(Color(.tertiarySystemFill))
            .frame(height: maxImageHeight)
            .overlay {
                Image(systemName: "photo")
                    .font(.title)
                    .foregroundStyle(.secondary)
            }
    }
}
