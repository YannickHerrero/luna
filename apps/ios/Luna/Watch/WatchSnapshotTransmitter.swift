import Foundation
import WatchConnectivity

@MainActor
final class WatchSnapshotTransmitter: NSObject {
    static let shared = WatchSnapshotTransmitter()

    private var session: WCSession?
    private var pendingData: Data?

    override init() {
        super.init()
        guard WCSession.isSupported() else { return }
        let session = WCSession.default
        self.session = session
        session.delegate = self
        session.activate()
    }

    func send(_ snapshot: ActiveAgentsSnapshot) {
        guard let data = try? LunaSnapshotCodec.encodeActiveAgents(snapshot) else { return }
        pendingData = data
        flush()
    }

    private func flush() {
        guard let session,
              session.activationState == .activated,
              session.isPaired,
              session.isWatchAppInstalled,
              let pendingData
        else {
            session?.activate()
            return
        }
        do {
            try session.updateApplicationContext([
                LunaAppGroup.watchActiveAgentsContextKey: pendingData,
            ])
            self.pendingData = nil
        } catch {
            // Watch delivery is opportunistic and must not interrupt the iOS client.
        }
    }
}

extension WatchSnapshotTransmitter: WCSessionDelegate {
    nonisolated func session(
        _ session: WCSession,
        activationDidCompleteWith activationState: WCSessionActivationState,
        error: (any Error)?
    ) {
        Task { @MainActor [weak self] in
            self?.flush()
        }
    }

    nonisolated func sessionDidBecomeInactive(_ session: WCSession) {}

    nonisolated func sessionDidDeactivate(_ session: WCSession) {
        Task { @MainActor [weak self] in
            self?.session?.activate()
        }
    }

    nonisolated func sessionWatchStateDidChange(_ session: WCSession) {
        Task { @MainActor [weak self] in
            self?.flush()
        }
    }
}
