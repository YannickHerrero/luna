import Foundation
import Testing
@testable import Luna

private actor PairingTransport: HTTPTransport {
    struct Stub: Sendable {
        let status: Int
        let data: Data
    }

    private var stubs: [Stub]
    private(set) var requests: [URLRequest] = []

    init(_ stubs: [Stub]) {
        self.stubs = stubs
    }

    func data(for request: URLRequest) throws -> HTTPResponse {
        requests.append(request)
        guard !stubs.isEmpty else {
            throw APIClientError.invalidResponse
        }
        let stub = stubs.removeFirst()
        let response = HTTPURLResponse(
            url: request.url!,
            statusCode: stub.status,
            httpVersion: "HTTP/1.1",
            headerFields: ["Content-Type": "application/json"]
        )!
        return HTTPResponse(data: stub.data, response: response)
    }

    func requestCount() -> Int { requests.count }
    func lastRequest() -> URLRequest? { requests.last }
}

@MainActor
struct PairingTests {
    private let server = URL(string: "https://mac.example.ts.net:8447")!

    @Test
    func normalizesAndPersistsPrivateServerURLs() throws {
        let defaults = temporaryDefaults()
        let configuration = ServerConfiguration(defaults: defaults, fallback: server)

        try configuration.update("MAC.EXAMPLE.TS.NET:8447/")

        #expect(configuration.serverURL.absoluteString == "https://mac.example.ts.net:8447")
        #expect(defaults.string(forKey: "luna-server-url") == configuration.serverURL.absoluteString)
        #expect(throws: ServerConfigurationError.insecureRemoteServer) {
            try configuration.update("http://mac.example.ts.net:9870")
        }
        #expect(try ServerURL.normalized("http://127.0.0.1:9870").absoluteString == "http://127.0.0.1:9870")
    }

    @Test
    func startsOnPairingWithoutStoredCredentials() async {
        let credentials = MemoryCredentialStore()
        let transport = PairingTransport([])
        let model = AppModel(
            configuration: ServerConfiguration(defaults: temporaryDefaults(), fallback: server),
            credentials: credentials,
            transport: transport
        )

        await model.start()

        #expect(model.phase == .pairing)
        #expect(await transport.requestCount() == 0)
    }

    @Test
    func removesAnExpiredCredentialAfterUnauthorizedBootstrap() async throws {
        let credentials = MemoryCredentialStore()
        await credentials.setToken("expired", for: server)
        let transport = PairingTransport([
            .init(
                status: 401,
                data: Data(
                    #"{"code":"authentication_required","message":"Pair this device.","retryable":false}"#.utf8
                )
            ),
        ])
        let model = AppModel(
            configuration: ServerConfiguration(defaults: temporaryDefaults(), fallback: server),
            credentials: credentials,
            transport: transport
        )

        await model.start()

        #expect(model.phase == .pairing)
        #expect(await credentials.token(for: server) == nil)
    }

    @Test
    func pairsAsANativeDeviceAndStoresTheBearerToken() async throws {
        let credentials = MemoryCredentialStore()
        let transport = PairingTransport([
            .init(status: 201, data: pairingResponseData()),
        ])
        let model = AppModel(
            configuration: ServerConfiguration(defaults: temporaryDefaults(), fallback: server),
            credentials: credentials,
            transport: transport
        )

        try await model.pair(code: "123456", deviceName: "Test iPhone")
        let request = try #require(await transport.lastRequest())
        let requestData = try #require(request.httpBody)
        let object = try #require(
            JSONSerialization.jsonObject(with: requestData) as? [String: Any]
        )

        #expect(model.phase == .ready)
        #expect(model.bootstrap?.cursor == 7)
        #expect(await credentials.token(for: server) == "native-token")
        #expect(request.value(forHTTPHeaderField: "Authorization") == nil)
        #expect(object["code"] as? String == "123456")
        #expect(object["deviceName"] as? String == "Test iPhone")
        #expect(["ios", "ipados"].contains(object["platform"] as? String ?? ""))
    }

    @Test
    func refreshesAndPublishesSanitizedAccountUsage() async throws {
        let credentials = MemoryCredentialStore()
        let directory = FileManager.default.temporaryDirectory
            .appending(path: UUID().uuidString, directoryHint: .isDirectory)
        defer { try? FileManager.default.removeItem(at: directory) }
        let snapshotStore = LunaSnapshotStore(directoryURL: directory)
        let transport = PairingTransport([
            .init(status: 201, data: pairingResponseData()),
            .init(
                status: 200,
                data: Data(
                    #"{"availability":"available","usedPercent":63,"resetsAt":"2030-03-17T17:46:40Z","collectedAt":"2026-08-01T00:00:00Z"}"#.utf8
                )
            ),
        ])
        let model = AppModel(
            configuration: ServerConfiguration(defaults: temporaryDefaults(), fallback: server),
            credentials: credentials,
            transport: transport,
            openAIUsageSnapshotPublisher: OpenAIUsageSnapshotPublisher(
                store: snapshotStore,
                reloadTimelines: {}
            )
        )

        try await model.pair(code: "123456", deviceName: "Usage iPhone")
        await model.refreshOpenAIWeeklyUsage()

        #expect(model.openAiWeeklyUsage?.usedPercent == 63)
        #expect(try snapshotStore.readOpenAIWeeklyUsage()?.usedPercent == 63)
        #expect(await transport.lastRequest()?.url?.path == "/v1/account/openai-usage")
    }

    @Test
    func rejectsAnIncompatibleProtocolBeforeSavingCredentials() async {
        let credentials = MemoryCredentialStore()
        let incompatible = Data(
            String(decoding: pairingResponseData(), as: UTF8.self)
                .replacingOccurrences(of: "\"protocolVersion\":1", with: "\"protocolVersion\":2")
                .utf8
        )
        let model = AppModel(
            configuration: ServerConfiguration(defaults: temporaryDefaults(), fallback: server),
            credentials: credentials,
            transport: PairingTransport([.init(status: 201, data: incompatible)])
        )

        await #expect(throws: AppModelError.protocolMismatch(server: 2, client: 1)) {
            try await model.pair(code: "123456", deviceName: "Test iPhone")
        }
        #expect(await credentials.token(for: server) == nil)
    }

    private func temporaryDefaults() -> UserDefaults {
        UserDefaults(suiteName: "LunaTests.\(UUID())")!
    }

    private func pairingResponseData() -> Data {
        Data(
            #"""
            {
              "deviceId":"00000000-0000-0000-0000-000000000001",
              "token":"native-token",
              "bootstrap":{
                "protocolVersion":1,
                "cursor":7,
                "device":{
                  "id":"00000000-0000-0000-0000-000000000001",
                  "name":"Test iPhone",
                  "platform":"ios",
                  "notificationsEnabled":false,
                  "createdAt":"2026-03-20T12:00:00Z",
                  "lastSeenAt":"2026-03-20T12:00:00Z"
                },
                "conversations":[]
              }
            }
            """#.utf8
        )
    }
}
