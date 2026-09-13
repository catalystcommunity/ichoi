import Foundation

public protocol TokenStore: Sendable {
    func token(for profileID: UUID) throws -> String?
    func save(token: String, for profileID: UUID) throws
    func removeToken(for profileID: UUID) throws
}

public enum TokenStoreError: Error { case unavailable, unexpectedStatus(Int32) }

/// Tokens are keyed by the profile UUID. A token can never be read for another server.
public final class KeychainTokenStore: TokenStore, @unchecked Sendable {
    private let service: String
    public init(service: String = "community.catalyst.ichoi.session") { self.service = service }
    public func token(for profileID: UUID) throws -> String? {
        #if canImport(Security)
        var query = baseQuery(profileID); query[kSecReturnData as String] = true; query[kSecMatchLimit as String] = kSecMatchLimitOne
        var result: AnyObject?; let status = SecItemCopyMatching(query as CFDictionary, &result)
        if status == errSecItemNotFound { return nil }; guard status == errSecSuccess, let data = result as? Data else { throw TokenStoreError.unexpectedStatus(status) }
        return String(data: data, encoding: .utf8)
        #else
        return nil
        #endif
    }
    public func save(token: String, for profileID: UUID) throws {
        #if canImport(Security)
        let data = Data(token.utf8); var query = baseQuery(profileID); query[kSecValueData as String] = data
        let status = SecItemAdd(query as CFDictionary, nil)
        if status == errSecDuplicateItem {
            let update = [kSecValueData as String: data]
            let updateStatus = SecItemUpdate(baseQuery(profileID) as CFDictionary, update as CFDictionary)
            guard updateStatus == errSecSuccess else { throw TokenStoreError.unexpectedStatus(updateStatus) }
        }
        else if status != errSecSuccess { throw TokenStoreError.unexpectedStatus(status) }
        #endif
    }
    public func removeToken(for profileID: UUID) throws {
        #if canImport(Security)
        let status = SecItemDelete(baseQuery(profileID) as CFDictionary); if status != errSecSuccess && status != errSecItemNotFound { throw TokenStoreError.unexpectedStatus(status) }
        #endif
    }
    #if canImport(Security)
    private func baseQuery(_ id: UUID) -> [String: Any] { [kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: service, kSecAttrAccount as String: id.uuidString] }
    #endif
}
