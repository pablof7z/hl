import Kingfisher
import SwiftUI

@main
struct HighlighterApp: App {
    @State private var store = HighlighterStore()

    init() {
        Self.configureImageCache()
    }

    var body: some Scene {
        WindowGroup {
            RootSceneView()
                .environment(store)
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

    private static func configureImageCache() {
        let cache = ImageCache.default
        cache.memoryStorage.config.totalCostLimit = 150 * 1024 * 1024   // 150 MB
        cache.memoryStorage.config.countLimit = 300
        cache.diskStorage.config.sizeLimit = 750 * 1024 * 1024          // 750 MB
        cache.diskStorage.config.expiration = .days(14)

        KingfisherManager.shared.downloader.downloadTimeout = 20
    }
}
