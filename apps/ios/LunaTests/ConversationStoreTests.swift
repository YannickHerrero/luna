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
    func allRequests() -> [URLRequest] { requests }
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
    func renamesConversationAndUpdatesLocalState() async throws {
        let credentials = MemoryCredentialStore()
        await credentials.setToken("token", for: server)
        let conversation = storeConversation(id: 1)
        var renamed = conversation
        renamed.title = "Native controls"
        let transport = StoreTransport([
            .init(status: 200, data: try JSONEncoder().encode(renamed)),
        ])
        let store = ConversationStore(
            client: APIClient(baseURL: server, credentials: credentials, transport: transport),
            bootstrap: storeBootstrap([conversation]),
            eventSource: EmptyEventSource()
        )

        try await store.renameConversation(conversation.id, title: "  Native controls  ")

        #expect(store.selectedConversation?.title == "Native controls")
        let request = try #require(await transport.lastRequest())
        #expect(request.httpMethod == "PATCH")
        #expect(request.url?.path == "/v1/conversations/\(conversation.id.uuidString)")
        let body = try JSONDecoder().decode(
            UpdateConversationRequest.self,
            from: request.httpBody ?? Data()
        )
        #expect(body.title == "Native controls")
    }

    @Test
    func archivesConversationAndDiscardsItsDraft() async throws {
        let credentials = MemoryCredentialStore()
        await credentials.setToken("token", for: server)
        let conversation = storeConversation(id: 1)
        let defaults = try #require(UserDefaults(suiteName: "ArchiveDraftTests.\(UUID())"))
        let persistence = ComposerDraftPersistence(defaults: defaults, prefix: "draft:")
        let transport = StoreTransport([.init(status: 204, data: Data())])
        let store = ConversationStore(
            client: APIClient(baseURL: server, credentials: credentials, transport: transport),
            bootstrap: storeBootstrap([conversation]),
            eventSource: EmptyEventSource(),
            draftPersistence: persistence
        )
        store.setDraftText("Unsaved note", for: conversation.id)

        try await store.archiveConversation(conversation.id)

        #expect(store.conversations.isEmpty)
        #expect(store.selectedConversationId == nil)
        #expect(persistence.text(for: conversation.id).isEmpty)
        let request = try #require(await transport.lastRequest())
        #expect(request.httpMethod == "POST")
        #expect(request.url?.path == "/v1/conversations/\(conversation.id.uuidString)/archive")
    }

    @Test
    func loadsUpdatesAndCompactsAgentState() async throws {
        let credentials = MemoryCredentialStore()
        await credentials.setToken("token", for: server)
        let conversation = storeConversation(id: 1)
        let initial = storeAgentState(thinking: .medium)
        let updated = storeAgentState(thinking: .high)
        let compacted = CompactConversationResponse(
            tokensBefore: 48_000,
            estimatedTokensAfter: 12_000
        )
        let transport = StoreTransport([
            .init(status: 200, data: try JSONEncoder().encode(initial)),
            .init(status: 200, data: try JSONEncoder().encode(updated)),
            .init(status: 200, data: try JSONEncoder().encode(compacted)),
        ])
        let store = ConversationStore(
            client: APIClient(baseURL: server, credentials: credentials, transport: transport),
            bootstrap: storeBootstrap([conversation]),
            eventSource: EmptyEventSource()
        )
        let request = UpdateConversationAgentRequest(
            model: AgentModelSelection(provider: "anthropic", modelId: "claude-sonnet-4"),
            thinkingLevel: .high
        )

        #expect(try await store.loadAgentState(for: conversation.id) == initial)
        #expect(try await store.updateAgentState(for: conversation.id, request: request) == updated)
        #expect(try await store.compactConversation(conversation.id) == compacted)

        let requests = await transport.allRequests()
        #expect(requests.map(\.httpMethod) == ["GET", "PATCH", "POST"])
        #expect(requests[0].url?.path.hasSuffix("/agent") == true)
        #expect(
            try JSONDecoder().decode(
                UpdateConversationAgentRequest.self,
                from: requests[1].httpBody ?? Data()
            ) == request
        )
        #expect(requests[2].url?.path.hasSuffix("/compact") == true)
    }

    @Test
    func restoresDraftTextWithoutPersistingAttachmentBytes() throws {
        let defaults = try #require(UserDefaults(suiteName: "ConversationDraftTests.\(UUID())"))
        let persistence = ComposerDraftPersistence(defaults: defaults, prefix: "draft:")
        let conversation = storeConversation(id: 1)
        let client = APIClient(
            baseURL: server,
            credentials: MemoryCredentialStore(),
            transport: StoreTransport([])
        )
        let firstStore = ConversationStore(
            client: client,
            bootstrap: storeBootstrap([conversation]),
            eventSource: EmptyEventSource(),
            draftPersistence: persistence
        )
        firstStore.setDraftText("Continue this conversation", for: conversation.id)
        firstStore.addDraftAttachments(
            [
                DraftAttachment(
                    data: Data("image".utf8),
                    fileName: "preview.png",
                    mimeType: "image/png"
                ),
            ],
            for: conversation.id
        )

        let restoredStore = ConversationStore(
            client: client,
            bootstrap: storeBootstrap([conversation]),
            eventSource: EmptyEventSource(),
            draftPersistence: persistence
        )

        #expect(restoredStore.composerDraft(for: conversation.id).text == "Continue this conversation")
        #expect(restoredStore.composerDraft(for: conversation.id).attachments.isEmpty)
    }

    @Test
    func uploadsAttachmentsAndAcceptsMessages() async throws {
        let credentials = MemoryCredentialStore()
        await credentials.setToken("token", for: server)
        let conversation = storeConversation(id: 1)
        let attachment = storeAttachment(id: 7)
        let accepted = storeMessage(
            id: 8,
            conversationId: conversation.id,
            text: "Review this"
        )
        let transport = StoreTransport([
            .init(
                status: 201,
                data: try JSONEncoder().encode(AttachmentResponse(attachment: attachment))
            ),
            .init(
                status: 200,
                data: try JSONEncoder().encode(
                    SendMessageResponse(accepted: true, message: accepted)
                )
            ),
        ])
        let store = ConversationStore(
            client: APIClient(baseURL: server, credentials: credentials, transport: transport),
            bootstrap: storeBootstrap([conversation]),
            eventSource: EmptyEventSource()
        )

        let submitted = try await store.submitMessage(
            in: conversation.id,
            text: " Review this ",
            attachments: [
                DraftAttachment(
                    data: Data("image".utf8),
                    fileName: "preview.png",
                    mimeType: "image/png"
                ),
            ]
        )

        let requests = await transport.allRequests()
        #expect(submitted)
        #expect(requests.count == 2)
        #expect(requests[0].url?.path == "/v1/attachments")
        #expect(requests[0].value(forHTTPHeaderField: "Content-Type")?.contains("multipart/form-data") == true)
        #expect(String(data: requests[0].httpBody ?? Data(), encoding: .utf8)?.contains(conversation.id.uuidString) == true)
        let body = try JSONDecoder().decode(SendMessageRequest.self, from: requests[1].httpBody ?? Data())
        #expect(body.text == "Review this")
        #expect(body.attachmentIds == [attachment.id])
        #expect(store.selectedMessages.contains { $0.id == accepted.id })
    }

    @Test
    func stopCommandAbortsBusyConversation() async throws {
        let credentials = MemoryCredentialStore()
        await credentials.setToken("token", for: server)
        let conversation = storeConversation(id: 1, state: .working)
        let transport = StoreTransport([.init(status: 204, data: Data())])
        let store = ConversationStore(
            client: APIClient(baseURL: server, credentials: credentials, transport: transport),
            bootstrap: storeBootstrap([conversation]),
            eventSource: EmptyEventSource()
        )

        let submitted = try await store.submitMessage(
            in: conversation.id,
            text: "/stop",
            attachments: []
        )

        #expect(submitted)
        #expect((await transport.lastRequest())?.url?.path.hasSuffix("/abort") == true)
    }

    @Test
    func uploadsAudioForTranscription() async throws {
        let credentials = MemoryCredentialStore()
        await credentials.setToken("token", for: server)
        let transport = StoreTransport([
            .init(
                status: 200,
                data: try JSONEncoder().encode(TranscriptionResponse(text: "spoken text"))
            ),
        ])
        let store = ConversationStore(
            client: APIClient(baseURL: server, credentials: credentials, transport: transport),
            bootstrap: storeBootstrap([storeConversation(id: 1)]),
            eventSource: EmptyEventSource()
        )

        let text = try await store.transcribe(
            Data("audio".utf8),
            fileName: "recording.m4a",
            mimeType: "audio/mp4"
        )

        #expect(text == "spoken text")
        let request = await transport.lastRequest()
        #expect(request?.url?.path == "/v1/transcriptions")
        #expect(String(data: request?.httpBody ?? Data(), encoding: .utf8)?.contains("recording.m4a") == true)
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

private func storeConversation(id: Int, state: SessionState = .idle) -> Conversation {
    Conversation(
        id: storeUUID(id),
        title: "Conversation \(id)",
        titleMode: .automatic,
        state: state,
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

private func storeAgentState(thinking: ThinkingLevel) -> ConversationAgentState {
    let model = AgentModel(
        provider: "anthropic",
        id: "claude-sonnet-4",
        name: "Claude Sonnet 4",
        reasoning: true,
        contextWindow: 200_000,
        supportedThinkingLevels: [.off, .low, .medium, .high]
    )
    return ConversationAgentState(
        model: model,
        thinkingLevel: thinking,
        availableModels: [model],
        contextUsage: ContextUsage(
            tokens: 48_000,
            contextWindow: 200_000,
            percent: 24
        ),
        autoCompactionEnabled: true
    )
}

private func storeAttachment(id: Int) -> Luna.Attachment {
    Luna.Attachment(
        id: storeUUID(id),
        fileName: "preview.png",
        mimeType: "image/png",
        byteSize: 5,
        width: 10,
        height: 10,
        status: .ready,
        contentUrl: "/v1/attachments/\(storeUUID(id))/content",
        thumbnailUrl: "/v1/attachments/\(storeUUID(id))/thumbnail",
        createdAt: "2026-03-20T10:00:00Z"
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
