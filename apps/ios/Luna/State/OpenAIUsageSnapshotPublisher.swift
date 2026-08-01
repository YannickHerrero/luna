import Foundation
import WidgetKit

@MainActor
final class OpenAIUsageSnapshotPublisher {
    private let store: LunaSnapshotStore
    private let reloadTimelines: () -> Void
    private var lastSnapshot: OpenAIWeeklyUsageSnapshot?

    init(
        store: LunaSnapshotStore = LunaSnapshotStore(),
        reloadTimelines: @escaping () -> Void = {
            WidgetCenter.shared.reloadTimelines(
                ofKind: LunaAppGroup.openAIWeeklyUsageWidgetKind
            )
        }
    ) {
        self.store = store
        self.reloadTimelines = reloadTimelines
        lastSnapshot = try? store.readOpenAIWeeklyUsage()
    }

    func publish(_ usage: OpenAiWeeklyUsage) {
        guard let snapshot = Self.snapshot(from: usage) else { return }
        guard snapshot != lastSnapshot else { return }
        do {
            try store.writeOpenAIWeeklyUsage(snapshot)
            lastSnapshot = snapshot
            reloadTimelines()
        } catch {
            // Usage widgets are best-effort and must never interrupt the app.
        }
    }

    private static func snapshot(from usage: OpenAiWeeklyUsage) -> OpenAIWeeklyUsageSnapshot? {
        guard usage.availability != .unavailable else {
            return OpenAIWeeklyUsageSnapshot(
                availability: .unavailable,
                usedPercent: nil,
                resetsAt: nil,
                collectedAt: nil
            )
        }
        guard let usedPercent = usage.usedPercent,
              let resetsAt = parse(usage.resetsAt),
              let collectedAt = parse(usage.collectedAt)
        else {
            return nil
        }
        return OpenAIWeeklyUsageSnapshot(
            availability: usage.availability == .available ? .available : .stale,
            usedPercent: usedPercent,
            resetsAt: resetsAt,
            collectedAt: collectedAt
        )
    }

    private static func parse(_ value: String?) -> Date? {
        guard let value else { return nil }
        return (try? Date.ISO8601FormatStyle(includingFractionalSeconds: true).parse(value))
            ?? (try? Date.ISO8601FormatStyle().parse(value))
    }
}
