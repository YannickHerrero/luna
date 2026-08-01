import Foundation
import Observation
import WatchConnectivity
import WidgetKit

@MainActor
@Observable
final class WatchSnapshotReceiver: NSObject {
    private(set) var snapshot: ActiveAgentsSnapshot?
    private(set) var isPhoneReachable = false
    private(set) var isSessionActivated = false

    @ObservationIgnored private let store: LunaSnapshotStore
    @ObservationIgnored private var session: WCSession?

    init(
        store: LunaSnapshotStore = LunaSnapshotStore(),
        activateSession: Bool = true
    ) {
        self.store = store
        snapshot = try? store.readActiveAgents()
        super.init()
        guard activateSession, WCSession.isSupported() else { return }
        let session = WCSession.default
        self.session = session
        session.delegate = self
        session.activate()
    }

    func installApplicationContext(_ context: [String: Any]) {
        guard let data = context[LunaAppGroup.watchActiveAgentsContextKey] as? Data else { return }
        install(data)
    }

    private func install(_ data: Data) {
        guard let snapshot = try? LunaSnapshotCodec.decodeActiveAgents(data) else { return }
        do {
            try store.writeActiveAgents(snapshot)
            self.snapshot = snapshot
            WidgetCenter.shared.reloadTimelines(
                ofKind: LunaAppGroup.watchActiveAgentsWidgetKind
            )
        } catch {
            // Keep the last valid Watch snapshot when the group container is unavailable.
        }
    }
}

extension WatchSnapshotReceiver: WCSessionDelegate {
    nonisolated func session(
        _ session: WCSession,
        activationDidCompleteWith activationState: WCSessionActivationState,
        error: (any Error)?
    ) {
        let data = session.receivedApplicationContext[
            LunaAppGroup.watchActiveAgentsContextKey
        ] as? Data
        let reachable = session.isReachable
        Task { @MainActor [weak self] in
            guard let self else { return }
            isSessionActivated = activationState == .activated
            isPhoneReachable = reachable
            if let data { install(data) }
        }
    }

    nonisolated func session(
        _ session: WCSession,
        didReceiveApplicationContext applicationContext: [String: Any]
    ) {
        guard let data = applicationContext[
            LunaAppGroup.watchActiveAgentsContextKey
        ] as? Data else { return }
        Task { @MainActor [weak self] in
            self?.install(data)
        }
    }

    nonisolated func sessionReachabilityDidChange(_ session: WCSession) {
        let reachable = session.isReachable
        Task { @MainActor [weak self] in
            self?.isPhoneReachable = reachable
        }
    }
}
