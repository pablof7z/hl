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
                    // Phase 7 bridge: wire the two-way weak references so
                    // profile snapshots flow from the kernel observer into
                    // the live-lane store (kernel → store) and so the store
                    // can open/close kernel profile views (store → kernel).
                    // Must happen before any subscription is opened so no
                    // initial snapshot is dropped.
                    store.kernel = kernel
                    kernel.store = store

                    // Kick the kernel's session-restore loop first so the
                    // route state is available as early as possible.
                    kernel.app.dispatch(.restoreSession)

                    // Kernel-lane: ask the kernel to evaluate what's-new
                    // entries. The observer pushes a WhatsNewSnapshot back via
                    // receive() → kernel.whatsNew; onChange below reacts to it.
                    kernel.app.dispatch(.prepareWhatsNew)
                }
                .onChange(of: kernel.whatsNew) { _, snapshot in
                    guard let snapshot, snapshot.shouldPresent else { return }
                    whatsNewPresentation = WhatsNewPresentation(entries: snapshot.entries)
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
                        // Share Extension handoff — drain the App Group queue.
                        Task { await ShareQueueProcessor.drain(app: store) }
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
