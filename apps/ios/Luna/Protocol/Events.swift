import Foundation

struct CommandAccepted: Decodable, Equatable, Sendable {
    let requestId: UUID
    let message: Message?
}

struct CommandRejected: Decodable, Equatable, Sendable {
    let requestId: UUID
    let error: LunaAPIError
}

struct MessageDelta: Decodable, Equatable, Sendable {
    let messageId: UUID
    let chunkIndex: Int64
    let delta: String
}

struct MessageCompleted: Decodable, Equatable, Sendable {
    let messageId: UUID
}

enum ActivityPhase: String, Decodable, Sendable {
    case thinking
    case working
    case compacting
    case retrying
}

struct AgentActivityChanged: Decodable, Equatable, Sendable {
    let active: Bool
    let phase: ActivityPhase
}

struct AgentTaskListChanged: Decodable, Equatable, Sendable {
    let taskList: AgentTaskList?
}

struct SteeringQueueChanged: Decodable, Equatable, Sendable {
    let pending: Int64
    let delivery: MessageDelivery
}

struct WorkspaceUpdated: Decodable, Equatable, Sendable {
    let workingDirectory: String
}

struct RepositoriesUpdated: Decodable, Equatable, Sendable {
    let repositories: [Repository]
}

struct ConversationTitleUpdated: Decodable, Equatable, Sendable {
    let title: String
    let automatic: Bool
}

struct NotificationTargetChanged: Decodable, Equatable, Sendable {
    let deviceId: UUID?
}

struct ServerWelcome: Decodable, Equatable, Sendable {
    let cursor: Int64
    let resumed: Bool
}

struct SessionStateChanged: Decodable, Equatable, Sendable {
    let state: SessionState
}

struct SyncResetRequired: Decodable, Equatable, Sendable {
    let cursor: Int64
}

struct ServerPong: Decodable, Equatable, Sendable {
    let requestId: UUID
}

enum ServerEvent: Equatable, Sendable {
    case serverWelcome(ServerWelcome)
    case commandAccepted(CommandAccepted)
    case commandRejected(CommandRejected)
    case conversationUpserted(Conversation)
    case conversationTitleUpdated(ConversationTitleUpdated)
    case messageUpserted(Message)
    case messageDelta(MessageDelta)
    case messageCompleted(MessageCompleted)
    case sessionStateChanged(SessionStateChanged)
    case agentActivityChanged(AgentActivityChanged)
    case agentActivitiesReset
    case agentActivityUpserted(AgentActivity)
    case agentTaskListChanged(AgentTaskListChanged)
    case steeringQueueChanged(SteeringQueueChanged)
    case workspaceUpdated(WorkspaceUpdated)
    case repositoriesUpdated(RepositoriesUpdated)
    case attachmentUpdated(Attachment)
    case notificationTargetChanged(NotificationTargetChanged)
    case syncResetRequired(SyncResetRequired)
    case error(LunaAPIError)
    case serverPong(ServerPong)
    case unknown(type: String)
}

struct ServerEventEnvelope: Decodable, Equatable, Sendable {
    let version: UInt8
    let eventId: Int64?
    let conversationId: UUID?
    let emittedAt: LunaTimestamp
    let event: ServerEvent

    init(
        version: UInt8 = 1,
        eventId: Int64? = nil,
        conversationId: UUID? = nil,
        emittedAt: LunaTimestamp = "1970-01-01T00:00:00Z",
        event: ServerEvent
    ) {
        self.version = version
        self.eventId = eventId
        self.conversationId = conversationId
        self.emittedAt = emittedAt
        self.event = event
    }

    private enum CodingKeys: String, CodingKey {
        case version
        case eventId
        case conversationId
        case emittedAt
        case type
        case payload
    }

    private enum EventType: String, Decodable {
        case serverWelcome = "server.welcome"
        case commandAccepted = "command.accepted"
        case commandRejected = "command.rejected"
        case conversationUpserted = "conversation.upserted"
        case conversationTitleUpdated = "conversation.title_updated"
        case messageUpserted = "message.upserted"
        case messageDelta = "message.delta"
        case messageCompleted = "message.completed"
        case sessionStateChanged = "session.state_changed"
        case agentActivityChanged = "agent.activity_changed"
        case agentActivitiesReset = "agent.activities_reset"
        case agentActivityUpserted = "agent.activity_upserted"
        case agentTaskListChanged = "agent.task_list_changed"
        case steeringQueueChanged = "steering.queue_changed"
        case workspaceUpdated = "workspace.updated"
        case repositoriesUpdated = "repositories.updated"
        case attachmentUpdated = "attachment.updated"
        case notificationTargetChanged = "notification_target.changed"
        case syncResetRequired = "sync.reset_required"
        case error
        case serverPong = "server.pong"
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        version = try container.decode(UInt8.self, forKey: .version)
        eventId = try container.decodeIfPresent(Int64.self, forKey: .eventId)
        conversationId = try container.decodeIfPresent(UUID.self, forKey: .conversationId)
        emittedAt = try container.decode(String.self, forKey: .emittedAt)

        let rawType = try container.decode(String.self, forKey: .type)
        guard let type = EventType(rawValue: rawType) else {
            event = .unknown(type: rawType)
            return
        }
        switch type {
        case .serverWelcome:
            event = .serverWelcome(try container.decode(ServerWelcome.self, forKey: .payload))
        case .commandAccepted:
            event = .commandAccepted(try container.decode(CommandAccepted.self, forKey: .payload))
        case .commandRejected:
            event = .commandRejected(try container.decode(CommandRejected.self, forKey: .payload))
        case .conversationUpserted:
            event = .conversationUpserted(try container.decode(Conversation.self, forKey: .payload))
        case .conversationTitleUpdated:
            event = .conversationTitleUpdated(
                try container.decode(ConversationTitleUpdated.self, forKey: .payload)
            )
        case .messageUpserted:
            event = .messageUpserted(try container.decode(Message.self, forKey: .payload))
        case .messageDelta:
            event = .messageDelta(try container.decode(MessageDelta.self, forKey: .payload))
        case .messageCompleted:
            event = .messageCompleted(try container.decode(MessageCompleted.self, forKey: .payload))
        case .sessionStateChanged:
            event = .sessionStateChanged(
                try container.decode(SessionStateChanged.self, forKey: .payload)
            )
        case .agentActivityChanged:
            event = .agentActivityChanged(
                try container.decode(AgentActivityChanged.self, forKey: .payload)
            )
        case .agentActivitiesReset:
            event = .agentActivitiesReset
        case .agentActivityUpserted:
            event = .agentActivityUpserted(
                try container.decode(AgentActivity.self, forKey: .payload)
            )
        case .agentTaskListChanged:
            event = .agentTaskListChanged(
                try container.decode(AgentTaskListChanged.self, forKey: .payload)
            )
        case .steeringQueueChanged:
            event = .steeringQueueChanged(
                try container.decode(SteeringQueueChanged.self, forKey: .payload)
            )
        case .workspaceUpdated:
            event = .workspaceUpdated(try container.decode(WorkspaceUpdated.self, forKey: .payload))
        case .repositoriesUpdated:
            event = .repositoriesUpdated(
                try container.decode(RepositoriesUpdated.self, forKey: .payload)
            )
        case .attachmentUpdated:
            event = .attachmentUpdated(try container.decode(Attachment.self, forKey: .payload))
        case .notificationTargetChanged:
            event = .notificationTargetChanged(
                try container.decode(NotificationTargetChanged.self, forKey: .payload)
            )
        case .syncResetRequired:
            event = .syncResetRequired(
                try container.decode(SyncResetRequired.self, forKey: .payload)
            )
        case .error:
            event = .error(try container.decode(LunaAPIError.self, forKey: .payload))
        case .serverPong:
            event = .serverPong(try container.decode(ServerPong.self, forKey: .payload))
        }
    }
}
