import Foundation

enum ErrorCode: String, Codable, Sendable {
    case authenticationRequired = "authentication_required"
    case forbidden
    case invalidRequest = "invalid_request"
    case notFound = "not_found"
    case conflict
    case agentUnavailable = "agent_unavailable"
    case agentRejected = "agent_rejected"
    case attachmentInvalid = "attachment_invalid"
    case transcriptionFailed = "transcription_failed"
    case rateLimited = "rate_limited"
    case internalError = "internal_error"
}

struct LunaAPIError: Codable, Error, Equatable, Sendable {
    let code: ErrorCode
    let message: String
    let retryable: Bool
    let requestId: String?
}

enum ThinkingLevel: String, Codable, CaseIterable, Sendable {
    case off
    case minimal
    case low
    case medium
    case high
    case xhigh
    case max

    var displayName: String {
        switch self {
        case .off: "Off"
        case .minimal: "Minimal"
        case .low: "Low"
        case .medium: "Medium"
        case .high: "High"
        case .xhigh: "Extra high"
        case .max: "Maximum"
        }
    }
}

struct AgentModel: Codable, Equatable, Identifiable, Sendable {
    let provider: String
    let id: String
    let name: String
    let reasoning: Bool
    let contextWindow: UInt64
    let supportedThinkingLevels: [ThinkingLevel]
}

struct ContextUsage: Codable, Equatable, Sendable {
    let tokens: UInt64?
    let contextWindow: UInt64
    let percent: Double?
}

struct ConversationAgentState: Codable, Equatable, Sendable {
    let model: AgentModel?
    let thinkingLevel: ThinkingLevel
    let availableModels: [AgentModel]
    let contextUsage: ContextUsage?
    let autoCompactionEnabled: Bool
}

struct AgentModelSelection: Codable, Equatable, Sendable {
    let provider: String
    let modelId: String
}

struct UpdateConversationAgentRequest: Codable, Equatable, Sendable {
    let model: AgentModelSelection?
    let thinkingLevel: ThinkingLevel?
}

struct CompactConversationResponse: Codable, Equatable, Sendable {
    let tokensBefore: UInt64
    let estimatedTokensAfter: UInt64
}

enum ApnsEnvironment: String, Codable, Sendable {
    case sandbox
    case production
}

struct UpsertApnsRegistrationRequest: Codable, Equatable, Sendable {
    let token: String
    let environment: ApnsEnvironment
    let topic: String
    let appVersion: String?
}

struct PairingExchangeRequest: Codable, Equatable, Sendable {
    let code: String
    let deviceName: String
    let platform: DevicePlatform
}

struct PairingExchangeResponse: Codable, Equatable, Sendable {
    let deviceId: UUID
    let token: String
    let bootstrap: Bootstrap
}

struct PairingCodeRequestResponse: Codable, Equatable, Sendable {
    let expiresAt: LunaTimestamp
}

struct CreateConversationRequest: Codable, Equatable, Sendable {}

struct UpdateConversationRequest: Codable, Equatable, Sendable {
    let title: String?
    let avatarAttachmentId: UUID?
}

struct SendMessageRequest: Codable, Equatable, Sendable {
    let clientMessageId: UUID
    let text: String
    let attachmentIds: [UUID]
}

struct SendMessageResponse: Codable, Equatable, Sendable {
    let accepted: Bool
    let message: Message
}

struct ConversationMessages: Codable, Equatable, Sendable {
    let messages: [Message]
    let nextBeforeOrdinal: Int64?
}

enum OpenAiUsageAvailability: String, Codable, Sendable {
    case available
    case stale
    case unavailable
}

struct OpenAiWeeklyUsage: Codable, Equatable, Sendable {
    let availability: OpenAiUsageAvailability
    let usedPercent: Int?
    let resetsAt: LunaTimestamp?
    let collectedAt: LunaTimestamp?
}

struct TranscriptionResponse: Codable, Equatable, Sendable {
    let text: String
}

struct SyncResponse: Decodable, Equatable, Sendable {
    let cursor: Int64
    let events: [ServerEventEnvelope]
    let resetRequired: Bool
}

struct ConversationList: Codable, Equatable, Sendable {
    let conversations: [Conversation]
}

struct AttachmentResponse: Codable, Equatable, Sendable {
    let attachment: Attachment
}
