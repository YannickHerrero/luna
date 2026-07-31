import SwiftUI
import WidgetKit

struct LunaWatchWidgetEntry: TimelineEntry {
    let date: Date
}

struct LunaWatchWidgetProvider: TimelineProvider {
    func placeholder(in context: Context) -> LunaWatchWidgetEntry {
        LunaWatchWidgetEntry(date: .now)
    }

    func getSnapshot(in context: Context, completion: @escaping (LunaWatchWidgetEntry) -> Void) {
        completion(LunaWatchWidgetEntry(date: .now))
    }

    func getTimeline(in context: Context, completion: @escaping (Timeline<LunaWatchWidgetEntry>) -> Void) {
        completion(Timeline(entries: [LunaWatchWidgetEntry(date: .now)], policy: .never))
    }
}

struct LunaWatchStatusWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "LunaWatchStatusWidget", provider: LunaWatchWidgetProvider()) { entry in
            LunaWatchStatusView(entry: entry)
                .containerBackground(for: .widget) { Color.clear }
        }
        .configurationDisplayName("Luna status")
        .description("Keep Luna close. Live conversation status is coming soon.")
        .supportedFamilies([.accessoryCircular, .accessoryRectangular, .accessoryInline])
    }
}

private struct LunaWatchStatusView: View {
    let entry: LunaWatchWidgetEntry
    @Environment(\.widgetFamily) private var family

    var body: some View {
        switch family {
        case .accessoryCircular:
            ZStack {
                AccessoryWidgetBackground()
                Image(systemName: "moon.stars.fill")
                    .font(.system(size: 22, weight: .semibold))
                    .widgetAccentable()
            }
            .accessibilityLabel("Luna companion")
        case .accessoryRectangular:
            VStack(alignment: .leading, spacing: 2) {
                Label("Luna", systemImage: "moon.stars.fill")
                    .font(.headline)
                    .widgetAccentable()
                Text("Companion ready")
                    .font(.caption)
                Text("Live status soon")
                    .font(.caption2)
                    .foregroundStyle(.secondary)
            }
            .accessibilityElement(children: .combine)
        case .accessoryInline:
            Label("Luna · Companion ready", systemImage: "moon.stars.fill")
        default:
            Image(systemName: "moon.stars.fill")
                .widgetAccentable()
        }
    }
}
