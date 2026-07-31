import Foundation
import Observation

enum ConnectionStatus: Equatable, Sendable {
    case disconnected
    case connecting
    case connected
    case waitingToReconnect
}

enum ConversationListScope: String, Sendable {
    case active
    case archived
}

@MainActor
@Observable
final class ConversationStore {
    private(set) var state: LunaClientState
    private(set) var connectionStatus = ConnectionStatus.disconnected
    private(set) var isLoadingMessages = false
    private(set) var composerDrafts: [UUID: ComposerDraft] = [:]
    private(set) var archivedConversations: [Conversation] = []
    private(set) var listScope = ConversationListScope.active
    var errorMessage: String?

    @ObservationIgnored private let client: APIClient
    @ObservationIgnored private let eventSource: any EventSource
    @ObservationIgnored let imageLoader: AuthenticatedImageLoader
    @ObservationIgnored private let draftPersistence: ComposerDraftPersistence
    @ObservationIgnored private var connectionTask: Task<Void, Never>?

    init(
        client: APIClient,
        bootstrap: Bootstrap,
        eventSource: any EventSource,
        draftPersistence: ComposerDraftPersistence = ComposerDraftPersistence()
    ) {
        self.client = client
        self.eventSource = eventSource
        self.draftPersistence = draftPersistence
        imageLoader = AuthenticatedImageLoader(client: client)
        var state = LunaClientState()
        state.install(bootstrap)
        self.state = state
    }

    var conversations: [Conversation] { state.conversations }
    var visibleConversations: [Conversation] {
        listScope == .active ? conversations : archivedConversations
    }
    var selectedConversation: Conversation? {
        state.selectedConversation
            ?? archivedConversations.first { $0.id == state.selectedConversationId }
    }
    var selectedMessages: [Message] { state.selectedMessages }
    var selectedConversationId: UUID? { state.selectedConversationId }
    var canLoadEarlier: Bool {
        guard let id = state.selectedConversationId else { return false }
        return state.nextBeforeOrdinal[id] != nil
    }

    func startRealtime() {
        guard connectionTask == nil else { return }
        connectionTask = Task { [weak self] in
            await self?.reconnectLoop()
        }
    }

    func stopRealtime() {
        connectionTask?.cancel()
        connectionTask = nil
        connectionStatus = .disconnected
    }

    func resumeRealtime() {
        stopRealtime()
        startRealtime()
    }

    func showConversationList() {
        state.select(nil)
    }

    func showActiveConversationList() {
        listScope = .active
        state.select(nil)
    }

    func showArchivedConversationList() async {
        errorMessage = nil
        do {
            let response: ConversationList = try await client.get(
                "/v1/conversations?scope=archived"
            )
            archivedConversations = LunaClientState.sorted(response.conversations)
            listScope = .archived
            state.select(nil)
        } catch {
            errorMessage = message(from: error)
        }
    }

    func selectConversation(_ id: UUID?) async {
        if let id, conversations.contains(where: { $0.id == id }) {
            listScope = .active
        } else if let id, archivedConversations.contains(where: { $0.id == id }) {
            listScope = .archived
        }
        state.select(id)
        guard let id else { return }
        await loadMessages(for: id, before: nil, replacing: true)
    }

    func loadSelectedMessages() async {
        guard let id = state.selectedConversationId else { return }
        await loadMessages(for: id, before: nil, replacing: true)
    }

    func loadEarlierMessages() async {
        guard let id = state.selectedConversationId,
              let before = state.nextBeforeOrdinal[id]
        else {
            return
        }
        await loadMessages(for: id, before: before, replacing: false)
    }

    func createConversation() async {
        errorMessage = nil
        do {
            let conversation: Conversation = try await client.post(
                "/v1/conversations",
                body: CreateConversationRequest()
            )
            listScope = .active
            state.upsertConversation(conversation)
            state.select(conversation.id)
            state.messages[conversation.id] = []
        } catch {
            errorMessage = message(from: error)
        }
    }

    func renameConversation(_ conversationId: UUID, title: String) async throws {
        let conversation: Conversation = try await client.patch(
            "/v1/conversations/\(conversationId.uuidString)",
            body: UpdateConversationRequest(
                title: title.trimmingCharacters(in: .whitespacesAndNewlines),
                avatarAttachmentId: nil
            )
        )
        if let index = archivedConversations.firstIndex(where: { $0.id == conversationId }) {
            archivedConversations[index] = conversation
            archivedConversations = LunaClientState.sorted(archivedConversations)
        } else {
            state.upsertConversation(conversation)
        }
    }

    func archiveConversation(_ conversationId: UUID) async throws {
        try await client.postVoid("/v1/conversations/\(conversationId.uuidString)/archive")
        state.removeConversation(conversationId)
        composerDrafts.removeValue(forKey: conversationId)
        draftPersistence.setText("", for: conversationId)
    }

    @discardableResult
    func restoreConversation(_ conversationId: UUID) async throws -> Conversation {
        let conversation: Conversation = try await client.post(
            "/v1/conversations/\(conversationId.uuidString)/restore"
        )
        archivedConversations.removeAll { $0.id == conversationId }
        state.upsertConversation(conversation)
        listScope = .active
        state.select(conversation.id)
        return conversation
    }

    func loadAgentState(for conversationId: UUID) async throws -> ConversationAgentState {
        try await client.get("/v1/conversations/\(conversationId.uuidString)/agent")
    }

    func updateAgentState(
        for conversationId: UUID,
        request: UpdateConversationAgentRequest
    ) async throws -> ConversationAgentState {
        try await client.patch(
            "/v1/conversations/\(conversationId.uuidString)/agent",
            body: request
        )
    }

    func compactConversation(_ conversationId: UUID) async throws -> CompactConversationResponse {
        try await client.post("/v1/conversations/\(conversationId.uuidString)/compact")
    }

    func composerDraft(for conversationId: UUID) -> ComposerDraft {
        composerDrafts[conversationId]
            ?? ComposerDraft(
                text: draftPersistence.text(for: conversationId),
                attachments: []
            )
    }

    func setDraftText(_ text: String, for conversationId: UUID) {
        var draft = composerDraft(for: conversationId)
        draft.text = text
        composerDrafts[conversationId] = draft
        draftPersistence.setText(text, for: conversationId)
    }

    func addDraftAttachments(_ attachments: [DraftAttachment], for conversationId: UUID) {
        var draft = composerDraft(for: conversationId)
        draft.attachments = Array((draft.attachments + attachments).prefix(6))
        composerDrafts[conversationId] = draft
    }

    func removeDraftAttachment(_ id: UUID, for conversationId: UUID) {
        var draft = composerDraft(for: conversationId)
        draft.attachments.removeAll { $0.id == id }
        composerDrafts[conversationId] = draft
    }

    func clearDraft(for conversationId: UUID) {
        composerDrafts[conversationId] = .empty
        draftPersistence.setText("", for: conversationId)
    }

    @discardableResult
    func submitMessage(
        in conversationId: UUID,
        text: String,
        attachments: [DraftAttachment]
    ) async throws -> Bool {
        switch ComposerSubmission.parse(text: text, attachmentCount: attachments.count) {
        case .empty:
            return false
        case .invalidShellAttachments:
            throw ComposerError.shellAttachments
        case .stop:
            if state.conversations.first(where: { $0.id == conversationId })?.state.isBusy == true {
                try await abortConversation(conversationId)
            }
            return true
        case let .message(messageText):
            var attachmentIds: [UUID] = []
            for attachment in attachments {
                var form = MultipartForm()
                form.addField(name: "conversationId", value: conversationId.uuidString)
                form.addFile(
                    name: "file",
                    fileName: attachment.fileName,
                    mimeType: attachment.mimeType,
                    data: attachment.data
                )
                let response: AttachmentResponse = try await client.upload(
                    "/v1/attachments",
                    form: form
                )
                attachmentIds.append(response.attachment.id)
            }
            let response: SendMessageResponse = try await client.post(
                "/v1/conversations/\(conversationId.uuidString)/messages",
                body: SendMessageRequest(
                    clientMessageId: UUID(),
                    text: messageText,
                    attachmentIds: attachmentIds
                )
            )
            state.upsertMessage(response.message)
            return true
        }
    }

    func abortConversation(_ conversationId: UUID) async throws {
        try await client.postVoid("/v1/conversations/\(conversationId.uuidString)/abort")
    }

    func transcribe(_ data: Data, fileName: String, mimeType: String) async throws -> String {
        var form = MultipartForm()
        form.addFile(name: "file", fileName: fileName, mimeType: mimeType, data: data)
        let response: TranscriptionResponse = try await client.upload(
            "/v1/transcriptions",
            form: form
        )
        return response.text
    }

    func apply(_ event: ServerEventEnvelope) {
        if case let .conversationUpserted(conversation) = event.event {
            if conversation.archivedAt == nil {
                archivedConversations.removeAll { $0.id == conversation.id }
            } else if listScope == .archived {
                if let index = archivedConversations.firstIndex(where: { $0.id == conversation.id }) {
                    archivedConversations[index] = conversation
                } else {
                    archivedConversations.append(conversation)
                }
                archivedConversations = LunaClientState.sorted(archivedConversations)
            }
        }
        state.apply(event)
    }

    func consumeEventsOnce() async throws {
        let request = try await client.makeRequest(
            path: "/v1/events?after=\(state.cursor)",
            method: .get,
            authenticated: true
        )
        connectionStatus = .connecting
        for try await envelope in eventSource.events(for: request) {
            if case .serverWelcome = envelope.event {
                connectionStatus = .connected
                continue
            }
            if case .syncResetRequired = envelope.event {
                try await reloadBootstrap()
                return
            }
            if case let .error(error) = envelope.event {
                errorMessage = error.message
            } else if case let .commandRejected(rejection) = envelope.event {
                errorMessage = rejection.error.message
            }
            apply(envelope)
            connectionStatus = .connected
        }
    }

    private func reconnectLoop() async {
        var delay: UInt64 = 1
        while !Task.isCancelled {
            do {
                try await consumeEventsOnce()
                delay = 1
            } catch is CancellationError {
                break
            } catch {
                connectionStatus = .waitingToReconnect
            }
            guard !Task.isCancelled else { break }
            connectionStatus = .waitingToReconnect
            do {
                try await Task.sleep(for: .seconds(delay))
            } catch {
                break
            }
            delay = min(delay * 2, 15)
        }
        connectionStatus = .disconnected
    }

    private func reloadBootstrap() async throws {
        let bootstrap: Bootstrap = try await client.get("/v1/bootstrap")
        guard bootstrap.protocolVersion == AppModel.protocolVersion else {
            throw AppModelError.protocolMismatch(
                server: bootstrap.protocolVersion,
                client: AppModel.protocolVersion
            )
        }
        state.install(bootstrap)
        archivedConversations = []
        listScope = .active
    }

    private func loadMessages(
        for conversationId: UUID,
        before: Int64?,
        replacing: Bool
    ) async {
        isLoadingMessages = true
        errorMessage = nil
        defer { isLoadingMessages = false }
        var path = "/v1/conversations/\(conversationId.uuidString)/messages"
        if let before {
            path += "?beforeOrdinal=\(before)"
        }
        do {
            let response: ConversationMessages = try await client.get(path)
            if replacing {
                state.messages[conversationId] = []
            }
            state.setMessages(response, for: conversationId)
        } catch {
            errorMessage = message(from: error)
        }
    }

    private func message(from error: Error) -> String {
        (error as? LocalizedError)?.errorDescription ?? error.localizedDescription
    }
}
