import SwiftUI
import WidgetKit

struct LunaWidgetProvider: TimelineProvider {
    private let store = LunaSnapshotStore()

    func placeholder(in context: Context) -> LunaWidgetEntry {
        .placeholder()
    }

    func getSnapshot(in context: Context, completion: @escaping (LunaWidgetEntry) -> Void) {
        if context.isPreview {
            completion(.placeholder())
        } else {
            completion(entry(at: .now))
        }
    }

    func getTimeline(in context: Context, completion: @escaping (Timeline<LunaWidgetEntry>) -> Void) {
        let date = Date.now
        completion(
            Timeline(
                entries: [entry(at: date)],
                policy: .after(date.addingTimeInterval(15 * 60))
            )
        )
    }

    private func entry(at date: Date) -> LunaWidgetEntry {
        LunaWidgetEntry(date: date, snapshot: try? store.readActiveAgents())
    }
}

struct LunaActiveAgentsWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(
            kind: LunaAppGroup.activeAgentsWidgetKind,
            provider: LunaWidgetProvider()
        ) { entry in
            LunaActiveAgentsWidgetView(entry: entry)
                .containerBackground(for: .widget) { LunaWidgetBackground() }
                .widgetURL(entry.featuredURL)
        }
        .configurationDisplayName("Active Agents")
        .description("See current Luna work and open its conversation.")
        .supportedFamilies([.systemSmall, .systemMedium])
    }
}

#Preview("A2 Small", as: .systemSmall) {
    LunaActiveAgentsWidget()
} timeline: {
    LunaWidgetEntry.placeholder()
}

#Preview("A2 Medium", as: .systemMedium) {
    LunaActiveAgentsWidget()
} timeline: {
    LunaWidgetEntry.placeholder()
}
