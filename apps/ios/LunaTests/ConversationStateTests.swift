import Foundation
import Testing
@testable import Luna

struct ConversationStateTests {
    @Test
    func bootstrapSortsByRecencyAndSelectsTheNewestConversation() {
        var state = LunaClientState()
        let older = conversation(id: 1, createdAt: "2026-03-20T10:00:00Z")
        let newer = conversation(id: 2, createdAt: "2026-03-20T11:00:00Z")

        state.install(bootstrap([older, newer], cursor: 4))

        #expect(state.conversations.map(\.id) == [newer.id, older.id])
        #expect(state.selectedConversationId == newer.id)
        #expect(state.cursor == 4)
    }

    @Test
    func appliesStreamingDeltasAndCompletionInCursorOrder() {
        var state = LunaClientState()
        let conversation = conversation(id: 1)
        let message = message(id: 2, conversationId: conversation.id, ordinal: 1, text: "")
        state.install(bootstrap([conversation], cursor: 2))
        state.messages[conversation.id] = [message]

        state.apply(
            envelope(
                id: 3,
                conversationId: conversation.id,
                event: .messageDelta(
                    MessageDelta(messageId: message.id, chunkIndex: 0, delta: "Hello")
                )
            )
        )
        state.apply(
            envelope(
                id: 4,
                conversationId: conversation.id,
                event: .messageCompleted(MessageCompleted(messageId: message.id))
            )
        )

        #expect(state.selectedMessages.first?.text == "Hello")
        #expect(state.selectedMessages.first?.status == .completed)
        #expect(state.cursor == 4)
    }

    @Test
    func movesAConversationToTheTopWhenANewerMessageArrives() {
        var state = LunaClientState()
        let first = conversation(id: 1, createdAt: "2026-03-20T10:00:00Z")
        let second = conversation(id: 2, createdAt: "2026-03-20T11:00:00Z")
        state.install(bootstrap([first, second]))
        let newest = message(
            id: 3,
            conversationId: first.id,
            ordinal: 1,
            text: "Update",
            createdAt: "2026-03-20T12:00:00Z"
        )

        state.apply(envelope(id: 5, conversationId: first.id, event: .messageUpserted(newest)))

        #expect(state.conversations.first?.id == first.id)
        #expect(state.conversations.first?.lastMessageAt == newest.createdAt)
        #expect(state.messages[first.id] == [newest])
    }

    @Test
    func removesAnArchivedSelectedConversation() {
        var state = LunaClientState()
        let active = conversation(id: 1)
        state.install(bootstrap([active]))
        state.messages[active.id] = [message(id: 2, conversationId: active.id, ordinal: 1)]
        let archived = conversation(id: 1, archivedAt: "2026-03-20T12:00:00Z")

        state.apply(envelope(id: 5, conversationId: active.id, event: .conversationUpserted(archived)))

        #expect(state.conversations.isEmpty)
        #expect(state.selectedConversationId == nil)
        #expect(state.messages[active.id] == nil)
    }

    @Test
    func updatesActivitiesTasksWorkspaceAndNotificationTarget() {
        var state = LunaClientState()
        let active = conversation(id: 1)
        state.install(bootstrap([active]))
        let activity = AgentActivity(
            id: uuid(10),
            sequence: 0,
            summary: "Planning native parity",
            createdAt: "2026-03-20T12:00:00Z",
            updatedAt: "2026-03-20T12:00:00Z"
        )
        let task = AgentTask(
            id: uuid(12),
            sequence: 1,
            text: "Build reducer",
            status: .inProgress,
            note: nil,
            createdAt: "2026-03-20T12:00:00Z",
            updatedAt: "2026-03-20T12:00:00Z"
        )
        let list = AgentTaskList(
            id: uuid(11),
            title: "Native parity",
            revision: 1,
            tasks: [task],
            createdAt: "2026-03-20T12:00:00Z",
            updatedAt: "2026-03-20T12:00:00Z"
        )

        state.apply(envelope(id: 6, conversationId: active.id, event: .agentActivityUpserted(activity)))
        state.apply(
            envelope(
                id: 7,
                conversationId: active.id,
                event: .agentTaskListChanged(AgentTaskListChanged(taskList: list))
            )
        )
        state.apply(
            envelope(
                id: 8,
                conversationId: active.id,
                event: .workspaceUpdated(WorkspaceUpdated(workingDirectory: "/tmp/luna"))
            )
        )
        state.apply(
            envelope(
                id: 9,
                conversationId: active.id,
                event: .notificationTargetChanged(NotificationTargetChanged(deviceId: uuid(99)))
            )
        )

        #expect(state.selectedConversation?.activities == [activity])
        #expect(state.selectedConversation?.taskList == list)
        #expect(state.selectedConversation?.activeWorkingDirectory == "/tmp/luna")
        #expect(state.selectedConversation?.notificationTargetDeviceId == uuid(99))

        state.apply(
            envelope(
                id: 10,
                conversationId: active.id,
                event: .notificationTargetChanged(NotificationTargetChanged(deviceId: nil))
            )
        )
        #expect(state.selectedConversation?.notificationTargetDeviceId == nil)
    }

    @Test
    func mergesPaginatedMessagesWithoutDuplicates() {
        var state = LunaClientState()
        let active = conversation(id: 1)
        state.install(bootstrap([active]))
        let second = message(id: 2, conversationId: active.id, ordinal: 2)
        let first = message(id: 1, conversationId: active.id, ordinal: 1)
        state.messages[active.id] = [second]

        state.setMessages(
            ConversationMessages(messages: [first, second], nextBeforeOrdinal: 1),
            for: active.id
        )

        #expect(state.messages[active.id]?.map(\.ordinal) == [1, 2])
        #expect(state.nextBeforeOrdinal[active.id] == 1)
    }
}

private func bootstrap(_ conversations: [Conversation], cursor: Int64 = 0) -> Bootstrap {
    Bootstrap(
        protocolVersion: 1,
        cursor: cursor,
        device: Device(
            id: uuid(90),
            name: "iPhone",
            platform: .ios,
            notificationsEnabled: false,
            createdAt: "2026-03-20T10:00:00Z",
            lastSeenAt: "2026-03-20T10:00:00Z"
        ),
        conversations: conversations
    )
}

private func conversation(
    id: Int,
    createdAt: String = "2026-03-20T10:00:00Z",
    archivedAt: String? = nil
) -> Conversation {
    Conversation(
        id: uuid(id),
        title: "Conversation \(id)",
        titleMode: .automatic,
        state: .idle,
        preview: "Preview",
        activeWorkingDirectory: "/Users/example",
        repositories: [],
        activities: [],
        taskList: nil,
        lastMessageAt: nil,
        notificationTargetDeviceId: nil,
        unreadCount: 0,
        archivedAt: archivedAt,
        createdAt: createdAt,
        updatedAt: createdAt,
        version: 1
    )
}

private func message(
    id: Int,
    conversationId: UUID,
    ordinal: Int64,
    text: String = "Message",
    createdAt: String = "2026-03-20T10:01:00Z"
) -> Message {
    Message(
        id: uuid(id),
        conversationId: conversationId,
        clientMessageId: nil,
        role: .assistant,
        status: .streaming,
        delivery: nil,
        text: text,
        attachments: [],
        sentByDeviceId: nil,
        ordinal: ordinal,
        createdAt: createdAt,
        updatedAt: createdAt
    )
}

private func envelope(
    id: Int64,
    conversationId: UUID?,
    event: ServerEvent
) -> ServerEventEnvelope {
    ServerEventEnvelope(
        eventId: id,
        conversationId: conversationId,
        emittedAt: "2026-03-20T12:00:00Z",
        event: event
    )
}

private func uuid(_ value: Int) -> UUID {
    UUID(uuidString: String(format: "00000000-0000-0000-0000-%012d", value))!
}
