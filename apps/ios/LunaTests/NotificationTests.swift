import Foundation
import Testing
import UserNotifications
@testable import Luna

@MainActor
private final class FakeNotificationAuthorization: NotificationAuthorizationClient {
    var status: UNAuthorizationStatus
    var authorizationResult: Bool
    private(set) var requestCount = 0

    init(status: UNAuthorizationStatus, authorizationResult: Bool = true) {
        self.status = status
        self.authorizationResult = authorizationResult
    }

    func authorizationStatus() async -> UNAuthorizationStatus { status }

    func requestAuthorization() async throws -> Bool {
        requestCount += 1
        status = authorizationResult ? .authorized : .denied
        return authorizationResult
    }
}

@MainActor
private final class RegistrationCounter {
    private(set) var value = 0

    func increment() {
        value += 1
    }
}

private actor NotificationTransport: HTTPTransport {
    private let response: HTTPResponse
    private(set) var requests: [URLRequest] = []

    init(deviceID: UUID, notificationsEnabled: Bool = true) {
        let url = URL(string: "https://mac.example.ts.net:8447")!
        response = HTTPResponse(
            data: Data(
                """
                {"id":"\(deviceID.uuidString)","name":"iPhone","platform":"ios","notificationsEnabled":\(notificationsEnabled),"createdAt":"2026-03-20T12:00:00Z","lastSeenAt":"2026-03-20T12:00:00Z"}
                """.utf8
            ),
            response: HTTPURLResponse(
                url: url,
                statusCode: 200,
                httpVersion: "HTTP/1.1",
                headerFields: ["Content-Type": "application/json"]
            )!
        )
    }

    func data(for request: URLRequest) -> HTTPResponse {
        requests.append(request)
        return response
    }

    func lastRequest() -> URLRequest? {
        requests.last
    }
}

struct NotificationTests {
    @Test @MainActor
    func requestsPermissionAndRegistersTheSanitizedAPNsToken() async throws {
        let serverURL = URL(string: "https://mac.example.ts.net:8447")!
        let defaults = try #require(UserDefaults(suiteName: "NotificationTests.\(UUID())"))
        let configuration = ServerConfiguration(defaults: defaults, fallback: serverURL)
        let credentials = MemoryCredentialStore()
        await credentials.setToken("native-token", for: serverURL)
        let transport = NotificationTransport(deviceID: PreviewFixtures.bootstrap.device.id)
        let authorization = FakeNotificationAuthorization(status: .notDetermined)
        let counter = RegistrationCounter()
        let coordinator = NotificationCoordinator(
            authorization: authorization,
            registerWithSystem: { counter.increment() },
            topic: { "com.yannickherrero.luna" },
            appVersion: { "1.0" }
        )
        let model = AppModel(
            configuration: configuration,
            credentials: credentials,
            transport: transport,
            notificationCoordinator: coordinator
        )

        model.install(PreviewFixtures.bootstrap)
        await coordinator.refreshRegistration()
        #expect(authorization.requestCount >= 1)
        #expect(counter.value >= 1)

        coordinator.didRegisterForRemoteNotifications(
            deviceToken: Data(repeating: 0xab, count: 32)
        )
        var request: URLRequest?
        for _ in 0 ..< 50 where request == nil {
            try await Task.sleep(for: .milliseconds(10))
            request = await transport.lastRequest()
        }
        let registered = try #require(request)
        #expect(registered.httpMethod == "PUT")
        let body = try #require(registered.httpBody)
        let object = try #require(
            JSONSerialization.jsonObject(with: body) as? [String: Any]
        )
        #expect(object["token"] as? String == String(repeating: "ab", count: 32))
        #expect(object["environment"] as? String == "sandbox")
        #expect(object["topic"] as? String == "com.yannickherrero.luna")
        #expect(object["appVersion"] as? String == "1.0")
        for _ in 0 ..< 50 where model.bootstrap?.device.notificationsEnabled != true {
            try await Task.sleep(for: .milliseconds(10))
        }
        #expect(model.bootstrap?.device.notificationsEnabled == true)
    }

    @Test @MainActor
    func disablesServerRegistrationWhenAuthorizationIsDenied() async throws {
        let serverURL = URL(string: "https://mac.example.ts.net:8447")!
        let defaults = try #require(UserDefaults(suiteName: "NotificationTests.\(UUID())"))
        let configuration = ServerConfiguration(defaults: defaults, fallback: serverURL)
        let credentials = MemoryCredentialStore()
        await credentials.setToken("native-token", for: serverURL)
        let transport = NotificationTransport(
            deviceID: PreviewFixtures.bootstrap.device.id,
            notificationsEnabled: false
        )
        let authorization = FakeNotificationAuthorization(status: .denied)
        let counter = RegistrationCounter()
        let coordinator = NotificationCoordinator(
            authorization: authorization,
            registerWithSystem: { counter.increment() }
        )
        let model = AppModel(
            configuration: configuration,
            credentials: credentials,
            transport: transport,
            notificationCoordinator: coordinator
        )

        model.install(PreviewFixtures.bootstrap)
        await coordinator.refreshRegistration()
        var request: URLRequest?
        for _ in 0 ..< 50 where request == nil {
            try await Task.sleep(for: .milliseconds(10))
            request = await transport.lastRequest()
        }
        let disabled = try #require(request)
        #expect(disabled.httpMethod == "DELETE")
        #expect(counter.value == 0)
        #expect(authorization.requestCount == 0)
        for _ in 0 ..< 50 where model.bootstrap?.device.notificationsEnabled != false {
            try await Task.sleep(for: .milliseconds(10))
        }
        #expect(model.bootstrap?.device.notificationsEnabled == false)
    }

    @Test
    func acceptsOnlyStableLunaConversationPayloads() {
        let conversationID = UUID()
        let directPayload: [AnyHashable: Any] = [
            "conversationId": conversationID.uuidString,
        ]
        let URLPayload: [AnyHashable: Any] = [
            "url": LunaRoute.conversation(conversationID).url.absoluteString,
        ]

        #expect(NotificationCoordinator.conversationID(in: directPayload) == conversationID)
        #expect(NotificationCoordinator.routeURL(in: directPayload) == LunaRoute.conversation(conversationID).url)
        #expect(NotificationCoordinator.conversationID(in: URLPayload) == conversationID)
        #expect(NotificationCoordinator.routeURL(in: URLPayload) == LunaRoute.conversation(conversationID).url)
        #expect(NotificationCoordinator.routeURL(in: ["url": "https://attacker.example/"]) == nil)
        #expect(NotificationCoordinator.routeURL(in: ["conversationId": "not-a-uuid"]) == nil)
    }

    @Test
    func suppressesOnlyTheSelectedForegroundConversation() {
        let selected = UUID()

        #expect(
            NotificationCoordinator.shouldSuppress(
                conversationID: selected,
                selectedConversationID: selected
            )
        )
        #expect(
            !NotificationCoordinator.shouldSuppress(
                conversationID: UUID(),
                selectedConversationID: selected
            )
        )
        #expect(
            !NotificationCoordinator.shouldSuppress(
                conversationID: selected,
                selectedConversationID: nil
            )
        )
    }
}
