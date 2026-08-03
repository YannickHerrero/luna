import Foundation

typealias LunaTimestamp = String

enum DevicePlatform: String, Codable, Sendable {
    case ios
    case ipados
    case tui
    case web
}

struct Device: Codable, Equatable, Identifiable, Sendable {
    let id: UUID
    let name: String
    let platform: DevicePlatform
    let notificationsEnabled: Bool
    let createdAt: LunaTimestamp
    let lastSeenAt: LunaTimestamp
}

enum SessionState: String, Codable, CaseIterable, Sendable {
    case creating
    case starting
    case idle
    case working
    case compacting
    case retrying
    case crashed
    case restoring
    case interrupted
    case stopped
    case error

    var isBusy: Bool {
        switch self {
        case .starting, .working, .compacting, .retrying, .restoring:
            true
        default:
            false
        }
    }
}

enum MessageRole: String, Codable, Sendable {
    case user
    case assistant
}

enum MessageStatus: String, Codable, Sendable {
    case pending
    case accepted
    case queued
    case streaming
    case completed
    case interrupted
    case failed
}

enum MessageDelivery: String, Codable, Sendable {
    case initial
    case steer
    case bash
}

enum AttachmentStatus: String, Codable, Sendable {
    case uploading
    case ready
    case attached
    case failed
    case deleted
}

struct Attachment: Codable, Equatable, Identifiable, Sendable {
    let id: UUID
    let fileName: String
    let mimeType: String
    let byteSize: Int64
    let width: UInt32
    let height: UInt32
    let status: AttachmentStatus
    let contentUrl: String
    let thumbnailUrl: String
    let createdAt: LunaTimestamp
}

struct Message: Codable, Equatable, Identifiable, Sendable {
    let id: UUID
    let conversationId: UUID
    let clientMessageId: UUID?
    let role: MessageRole
    var status: MessageStatus
    let delivery: MessageDelivery?
    var text: String
    let attachments: [Attachment]
    let sentByDeviceId: UUID?
    let ordinal: Int64
    let createdAt: LunaTimestamp
    let updatedAt: LunaTimestamp
}

struct RepositoryIcon: Codable, Equatable, Sendable {
    let repositoryId: UUID
    let contentUrl: String?
    let fallbackText: String
    let fallbackColor: String
}

struct Repository: Codable, Equatable, Identifiable, Sendable {
    let id: UUID
    let displayName: String
    let rootPath: String
    let branch: String?
    let active: Bool
    let icon: RepositoryIcon
    let firstSeenAt: LunaTimestamp
    let lastSeenAt: LunaTimestamp
}

enum AgentTaskStatus: String, Codable, Sendable {
    case pending
    case inProgress = "in_progress"
    case completed
    case blocked
    case skipped
}

struct AgentTask: Codable, Equatable, Identifiable, Sendable {
    let id: UUID
    let sequence: Int64
    let text: String
    let status: AgentTaskStatus
    let note: String?
    let createdAt: LunaTimestamp
    let updatedAt: LunaTimestamp
}

struct AgentTaskList: Codable, Equatable, Identifiable, Sendable {
    let id: UUID
    let title: String?
    let revision: Int64
    let tasks: [AgentTask]
    let createdAt: LunaTimestamp
    let updatedAt: LunaTimestamp
}

struct AgentActivity: Codable, Equatable, Identifiable, Sendable {
    let id: UUID
    let sequence: Int64
    let summary: String
    let createdAt: LunaTimestamp
    let updatedAt: LunaTimestamp
}

enum TitleMode: String, Codable, Sendable {
    case automatic
    case manual
}

struct Conversation: Codable, Equatable, Identifiable, Sendable {
    let id: UUID
    var title: String
    let titleMode: TitleMode
    var state: SessionState
    let preview: String
    var activeWorkingDirectory: String
    var repositories: [Repository]
    var activities: [AgentActivity]
    var taskList: AgentTaskList?
    var lastMessageAt: LunaTimestamp?
    var notificationTargetDeviceId: UUID?
    let unreadCount: Int64
    var archivedAt: LunaTimestamp?
    let createdAt: LunaTimestamp
    let updatedAt: LunaTimestamp
    let version: Int64
}

struct Bootstrap: Codable, Equatable, Sendable {
    let protocolVersion: UInt8
    let cursor: Int64
    let device: Device
    let conversations: [Conversation]
}
