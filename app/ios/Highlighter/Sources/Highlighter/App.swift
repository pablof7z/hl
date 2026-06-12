import SwiftUI

@main
struct HighlighterApp: App {
    @State private var store = HighlighterStore()

    var body: some Scene {
        WindowGroup {
            RootSceneView()
                .environment(store)
                .sheet(isPresented: whatsNewPresented) {
                    WhatsNewSheet(
                        entries: store.nmpState.whatsNew.entries,
                        onDismiss: { store.dismissWhatsNew() }
                    )
                }
                .onOpenURL { url in
                    if ShareURLScheme.isProcessShare(url) {
                        // Share Extension handoff — drain the App Group queue.
                        Task { await ShareQueueProcessor.drain(app: store) }
                        return
                    }
                    if ShareLinkRouter.route(url, store: store) {
                        return
                    }
                    // highlighter://nip46 callback brings us back from a signer app.
                    // Nothing to do — the actual pairing happens on the relay
                    // subscription started in the login view.
                }
                .onContinueUserActivity(NSUserActivityTypeBrowsingWeb) { activity in
                    // Universal link (https://beta.highlighter.com/highlight/…).
                    // Requires the associated-domains entitlement plus an
                    // apple-app-site-association file served by the domain.
                    if let url = activity.webpageURL {
                        ShareLinkRouter.route(url, store: store)
                    }
                }
        }
    }

    private var whatsNewPresented: Binding<Bool> {
        Binding(
            get: { !store.nmpState.whatsNew.entries.isEmpty },
            set: { isPresented in
                if !isPresented {
                    store.dismissWhatsNew()
                }
            }
        )
    }
}
