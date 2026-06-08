#if DEBUG
import SwiftUI

/// DEBUG-only visual catalog of the Editorial Warm design system.
/// Reachable via #Preview; not compiled into release builds.
struct DesignSystemCatalogView: View {
    private let swatches: [(String, Color)] = [
        ("Paper", Theme.Color.paper),
        ("Surface", Theme.Color.surface),
        ("SurfaceBorder", Theme.Color.surfaceBorder),
        ("InkPrimary", Theme.Color.inkPrimary),
        ("InkSecondary", Theme.Color.inkSecondary),
        ("HumanBubble", Theme.Color.humanBubble),
        ("HumanBubbleText", Theme.Color.humanBubbleText),
        ("Accent", Theme.Color.accent),
        ("AccentSoft", Theme.Color.accentSoft),
        ("Separator", Theme.Color.separator),
    ]
    private let sampleAddresses = [
        "assistant@localhost", "researcher@acme.test", "sam@home.test",
        "alice@example.com", "bob@example.com", "olivia@studio.test",
    ]

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Theme.Spacing.xl) {
                section("Colors") {
                    ForEach(swatches, id: \.0) { name, color in
                        HStack(spacing: Theme.Spacing.md) {
                            RoundedRectangle(cornerRadius: Theme.Radius.chip)
                                .fill(color)
                                .frame(width: 44, height: 44)
                                .overlay(RoundedRectangle(cornerRadius: Theme.Radius.chip)
                                    .stroke(Theme.Color.separator))
                            Text(name).font(Theme.Typography.body())
                        }
                    }
                }
                section("Identity palette") {
                    HStack(spacing: Theme.Spacing.sm) {
                        ForEach(sampleAddresses, id: \.self) { addr in
                            AvatarView(participantType: "agent", size: 40,
                                       address: addr, displayName: addr, showAgentHalo: true)
                        }
                    }
                }
                section("Typography") {
                    Text("Display Large").font(Theme.Typography.displayLarge())
                    Text("Display Title").font(Theme.Typography.displayTitle())
                    Text("Sender Name").font(Theme.Typography.senderName())
                    Text("Agent prose — considered, serif.").font(Theme.Typography.agentProse())
                    Text("Body text in SF.").font(Theme.Typography.body())
                    Text("Callout").font(Theme.Typography.callout())
                    Text("LABEL").font(Theme.Typography.label())
                    Text("Caption").font(Theme.Typography.caption())
                }
            }
            .padding(Theme.Spacing.lg)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .background(Theme.Color.paper)
        .foregroundStyle(Theme.Color.inkPrimary)
    }

    @ViewBuilder
    private func section(_ title: String, @ViewBuilder _ content: () -> some View) -> some View {
        VStack(alignment: .leading, spacing: Theme.Spacing.md) {
            Text(title).font(Theme.Typography.label()).foregroundStyle(Theme.Color.inkSecondary)
            content()
        }
    }
}

#Preview("Catalog — Light") {
    DesignSystemCatalogView().preferredColorScheme(.light)
}
#Preview("Catalog — Dark") {
    DesignSystemCatalogView().preferredColorScheme(.dark)
}
#endif
