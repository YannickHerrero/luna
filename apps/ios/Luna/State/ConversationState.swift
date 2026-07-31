import Foundation

struct LunaClientState: Equatable, Sendable {
    var conversations: [Conversation] = []
    var messages: [UUID: [Message]] = [:]
    var selectedConversationId: UUID?
    var nextBeforeOrdinal: [UUID: Int64] = [:]
    var cursor: Int64 = 0

    var selectedConversation: Conversation? {
        conversations.first { $0.id == selectedConversationId }
    }

    var selectedMessages: [Message] {
        guard let selectedConversationId else { return [] }
        return messages[selectedConversationId] ?? []
    }

    mutating func install(_ bootstrap: Bootstrap) {
        conversations = Self.sorted(bootstrap.conversations)
        messages = [:]
        selectedConversationId = conversations.first?.id
        nextBeforeOrdinal = [:]
        cursor = bootstrap.cursor
    }

    mutating func select(_ conversationId: UUID?) {
        selectedConversationId = conversationId
    }

    mutating func setMessages(_ response: ConversationMessages, for conversationId: UUID) {
        messages[conversationId] = Self.merge(
            response.messages,
            messages[conversationId] ?? []
        )
        if let next = response.nextBeforeOrdinal {
            nextBeforeOrdinal[conversationId] = next
        } else {
            nextBeforeOrdinal.removeValue(forKey: conversationId)
        }
    }

    mutating func apply(_ envelope: ServerEventEnvelope) {
        cursor = max(cursor, envelope.eventId ?? cursor)
        switch envelope.event {
        case let .conversationUpserted(conversation):
            if conversation.archivedAt != nil {
                removeConversation(conversation.id)
            } else {
                upsertConversation(conversation)
            }
        case let .messageUpserted(message):
            applyLatestMessage(message)
            messages[message.conversationId] = Self.upsert(
                message,
                into: messages[message.conversationId] ?? []
            )
        case let .messageDelta(delta):
            updateMessage(delta.messageId, conversationId: envelope.conversationId) { message in
                message.text += delta.delta
                message.status = .streaming
            }
        case let .messageCompleted(completed):
            updateMessage(completed.messageId, conversationId: envelope.conversationId) { message in
                message.status = .completed
            }
        case .agentActivitiesReset:
            updateConversation(envelope.conversationId) { $0.activities = [] }
        case let .agentActivityUpserted(activity):
            updateConversation(envelope.conversationId) { conversation in
                if let index = conversation.activities.firstIndex(where: { $0.id == activity.id }) {
                    conversation.activities[index] = activity
                } else {
                    conversation.activities.append(activity)
                }
                conversation.activities.sort { $0.sequence < $1.sequence }
            }
        case let .agentTaskListChanged(change):
            updateConversation(envelope.conversationId) { $0.taskList = change.taskList }
        case let .sessionStateChanged(change):
            updateConversation(envelope.conversationId) { $0.state = change.state }
        case let .workspaceUpdated(update):
            updateConversation(envelope.conversationId) {
                $0.activeWorkingDirectory = update.workingDirectory
            }
        case let .repositoriesUpdated(update):
            updateConversation(envelope.conversationId) { $0.repositories = update.repositories }
        case let .conversationTitleUpdated(update):
            updateConversation(envelope.conversationId) { $0.title = update.title }
        case let .notificationTargetChanged(update):
            updateConversation(envelope.conversationId) {
                $0.notificationTargetDeviceId = update.deviceId
            }
        case .serverWelcome,
             .commandAccepted,
             .commandRejected,
             .agentActivityChanged,
             .steeringQueueChanged,
             .attachmentUpdated,
             .syncResetRequired,
             .error,
             .serverPong,
             .unknown:
            break
        }
    }

    mutating func upsertConversation(_ conversation: Conversation) {
        if let index = conversations.firstIndex(where: { $0.id == conversation.id }) {
            conversations[index] = conversation
        } else {
            conversations.append(conversation)
        }
        conversations = Self.sorted(conversations)
    }

    mutating func removeConversation(_ conversationId: UUID) {
        conversations.removeAll { $0.id == conversationId }
        messages.removeValue(forKey: conversationId)
        nextBeforeOrdinal.removeValue(forKey: conversationId)
        if selectedConversationId == conversationId {
            selectedConversationId = nil
        }
    }

    mutating func upsertMessage(_ message: Message) {
        applyLatestMessage(message)
        messages[message.conversationId] = Self.upsert(
            message,
            into: messages[message.conversationId] ?? []
        )
    }

    private mutating func applyLatestMessage(_ message: Message) {
        guard let index = conversations.firstIndex(where: { $0.id == message.conversationId }) else {
            return
        }
        let current = conversations[index].lastMessageAt
        if current == nil || Self.timestamp(current!) < Self.timestamp(message.createdAt) {
            conversations[index].lastMessageAt = message.createdAt
            conversations = Self.sorted(conversations)
        }
    }

    private mutating func updateConversation(
        _ id: UUID?,
        update: (inout Conversation) -> Void
    ) {
        guard let id, let index = conversations.firstIndex(where: { $0.id == id }) else {
            return
        }
        update(&conversations[index])
    }

    private mutating func updateMessage(
        _ messageId: UUID,
        conversationId: UUID?,
        update: (inout Message) -> Void
    ) {
        if let conversationId,
           let index = messages[conversationId]?.firstIndex(where: { $0.id == messageId })
        {
            update(&messages[conversationId]![index])
            return
        }
        for id in messages.keys {
            if let index = messages[id]?.firstIndex(where: { $0.id == messageId }) {
                update(&messages[id]![index])
                return
            }
        }
    }

    private static func upsert(_ message: Message, into messages: [Message]) -> [Message] {
        var result = messages
        if let index = result.firstIndex(where: { $0.id == message.id }) {
            result[index] = message
        } else {
            result.append(message)
        }
        return result.sorted { $0.ordinal < $1.ordinal }
    }

    private static func merge(_ earlier: [Message], _ current: [Message]) -> [Message] {
        var indexed = Dictionary(uniqueKeysWithValues: current.map { ($0.id, $0) })
        for message in earlier {
            indexed[message.id] = message
        }
        return indexed.values.sorted { $0.ordinal < $1.ordinal }
    }

    static func sorted(_ conversations: [Conversation]) -> [Conversation] {
        conversations.sorted { left, right in
            let leftDate = timestamp(left.lastMessageAt ?? left.createdAt)
            let rightDate = timestamp(right.lastMessageAt ?? right.createdAt)
            if leftDate != rightDate {
                return leftDate > rightDate
            }
            return left.id.uuidString > right.id.uuidString
        }
    }

    private static func timestamp(_ value: String) -> Date {
        (try? Date.ISO8601FormatStyle(includingFractionalSeconds: true).parse(value))
            ?? (try? Date.ISO8601FormatStyle().parse(value))
            ?? .distantPast
    }
}
