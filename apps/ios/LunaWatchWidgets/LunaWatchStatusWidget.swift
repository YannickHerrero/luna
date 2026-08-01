import SwiftUI
import WidgetKit

struct LunaWatchWidgetProvider: TimelineProvider {
    private let store = LunaSnapshotStore()

    func placeholder(in context: Context) -> LunaWatchWidgetEntry {
        .placeholder()
    }

    func getSnapshot(
        in context: Context,
        completion: @escaping (LunaWatchWidgetEntry) -> Void
    ) {
        completion(context.isPreview ? .placeholder() : entry(at: .now))
    }

    func getTimeline(
        in context: Context,
        completion: @escaping (Timeline<LunaWatchWidgetEntry>) -> Void
    ) {
        let date = Date.now
        completion(
            Timeline(
                entries: [entry(at: date)],
                policy: .after(date.addingTimeInterval(15 * 60))
            )
        )
    }

    private func entry(at date: Date) -> LunaWatchWidgetEntry {
        LunaWatchWidgetEntry(date: date, snapshot: try? store.readActiveAgents())
    }
}

struct LunaWatchStatusWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(
            kind: LunaAppGroup.watchActiveAgentsWidgetKind,
            provider: LunaWatchWidgetProvider()
        ) { entry in
            LunaWatchStatusView(entry: entry)
                .containerBackground(for: .widget) { Color.clear }
                .widgetURL(URL(string: "luna-watch://status"))
        }
        .configurationDisplayName("Active Agents")
        .description("Pin Luna's current work in your Smart Stack.")
        .supportedFamilies([.accessoryRectangular])
    }
}


#Preview("C3 Work pulse", as: .accessoryRectangular) {
    LunaWatchStatusWidget()
} timeline: {
    LunaWatchWidgetEntry.placeholder()
}
