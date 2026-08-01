import Foundation
import Observation
import UIKit

enum AppPhase: Equatable, Sendable {
    case loading
    case pairing
    case ready
}

enum AppModelError: Error, Equatable, LocalizedError, Sendable {
    case protocolMismatch(server: UInt8, client: UInt8)

    var errorDescription: String? {
        switch self {
        case let .protocolMismatch(server, client):
            "This Luna server uses protocol \(server), but the app supports protocol \(client)."
        }
    }
}

@MainActor
@Observable
final class AppModel {
    static let protocolVersion: UInt8 = 1

    private(set) var phase = AppPhase.loading
    private(set) var bootstrap: Bootstrap?
    private(set) var conversationStore: ConversationStore?
    var errorMessage: String?

    @ObservationIgnored private var pendingRoute: LunaRoute?

    let configuration: ServerConfiguration
    @ObservationIgnored private let credentials: any CredentialStore
    @ObservationIgnored private let transport: any HTTPTransport
    @ObservationIgnored private let eventSource: any EventSource
    @ObservationIgnored private let draftPersistence: ComposerDraftPersistence
    @ObservationIgnored private let activeAgentSnapshotPublisher: ActiveAgentSnapshotPublisher

    init(
        configuration: ServerConfiguration = ServerConfiguration(),
        credentials: any CredentialStore = KeychainCredentialStore.shared,
        transport: any HTTPTransport = URLSessionHTTPTransport(),
        eventSource: any EventSource = URLSessionEventSource(),
        draftPersistence: ComposerDraftPersistence = ComposerDraftPersistence(),
        activeAgentSnapshotPublisher: ActiveAgentSnapshotPublisher? = nil
    ) {
        self.configuration = configuration
        self.credentials = credentials
        self.transport = transport
        self.eventSource = eventSource
        self.draftPersistence = draftPersistence
        self.activeAgentSnapshotPublisher = activeAgentSnapshotPublisher
            ?? ActiveAgentSnapshotPublisher(snapshotDidChange: { snapshot in
                WatchSnapshotTransmitter.shared.send(snapshot)
            })
    }

    var client: APIClient {
        APIClient(
            baseURL: configuration.serverURL,
            credentials: credentials,
            transport: transport
        )
    }

    func start() async {
        phase = .loading
        errorMessage = nil
        do {
            let bootstrap: Bootstrap = try await client.get("/v1/bootstrap")
            try validateProtocol(bootstrap)
            install(bootstrap)
        } catch APIClientError.authenticationRequired {
            phase = .pairing
        } catch APIClientError.server(status: 401, error: _) {
            try? await credentials.removeToken(for: configuration.serverURL)
            phase = .pairing
        } catch {
            errorMessage = message(from: error)
            phase = .pairing
        }
    }

    func requestPairingCode() async throws -> PairingCodeRequestResponse {
        try await client.post("/v1/pairing/request", authenticated: false)
    }

    func pair(code: String, deviceName: String) async throws {
        let response: PairingExchangeResponse = try await client.post(
            "/v1/pairing/exchange",
            body: PairingExchangeRequest(
                code: code,
                deviceName: deviceName,
                platform: Self.devicePlatform
            ),
            authenticated: false
        )
        try validateProtocol(response.bootstrap)
        try await credentials.setToken(response.token, for: configuration.serverURL)
        install(response.bootstrap)
    }

    func changeServer(to input: String) async throws {
        conversationStore?.stopRealtime()
        try configuration.update(input)
        bootstrap = nil
        conversationStore = nil
        await start()
    }

    func open(_ url: URL) async {
        guard let route = LunaRoute(url: url) else { return }
        guard phase == .ready else {
            pendingRoute = route
            return
        }
        await apply(route)
    }

    func resolvePendingRoute() async {
        guard phase == .ready, let route = pendingRoute else { return }
        pendingRoute = nil
        await apply(route)
    }

    func install(_ bootstrap: Bootstrap) {
        conversationStore?.stopRealtime()
        self.bootstrap = bootstrap
        conversationStore = ConversationStore(
            client: client,
            bootstrap: bootstrap,
            eventSource: eventSource,
            draftPersistence: draftPersistence,
            activeAgentSnapshotPublisher: activeAgentSnapshotPublisher
        )
        phase = .ready
        errorMessage = nil
    }

    private func apply(_ route: LunaRoute) async {
        guard let store = conversationStore else {
            pendingRoute = route
            return
        }
        switch route {
        case .home:
            store.showConversationList()
        case let .conversation(id):
            guard store.conversations.contains(where: { $0.id == id }) else {
                store.errorMessage = "This conversation is unavailable."
                return
            }
            await store.selectConversation(id)
        }
    }

    private func validateProtocol(_ bootstrap: Bootstrap) throws {
        guard bootstrap.protocolVersion == Self.protocolVersion else {
            throw AppModelError.protocolMismatch(
                server: bootstrap.protocolVersion,
                client: Self.protocolVersion
            )
        }
    }

    private func message(from error: Error) -> String {
        (error as? LocalizedError)?.errorDescription
            ?? error.localizedDescription
    }

    private static var devicePlatform: DevicePlatform {
        UIDevice.current.userInterfaceIdiom == .pad ? .ipados : .ios
    }
}
