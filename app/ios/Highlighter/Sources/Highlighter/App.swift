import SwiftUI

@main
// Renamed from `HighlighterApp` to avoid collision with the UniFFI-generated
// `open class HighlighterApp` (Phase 1 nmp-lane kernel FFI object).
struct AppEntry: App {
    @State private var store = HighlighterStore()
    @State private var kernel = HighlighterAppKernel()

    // MARK: - What's-new sheet wiring
    //
    // Evaluated once on cold launch. Uses `.sheet(item:)` rather than
    // `.sheet(isPresented:)` so the entries are bundled with the trigger —
    // avoids a SwiftUI render-race where a `fullScreenCover` (onboarding)
    // sitting on top causes the sheet's content closure to read stale entries.
    @State private var whatsNewPresentation: WhatsNewPresentation?

    @Environment(\.scenePhase) private var scenePhase

    var body: some Scene {
        WindowGroup {
            RootSceneView()
                .environment(store)
                .environment(kernel)
                .task {
                    // Bridge communities/bookmarks/profiles from the kernel into
                    // the app-scope store (Phase 7 C2). The store writes bookmark
                    // toggles back through the kernel.
                    store.kernel = kernel
                    kernel.store = store

                    // Phase 7 cutover: give PodcastPlayerStore a back-reference
                    // to the kernel so it can dispatch audio actions (play/pause/
                    // seek) without going through HighlighterStore.
                    store.podcastPlayer.kernel = kernel

                    if let communities = kernel.communities {
                        store.applyCommunitiesSnapshot(communities)
                    }
                    if let bookmarks = kernel.bookmarks {
                        store.applyBookmarksSnapshot(bookmarks)
                    }
                    if !kernel.profileSnapshots.isEmpty {
                        store.applyKernelProfiles(kernel.profileSnapshots)
                    }

                    // Kick the kernel's session-restore loop first so the
                    // route state is available as early as possible.
                    kernel.app.dispatch(.restoreSession)

                    // Phase 7 C1: prepare the what's-new sheet via the kernel.
                    // The result arrives asynchronously as a `WhatsNewSnapshot`
                    // on `kernel.whatsNew` (see `.onChange` below).
                    kernel.app.dispatch(.prepareWhatsNew)
                }
                .onChange(of: kernel.whatsNew) { _, snap in
                    guard let snap, snap.shouldPresent,
                          whatsNewPresentation == nil else { return }
                    whatsNewPresentation = WhatsNewPresentation(entries: snap.entries)
                }
                .onChange(of: kernel.communities) { _, communities in
                    guard let communities else { return }
                    store.applyCommunitiesSnapshot(communities)
                }
                .onChange(of: kernel.bookmarks) { _, bookmarks in
                    guard let bookmarks else { return }
                    store.applyBookmarksSnapshot(bookmarks)
                }
                .onChange(of: kernel.profileSnapshots) { _, profiles in
                    store.applyKernelProfiles(profiles)
                }
                // Phase 7 Part C: kernel is now the authoritative session source.
                // When the kernel signs in (restore or new login), propagate
                // CurrentUser into the store so existing per-user UI continues
                // to function without a bespoke lane call.
                .onChange(of: kernel.appRoot) { _, appRoot in
                    if appRoot.sessionPresent,
                       let hex = appRoot.activePubkeyHex,
                       store.currentUser?.pubkey != hex {
                        let npub = appRoot.activePubkeyNpub ?? hex
                        store.currentUser = CurrentUser(pubkey: hex, npub: npub)
                        Task { await store.loadAppScopeData() }
                    }
                }
                .onChange(of: scenePhase) { _, newPhase in
                    // Forward app lifecycle to the kernel (belt-and-suspenders:
                    // RootSceneView also handles the live-lane side effects).
                    switch newPhase {
                    case .active:
                        kernel.app.resume()
                    case .background, .inactive:
                        kernel.app.suspend()
                    @unknown default:
                        break
                    }
                }
                .sheet(item: $whatsNewPresentation) { presentation in
                    WhatsNewSheet(entries: presentation.entries) { entry in
                        kernel.app.dispatch(
                            .markWhatsNewSeen(shippedAtUnix: entry.shippedAtUnix)
                        )
                    }
                }
                .onOpenURL { url in
                    if ShareURLScheme.isProcessShare(url) {
                        // Share Extension handoff — drain + publish via the kernel.
                        ShareQueueProcessor.drain(app: store, kernel: kernel)
                        return
                    }
                    // highlighter://nip46 callback brings us back from a signer app.
                    // Nothing to do — the actual pairing happens on the relay
                    // subscription started in the login view.
                }
        }
    }

}

/// Bundles entries with the trigger so the `.sheet(item:)` content closure
/// receives them atomically — see the wiring note above.
private struct WhatsNewPresentation: Identifiable {
    let id = UUID()
    let entries: [WhatsNewEntryRow]
}
