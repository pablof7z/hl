import Foundation

/// Routes inbound Highlighter share links to the right destination.
///
/// The Rust core mints `https://beta.highlighter.com/highlight/{bech32}`
/// (see `app/core/src/share_links.rs`); `highlighter://highlight/{bech32}`
/// is accepted as a custom-scheme fallback. The bech32 token is decoded by
/// the core (`decodeNostrEntity`) so npub/nprofile/note/nevent/naddr all
/// route without Swift-side NIP-19 parsing.
@MainActor
enum ShareLinkRouter {
    static let shareLinkHost = "beta.highlighter.com"
    private static let highlightPathPrefix = "highlight"

    /// Extract the bech32 entity token from a share link, or nil when the
    /// URL is not a Highlighter share link.
    static func entityToken(from url: URL) -> String? {
        if url.scheme == "https" || url.scheme == "http" {
            guard url.host?.lowercased() == shareLinkHost else { return nil }
            let parts = url.path.split(separator: "/").map(String.init)
            guard parts.count >= 2, parts[0] == highlightPathPrefix else { return nil }
            return parts[1]
        }
        if url.scheme == ShareURLScheme.scheme {
            // highlighter://highlight/{token}
            guard url.host == highlightPathPrefix else { return nil }
            let token = url.path.split(separator: "/").map(String.init).first
            return token
        }
        return nil
    }

    /// Decode and dispatch. Returns true when the URL was a share link this
    /// router consumed (even if the token failed to decode — the link was
    /// ours, there is just nowhere to go).
    @discardableResult
    static func route(_ url: URL, store: HighlighterStore) -> Bool {
        guard let token = entityToken(from: url) else { return false }
        guard let entity = store.nmpApp.decodeNostrEntity(input: token) else { return true }
        switch entity {
        case let .profile(pubkeyHex, _):
            store.openProfile(pubkeyHex: pubkeyHex)
        case let .event(eventIdHex, _, _, kindHint):
            let kind = kindHint.flatMap { UInt16(exactly: $0) } ?? 9802
            store.openComments(rootTagName: "e", rootTagValue: eventIdHex, rootKind: kind)
        case let .address(kind, pubkeyHex, dTag, _):
            guard kind == 30023 else { return true }
            store.openArticleReader(pubkeyHex: pubkeyHex, dTag: dTag, seed: nil)
        }
        return true
    }
}
