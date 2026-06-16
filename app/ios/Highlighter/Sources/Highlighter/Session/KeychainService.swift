import Foundation
import Security

/// iOS Keychain capability wrapper for the user's nsec and bunker URI.
/// Swift executes secure-storage calls; Rust owns session restore policy.
enum KeychainService {
    private static let service = "com.highlighter.app"
    private static let nsecAccount = "nsec"
    private static let bunkerAccount = "bunker-uri"

    // MARK: - Nsec

    static func saveNsec(_ nsec: String) -> Bool { save(nsec, account: nsecAccount) }
    static func loadNsec() -> String? { load(account: nsecAccount) }
    static func deleteNsec() { delete(account: nsecAccount) }

    // MARK: - Bunker URI

    static func saveBunkerURI(_ uri: String) -> Bool { save(uri, account: bunkerAccount) }
    static func loadBunkerURI() -> String? { load(account: bunkerAccount) }
    static func deleteBunkerURI() { delete(account: bunkerAccount) }

    // MARK: - Private helpers

    private static func save(_ value: String, account: String) -> Bool {
        let data = Data(value.utf8)
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account
        ]
        SecItemDelete(query as CFDictionary)
        let add = query.merging([
            kSecValueData as String: data,
            kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlock
        ]) { $1 }
        let status = SecItemAdd(add as CFDictionary, nil)
        return status == errSecSuccess
    }

    private static func load(account: String) -> String? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne
        ]
        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        guard status == errSecSuccess, let data = result as? Data else { return nil }
        return String(data: data, encoding: .utf8)
    }

    private static func delete(account: String) {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account
        ]
        SecItemDelete(query as CFDictionary)
    }
}
