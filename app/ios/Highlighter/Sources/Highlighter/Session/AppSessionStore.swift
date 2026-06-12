import Foundation
import os

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
            do {
                try KeychainService.saveNsec(nsec)
            } catch {
                Logger.highlighter.error("Failed to persist nsec credential to keychain: \(error.localizedDescription, privacy: .public)")
            }
            KeychainService.deleteBunkerURI()
        case .bunkerUri(let uri):
            do {
                try KeychainService.saveBunkerURI(uri)
            } catch {
                Logger.highlighter.error("Failed to persist bunker URI credential to keychain: \(error.localizedDescription, privacy: .public)")
            }
            KeychainService.deleteNsec()
        }
    }

    func clear() {
        KeychainService.deleteNsec()
        KeychainService.deleteBunkerURI()
    }
}
