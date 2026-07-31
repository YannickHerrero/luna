import Foundation
import Testing
@testable import Luna

private struct FixedEventSource: EventSource {
    let envelopes: [ServerEventEnvelope]

    func events(for request: URLRequest) -> AsyncThrowingStream<ServerEventEnvelope, Error> {
        AsyncThrowingStream { continuation in
            for envelope in envelopes {
                continuation.yield(envelope)
            }
            continuation.finish()
        }
    }
}

private actor StoreTransport: HTTPTransport {
    struct Stub: Sendable {
        let status: Int
        let data: Data
    }

    private var stubs: [Stub]
    private(set) var requests: [URLRequest] = []

    init(_ stubs: [Stub] = []) {
        self.stubs = stubs
    }

    func data(for request: URLRequest) throws -> HTTPResponse {
        requests.append(request)
        guard !stubs.isEmpty else { throw APIClientError.invalidResponse }
        let stub = stubs.removeFirst()
        let response = HTTPURLResponse(
            url: request.url!,
            statusCode: stub.status,
            httpVersion: "HTTP/1.1",
            headerFields: nil
        )!
        return HTTPResponse(data: stub.data, response: response)
    }

    func lastRequest() -> URLRequest? { requests.last }
}

@MainActor
struct ConversationStoreTests {
    private let server = URL(string: "https://mac.example.ts.net:8447")!

    @Test
    func consumesWelcomeUpsertDeltaAndCompletion() async throws {
        let credentials = MemoryCredentialStore()
        await credentials.setToken("token", for: server)
        let conversation = storeConversation(id: 1)
        let message = storeMessage(id: 2, conversationId: conversation.id, text: "")
        let source = FixedEventSource(envelopes: [
            storeEnvelope(id: nil, conversationId: nil, event: .serverWelcome(ServerWelcome(cursor: 4, resumed: true))),
            storeEnvelope(id: 5, conversationId: conversation.id, event: .messageUpserted(message)),
            storeEnvelope(
                id: 6,
                conversationId: conversation.id,
                event: .messageDelta(MessageDelta(messageId: message.id, chunkIndex: 0, delta: "Hello"))
            ),
            storeEnvelope(
                id: 7,
                conversationId: conversation.id,
                event: .messageCompleted(MessageCompleted(messageId: message.id))
            ),
        ])
        let store = ConversationStore(
            client: APIClient(
                baseURL: server,
                credentials: credentials,
                transport: StoreTransport()
            ),
            bootstrap: storeBootstrap([conversation], cursor: 4),
            eventSource: source
        )

        try await store.consumeEventsOnce()

        #expect(store.connectionStatus == .connected)
        #expect(store.state.cursor == 7)
        #expect(store.selectedMessages.first?.text == "Hello")
        #expect(store.selectedMessages.first?.status == .completed)
    }

    @Test
    func reloadsBootstrapWhenTheServerRequiresAReset() async throws {
        let credentials = MemoryCredentialStore()
        await credentials.setToken("token", for: server)
        let replacement = storeConversation(id: 2)
        let response = try JSONEncoder().encode(storeBootstrap([replacement], cursor: 40))
        let store = ConversationStore(
            client: APIClient(
                baseURL: server,
                credentials: credentials,
                transport: StoreTransport([.init(status: 200, data: response)])
            ),
            bootstrap: storeBootstrap([storeConversation(id: 1)], cursor: 3),
            eventSource: FixedEventSource(envelopes: [
                storeEnvelope(
                    id: nil,
                    conversationId: nil,
                    event: .syncResetRequired(SyncResetRequired(cursor: 40))
                ),
            ])
        )

        try await store.consumeEventsOnce()

        #expect(store.state.cursor == 40)
        #expect(store.conversations.map(\.id) == [replacement.id])
        #expect(store.selectedConversationId == replacement.id)
    }

    @Test
    func loadsAndPaginatesSelectedMessages() async throws {
        let credentials = MemoryCredentialStore()
        await credentials.setToken("token", for: server)
        let conversation = storeConversation(id: 1)
        let newest = storeMessage(id: 3, conversationId: conversation.id, ordinal: 3)
        let oldest = storeMessage(id: 1, conversationId: conversation.id, ordinal: 1)
        let middle = storeMessage(id: 2, conversationId: conversation.id, ordinal: 2)
        let transport = StoreTransport([
            .init(
                status: 200,
                data: try JSONEncoder().encode(
                    ConversationMessages(messages: [middle, newest], nextBeforeOrdinal: 2)
                )
            ),
            .init(
                status: 200,
                data: try JSONEncoder().encode(
                    ConversationMessages(messages: [oldest, middle], nextBeforeOrdinal: nil)
                )
            ),
        ])
        let store = ConversationStore(
            client: APIClient(baseURL: server, credentials: credentials, transport: transport),
            bootstrap: storeBootstrap([conversation]),
            eventSource: EmptyEventSource()
        )

        await store.loadSelectedMessages()
        await store.loadEarlierMessages()

        #expect(store.selectedMessages.map(\.ordinal) == [1, 2, 3])
        #expect(!store.canLoadEarlier)
        #expect((await transport.lastRequest())?.url?.query == "beforeOrdinal=2")
    }
}

private func storeBootstrap(_ conversations: [Conversation], cursor: Int64 = 0) -> Bootstrap {
    Bootstrap(
        protocolVersion: 1,
        cursor: cursor,
        device: Device(
            id: storeUUID(90),
            name: "iPhone",
            platform: .ios,
            notificationsEnabled: false,
            createdAt: "2026-03-20T10:00:00Z",
            lastSeenAt: "2026-03-20T10:00:00Z"
        ),
        conversations: conversations
    )
}

private func storeConversation(id: Int) -> Conversation {
    Conversation(
        id: storeUUID(id),
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
        archivedAt: nil,
        createdAt: "2026-03-20T10:00:00Z",
        updatedAt: "2026-03-20T10:00:00Z",
        version: 1
    )
}

private func storeMessage(
    id: Int,
    conversationId: UUID,
    ordinal: Int64 = 1,
    text: String = "Message"
) -> Message {
    Message(
        id: storeUUID(id),
        conversationId: conversationId,
        clientMessageId: nil,
        role: .assistant,
        status: .streaming,
        delivery: nil,
        text: text,
        attachments: [],
        sentByDeviceId: nil,
        ordinal: ordinal,
        createdAt: "2026-03-20T10:01:00Z",
        updatedAt: "2026-03-20T10:01:00Z"
    )
}

private func storeEnvelope(
    id: Int64?,
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

private func storeUUID(_ value: Int) -> UUID {
    UUID(uuidString: String(format: "00000000-0000-0000-0000-%012d", value))!
}
