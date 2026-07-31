import SwiftUI
import WidgetKit

struct LunaWidgetEntry: TimelineEntry {
    let date: Date
}

struct LunaWidgetProvider: TimelineProvider {
    func placeholder(in context: Context) -> LunaWidgetEntry {
        LunaWidgetEntry(date: .now)
    }

    func getSnapshot(in context: Context, completion: @escaping (LunaWidgetEntry) -> Void) {
        completion(LunaWidgetEntry(date: .now))
    }

    func getTimeline(in context: Context, completion: @escaping (Timeline<LunaWidgetEntry>) -> Void) {
        completion(Timeline(entries: [LunaWidgetEntry(date: .now)], policy: .never))
    }
}

struct LunaContinueWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "LunaContinueWidget", provider: LunaWidgetProvider()) { entry in
            LunaWidgetView(entry: entry)
                .containerBackground(for: .widget) { LunaWidgetBackground() }
                .widgetURL(URL(string: "luna://home"))
        }
        .configurationDisplayName("Continue with Luna")
        .description("Open Luna from your Home Screen. Conversation status is coming soon.")
        .supportedFamilies([.systemSmall, .systemMedium])
    }
}

private struct LunaWidgetView: View {
    let entry: LunaWidgetEntry
    @Environment(\.widgetFamily) private var family
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

    var body: some View {
        VStack(alignment: .leading, spacing: family == .systemSmall ? 8 : 11) {
            HStack(spacing: 8) {
                Image(systemName: "moon.stars.fill")
                    .font(.system(size: 18, weight: .semibold))
                    .foregroundStyle(accent)
                Text("Luna")
                    .font(.system(.headline, design: .rounded, weight: .bold))
                    .foregroundStyle(foreground)
            }
            Spacer(minLength: 0)
            Text(family == .systemSmall ? "Continue your conversation" : "Your Pi conversations, one tap away.")
                .font(.system(family == .systemSmall ? .subheadline : .title3, design: .rounded, weight: .semibold))
                .foregroundStyle(foreground)
                .lineLimit(2)
            Text("Live status coming soon")
                .font(.caption)
                .foregroundStyle(muted)
                .lineLimit(1)
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel("Open Luna. Live conversation status coming soon.")
    }
}

private struct LunaWidgetBackground: View {
    @Environment(\.colorScheme) private var colorScheme

    var body: some View {
        LinearGradient(
            colors: colorScheme == .dark
                ? [LunaColors.Mocha.surface, LunaColors.Mocha.background]
                : [LunaColors.Latte.surface, LunaColors.Latte.background],
            startPoint: .topLeading,
            endPoint: .bottomTrailing
        )
    }
}
