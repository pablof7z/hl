import Foundation

/// Auto-login state machine. On app bootstrap, attempts to restore the saved
/// session (nsec first, bunker URI second). Mirrors TENEX's
/// `AppSessionStore.attemptAutoLogin` but simpler.
@MainActor
final class AppSessionStore {
    static let shared = AppSessionStore()
    private init() {}

    /// Returns the logged-in user if a saved credential succeeds, nil otherwise.
    func restoreSession(into core: SafeHighlighterCore) async -> CurrentUser? {
        if let nsec = KeychainService.loadNsec() {
            let snapshot = await core.loginNsec(nsec)
            if snapshot.isAuthenticated, let user = snapshot.user {
                return user
            }
            // Stale/invalid nsec — clear so we don't keep retrying.
            KeychainService.deleteNsec()
        }

        if let uri = KeychainService.loadBunkerURI() {
            let snapshot = await core.pairBunker(uri)
            if snapshot.isAuthenticated, let user = snapshot.user {
                return user
            }
            KeychainService.deleteBunkerURI()
        }

        return nil
    }

    func persistNsec(_ nsec: String) {
        _ = KeychainService.saveNsec(nsec)
    }

    func persistBunkerURI(_ uri: String) {
        _ = KeychainService.saveBunkerURI(uri)
    }

    func clear() {
        KeychainService.deleteNsec()
        KeychainService.deleteBunkerURI()
    }
}
