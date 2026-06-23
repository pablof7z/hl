import Foundation

/// Keychain-backed session capability. Rust owns restore policy; iOS only
/// supplies saved secrets and executes explicit cleanup instructions.
@MainActor
final class AppSessionStore {
    static let shared = AppSessionStore()
    private init() {}

    /// Returns the logged-in user if a saved credential succeeds, nil otherwise.
    func restoreSession(into core: HighlighterCore) async -> CurrentUser? {
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

    func persistAuthInstructions(
        _ snapshot: AuthSessionSnapshot,
        core: HighlighterCore
    ) -> SessionStorageWriteSnapshot {
        var nsecSucceeded = false
        var bunkerSucceeded = false
        if let nsec = snapshot.persistNsec {
            nsecSucceeded = KeychainService.saveNsec(nsec)
        }
        if let uri = snapshot.persistBunkerUri {
            bunkerSucceeded = KeychainService.saveBunkerURI(uri)
        }
        return core.projectSessionStorageWrite(input: SessionStorageWriteInput(
            nsecRequested: snapshot.persistNsec != nil,
            nsecSucceeded: nsecSucceeded,
            bunkerUriRequested: snapshot.persistBunkerUri != nil,
            bunkerUriSucceeded: bunkerSucceeded
        ))
    }

    func persistAccountInstructions(
        _ snapshot: AccountGenerationSnapshot,
        core: HighlighterCore
    ) -> SessionStorageWriteSnapshot {
        var nsecSucceeded = false
        if let nsec = snapshot.persistNsec {
            nsecSucceeded = KeychainService.saveNsec(nsec)
        }
        return core.projectSessionStorageWrite(input: SessionStorageWriteInput(
            nsecRequested: snapshot.persistNsec != nil,
            nsecSucceeded: nsecSucceeded,
            bunkerUriRequested: false,
            bunkerUriSucceeded: false
        ))
    }

    func clear() {
        KeychainService.deleteNsec()
        KeychainService.deleteBunkerURI()
    }
}
