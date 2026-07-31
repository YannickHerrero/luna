import Foundation
import Security

protocol CredentialStore: Sendable {
    func token(for server: URL) async throws -> String?
    func setToken(_ token: String, for server: URL) async throws
    func removeToken(for server: URL) async throws
}

enum CredentialStoreError: Error, Equatable, Sendable {
    case invalidTokenData
    case keychain(OSStatus)
}

actor KeychainCredentialStore: CredentialStore {
    static let shared = KeychainCredentialStore()

    private let service: String

    init(service: String = "com.yannickherrero.luna.device") {
        self.service = service
    }

    func token(for server: URL) throws -> String? {
        var query = baseQuery(for: server)
        query[kSecReturnData as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne

        var result: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        if status == errSecItemNotFound {
            return nil
        }
        guard status == errSecSuccess else {
            throw CredentialStoreError.keychain(status)
        }
        guard let data = result as? Data, let token = String(data: data, encoding: .utf8) else {
            throw CredentialStoreError.invalidTokenData
        }
        return token
    }

    func setToken(_ token: String, for server: URL) throws {
        let data = Data(token.utf8)
        let query = baseQuery(for: server)
        let attributes = [kSecValueData as String: data]
        let updateStatus = SecItemUpdate(query as CFDictionary, attributes as CFDictionary)
        if updateStatus == errSecSuccess {
            return
        }
        guard updateStatus == errSecItemNotFound else {
            throw CredentialStoreError.keychain(updateStatus)
        }

        var addition = query
        addition[kSecValueData as String] = data
        addition[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
        let addStatus = SecItemAdd(addition as CFDictionary, nil)
        guard addStatus == errSecSuccess else {
            throw CredentialStoreError.keychain(addStatus)
        }
    }

    func removeToken(for server: URL) throws {
        let status = SecItemDelete(baseQuery(for: server) as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw CredentialStoreError.keychain(status)
        }
    }

    private func baseQuery(for server: URL) -> [String: Any] {
        [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: server.absoluteString,
        ]
    }
}

actor MemoryCredentialStore: CredentialStore {
    private var tokens: [URL: String] = [:]

    func token(for server: URL) -> String? {
        tokens[server]
    }

    func setToken(_ token: String, for server: URL) {
        tokens[server] = token
    }

    func removeToken(for server: URL) {
        tokens.removeValue(forKey: server)
    }
}
