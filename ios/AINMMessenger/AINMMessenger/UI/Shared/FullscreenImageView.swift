import SwiftUI

/// Full-screen image viewer with pinch-to-zoom and double-tap to reset.
/// Presented from ImageBlockView on tap.
struct FullscreenImageView: View {
    let url: String
    let alt: String
    @Environment(\.dismiss) private var dismiss
    @State private var scale: CGFloat = 1.0
    @GestureState private var magnification: CGFloat = 1.0

    var body: some View {
        NavigationStack {
            ZStack {
                Color.black.ignoresSafeArea()

                AsyncImage(url: URL(string: url)) { phase in
                    switch phase {
                    case .success(let image):
                        image
                            .resizable()
                            .aspectRatio(contentMode: .fit)
                            .scaleEffect(scale * magnification)
                            .gesture(magnifyGesture)
                            .onTapGesture(count: 2) {
                                withAnimation(.spring()) {
                                    scale = 1.0
                                }
                            }
                    case .failure:
                        Image(systemName: "exclamationmark.triangle")
                            .font(.largeTitle)
                            .foregroundStyle(.white)
                    case .empty:
                        ProgressView()
                            .tint(.white)
                    @unknown default:
                        EmptyView()
                    }
                }
            }
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    Button {
                        dismiss()
                    } label: {
                        Image(systemName: "xmark")
                            .foregroundStyle(.white)
                            .fontWeight(.semibold)
                    }
                }
            }
            .toolbarBackground(.hidden, for: .navigationBar)
        }
        .accessibilityLabel(alt)
    }

    private var magnifyGesture: some Gesture {
        MagnifyGesture()
            .updating($magnification) { value, state, _ in
                state = value.magnification
            }
            .onEnded { value in
                scale *= value.magnification
                scale = max(0.5, min(scale, 5.0))
            }
    }
}
