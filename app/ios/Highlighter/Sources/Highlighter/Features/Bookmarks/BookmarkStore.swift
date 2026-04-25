import Foundation
import Observation

@MainActor
@Observable
final class BookmarkStore {
    var articles: [ArticleRecord] = []
    var isLoading = false

    func load(addresses: Set<String>, core: SafeHighlighterCore) async {
        guard !addresses.isEmpty else {
            articles = []
            return
        }
        isLoading = true
        defer { isLoading = false }

        var loaded: [ArticleRecord] = []
        for address in addresses {
            let parts = address.split(separator: ":", maxSplits: 2, omittingEmptySubsequences: false)
            guard parts.count == 3, parts[0] == "30023" else { continue }
            let pubkey = String(parts[1])
            let dTag = String(parts[2])
            guard !pubkey.isEmpty, !dTag.isEmpty else { continue }
            if let article = try? await core.getArticle(pubkeyHex: pubkey, dTag: dTag) {
                loaded.append(article)
            }
        }
        articles = loaded.sorted {
            ($0.publishedAt ?? $0.createdAt ?? 0) > ($1.publishedAt ?? $1.createdAt ?? 0)
        }
    }
}
