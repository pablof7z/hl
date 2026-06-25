import Foundation
import Security

/// iOS Keychain capability wrapper for the user's nsec and bunker URI.
/// Swift executes secure-storage calls; Rust owns session restore policy.
enum KeychainService {
    private static let service = "com.highlighter.app"
    private static let nsecAccount = "nsec"
    private static let bunkerAccount = "bunker-uri"
    private static let nmpAccountPrefix = "nmp.keyring."

    // MARK: - Nsec

    static func saveNsec(_ nsec: String) -> Bool { save(nsec, account: nsecAccount) }
    static func loadNsec() -> String? { load(account: nsecAccount) }
    static func deleteNsec() { delete(account: nsecAccount) }

    // MARK: - Bunker URI

    static func saveBunkerURI(_ uri: String) -> Bool { save(uri, account: bunkerAccount) }
    static func loadBunkerURI() -> String? { load(account: bunkerAccount) }
    static func deleteBunkerURI() { delete(account: bunkerAccount) }

    // MARK: - NMP keyring capability

    static func handleNmpKeyringRequestJSON(_ requestJSON: String) -> String {
        guard
            let requestData = requestJSON.data(using: .utf8),
            let requestObject = try? JSONSerialization.jsonObject(with: requestData) as? [String: Any]
        else {
            return nmpEnvelope(namespace: "nmp.keyring.capability",
                               correlationID: "",
                               result: nmpResult(status: "error", osStatus: Int(errSecParam)))
        }

        let namespace = requestObject["namespace"] as? String ?? "nmp.keyring.capability"
        let correlationID = requestObject["correlation_id"] as? String ?? ""
        guard
            namespace == "nmp.keyring.capability",
            let payloadJSON = requestObject["payload_json"] as? String,
            let payloadData = payloadJSON.data(using: .utf8),
            let payload = try? JSONSerialization.jsonObject(with: payloadData) as? [String: Any],
            let op = payload["op"] as? String,
            let accountID = payload["account_id"] as? String,
            !accountID.isEmpty
        else {
            return nmpEnvelope(namespace: namespace,
                               correlationID: correlationID,
                               result: nmpResult(status: "error", osStatus: Int(errSecParam)))
        }

        let account = nmpAccountPrefix + accountID
        switch op {
        case "store":
            guard let secret = payload["secret"] as? String else {
                return nmpEnvelope(namespace: namespace,
                                   correlationID: correlationID,
                                   result: nmpResult(status: "error", osStatus: Int(errSecParam)))
            }
            let status = saveWithStatus(secret, account: account)
            return nmpEnvelope(namespace: namespace,
                               correlationID: correlationID,
                               result: status == errSecSuccess
                                   ? nmpResult(status: "ok")
                                   : nmpResult(status: "error", osStatus: Int(status)))
        case "retrieve":
            let loaded = loadWithStatus(account: account)
            if loaded.status == errSecSuccess, let secret = loaded.value {
                return nmpEnvelope(namespace: namespace,
                                   correlationID: correlationID,
                                   result: nmpResult(status: "ok", secret: secret))
            }
            if loaded.status == errSecItemNotFound {
                return nmpEnvelope(namespace: namespace,
                                   correlationID: correlationID,
                                   result: nmpResult(status: "not_found"))
            }
            return nmpEnvelope(namespace: namespace,
                               correlationID: correlationID,
                               result: nmpResult(status: "error", osStatus: Int(loaded.status)))
        case "delete":
            let status = deleteWithStatus(account: account)
            return nmpEnvelope(namespace: namespace,
                               correlationID: correlationID,
                               result: (status == errSecSuccess || status == errSecItemNotFound)
                                   ? nmpResult(status: "ok")
                                   : nmpResult(status: "error", osStatus: Int(status)))
        default:
            return nmpEnvelope(namespace: namespace,
                               correlationID: correlationID,
                               result: nmpResult(status: "error", osStatus: Int(errSecParam)))
        }
    }

    // MARK: - Private helpers

    private static func save(_ value: String, account: String) -> Bool {
        saveWithStatus(value, account: account) == errSecSuccess
    }

    private static func saveWithStatus(_ value: String, account: String) -> OSStatus {
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
        return SecItemAdd(add as CFDictionary, nil)
    }

    private static func load(account: String) -> String? {
        let result = loadWithStatus(account: account)
        guard result.status == errSecSuccess else { return nil }
        return result.value
    }

    private static func loadWithStatus(account: String) -> (status: OSStatus, value: String?) {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne
        ]
        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        guard status == errSecSuccess, let data = result as? Data else {
            return (status, nil)
        }
        return (status, String(data: data, encoding: .utf8))
    }

    private static func delete(account: String) {
        _ = deleteWithStatus(account: account)
    }

    private static func deleteWithStatus(account: String) -> OSStatus {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account
        ]
        return SecItemDelete(query as CFDictionary)
    }

    private static func nmpResult(status: String, secret: String? = nil, osStatus: Int? = nil) -> String {
        var result: [String: Any] = ["status": status]
        if let secret {
            result["secret"] = secret
        }
        if let osStatus {
            result["os_status"] = osStatus
        }
        return jsonString(result)
    }

    private static func nmpEnvelope(namespace: String, correlationID: String, result: String) -> String {
        jsonString([
            "namespace": namespace,
            "correlation_id": correlationID,
            "result_json": result
        ])
    }

    private static func jsonString(_ object: [String: Any]) -> String {
        guard
            JSONSerialization.isValidJSONObject(object),
            let data = try? JSONSerialization.data(withJSONObject: object, options: []),
            let string = String(data: data, encoding: .utf8)
        else {
            return "{}"
        }
        return string
    }
}
