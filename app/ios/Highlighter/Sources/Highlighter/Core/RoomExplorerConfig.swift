import Foundation

/// Configuration for the rooms explorer's curated shelves. The curator
/// pubkey signs NIP-51 kind:10012 lists that drive the "Featured" row; in
/// production this is `relay.highlighter.com`'s own pubkey, so the relay
/// admin UI ("croissant") is the single source of editorial curation.
///
/// The pubkey is discovered at runtime from the relay's NIP-11 document
/// the first time the explorer appears, then cached in `UserDefaults` so
/// later appearances don't re-fetch. An empty cached value means the
/// document hasn't been retrieved yet — the featured shelf will be empty
/// in that case and populates on the next appear.
enum RoomExplorerConfig {
    static let curatorRelayURL = URL(string: "https://relay.highlighter.com")!

    private static let cachedCuratorKey = "highlighter.explorer.curatorPubkeyHex"

    /// The curator pubkey cached from a previous NIP-11 fetch. Empty string
    /// until the first successful fetch completes.
    static var cachedCuratorPubkeyHex: String {
        get { UserDefaults.standard.string(forKey: cachedCuratorKey) ?? "" }
        set { UserDefaults.standard.set(newValue, forKey: cachedCuratorKey) }
    }

    /// Fetch the curator relay's NIP-11 info document and return its pubkey.
    /// Caches the result in `UserDefaults` on success. Returns `nil` on any
    /// failure (network, malformed JSON, missing pubkey field); callers
    /// should treat that as "featured shelf unavailable this session".
    static func fetchCuratorPubkey() async -> String? {
        var request = URLRequest(url: curatorRelayURL)
        request.setValue("application/nostr+json", forHTTPHeaderField: "Accept")
        do {
            let (data, _) = try await URLSession.shared.data(for: request)
            guard
                let object = try JSONSerialization.jsonObject(with: data) as? [String: Any],
                let pubkey = object["pubkey"] as? String,
                !pubkey.isEmpty
            else {
                return nil
            }
            cachedCuratorPubkeyHex = pubkey
            return pubkey
        } catch {
            return nil
        }
    }
}
