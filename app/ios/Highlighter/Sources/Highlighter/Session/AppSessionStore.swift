import Foundation

/// Keychain-backed session capability. Rust owns restore policy; iOS only
/// supplies saved secrets and executes explicit cleanup instructions.
@MainActor
final class AppSessionStore {
    static let shared = AppSessionStore()
    private init() {}

    /// Returns the logged-in user if a saved credential succeeds, nil otherwise.
    func restoreSession(into core: SafeHighlighterCore) async -> CurrentUser? {
        let snapshot = await core.restoreSessionSnapshot(
            nsec: KeychainService.loadNsec(),
            bunkerUri: KeychainService.loadBunkerURI()
        )

        if snapshot.clearNsec {
            KeychainService.deleteNsec()
        }
        if snapshot.clearBunkerUri {
            KeychainService.deleteBunkerURI()
        }

        return snapshot.isAuthenticated ? snapshot.user : nil
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
