import Foundation

extension HighlighterStore: NostrProfileHost {
    public func profile(forPubkey pubkey: String) -> ProfileWire? {
        guard let meta = profile(pubkeyHex: pubkey) else { return nil }
        let display = firstNonEmpty(meta.displayName, meta.name)
        let hexShort = String(pubkey.prefix(10))
        return ProfileWire(
            pubkey: pubkey,
            displayName: display,
            about: meta.about.isEmpty ? nil : meta.about,
            pictureUrl: meta.picture.isEmpty ? nil : meta.picture,
            nip05: meta.nip05.isEmpty ? nil : meta.nip05,
            npub: pubkey,
            npubShort: hexShort
        )
    }

    public func claimProfile(pubkey: String, consumerID: String) {
        requestProfile(pubkeyHex: pubkey)
    }

    public func releaseProfile(pubkey: String, consumerID: String) {}

    static func profileWire(from meta: ProfileMetadata, pubkeyHex: String) -> ProfileWire {
        let display = firstNonEmpty(meta.displayName, meta.name)
        return ProfileWire(
            pubkey: pubkeyHex,
            displayName: display,
            about: meta.about.isEmpty ? nil : meta.about,
            pictureUrl: meta.picture.isEmpty ? nil : meta.picture,
            nip05: meta.nip05.isEmpty ? nil : meta.nip05,
            npub: pubkeyHex,
            npubShort: String(pubkeyHex.prefix(10))
        )
    }

    private static func firstNonEmpty(_ values: String...) -> String? {
        values.first(where: { !$0.isEmpty })
    }
}
