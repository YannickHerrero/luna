import SwiftUI

enum LunaWatchCopy {
    static let title = "Luna"
    static let unavailable = "Waiting for iPhone"
    static let empty = "No agents running"
}

struct LunaWatchPresentation: Equatable {
    let status: String
    let title: String?
    let activity: String?
    let detail: String

    static func make(
        snapshot: ActiveAgentsSnapshot?,
        date: Date,
        isPhoneReachable: Bool
    ) -> LunaWatchPresentation {
        guard let snapshot,
              snapshot.freshness(at: date) != .unavailable
        else {
            return LunaWatchPresentation(
                status: LunaWatchCopy.unavailable,
                title: nil,
                activity: nil,
                detail: "Open Luna on your iPhone to refresh."
            )
        }
        let age = max(0, date.timeIntervalSince(snapshot.generatedAt))
        let stale = snapshot.freshness(at: date) == .stale
        if snapshot.agents.isEmpty {
            return LunaWatchPresentation(
                status: stale ? "Last known · no agents" : LunaWatchCopy.empty,
                title: nil,
                activity: nil,
                detail: "Updated \(ageLabel(age))"
            )
        }
        let first = snapshot.agents[0]
        let status = stale
            ? "Last known · \(snapshot.agents.count) active"
            : "\(snapshot.agents.count) active \(snapshot.agents.count == 1 ? "agent" : "agents")"
        let detail = stale && !isPhoneReachable
            ? "iPhone unreachable · \(ageLabel(age))"
            : "Updated \(ageLabel(age))"
        return LunaWatchPresentation(
            status: status,
            title: first.title,
            activity: first.activity ?? first.state.displayName,
            detail: detail
        )
    }

    private static func ageLabel(_ age: TimeInterval) -> String {
        if age < 60 { return "now" }
        if age < 60 * 60 { return "\(Int(age / 60))m ago" }
        return "\(Int(age / 3_600))h ago"
    }
}

struct LunaWatchHomeView: View {
    let snapshot: ActiveAgentsSnapshot?
    let isPhoneReachable: Bool
    let dateOverride: Date?

    @Environment(\.colorScheme) private var colorScheme

    init(
        snapshot: ActiveAgentsSnapshot? = nil,
        isPhoneReachable: Bool = false,
        date: Date? = nil
    ) {
        self.snapshot = snapshot
        self.isPhoneReachable = isPhoneReachable
        dateOverride = date
    }

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
        TimelineView(.periodic(from: .now, by: 60)) { context in
            content(
                LunaWatchPresentation.make(
                    snapshot: snapshot,
                    date: dateOverride ?? context.date,
                    isPhoneReachable: isPhoneReachable
                )
            )
        }
    }

    private func content(_ presentation: LunaWatchPresentation) -> some View {
        ScrollView {
            VStack(spacing: 8) {
                Image(systemName: "moon.stars.fill")
                    .font(.system(.title2, weight: .semibold))
                    .foregroundStyle(accent)
                    .accessibilityHidden(true)
                Text(LunaWatchCopy.title)
                    .font(.system(.title3, design: .rounded, weight: .bold))
                    .foregroundStyle(foreground)
                Text(presentation.status)
                    .font(.system(.caption, design: .rounded, weight: .semibold))
                    .foregroundStyle(accent)
                    .multilineTextAlignment(.center)
                    .accessibilityIdentifier("watch-companion-status")
                if let title = presentation.title {
                    Text(title)
                        .font(.system(.headline, design: .rounded, weight: .semibold))
                        .foregroundStyle(foreground)
                        .multilineTextAlignment(.center)
                        .lineLimit(2)
                }
                if let activity = presentation.activity {
                    Text(activity)
                        .font(.footnote)
                        .foregroundStyle(muted)
                        .multilineTextAlignment(.center)
                        .lineLimit(2)
                }
                Text(presentation.detail)
                    .font(.caption2)
                    .foregroundStyle(muted)
                    .multilineTextAlignment(.center)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 7)
        }
        .containerBackground(background.gradient, for: .navigation)
    }
}

private extension AgentSnapshotState {
    var displayName: String {
        rawValue.capitalized
    }
}
