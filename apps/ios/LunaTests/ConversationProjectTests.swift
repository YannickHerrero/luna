import Foundation
import Testing
@testable import Luna

struct ConversationProjectTests {
    @Test
    func matchesSharedPrimaryRepositoryFixtures() throws {
        let fixture = try #require(Bundle(for: BundleToken.self).url(
            forResource: "project-grouping",
            withExtension: "json"
        ))
        let suite = try JSONDecoder().decode(
            ProjectFixtureSuite.self,
            from: Data(contentsOf: fixture)
        )

        for testCase in suite.cases {
            let conversation = makeConversation(
                repositories: testCase.repositories.map(makeRepository),
                workingDirectory: testCase.workingDirectory
            )
            #expect(primaryRepository(for: conversation)?.id == testCase.expectedRepositoryId)
        }
    }

    @Test
    func ordersProjectsByNewestConversationAndConversationsByRecency() {
        let first = makeRepository(
            ProjectRepositoryFixture(
                id: uuid(101),
                displayName: "First",
                rootPath: "/projects/first",
                active: true,
                lastSeenAt: "2026-03-20T10:00:00Z"
            )
        )
        let second = makeRepository(
            ProjectRepositoryFixture(
                id: uuid(102),
                displayName: "Second",
                rootPath: "/projects/second",
                active: true,
                lastSeenAt: "2026-03-20T10:00:00Z"
            )
        )
        let sections = conversationProjectSections([
            makeConversation(
                id: uuid(201),
                repositories: [first],
                workingDirectory: "/projects/first",
                lastMessageAt: "2026-03-20T10:00:00Z"
            ),
            makeConversation(
                id: uuid(202),
                repositories: [second],
                workingDirectory: "/projects/second",
                lastMessageAt: "2026-03-20T12:00:00Z"
            ),
            makeConversation(
                id: uuid(203),
                repositories: [first],
                workingDirectory: "/projects/first/subdirectory",
                lastMessageAt: "2026-03-20T11:00:00Z"
            ),
            makeConversation(
                id: uuid(204),
                repositories: [],
                workingDirectory: "/tmp",
                lastMessageAt: "2026-03-20T09:00:00Z"
            ),
        ])

        #expect(sections.map(\.id) == [.repository(second.id), .repository(first.id), .noProject])
        #expect(sections[1].conversations.map(\.id) == [uuid(203), uuid(201)])
    }
}

private final class BundleToken {}

private struct ProjectFixtureSuite: Decodable {
    let cases: [ProjectFixtureCase]
}

private struct ProjectFixtureCase: Decodable {
    let name: String
    let workingDirectory: String
    let repositories: [ProjectRepositoryFixture]
    let expectedRepositoryId: UUID?
}

private struct ProjectRepositoryFixture: Decodable {
    let id: UUID
    let displayName: String
    let rootPath: String
    let active: Bool
    let lastSeenAt: String
}

private func makeRepository(_ fixture: ProjectRepositoryFixture) -> Repository {
    Repository(
        id: fixture.id,
        displayName: fixture.displayName,
        rootPath: fixture.rootPath,
        branch: nil,
        active: fixture.active,
        icon: RepositoryIcon(
            repositoryId: fixture.id,
            contentUrl: nil,
            fallbackText: String(fixture.displayName.prefix(1)),
            fallbackColor: "#7287fd"
        ),
        firstSeenAt: "2026-03-20T08:00:00Z",
        lastSeenAt: fixture.lastSeenAt
    )
}

private func makeConversation(
    id: UUID = uuid(100),
    repositories: [Repository],
    workingDirectory: String,
    lastMessageAt: String? = nil
) -> Conversation {
    Conversation(
        id: id,
        title: "Conversation",
        titleMode: .automatic,
        state: .idle,
        preview: "",
        activeWorkingDirectory: workingDirectory,
        repositories: repositories,
        activities: [],
        taskList: nil,
        lastMessageAt: lastMessageAt,
        notificationTargetDeviceId: nil,
        unreadCount: 0,
        archivedAt: nil,
        createdAt: "2026-03-20T08:00:00Z",
        updatedAt: "2026-03-20T08:00:00Z",
        version: 1
    )
}

private func uuid(_ suffix: Int) -> UUID {
    UUID(uuidString: String(format: "00000000-0000-0000-0000-%012d", suffix))!
}
