import SwiftUI

@main
struct LunaWatchApp: App {
    @State private var receiver = WatchSnapshotReceiver()

    var body: some Scene {
        WindowGroup {
#if DEBUG
            if ProcessInfo.processInfo.arguments.contains("-ui-testing-widget-preview") {
                WatchWidgetPreviewGallery()
            } else {
                watchHome
            }
#else
            watchHome
#endif
        }
    }

    private var watchHome: some View {
        LunaWatchHomeView(
            snapshot: receiver.snapshot,
            isPhoneReachable: receiver.isPhoneReachable
        )
    }
}

#if DEBUG
private struct WatchWidgetPreviewGallery: View {
    @Environment(\.colorScheme) private var colorScheme

    private var background: Color {
        colorScheme == .dark ? LunaColors.Mocha.background : LunaColors.Latte.background
    }

    private var surface: Color {
        colorScheme == .dark ? LunaColors.Mocha.surface : LunaColors.Latte.surface
    }

    private var foreground: Color {
        colorScheme == .dark ? LunaColors.Mocha.foreground : LunaColors.Latte.foreground
    }

    var body: some View {
        VStack(spacing: 10) {
            Text("C3 · Work pulse")
                .font(.system(.caption, design: .serif, weight: .semibold))
                .foregroundStyle(foreground)
            LunaWatchStatusView(entry: .placeholder())
                .tint(LunaColors.Mocha.accent)
                .padding(.horizontal, 10)
                .padding(.vertical, 8)
                .frame(width: 184, height: 78)
                .background(surface)
                .clipShape(RoundedRectangle(cornerRadius: 20, style: .continuous))
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(background)
        .accessibilityIdentifier("watch-widget-preview")
    }
}
#endif
