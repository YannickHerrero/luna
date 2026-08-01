import SwiftUI
import WidgetKit

struct OpenAIUsageWidgetProvider: TimelineProvider {
    private let store = LunaSnapshotStore()

    func placeholder(in context: Context) -> OpenAIUsageWidgetEntry {
        .placeholder()
    }

    func getSnapshot(
        in context: Context,
        completion: @escaping (OpenAIUsageWidgetEntry) -> Void
    ) {
        completion(context.isPreview ? .placeholder() : entry(at: .now))
    }

    func getTimeline(
        in context: Context,
        completion: @escaping (Timeline<OpenAIUsageWidgetEntry>) -> Void
    ) {
        let date = Date.now
        completion(
            Timeline(
                entries: [entry(at: date)],
                policy: .after(date.addingTimeInterval(15 * 60))
            )
        )
    }

    private func entry(at date: Date) -> OpenAIUsageWidgetEntry {
        OpenAIUsageWidgetEntry(
            date: date,
            snapshot: try? store.readOpenAIWeeklyUsage()
        )
    }
}

struct LunaOpenAIWeeklyUsageWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(
            kind: LunaAppGroup.openAIWeeklyUsageWidgetKind,
            provider: OpenAIUsageWidgetProvider()
        ) { entry in
            OpenAIWeeklyUsageWidgetView(entry: entry)
                .containerBackground(for: .widget) { OpenAIUsageWidgetBackground() }
                .widgetURL(URL(string: "luna://home"))
        }
        .configurationDisplayName("OpenAI Weekly Limit")
        .description("See the account-level weekly limit used by Pi and Codex.")
        .supportedFamilies([.systemSmall, .systemMedium])
    }
}

#Preview("B2 Small", as: .systemSmall) {
    LunaOpenAIWeeklyUsageWidget()
} timeline: {
    OpenAIUsageWidgetEntry.placeholder()
}

#Preview("B2 Medium", as: .systemMedium) {
    LunaOpenAIWeeklyUsageWidget()
} timeline: {
    OpenAIUsageWidgetEntry.placeholder()
}
