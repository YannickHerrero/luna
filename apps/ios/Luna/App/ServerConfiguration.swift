import Foundation
import Observation

enum ServerConfigurationError: Error, Equatable, LocalizedError, Sendable {
    case invalidURL
    case unsupportedScheme
    case insecureRemoteServer

    var errorDescription: String? {
        switch self {
        case .invalidURL:
            "Enter a valid Luna server URL."
        case .unsupportedScheme:
            "Luna server URLs must use HTTPS."
        case .insecureRemoteServer:
            "Only local development servers may use HTTP."
        }
    }
}

enum ServerURL {
    static let productionDefault = URL(string: "https://your-mac.example.ts.net:8447")!

    static func normalized(_ input: String) throws -> URL {
        let trimmed = input.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw ServerConfigurationError.invalidURL
        }
        let candidate = trimmed.contains("://") ? trimmed : "https://\(trimmed)"
        guard var components = URLComponents(string: candidate),
              let scheme = components.scheme?.lowercased(),
              let host = components.host,
              !host.isEmpty,
              components.user == nil,
              components.password == nil,
              components.query == nil,
              components.fragment == nil,
              components.path.isEmpty || components.path == "/"
        else {
            throw ServerConfigurationError.invalidURL
        }
        guard scheme == "https" || scheme == "http" else {
            throw ServerConfigurationError.unsupportedScheme
        }
        if scheme == "http" && !isLoopback(host) {
            throw ServerConfigurationError.insecureRemoteServer
        }
        components.scheme = scheme
        components.host = host.lowercased()
        components.path = ""
        guard let url = components.url else {
            throw ServerConfigurationError.invalidURL
        }
        return url
    }

    private static func isLoopback(_ host: String) -> Bool {
        ["localhost", "127.0.0.1", "::1"].contains(host.lowercased())
    }
}

@MainActor
@Observable
final class ServerConfiguration {
    private static let key = "luna-server-url"

    private let defaults: UserDefaults
    private(set) var serverURL: URL

    init(defaults: UserDefaults = .standard, fallback: URL? = nil) {
        self.defaults = defaults
        let processOverride = ProcessInfo.processInfo.environment["LUNA_SERVER_URL"]
            .flatMap { try? ServerURL.normalized($0) }
        let stored = defaults.string(forKey: Self.key)
            .flatMap { try? ServerURL.normalized($0) }
        serverURL = processOverride ?? stored ?? fallback ?? ServerURL.productionDefault
    }

    func update(_ input: String) throws {
        let next = try ServerURL.normalized(input)
        serverURL = next
        defaults.set(next.absoluteString, forKey: Self.key)
    }
}
