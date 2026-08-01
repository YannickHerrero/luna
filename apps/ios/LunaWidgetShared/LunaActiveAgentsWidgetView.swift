import SwiftUI
import WidgetKit

struct LunaWidgetEntry: TimelineEntry {
    let date: Date
    let snapshot: ActiveAgentsSnapshot?

    static func placeholder(date: Date = .now) -> LunaWidgetEntry {
        LunaWidgetEntry(
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
                        title: "Improve iOS pairing flow",
                        state: .compacting,
                        activity: "Preserving recent work",
                        updatedAt: date.addingTimeInterval(-180)
                    ),
                    ActiveAgentSnapshot(
                        id: UUID(uuidString: "00000000-0000-0000-0000-000000000003")!,
                        title: "Audit offline sync behavior",
                        state: .retrying,
                        activity: "Waiting to retry",
                        updatedAt: date.addingTimeInterval(-240)
                    ),
                ]
            )
        )
    }
}

struct LunaActiveAgentsWidgetView: View {
    let entry: LunaWidgetEntry
    private let familyOverride: WidgetFamily?

    @Environment(\.widgetFamily) private var environmentFamily
    @Environment(\.colorScheme) private var colorScheme

    init(entry: LunaWidgetEntry, family: WidgetFamily? = nil) {
        self.entry = entry
        familyOverride = family
    }

    private var family: WidgetFamily { familyOverride ?? environmentFamily }
    private var palette: LunaWidgetPalette { LunaWidgetPalette(colorScheme: colorScheme) }

    var body: some View {
        Group {
            if family == .systemMedium {
                mediumContent
            } else {
                smallContent
            }
        }
        .foregroundStyle(palette.foreground)
    }

    private var smallContent: some View {
        VStack(alignment: .leading, spacing: 5) {
            HStack(spacing: 6) {
                Text("LUNA")
                    .font(.system(.caption2, design: .monospaced, weight: .semibold))
                    .tracking(0.8)
                Spacer(minLength: 4)
                Text(entry.freshnessLabel)
                    .font(.system(.caption2, design: .monospaced))
            }
            .foregroundStyle(palette.muted)

            Spacer(minLength: 0)
            activityOrbit(size: 72)
                .frame(maxWidth: .infinity)
            Spacer(minLength: 0)

            if entry.isUnavailable {
                statusCopy(title: "Status unavailable", detail: "Open Luna to refresh")
            } else if let agent = entry.agents.first {
                statusCopy(
                    title: agent.title,
                    detail: entry.isStale
                        ? "Last known · \(entry.freshnessLabel)"
                        : "\(agent.state.displayName) · \(agent.activity ?? "Active") ›"
                )
            } else {
                statusCopy(title: "All quiet", detail: "No active agents")
            }
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel(entry.accessibilitySummary)
    }

    private var mediumContent: some View {
        HStack(spacing: 12) {
            VStack(spacing: 5) {
                activityOrbit(size: 102)
                Text(entry.orbitCaption)
                    .font(.system(.caption2, design: .monospaced))
                    .foregroundStyle(palette.muted)
                    .lineLimit(1)
            }
            .frame(width: 112)
            .accessibilityElement(children: .ignore)
            .accessibilityLabel(entry.orbitAccessibilityLabel)

            VStack(alignment: .leading, spacing: 5) {
                HStack {
                    Text(entry.isStale ? "LAST KNOWN WORK" : "AGENTS IN MOTION")
                        .font(.system(.caption2, design: .monospaced, weight: .semibold))
                        .tracking(0.6)
                    Spacer(minLength: 4)
                    Text(entry.freshnessLabel)
                        .font(.system(.caption2, design: .monospaced))
                }
                .foregroundStyle(palette.muted)

                if entry.isUnavailable {
                    mediumStatus(title: "Status unavailable", detail: "Open Luna to refresh")
                } else if entry.agents.isEmpty {
                    mediumStatus(title: "All quiet", detail: "No active agents")
                } else {
                    ForEach(entry.agents.prefix(3)) { agent in
                        Link(destination: agent.conversationURL) {
                            activityCard(agent)
                        }
                    }
                }
            }
        }
    }

    private func activityOrbit(size: CGFloat) -> some View {
        ZStack {
            Circle()
                .stroke(palette.raised, lineWidth: size < 90 ? 8 : 10)
            if !entry.isUnavailable, !entry.agents.isEmpty {
                Circle()
                    .trim(from: 0.02, to: 0.74)
                    .stroke(
                        entry.isStale ? palette.overlay : palette.accent,
                        style: StrokeStyle(
                            lineWidth: size < 90 ? 8 : 10,
                            lineCap: .round
                        )
                    )
                    .rotationEffect(.degrees(-90))
            }
            Text(entry.orbitValue)
                .font(.system(.largeTitle, design: .serif, weight: .semibold))
                .contentTransition(.numericText())
        }
        .frame(width: size, height: size)
        .accessibilityHidden(true)
    }

    private func statusCopy(title: String, detail: String) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(title)
                .font(.system(.caption, design: .rounded, weight: .semibold))
                .lineLimit(1)
            Text(detail)
                .font(.system(.caption2, design: .rounded))
                .foregroundStyle(palette.muted)
                .lineLimit(1)
        }
    }

    private func activityCard(_ agent: ActiveAgentSnapshot) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(entry.isStale ? agent.title : agent.activity ?? "Active")
                .font(.system(.caption, design: .rounded, weight: .semibold))
                .foregroundStyle(palette.foreground)
                .lineLimit(1)
            HStack(spacing: 4) {
                Circle()
                    .fill(entry.isStale ? palette.overlay : palette.green)
                    .frame(width: 5, height: 5)
                    .accessibilityHidden(true)
                Text(
                    entry.isStale
                        ? "Last known · \(agent.state.displayName)"
                        : "\(agent.state.displayName) · \(agent.title)"
                )
                .lineLimit(1)
                Spacer(minLength: 2)
                Image(systemName: "chevron.right")
                    .font(.system(.caption2, weight: .bold))
                    .accessibilityHidden(true)
            }
            .font(.system(.caption2, design: .rounded))
            .foregroundStyle(palette.muted)
        }
        .padding(.horizontal, 9)
        .padding(.vertical, 6)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(palette.raised)
        .clipShape(RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay {
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .stroke(palette.border, lineWidth: 0.7)
        }
        .accessibilityElement(children: .combine)
        .accessibilityLabel(
            "\(agent.activity ?? "Active"), \(agent.title), \(agent.state.displayName)"
        )
    }

    private func mediumStatus(title: String, detail: String) -> some View {
        VStack(alignment: .leading, spacing: 5) {
            Text(title)
                .font(.system(.title3, design: .serif, weight: .semibold))
            Text(detail)
                .font(.system(.caption, design: .rounded))
                .foregroundStyle(palette.muted)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
        .accessibilityElement(children: .combine)
    }
}

private struct LunaWidgetPalette {
    let foreground: Color
    let muted: Color
    let border: Color
    let accent: Color
    let raised: Color
    let overlay: Color
    let green: Color

    init(colorScheme: ColorScheme) {
        if colorScheme == .dark {
            foreground = LunaColors.Mocha.foreground
            muted = LunaColors.Mocha.muted
            border = LunaColors.Mocha.border
            accent = LunaColors.Mocha.accent
            raised = LunaColors.Mocha.raised
            overlay = LunaColors.Mocha.overlay
            green = LunaColors.Mocha.green
        } else {
            foreground = LunaColors.Latte.foreground
            muted = LunaColors.Latte.muted
            border = LunaColors.Latte.border
            accent = LunaColors.Latte.accent
            raised = LunaColors.Latte.raised
            overlay = LunaColors.Latte.overlay
            green = LunaColors.Latte.green
        }
    }
}

struct LunaWidgetBackground: View {
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

extension LunaWidgetEntry {
    static let staleAfter: TimeInterval = 15 * 60
    static let unavailableAfter: TimeInterval = 24 * 60 * 60

    var age: TimeInterval {
        guard let snapshot else { return .infinity }
        return max(0, date.timeIntervalSince(snapshot.generatedAt))
    }

    var isStale: Bool { age > Self.staleAfter && age <= Self.unavailableAfter }
    var isUnavailable: Bool { snapshot == nil || age > Self.unavailableAfter }
    var agents: [ActiveAgentSnapshot] { isUnavailable ? [] : snapshot?.agents ?? [] }

    var orbitValue: String {
        isUnavailable ? "?" : String(agents.count)
    }

    var orbitCaption: String {
        if isUnavailable { return "unavailable" }
        if isStale { return "last known" }
        return "active now"
    }

    var freshnessLabel: String {
        guard !isUnavailable else { return "Unavailable" }
        if age < 60 { return "Now" }
        if age < 60 * 60 { return "\(Int(age / 60))m" }
        return "\(Int(age / 3_600))h"
    }

    var featuredURL: URL {
        agents.first?.conversationURL ?? URL(string: "luna://home")!
    }

    var orbitAccessibilityLabel: String {
        if isUnavailable { return "Agent status unavailable" }
        let state = isStale ? "last known" : "active"
        return "\(agents.count) agents \(state)"
    }

    var accessibilitySummary: String {
        if isUnavailable { return "Luna agent status unavailable. Open Luna to refresh." }
        if agents.isEmpty { return "Luna. No active agents." }
        let state = isStale ? "last known" : "active"
        return "Luna. \(agents.count) agents \(state). \(agents[0].title)."
    }
}

private extension ActiveAgentSnapshot {
    var conversationURL: URL {
        URL(string: "luna://conversation/\(id.uuidString)")!
    }
}

private extension AgentSnapshotState {
    var displayName: String {
        switch self {
        case .creating:
            "Creating"
        case .starting:
            "Starting"
        case .idle:
            "Idle"
        case .working:
            "Working"
        case .compacting:
            "Compacting"
        case .retrying:
            "Retrying"
        case .crashed:
            "Crashed"
        case .restoring:
            "Restoring"
        case .interrupted:
            "Interrupted"
        case .stopped:
            "Stopped"
        case .error:
            "Error"
        }
    }
}
