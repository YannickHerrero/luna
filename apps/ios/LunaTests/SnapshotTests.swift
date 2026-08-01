import Foundation
import Testing
@testable import Luna

struct SnapshotTests {
    @Test
    func sanitizesAndBoundsWidgetFields() throws {
        let snapshot = ActiveAgentSnapshot(
            id: UUID(),
            title: "  Ship\n\tLuna \u{0000} ",
            state: .working,
            activity: String(repeating: "a", count: 140),
            updatedAt: Date(timeIntervalSince1970: 1_700_000_000)
        )

        #expect(snapshot.title == "Ship Luna")
        #expect(snapshot.activity?.count == ActiveAgentSnapshot.maximumActivityLength)

        let redacted = ActiveAgentSnapshot(
            id: UUID(),
            title: "Review /Users/private/repository with token secret-value",
            state: .working,
            activity: "Open https://private.example/repository using sk-private-value",
            updatedAt: snapshot.updatedAt
        )
        #expect(redacted.title == "Review ••• with ••• •••")
        #expect(redacted.activity == "Open ••• using •••")

        let manyAgents = Array(repeating: snapshot, count: 10)
        #expect(
            ActiveAgentsSnapshot(generatedAt: snapshot.updatedAt, agents: manyAgents).agents.count
                == ActiveAgentsSnapshot.maximumAgentCount
        )
    }

    @Test
    func persistsOnlyTheVersionedActiveAgentAllowlist() throws {
        let directory = FileManager.default.temporaryDirectory
            .appending(path: UUID().uuidString, directoryHint: .isDirectory)
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = LunaSnapshotStore(directoryURL: directory)
        let agent = ActiveAgentSnapshot(
            id: UUID(uuidString: "00000000-0000-0000-0000-000000000001")!,
            title: "Launch Luna",
            state: .working,
            activity: "Running tests",
            updatedAt: Date(timeIntervalSince1970: 1_700_000_000)
        )
        let snapshot = ActiveAgentsSnapshot(generatedAt: agent.updatedAt, agents: [agent])

        try store.writeActiveAgents(snapshot)
        #expect(try store.readActiveAgents() == snapshot)

        let data = try Data(
            contentsOf: directory.appending(path: "active-agents-v1.json")
        )
        let object = try #require(JSONSerialization.jsonObject(with: data) as? [String: Any])
        #expect(Set(object.keys) == ["schemaVersion", "generatedAt", "agents"])
        let agents = try #require(object["agents"] as? [[String: Any]])
        #expect(
            Set(try #require(agents.first).keys)
                == ["id", "title", "state", "activity", "updatedAt"]
        )
        let serialized = try #require(String(data: data, encoding: .utf8))
        #expect(!serialized.contains("token"))
        #expect(!serialized.contains("credential"))
        #expect(!serialized.contains("repository"))
        #expect(!serialized.contains("message"))
    }

    @Test
    func widgetDistinguishesCurrentStaleAndUnavailableSnapshots() {
        let date = Date(timeIntervalSince1970: 1_700_000_000)
        let agent = ActiveAgentSnapshot(
            id: snapshotUUID(1),
            title: "Current work",
            state: .working,
            activity: "Running checks",
            updatedAt: date
        )
        let current = LunaWidgetEntry(
            date: date,
            snapshot: ActiveAgentsSnapshot(generatedAt: date, agents: [agent])
        )
        let stale = LunaWidgetEntry(
            date: date,
            snapshot: ActiveAgentsSnapshot(
                generatedAt: date.addingTimeInterval(-30 * 60),
                agents: [agent]
            )
        )
        let unavailable = LunaWidgetEntry(
            date: date,
            snapshot: ActiveAgentsSnapshot(
                generatedAt: date.addingTimeInterval(-25 * 60 * 60),
                agents: [agent]
            )
        )

        #expect(!current.isStale)
        #expect(current.agents == [agent])
        #expect(stale.isStale)
        #expect(stale.agents == [agent])
        #expect(unavailable.isUnavailable)
        #expect(unavailable.agents.isEmpty)
    }

    @Test @MainActor
    func publishesOnlyActiveAgentsWithoutLeakingRawActivity() throws {
        let directory = FileManager.default.temporaryDirectory
            .appending(path: UUID().uuidString, directoryHint: .isDirectory)
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = LunaSnapshotStore(directoryURL: directory)
        var reloadCount = 0
        var transmittedSnapshot: ActiveAgentsSnapshot?
        let publisher = ActiveAgentSnapshotPublisher(
            store: store,
            snapshotDidChange: { transmittedSnapshot = $0 },
            reloadTimelines: { reloadCount += 1 }
        )
        let date = Date(timeIntervalSince1970: 1_700_000_000)
        var working = snapshotConversation(id: 1, state: .working)
        working.activities = [
            AgentActivity(
                id: UUID(),
                sequence: 1,
                summary: "Review /Users/private/repository with bearer-token-secret",
                createdAt: "2026-01-01T00:00:00Z",
                updatedAt: "2026-01-01T00:00:00Z"
            ),
        ]

        publisher.publish(
            conversations: [
                working,
                snapshotConversation(id: 2, state: .idle),
                snapshotConversation(id: 3, state: .creating),
                snapshotConversation(id: 4, state: .compacting),
            ],
            at: date
        )

        let snapshot = try #require(try store.readActiveAgents())
        #expect(snapshot.agents.map(\.id) == [working.id, snapshotUUID(4)])
        #expect(snapshot.agents.first?.activity == "Reviewing files")
        #expect(transmittedSnapshot == snapshot)
        let serialized = try String(
            contentsOf: directory.appending(path: "active-agents-v1.json"),
            encoding: .utf8
        )
        #expect(!serialized.contains("/Users/private"))
        #expect(!serialized.contains("bearer-token-secret"))
        #expect(reloadCount == 1)

        publisher.publish(conversations: [working], at: date.addingTimeInterval(60))
        #expect(reloadCount == 2)
        publisher.publish(conversations: [working], at: date.addingTimeInterval(120))
        #expect(reloadCount == 2)
    }

    @Test
    func rejectsUnknownSnapshotVersions() throws {
        let directory = FileManager.default.temporaryDirectory
            .appending(path: UUID().uuidString, directoryHint: .isDirectory)
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = LunaSnapshotStore(directoryURL: directory)
        try store.writeActiveAgents(ActiveAgentsSnapshot(generatedAt: .now, agents: []))
        let url = directory.appending(path: "active-agents-v1.json")
        var object = try #require(
            JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any]
        )
        object["schemaVersion"] = 2
        try JSONSerialization.data(withJSONObject: object).write(to: url, options: .atomic)

        #expect(throws: LunaSnapshotStoreError.unsupportedSchemaVersion) {
            try store.readActiveAgents()
        }
    }
}

private func snapshotConversation(id: Int, state: SessionState) -> Conversation {
    Conversation(
        id: snapshotUUID(id),
        title: "Conversation \(id)",
        titleMode: .automatic,
        state: state,
        preview: "",
        activeWorkingDirectory: "",
        repositories: [],
        activities: [],
        taskList: nil,
        lastMessageAt: nil,
        notificationTargetDeviceId: nil,
        unreadCount: 0,
        archivedAt: nil,
        createdAt: "2026-01-01T00:00:00Z",
        updatedAt: "2026-01-01T00:00:00Z",
        version: 1
    )
}

private func snapshotUUID(_ value: Int) -> UUID {
    UUID(uuidString: String(format: "00000000-0000-0000-0000-%012d", value))!
}
