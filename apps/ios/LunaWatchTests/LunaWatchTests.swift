import Foundation
import Testing
@testable import LunaWatch

struct LunaWatchTests {
    @Test
    func companionPresentationHandlesCurrentStaleEmptyAndUnavailableStates() {
        let date = Date(timeIntervalSince1970: 1_700_000_000)
        let agent = watchAgent(date: date)
        let current = ActiveAgentsSnapshot(generatedAt: date, agents: [agent])
        let stale = ActiveAgentsSnapshot(
            generatedAt: date.addingTimeInterval(-30 * 60),
            agents: [agent]
        )
        let empty = ActiveAgentsSnapshot(generatedAt: date, agents: [])

        #expect(
            LunaWatchPresentation.make(
                snapshot: current,
                date: date,
                isPhoneReachable: true
            ).status == "1 active agent"
        )
        #expect(
            LunaWatchPresentation.make(
                snapshot: stale,
                date: date,
                isPhoneReachable: false
            ).detail == "iPhone unreachable · 30m ago"
        )
        #expect(
            LunaWatchPresentation.make(
                snapshot: empty,
                date: date,
                isPhoneReachable: true
            ).status == LunaWatchCopy.empty
        )
        #expect(
            LunaWatchPresentation.make(
                snapshot: nil,
                date: date,
                isPhoneReachable: false
            ).status == LunaWatchCopy.unavailable
        )
    }

    @Test
    func workPulseCapsSegmentsButKeepsTheExactCount() {
        let date = Date(timeIntervalSince1970: 1_700_000_000)
        let snapshot = ActiveAgentsSnapshot(
            generatedAt: date,
            agents: (1...5).map { watchAgent(id: $0, date: date) }
        )
        let current = LunaWatchWidgetEntry(date: date, snapshot: snapshot)
        let stale = LunaWatchWidgetEntry(
            date: date.addingTimeInterval(30 * 60),
            snapshot: snapshot
        )
        let unavailable = LunaWatchWidgetEntry(
            date: date.addingTimeInterval(25 * 60 * 60),
            snapshot: snapshot
        )

        #expect(current.filledSegmentCount == 4)
        #expect(current.countLabel == "4+")
        #expect(current.label == "WORKING · 5 ACTIVE")
        #expect(stale.isStale)
        #expect(stale.title == "iPhone unreachable")
        #expect(unavailable.freshness == .unavailable)
        #expect(unavailable.countLabel == "?")
    }

    @Test @MainActor
    func receiverPersistsOnlyValidApplicationContexts() throws {
        let directory = FileManager.default.temporaryDirectory
            .appending(path: UUID().uuidString, directoryHint: .isDirectory)
        defer { try? FileManager.default.removeItem(at: directory) }
        let store = LunaSnapshotStore(directoryURL: directory)
        let receiver = WatchSnapshotReceiver(store: store, activateSession: false)
        let snapshot = ActiveAgentsSnapshot(
            generatedAt: Date(timeIntervalSince1970: 1_700_000_000),
            agents: [watchAgent(date: Date(timeIntervalSince1970: 1_700_000_000))]
        )

        receiver.installApplicationContext([
            LunaAppGroup.watchActiveAgentsContextKey:
                try LunaSnapshotCodec.encodeActiveAgents(snapshot),
        ])
        #expect(receiver.snapshot == snapshot)
        #expect(try store.readActiveAgents() == snapshot)

        receiver.installApplicationContext([
            LunaAppGroup.watchActiveAgentsContextKey: Data("not-json".utf8),
        ])
        #expect(receiver.snapshot == snapshot)
    }
}

private func watchAgent(id: Int = 1, date: Date) -> ActiveAgentSnapshot {
    ActiveAgentSnapshot(
        id: UUID(
            uuidString: String(format: "00000000-0000-0000-0000-%012d", id)
        )!,
        title: "Prepare release notes",
        state: .working,
        activity: "Reviewing files",
        updatedAt: date
    )
}
