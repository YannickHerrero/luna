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

    let configuration: ServerConfiguration
    @ObservationIgnored private let credentials: any CredentialStore
    @ObservationIgnored private let transport: any HTTPTransport
    @ObservationIgnored private let eventSource: any EventSource

    init(
        configuration: ServerConfiguration = ServerConfiguration(),
        credentials: any CredentialStore = KeychainCredentialStore.shared,
        transport: any HTTPTransport = URLSessionHTTPTransport(),
        eventSource: any EventSource = URLSessionEventSource()
    ) {
        self.configuration = configuration
        self.credentials = credentials
        self.transport = transport
        self.eventSource = eventSource
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

    func install(_ bootstrap: Bootstrap) {
        conversationStore?.stopRealtime()
        self.bootstrap = bootstrap
        conversationStore = ConversationStore(
            client: client,
            bootstrap: bootstrap,
            eventSource: eventSource
        )
        phase = .ready
        errorMessage = nil
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
