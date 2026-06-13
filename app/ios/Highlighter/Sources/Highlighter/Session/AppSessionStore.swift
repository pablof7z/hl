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
        case .nip55SignerPackage:
            // NIP-55 (Amber / external signer) is Android-only. iOS has no signer
            // app to hand off to, so a NIP-55 package is never a usable iOS
            // credential and is not written to the iOS keychain. Clear any other
            // stored credential to keep a single source of truth, matching the
            // mutual exclusion the other cases enforce.
            Logger.highlighter.error("Ignoring NIP-55 signer-package credential on iOS: external signers are Android-only; not persisting to keychain.")
            KeychainService.deleteNsec()
            KeychainService.deleteBunkerURI()
        }
    }

    func clear() {
        KeychainService.deleteNsec()
        KeychainService.deleteBunkerURI()
    }
}
