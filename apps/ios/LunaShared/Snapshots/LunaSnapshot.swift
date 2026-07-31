import Foundation

enum LunaAppGroup {
    static let identifier = "group.com.yannickherrero.luna"
}

enum AgentSnapshotState: String, Codable, CaseIterable, Sendable {
    case creating
    case starting
    case idle
    case working
    case compacting
    case retrying
    case crashed
    case restoring
    case interrupted
    case stopped
    case error
}

struct ActiveAgentSnapshot: Codable, Equatable, Identifiable, Sendable {
    static let maximumTitleLength = 80
    static let maximumActivityLength = 120

    let id: UUID
    let title: String
    let state: AgentSnapshotState
    let activity: String?
    let updatedAt: Date

    init(
        id: UUID,
        title: String,
        state: AgentSnapshotState,
        activity: String?,
        updatedAt: Date
    ) {
        self.id = id
        self.title = Self.cleaned(
            title,
            maximumLength: Self.maximumTitleLength,
            fallback: "Untitled conversation"
        )
        self.state = state
        self.activity = activity.flatMap {
            let value = Self.cleaned(
                $0,
                maximumLength: Self.maximumActivityLength,
                fallback: ""
            )
            return value.isEmpty ? nil : value
        }
        self.updatedAt = updatedAt
    }

    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        self.init(
            id: try values.decode(UUID.self, forKey: .id),
            title: try values.decode(String.self, forKey: .title),
            state: try values.decode(AgentSnapshotState.self, forKey: .state),
            activity: try values.decodeIfPresent(String.self, forKey: .activity),
            updatedAt: try values.decode(Date.self, forKey: .updatedAt)
        )
    }

    private enum CodingKeys: String, CodingKey {
        case id
        case title
        case state
        case activity
        case updatedAt
    }

    private static func cleaned(
        _ value: String,
        maximumLength: Int,
        fallback: String
    ) -> String {
        let safe = value.unicodeScalars.map {
            CharacterSet.controlCharacters.contains($0) ? " " : String($0)
        }
        let cleaned = safe.joined()
            .components(separatedBy: .whitespacesAndNewlines)
            .filter { !$0.isEmpty }
            .joined(separator: " ")
        guard !cleaned.isEmpty else { return fallback }
        return String(cleaned.prefix(maximumLength))
    }
}

struct ActiveAgentsSnapshot: Codable, Equatable, Sendable {
    static let currentSchemaVersion = 1
    static let maximumAgentCount = 8

    let schemaVersion: Int
    let generatedAt: Date
    let agents: [ActiveAgentSnapshot]

    init(generatedAt: Date, agents: [ActiveAgentSnapshot]) {
        schemaVersion = Self.currentSchemaVersion
        self.generatedAt = generatedAt
        self.agents = Array(agents.prefix(Self.maximumAgentCount))
    }

    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        schemaVersion = try values.decode(Int.self, forKey: .schemaVersion)
        generatedAt = try values.decode(Date.self, forKey: .generatedAt)
        agents = Array(
            try values.decode([ActiveAgentSnapshot].self, forKey: .agents)
                .prefix(Self.maximumAgentCount)
        )
    }

    private enum CodingKeys: String, CodingKey {
        case schemaVersion
        case generatedAt
        case agents
    }
}

enum LunaSnapshotStoreError: Error, Equatable {
    case appGroupUnavailable
    case unsupportedSchemaVersion
}

struct LunaSnapshotStore {
    private static let activeAgentsFileName = "active-agents-v1.json"

    private let directoryURL: URL?
    private let fileManager: FileManager

    init(fileManager: FileManager = .default) {
        self.fileManager = fileManager
        directoryURL = fileManager
            .containerURL(forSecurityApplicationGroupIdentifier: LunaAppGroup.identifier)?
            .appending(path: "Snapshots", directoryHint: .isDirectory)
    }

    init(directoryURL: URL, fileManager: FileManager = .default) {
        self.directoryURL = directoryURL
        self.fileManager = fileManager
    }

    func writeActiveAgents(_ snapshot: ActiveAgentsSnapshot) throws {
        let url = try snapshotURL(fileName: Self.activeAgentsFileName, createDirectory: true)
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        encoder.outputFormatting = [.sortedKeys]
        try encoder.encode(snapshot).write(to: url, options: .atomic)
    }

    func readActiveAgents() throws -> ActiveAgentsSnapshot? {
        let url = try snapshotURL(fileName: Self.activeAgentsFileName, createDirectory: false)
        guard fileManager.fileExists(atPath: url.path) else { return nil }
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        let snapshot = try decoder.decode(ActiveAgentsSnapshot.self, from: Data(contentsOf: url))
        guard snapshot.schemaVersion == ActiveAgentsSnapshot.currentSchemaVersion else {
            throw LunaSnapshotStoreError.unsupportedSchemaVersion
        }
        return snapshot
    }

    private func snapshotURL(fileName: String, createDirectory: Bool) throws -> URL {
        guard let directoryURL else { throw LunaSnapshotStoreError.appGroupUnavailable }
        if createDirectory {
            try fileManager.createDirectory(at: directoryURL, withIntermediateDirectories: true)
        }
        return directoryURL.appending(path: fileName, directoryHint: .notDirectory)
    }
}
