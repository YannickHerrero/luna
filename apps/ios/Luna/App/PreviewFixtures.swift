#if DEBUG
import Foundation

@MainActor
enum PreviewFixtures {
    static func appModel(showConversationList: Bool) -> AppModel {
        let server = URL(string: "https://fixture.luna.test")!
        let model = AppModel(
            configuration: ServerConfiguration(
                defaults: UserDefaults(suiteName: "LunaPreview.\(UUID())")!,
                fallback: server
            ),
            credentials: FixtureCredentialStore(),
            transport: FixtureHTTPTransport(messages: messages),
            eventSource: EmptyEventSource()
        )
        model.install(bootstrap)
        if showConversationList {
            model.conversationStore?.showConversationList()
        }
        return model
    }

    static let conversations = [
        conversation(
            id: 1,
            title: "Launch Luna",
            state: .idle,
            preview: "Persistent Pi conversations are ready.",
            repository: "Luna",
            fallback: "☾",
            lastMessageAt: "2026-07-31T07:40:00Z"
        ),
        conversation(
            id: 2,
            title: "Native iOS parity",
            state: .working,
            preview: "Matching the responsive conversation shell…",
            repository: "Luna",
            fallback: "L",
            lastMessageAt: "2026-07-31T07:38:00Z"
        ),
        conversation(
            id: 3,
            title: "Notification service",
            state: .error,
            preview: "Waiting for APNs configuration.",
            repository: "Relay",
            fallback: "R",
            lastMessageAt: "2026-07-29T09:00:00Z"
        ),
    ]

    static let bootstrap = Bootstrap(
        protocolVersion: 1,
        cursor: 12,
        device: Device(
            id: id(90),
            name: "Preview iPhone",
            platform: .ios,
            notificationsEnabled: false,
            createdAt: "2026-07-31T07:00:00Z",
            lastSeenAt: "2026-07-31T07:40:00Z"
        ),
        conversations: conversations
    )

    static let messages: [UUID: [Message]] = [
        id(1): [
            Message(
                id: id(101),
                conversationId: id(1),
                clientMessageId: id(201),
                role: .user,
                status: .completed,
                delivery: .initial,
                text: "Give me a concise Markdown project status.",
                attachments: [],
                sentByDeviceId: id(90),
                ordinal: 1,
                createdAt: "2026-07-31T07:39:00Z",
                updatedAt: "2026-07-31T07:39:00Z"
            ),
            Message(
                id: id(102),
                conversationId: id(1),
                clientMessageId: nil,
                role: .assistant,
                status: .completed,
                delivery: nil,
                text: "# Luna is ready\n\nPersistent Pi conversations are enabled and ready to use.\n\n- Conversation persistence configured\n- History restoration verified\n- Project setup completed",
                attachments: [],
                sentByDeviceId: nil,
                ordinal: 2,
                createdAt: "2026-07-31T07:40:00Z",
                updatedAt: "2026-07-31T07:40:00Z"
            ),
        ],
    ]

    private static func conversation(
        id value: Int,
        title: String,
        state: SessionState,
        preview: String,
        repository: String,
        fallback: String,
        lastMessageAt: String
    ) -> Conversation {
        let repositoryId = id(value + 20)
        return Conversation(
            id: id(value),
            title: title,
            titleMode: .automatic,
            state: state,
            preview: preview,
            activeWorkingDirectory: "/Users/yannickherrero/dev/\(repository.lowercased())",
            repositories: [
                Repository(
                    id: repositoryId,
                    displayName: repository,
                    rootPath: "/Users/yannickherrero/dev/\(repository.lowercased())",
                    branch: "main",
                    active: true,
                    icon: RepositoryIcon(
                        repositoryId: repositoryId,
                        contentUrl: nil,
                        fallbackText: fallback,
                        fallbackColor: "#7287fd"
                    ),
                    firstSeenAt: "2026-07-31T07:00:00Z",
                    lastSeenAt: lastMessageAt
                ),
            ],
            activities: [],
            taskList: nil,
            lastMessageAt: lastMessageAt,
            notificationTargetDeviceId: id(90),
            unreadCount: 0,
            archivedAt: nil,
            createdAt: "2026-07-31T07:00:00Z",
            updatedAt: lastMessageAt,
            version: 1
        )
    }

    private static func id(_ value: Int) -> UUID {
        UUID(uuidString: String(format: "00000000-0000-0000-0000-%012d", value))!
    }
}

private actor FixtureCredentialStore: CredentialStore {
    func token(for server: URL) -> String? { "fixture-token" }
    func setToken(_ token: String, for server: URL) {}
    func removeToken(for server: URL) {}
}

private actor FixtureHTTPTransport: HTTPTransport {
    let messages: [UUID: [Message]]

    init(messages: [UUID: [Message]]) {
        self.messages = messages
    }

    func data(for request: URLRequest) throws -> HTTPResponse {
        let data: Data
        if request.url?.path.hasSuffix("/messages") == true,
           let idString = request.url?.pathComponents.dropLast().last,
           let conversationId = UUID(uuidString: idString)
        {
            data = try JSONEncoder().encode(
                ConversationMessages(
                    messages: messages[conversationId] ?? [],
                    nextBeforeOrdinal: nil
                )
            )
        } else {
            throw APIClientError.invalidResponse
        }
        let response = HTTPURLResponse(
            url: request.url!,
            statusCode: 200,
            httpVersion: "HTTP/1.1",
            headerFields: ["Content-Type": "application/json"]
        )!
        return HTTPResponse(data: data, response: response)
    }
}
#endif
