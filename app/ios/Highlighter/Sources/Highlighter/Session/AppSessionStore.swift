import Foundation

/// Keychain capability for session credentials. Rust owns authentication
/// policy; Swift only loads, stores, and clears raw credentials.
@MainActor
final class AppSessionStore {
    static let shared = AppSessionStore()
    private init() {}

    func storedCredential() -> HighlighterSessionCredential? {
        if let nsec = KeychainService.loadNsec() {
            return .nsec(nsec: nsec)
        }

        if let uri = KeychainService.loadBunkerURI() {
            return .bunkerUri(uri: uri)
        }

        return nil
    }

    func persist(_ credential: HighlighterSessionCredential) {
        switch credential {
        case .nsec(let nsec):
            try? KeychainService.saveNsec(nsec)
            KeychainService.deleteBunkerURI()
        case .bunkerUri(let uri):
            try? KeychainService.saveBunkerURI(uri)
            KeychainService.deleteNsec()
        }
    }

    func clear() {
        KeychainService.deleteNsec()
        KeychainService.deleteBunkerURI()
    }
}
