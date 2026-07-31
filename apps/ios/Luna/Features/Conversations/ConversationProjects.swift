import Foundation

enum ConversationProjectID: Hashable, Sendable {
    case repository(UUID)
    case noProject
}

struct ConversationProjectSection: Identifiable, Sendable {
    let id: ConversationProjectID
    let repository: Repository?
    var conversations: [Conversation]
}

func primaryRepository(for conversation: Conversation) -> Repository? {
    guard !conversation.repositories.isEmpty else { return nil }

    let matching = conversation.repositories
        .filter {
            containsPath(
                rootPath: $0.rootPath,
                workingDirectory: conversation.activeWorkingDirectory
            )
        }
        .sorted { left, right in
            let leftLength = normalizedPath(left.rootPath).count
            let rightLength = normalizedPath(right.rootPath).count
            if leftLength != rightLength { return leftLength > rightLength }
            return left.id.uuidString < right.id.uuidString
        }
    if let repository = matching.first { return repository }

    let active = conversation.repositories.filter(\.active).sorted(by: repositoryFallbackOrder)
    return active.first ?? conversation.repositories.sorted(by: repositoryFallbackOrder).first
}

func conversationProjectSections(_ conversations: [Conversation]) -> [ConversationProjectSection] {
    var sections: [ConversationProjectSection] = []
    var indexes: [ConversationProjectID: Int] = [:]

    for conversation in LunaClientState.sorted(conversations) {
        let repository = primaryRepository(for: conversation)
        let id = repository.map { ConversationProjectID.repository($0.id) } ?? .noProject
        if let index = indexes[id] {
            sections[index].conversations.append(conversation)
        } else {
            indexes[id] = sections.count
            sections.append(
                ConversationProjectSection(
                    id: id,
                    repository: repository,
                    conversations: [conversation]
                )
            )
        }
    }
    return sections
}

private func repositoryFallbackOrder(_ left: Repository, _ right: Repository) -> Bool {
    let leftDate = projectTimestamp(left.lastSeenAt)
    let rightDate = projectTimestamp(right.lastSeenAt)
    if leftDate != rightDate { return leftDate > rightDate }
    return left.id.uuidString < right.id.uuidString
}

private func containsPath(rootPath: String, workingDirectory: String) -> Bool {
    let root = normalizedPath(rootPath)
    let working = normalizedPath(workingDirectory)
    if root == "/" { return working.hasPrefix("/") }
    return working == root || working.hasPrefix("\(root)/")
}

private func normalizedPath(_ path: String) -> String {
    guard path != "/" else { return path }
    return path.replacing(/\/+$/, with: "")
}

private func projectTimestamp(_ value: String) -> Date {
    (try? Date.ISO8601FormatStyle(includingFractionalSeconds: true).parse(value))
        ?? (try? Date.ISO8601FormatStyle().parse(value))
        ?? .distantPast
}
