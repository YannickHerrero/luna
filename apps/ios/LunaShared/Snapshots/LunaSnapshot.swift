import Foundation

enum LunaAppGroup {
    static let identifier = "group.com.yannickherrero.luna"
    static let activeAgentsWidgetKind = "LunaActiveAgentsWidget"
    static let watchActiveAgentsWidgetKind = "LunaWatchActiveAgentsWidget"
    static let watchActiveAgentsContextKey = "activeAgentsSnapshotV1"
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
        let redacted = redactSensitiveWords(in: cleaned)
        guard !redacted.isEmpty else { return fallback }
        return String(redacted.prefix(maximumLength))
    }

    private static func redactSensitiveWords(in value: String) -> String {
        let sensitiveLabels = [
            "bearer", "credential", "password", "secret", "token", "api-key", "api_key",
        ]
        let sensitivePrefixes = [
            "bearer-", "bearer_", "ghp_", "github_pat_", "password=", "secret=", "sk-",
            "sk_", "token=",
        ]
        var redactNext = false
        return value.split(separator: " ").map { substring in
            if redactNext {
                redactNext = false
                return "•••"
            }
            let word = String(substring)
            let normalized = word
                .trimmingCharacters(in: .punctuationCharacters)
                .lowercased()
            if word.contains("/")
                || word.contains("\\")
                || word.contains("://")
                || sensitivePrefixes.contains(where: { normalized.hasPrefix($0) })
            {
                return "•••"
            }
            if sensitiveLabels.contains(normalized) {
                redactNext = true
                return "•••"
            }
            return word
        }
        .joined(separator: " ")
    }
}

enum ActiveAgentsSnapshotFreshness: Equatable, Sendable {
    case current
    case stale
    case unavailable
}

struct ActiveAgentsSnapshot: Codable, Equatable, Sendable {
    static let currentSchemaVersion = 1
    static let maximumAgentCount = 8
    static let staleAfter: TimeInterval = 15 * 60
    static let unavailableAfter: TimeInterval = 24 * 60 * 60

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

    func freshness(at date: Date) -> ActiveAgentsSnapshotFreshness {
        let age = max(0, date.timeIntervalSince(generatedAt))
        if age > Self.unavailableAfter { return .unavailable }
        if age > Self.staleAfter { return .stale }
        return .current
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

enum LunaSnapshotCodec {
    static func encodeActiveAgents(_ snapshot: ActiveAgentsSnapshot) throws -> Data {
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        encoder.outputFormatting = [.sortedKeys]
        return try encoder.encode(snapshot)
    }

    static func decodeActiveAgents(_ data: Data) throws -> ActiveAgentsSnapshot {
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .iso8601
        let snapshot = try decoder.decode(ActiveAgentsSnapshot.self, from: data)
        guard snapshot.schemaVersion == ActiveAgentsSnapshot.currentSchemaVersion else {
            throw LunaSnapshotStoreError.unsupportedSchemaVersion
        }
        return snapshot
    }
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
        try LunaSnapshotCodec.encodeActiveAgents(snapshot).write(to: url, options: .atomic)
    }

    func readActiveAgents() throws -> ActiveAgentsSnapshot? {
        let url = try snapshotURL(fileName: Self.activeAgentsFileName, createDirectory: false)
        guard fileManager.fileExists(atPath: url.path) else { return nil }
        return try LunaSnapshotCodec.decodeActiveAgents(Data(contentsOf: url))
    }

    private func snapshotURL(fileName: String, createDirectory: Bool) throws -> URL {
        guard let directoryURL else { throw LunaSnapshotStoreError.appGroupUnavailable }
        if createDirectory {
            try fileManager.createDirectory(at: directoryURL, withIntermediateDirectories: true)
        }
        return directoryURL.appending(path: fileName, directoryHint: .notDirectory)
    }
}
