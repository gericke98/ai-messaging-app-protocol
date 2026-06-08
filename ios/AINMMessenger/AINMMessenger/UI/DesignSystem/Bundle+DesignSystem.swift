import Foundation

/// Marker class whose bundle locates the DesignSystem asset catalog.
/// Using a class from the app module makes `Bundle.designSystem` resolve correctly
/// from both the app target and the (app-hosted) unit test target.
final class DesignSystemBundleMarker {}

extension Bundle {
    static let designSystem = Bundle(for: DesignSystemBundleMarker.self)
}
