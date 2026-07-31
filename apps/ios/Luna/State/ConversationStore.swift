import Foundation
import Observation

enum ConnectionStatus: Equatable, Sendable {
    case disconnected
    case connecting
    case connected
    case waitingToReconnect
}

@MainActor
@Observable
final class ConversationStore {
    private(set) var state: LunaClientState
    private(set) var connectionStatus = ConnectionStatus.disconnected
    private(set) var isLoadingMessages = false
    var errorMessage: String?

    @ObservationIgnored private let client: APIClient
    @ObservationIgnored private let eventSource: any EventSource
    @ObservationIgnored private var connectionTask: Task<Void, Never>?

    init(client: APIClient, bootstrap: Bootstrap, eventSource: any EventSource) {
        self.client = client
        self.eventSource = eventSource
        var state = LunaClientState()
        state.install(bootstrap)
        self.state = state
    }

    var conversations: [Conversation] { state.conversations }
    var selectedConversation: Conversation? { state.selectedConversation }
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

    func selectConversation(_ id: UUID?) async {
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
            state.upsertConversation(conversation)
            state.select(conversation.id)
            state.messages[conversation.id] = []
        } catch {
            errorMessage = message(from: error)
        }
    }

    func apply(_ event: ServerEventEnvelope) {
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
            state.apply(envelope)
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
