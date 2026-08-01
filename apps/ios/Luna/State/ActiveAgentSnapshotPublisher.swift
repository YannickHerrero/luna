import Foundation
import WidgetKit

@MainActor
final class ActiveAgentSnapshotPublisher {
    private struct AgentContent: Equatable {
        let id: UUID
        let title: String
        let state: AgentSnapshotState
        let activity: String
    }

    private let store: LunaSnapshotStore
    private let snapshotDidChange: (ActiveAgentsSnapshot) -> Void
    private let reloadTimelines: () -> Void
    private var lastPublishedContent: [AgentContent]?

    init(
        store: LunaSnapshotStore = LunaSnapshotStore(),
        snapshotDidChange: @escaping (ActiveAgentsSnapshot) -> Void = { _ in },
        reloadTimelines: @escaping () -> Void = {
            WidgetCenter.shared.reloadTimelines(ofKind: LunaAppGroup.activeAgentsWidgetKind)
        }
    ) {
        self.store = store
        self.snapshotDidChange = snapshotDidChange
        self.reloadTimelines = reloadTimelines
    }

    func publish(conversations: [Conversation], at date: Date = .now) {
        let content = Self.activeContent(from: conversations)
        guard content != lastPublishedContent else { return }
        let agents = content.map {
            ActiveAgentSnapshot(
                id: $0.id,
                title: $0.title,
                state: $0.state,
                activity: $0.activity,
                updatedAt: date
            )
        }
        do {
            let snapshot = ActiveAgentsSnapshot(generatedAt: date, agents: agents)
            try store.writeActiveAgents(snapshot)
            lastPublishedContent = content
            snapshotDidChange(snapshot)
            reloadTimelines()
        } catch {
            // Snapshot publication must never interrupt the authenticated app experience.
        }
    }

    private static func activeContent(from conversations: [Conversation]) -> [AgentContent] {
        conversations.compactMap { conversation in
            guard isActive(conversation.state),
                  let state = AgentSnapshotState(rawValue: conversation.state.rawValue)
            else {
                return nil
            }
            return AgentContent(
                id: conversation.id,
                title: conversation.title,
                state: state,
                activity: safeActivitySummary(for: conversation)
            )
        }
    }

    private static func isActive(_ state: SessionState) -> Bool {
        switch state {
        case .starting, .working, .compacting, .retrying, .restoring:
            true
        default:
            false
        }
    }

    private static func safeActivitySummary(for conversation: Conversation) -> String {
        let summary = conversation.activities.last?.summary.lowercased() ?? ""
        if summary.contains("test") || summary.contains("verify") {
            return "Running checks"
        }
        if summary.contains("build") || summary.contains("compile") {
            return "Building changes"
        }
        if ["read", "review", "inspect", "search", "explore", "list"].contains(
            where: { summary.contains($0) }
        ) {
            return "Reviewing files"
        }
        if ["edit", "write", "update", "patch", "implement"].contains(
            where: { summary.contains($0) }
        ) {
            return "Updating files"
        }
        if summary.contains("commit") || summary.contains("git") {
            return "Preparing changes"
        }
        if summary.contains("plan") || summary.contains("task") {
            return "Updating the plan"
        }
        if ["command", "shell", "terminal", "bash"].contains(
            where: { summary.contains($0) }
        ) {
            return "Running a command"
        }
        switch conversation.state {
        case .starting:
            return "Starting session"
        case .working:
            return "Working"
        case .compacting:
            return "Preserving recent work"
        case .retrying:
            return "Waiting to retry"
        case .restoring:
            return "Restoring session"
        default:
            return "Active"
        }
    }
}
