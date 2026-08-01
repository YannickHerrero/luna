import SwiftUI
import WidgetKit

struct OpenAIUsageWidgetEntry: TimelineEntry {
    let date: Date
    let snapshot: OpenAIWeeklyUsageSnapshot?

    static func placeholder(date: Date = .now) -> OpenAIUsageWidgetEntry {
        OpenAIUsageWidgetEntry(
            date: date,
            snapshot: OpenAIWeeklyUsageSnapshot(
                availability: .available,
                usedPercent: 63,
                resetsAt: date.addingTimeInterval(3 * 24 * 60 * 60),
                collectedAt: date.addingTimeInterval(-12 * 60)
            )
        )
    }
}

struct OpenAIWeeklyUsageWidgetView: View {
    let entry: OpenAIUsageWidgetEntry
    private let familyOverride: WidgetFamily?

    @Environment(\.widgetFamily) private var environmentFamily
    @Environment(\.colorScheme) private var colorScheme

    init(entry: OpenAIUsageWidgetEntry, family: WidgetFamily? = nil) {
        self.entry = entry
        familyOverride = family
    }

    private var family: WidgetFamily { familyOverride ?? environmentFamily }
    private var palette: UsageWidgetPalette { UsageWidgetPalette(colorScheme: colorScheme) }

    var body: some View {
        Group {
            if family == .systemMedium {
                mediumContent
            } else {
                smallContent
            }
        }
        .foregroundStyle(palette.foreground)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(entry.accessibilityLabel)
    }

    private var smallContent: some View {
        VStack(alignment: .leading, spacing: 8) {
            header
            Spacer(minLength: 0)
            if let percent = entry.usedPercent {
                VStack(alignment: .leading, spacing: 1) {
                    Text("\(percent)%")
                        .font(.system(.largeTitle, design: .serif, weight: .semibold))
                        .contentTransition(.numericText())
                    Text("used")
                        .font(.system(.caption, design: .rounded, weight: .semibold))
                }
            } else {
                Text("Unavailable")
                    .font(.system(.title3, design: .serif, weight: .semibold))
                Text("Open Luna to refresh")
                    .font(.caption2)
                    .foregroundStyle(palette.muted)
            }
            Spacer(minLength: 0)
            VStack(spacing: 5) {
                capacityLine(height: 14)
                resetText
                    .font(.system(.caption2, design: .monospaced))
                    .foregroundStyle(palette.muted)
                    .lineLimit(1)
                    .minimumScaleFactor(0.75)
            }
        }
    }

    private var mediumContent: some View {
        VStack(alignment: .leading, spacing: 9) {
            header
            Spacer(minLength: 0)
            if let percent = entry.usedPercent {
                HStack(alignment: .firstTextBaseline) {
                    Text("\(percent)% used")
                        .font(.system(.title2, design: .serif, weight: .semibold))
                    Spacer(minLength: 8)
                    resetText
                        .font(.system(.caption, design: .monospaced))
                        .foregroundStyle(palette.muted)
                        .lineLimit(1)
                }
                capacityLine(height: 20)
                HStack {
                    Text("0%")
                    Spacer()
                    Text("100%")
                }
                .font(.system(.caption2, design: .monospaced))
                .foregroundStyle(palette.muted)
            } else {
                VStack(alignment: .leading, spacing: 5) {
                    Text("Weekly limit unavailable")
                        .font(.system(.title3, design: .serif, weight: .semibold))
                    Text("Open Luna while your server is available to refresh this widget.")
                        .font(.caption)
                        .foregroundStyle(palette.muted)
                        .lineLimit(2)
                }
                .frame(maxHeight: .infinity, alignment: .center)
            }
            Spacer(minLength: 0)
        }
    }

    private var header: some View {
        HStack(spacing: 6) {
            Text(family == .systemSmall ? "WEEKLY" : "WEEKLY USAGE")
                .font(.system(.caption2, design: .monospaced, weight: .semibold))
                .tracking(0.7)
            Spacer(minLength: 4)
            Text(family == .systemSmall ? entry.compactFreshnessLabel : entry.freshnessLabel)
                .font(.system(.caption2, design: .monospaced))
                .lineLimit(1)
        }
        .foregroundStyle(entry.isStale ? palette.peach : palette.muted)
    }

    private func capacityLine(height: CGFloat) -> some View {
        GeometryReader { geometry in
            ZStack(alignment: .leading) {
                Capsule().fill(palette.raised)
                Capsule()
                    .fill(entry.isStale ? palette.peach : palette.accent)
                    .frame(
                        width: geometry.size.width
                            * CGFloat(entry.usedPercent ?? 0) / 100
                    )
            }
        }
        .frame(height: height)
        .accessibilityHidden(true)
    }

    @ViewBuilder
    private var resetText: some View {
        if let resetsAt = entry.resetsAt {
            Text("Reset ")
                + Text(
                    resetsAt,
                    format: .dateTime
                        .weekday(.abbreviated)
                        .day()
                        .month(.abbreviated)
                )
                + Text(" · ")
                + Text(resetsAt, format: .dateTime.hour().minute())
        } else {
            Text("Reset unavailable")
        }
    }
}

private struct UsageWidgetPalette {
    let foreground: Color
    let muted: Color
    let accent: Color
    let raised: Color
    let peach: Color

    init(colorScheme: ColorScheme) {
        if colorScheme == .dark {
            foreground = LunaColors.Mocha.foreground
            muted = LunaColors.Mocha.muted
            accent = LunaColors.Mocha.accent
            raised = LunaColors.Mocha.raised
            peach = LunaColors.Mocha.peach
        } else {
            foreground = LunaColors.Latte.foreground
            muted = LunaColors.Latte.muted
            accent = LunaColors.Latte.accent
            raised = LunaColors.Latte.raised
            peach = LunaColors.Latte.peach
        }
    }
}

extension OpenAIUsageWidgetEntry {
    var freshness: LunaSnapshotFreshness {
        snapshot?.freshness(at: date) ?? .unavailable
    }

    var isStale: Bool { freshness == .stale }
    var usedPercent: Int? { freshness == .unavailable ? nil : snapshot?.usedPercent }
    var resetsAt: Date? { freshness == .unavailable ? nil : snapshot?.resetsAt }

    var freshnessLabel: String {
        guard freshness != .unavailable else { return "Unavailable" }
        let age = ageLabel(includeAgo: true)
        return isStale ? "Out of date · \(age)" : "Updated \(age)"
    }

    var compactFreshnessLabel: String {
        guard freshness != .unavailable else { return "Unavailable" }
        let age = ageLabel(includeAgo: false)
        return isStale ? "Stale · \(age)" : age
    }

    private func ageLabel(includeAgo: Bool) -> String {
        guard let collectedAt = snapshot?.collectedAt else { return "Unavailable" }
        let age = max(0, date.timeIntervalSince(collectedAt))
        if age < 60 { return "now" }
        let value = age < 60 * 60 ? "\(Int(age / 60))m" : "\(Int(age / 3_600))h"
        return includeAgo ? "\(value) ago" : value
    }

    var accessibilityLabel: String {
        guard let usedPercent, let resetsAt else {
            return "OpenAI weekly usage unavailable. Open Luna to refresh."
        }
        let reset = resetsAt.formatted(date: .abbreviated, time: .shortened)
        let stale = isStale ? " Data is out of date." : ""
        return "OpenAI weekly limit. \(usedPercent) percent used. Resets \(reset). \(freshnessLabel).\(stale)"
    }
}

struct OpenAIUsageWidgetBackground: View {
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
