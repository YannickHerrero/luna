import SwiftUI
import WidgetKit

struct LunaWatchWidgetEntry: TimelineEntry {
    let date: Date
    let snapshot: ActiveAgentsSnapshot?

    static func placeholder(date: Date = .now) -> LunaWatchWidgetEntry {
        LunaWatchWidgetEntry(
            date: date,
            snapshot: ActiveAgentsSnapshot(
                generatedAt: date.addingTimeInterval(-120),
                agents: [
                    ActiveAgentSnapshot(
                        id: UUID(uuidString: "00000000-0000-0000-0000-000000000001")!,
                        title: "Prepare release notes",
                        state: .working,
                        activity: "Reviewing files",
                        updatedAt: date.addingTimeInterval(-120)
                    ),
                    ActiveAgentSnapshot(
                        id: UUID(uuidString: "00000000-0000-0000-0000-000000000002")!,
                        title: "Improve pairing",
                        state: .compacting,
                        activity: "Preserving recent work",
                        updatedAt: date.addingTimeInterval(-180)
                    ),
                ]
            )
        )
    }
}

struct LunaWatchStatusView: View {
    let entry: LunaWatchWidgetEntry

    var body: some View {
        HStack(spacing: 7) {
            VStack(alignment: .leading, spacing: 3) {
                Text(entry.label)
                    .font(.system(.caption2, design: .monospaced, weight: .semibold))
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                    .minimumScaleFactor(0.7)
                Text(entry.title)
                    .font(.system(.headline, design: .rounded, weight: .semibold))
                    .lineLimit(1)
                HStack(spacing: 3) {
                    ForEach(0..<4, id: \.self) { index in
                        Capsule()
                            .fill(
                                index < entry.filledSegmentCount
                                    ? Color.accentColor : Color.secondary
                            )
                            .opacity(index < entry.filledSegmentCount ? 1 : 0.3)
                            .frame(height: 4)
                    }
                }
                .widgetAccentable()
                .accessibilityHidden(true)
            }
            .frame(maxWidth: .infinity, alignment: .leading)

            ZStack {
                Circle()
                    .stroke(.secondary.opacity(0.3), lineWidth: 5)
                if entry.filledSegmentCount > 0 {
                    Circle()
                        .trim(from: 0, to: 0.7)
                        .stroke(
                            entry.isStale ? Color.secondary : Color.accentColor,
                            style: StrokeStyle(lineWidth: 5, lineCap: .round)
                        )
                        .rotationEffect(.degrees(-90))
                        .widgetAccentable()
                }
                Text(entry.countLabel)
                    .font(.system(.caption, design: .monospaced, weight: .semibold))
            }
            .frame(width: 40, height: 40)
            .accessibilityHidden(true)
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(entry.accessibilityLabel)
    }
}

extension LunaWatchWidgetEntry {
    var freshness: ActiveAgentsSnapshotFreshness {
        snapshot?.freshness(at: date) ?? .unavailable
    }

    var isStale: Bool { freshness == .stale }
    var agents: [ActiveAgentSnapshot] {
        freshness == .unavailable ? [] : snapshot?.agents ?? []
    }
    var count: Int { agents.count }
    var filledSegmentCount: Int { min(count, 4) }
    var countLabel: String {
        if freshness == .unavailable { return "?" }
        return count > 4 ? "4+" : String(count)
    }

    var label: String {
        if freshness == .unavailable { return "LUNA" }
        if isStale { return "LAST KNOWN · \(count) · \(ageLabel)" }
        guard let first = agents.first else { return "LUNA" }
        if count == 1 { return first.title }
        return "\(first.state.rawValue.uppercased()) · \(count) ACTIVE"
    }

    var title: String {
        if freshness == .unavailable { return "iPhone unavailable" }
        if isStale { return "iPhone unreachable" }
        return agents.first?.activity ?? "No agents running"
    }

    var ageLabel: String {
        guard let snapshot else { return "" }
        let age = max(0, date.timeIntervalSince(snapshot.generatedAt))
        if age < 60 { return "NOW" }
        if age < 60 * 60 { return "\(Int(age / 60))M" }
        return "\(Int(age / 3_600))H"
    }

    var accessibilityLabel: String {
        if freshness == .unavailable {
            return "Luna. iPhone unavailable. Agent status unavailable."
        }
        if count == 0 { return "Luna. No active agents." }
        let state = isStale ? "last known" : "active"
        return "Luna. \(count) agents \(state). \(agents[0].activity ?? agents[0].title)."
    }
}
