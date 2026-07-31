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
