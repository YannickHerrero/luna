import SwiftUI

enum LunaWatchCopy {
    static let title = "Luna"
    static let status = "Companion ready"
    static let detail = "Conversation sync is coming soon. Pair Luna on your iPhone to get ready."
}

struct LunaWatchHomeView: View {
    @Environment(\.colorScheme) private var colorScheme

    private var foreground: Color {
        colorScheme == .dark ? LunaColors.Mocha.foreground : LunaColors.Latte.foreground
    }

    private var muted: Color {
        colorScheme == .dark ? LunaColors.Mocha.muted : LunaColors.Latte.muted
    }

    private var accent: Color {
        colorScheme == .dark ? LunaColors.Mocha.accent : LunaColors.Latte.accent
    }

    private var background: Color {
        colorScheme == .dark ? LunaColors.Mocha.background : LunaColors.Latte.background
    }

    var body: some View {
        ScrollView {
            VStack(spacing: 10) {
                Image(systemName: "moon.stars.fill")
                    .font(.system(size: 30, weight: .semibold))
                    .foregroundStyle(accent)
                    .accessibilityHidden(true)
                Text(LunaWatchCopy.title)
                    .font(.system(.title2, design: .rounded, weight: .bold))
                    .foregroundStyle(foreground)
                Text(LunaWatchCopy.status)
                    .font(.system(.caption, design: .rounded, weight: .semibold))
                    .foregroundStyle(accent)
                    .accessibilityIdentifier("watch-companion-status")
                Text(LunaWatchCopy.detail)
                    .font(.footnote)
                    .foregroundStyle(muted)
                    .multilineTextAlignment(.center)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 8)
        }
        .containerBackground(background.gradient, for: .navigation)
    }
}
