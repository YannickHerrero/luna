import Foundation
import UIKit
@preconcurrency import UserNotifications

@MainActor
protocol NotificationCoordinating: AnyObject {
    func appDidBecomeReady(_ model: AppModel)
    func refreshRegistration() async
}

@MainActor
final class NoopNotificationCoordinator: NotificationCoordinating {
    static let shared = NoopNotificationCoordinator()

    private init() {}

    func appDidBecomeReady(_ model: AppModel) {}
    func refreshRegistration() async {}
}

@MainActor
protocol NotificationAuthorizationClient: AnyObject {
    func authorizationStatus() async -> UNAuthorizationStatus
    func requestAuthorization() async throws -> Bool
}

@MainActor
final class SystemNotificationAuthorizationClient: NotificationAuthorizationClient {
    private let center: UNUserNotificationCenter

    init(center: UNUserNotificationCenter = .current()) {
        self.center = center
    }

    func authorizationStatus() async -> UNAuthorizationStatus {
        await center.notificationSettings().authorizationStatus
    }

    func requestAuthorization() async throws -> Bool {
        try await center.requestAuthorization(options: [.alert, .badge, .sound])
    }
}

@MainActor
final class NotificationCoordinator: NSObject, NotificationCoordinating,
    UNUserNotificationCenterDelegate
{
    static let shared = NotificationCoordinator()

    private struct RegisteredToken: Equatable {
        let serverURL: URL
        let value: String
    }

    private weak var model: AppModel?
    private let authorization: any NotificationAuthorizationClient
    private let registerWithSystem: @MainActor () -> Void
    private let topic: @MainActor () -> String
    private let appVersion: @MainActor () -> String?
    private var deviceToken: String?
    private var registeredToken: RegisteredToken?
    private var disabledServerURL: URL?
    private var pendingResponseURL: URL?

    init(
        authorization: any NotificationAuthorizationClient = SystemNotificationAuthorizationClient(),
        registerWithSystem: @escaping @MainActor () -> Void = {
            UIApplication.shared.registerForRemoteNotifications()
        },
        topic: @escaping @MainActor () -> String = {
            Bundle.main.bundleIdentifier ?? "com.yannickherrero.luna"
        },
        appVersion: @escaping @MainActor () -> String? = {
            Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String
        }
    ) {
        self.authorization = authorization
        self.registerWithSystem = registerWithSystem
        self.topic = topic
        self.appVersion = appVersion
        super.init()
    }

    func configure(center: UNUserNotificationCenter = .current()) {
        center.delegate = self
    }

    func appDidBecomeReady(_ model: AppModel) {
        self.model = model
        if let pendingResponseURL {
            self.pendingResponseURL = nil
            Task { await model.open(pendingResponseURL) }
        }
        Task { await refreshRegistration() }
    }

    func refreshRegistration() async {
        guard let model, model.phase == .ready else { return }
        switch await authorization.authorizationStatus() {
        case .notDetermined:
            do {
                if try await authorization.requestAuthorization() {
                    disabledServerURL = nil
                    registerWithSystem()
                } else {
                    await disableRegistration(for: model)
                }
            } catch {
                // Authorization can be requested again on the next foreground activation.
            }
        case .authorized, .provisional, .ephemeral:
            disabledServerURL = nil
            if let deviceToken {
                await register(token: deviceToken, with: model)
            }
            registerWithSystem()
        case .denied:
            await disableRegistration(for: model)
        @unknown default:
            break
        }
    }

    func didRegisterForRemoteNotifications(deviceToken: Data) {
        let token = deviceToken.map { String(format: "%02x", $0) }.joined()
        guard !token.isEmpty else { return }
        self.deviceToken = token
        guard let model, model.phase == .ready else { return }
        Task { await register(token: token, with: model) }
    }

    func didFailToRegisterForRemoteNotifications() {
        // Keep the server registration: failures can be transient and must not disable delivery.
    }

    nonisolated func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        willPresent notification: UNNotification,
        withCompletionHandler completionHandler: @escaping @Sendable (UNNotificationPresentationOptions) -> Void
    ) {
        let conversationID = Self.conversationID(
            in: notification.request.content.userInfo
        )
        Task { @MainActor [weak self] in
            let selectedConversationID = self?.model?.conversationStore?.selectedConversationId
            let shouldSuppress = Self.shouldSuppress(
                conversationID: conversationID,
                selectedConversationID: selectedConversationID
            )
            completionHandler(shouldSuppress ? [] : [.banner, .list, .sound])
        }
    }

    nonisolated func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        didReceive response: UNNotificationResponse,
        withCompletionHandler completionHandler: @escaping @Sendable () -> Void
    ) {
        let url = Self.routeURL(in: response.notification.request.content.userInfo)
        Task { @MainActor [weak self] in
            defer { completionHandler() }
            guard let self, let url else { return }
            guard let model = self.model else {
                self.pendingResponseURL = url
                return
            }
            await model.open(url)
        }
    }

    nonisolated static func shouldSuppress(
        conversationID: UUID?,
        selectedConversationID: UUID?
    ) -> Bool {
        conversationID != nil && conversationID == selectedConversationID
    }

    nonisolated static func conversationID(
        in userInfo: [AnyHashable: Any]
    ) -> UUID? {
        if let value = userInfo["conversationId"] as? String,
           let id = UUID(uuidString: value)
        {
            return id
        }
        guard let url = routeURL(in: userInfo),
              case let .conversation(id) = LunaRoute(url: url)
        else {
            return nil
        }
        return id
    }

    nonisolated static func routeURL(in userInfo: [AnyHashable: Any]) -> URL? {
        if let value = userInfo["url"] as? String,
           let url = URL(string: value),
           LunaRoute(url: url) != nil
        {
            return url
        }
        if let value = userInfo["conversationId"] as? String,
           let id = UUID(uuidString: value)
        {
            return LunaRoute.conversation(id).url
        }
        return nil
    }

    private func register(token: String, with model: AppModel) async {
        let key = RegisteredToken(serverURL: model.configuration.serverURL, value: token)
        guard registeredToken != key else { return }
        do {
            _ = try await model.registerAPNsToken(
                token,
                environment: Self.environment,
                topic: topic(),
                appVersion: appVersion()
            )
            registeredToken = key
            disabledServerURL = nil
        } catch {
            // A later foreground activation or APNs callback retries registration.
        }
    }

    private func disableRegistration(for model: AppModel) async {
        let serverURL = model.configuration.serverURL
        guard disabledServerURL != serverURL else { return }
        do {
            _ = try await model.disableAPNsRegistration()
            disabledServerURL = serverURL
            if registeredToken?.serverURL == serverURL {
                registeredToken = nil
            }
        } catch {
            // Retry when the app next becomes active.
        }
    }

    private static var environment: ApnsEnvironment {
#if DEBUG
        .sandbox
#else
        .production
#endif
    }
}

@MainActor
final class LunaApplicationDelegate: NSObject, UIApplicationDelegate {
    func application(
        _ application: UIApplication,
        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]? = nil
    ) -> Bool {
        NotificationCoordinator.shared.configure()
        return true
    }

    func application(
        _ application: UIApplication,
        didRegisterForRemoteNotificationsWithDeviceToken deviceToken: Data
    ) {
        NotificationCoordinator.shared.didRegisterForRemoteNotifications(
            deviceToken: deviceToken
        )
    }

    func application(
        _ application: UIApplication,
        didFailToRegisterForRemoteNotificationsWithError error: Error
    ) {
        NotificationCoordinator.shared.didFailToRegisterForRemoteNotifications()
    }
}
