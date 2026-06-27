import SwiftUI

/// Inline display-name text for a Nostr profile.
///
/// Two construction modes:
///   - `NostrProfileName(profile:)` renders a caller-provided `ProfileWire`
///     while resolving that profile's pubkey on mount so the value stays fresh.
///   - `NostrProfileName(pubkey:)` resolves the profile reference itself,
///     reads the host projection reactively, and releases on disappear.
///
/// Display always comes from a Rust-formatted source: `displayName` when set,
/// else `npubShort`. Until the host has a profile for a self-resolving pubkey,
/// the view renders nothing rather than synthesize a Swift-side abbreviation.
///
/// Depends on `swiftui/user-avatar` for `ProfileWire` and `NostrProfileHost`.
public struct NostrProfileName: View {
    @Environment(\.nostrProfileHost) private var profileHost

    private let staticProfile: ProfileWire?
    private let pubkey: String
    public var font: Font
    public var color: Color

    @State private var generatedConsumerID: String
    @State private var resolvedPubkey: String?

    public init(
        profile: ProfileWire,
        font: Font = .headline,
        color: Color = .primary
    ) {
        self.staticProfile = profile
        self.pubkey = profile.pubkey
        self.font = font
        self.color = color
        self._generatedConsumerID = State(
            initialValue: "nostr-profile-name.\(UUID().uuidString)"
        )
        self._resolvedPubkey = State(initialValue: nil)
    }

    public init(
        pubkey: String,
        font: Font = .body,
        color: Color = .primary,
        consumerID: String? = nil
    ) {
        self.staticProfile = nil
        self.pubkey = pubkey
        self.font = font
        self.color = color
        self._generatedConsumerID = State(
            initialValue: consumerID ?? "nostr-profile-name.\(UUID().uuidString)"
        )
        self._resolvedPubkey = State(initialValue: nil)
    }

    public var body: some View {
        let resolved = staticProfile ?? profileHost?.profile(forPubkey: pubkey)
        return Group {
            if let resolved {
                label(for: resolved)
            } else {
                EmptyView()
            }
        }
        .task(id: pubkey) {
            await MainActor.run {
                if let resolvedPubkey, resolvedPubkey != pubkey {
                    profileHost?.releaseProfileRef(
                        pubkey: resolvedPubkey,
                        consumerID: generatedConsumerID
                    )
                }
                resolvedPubkey = pubkey
                profileHost?.resolveProfileRef(pubkey: pubkey, consumerID: generatedConsumerID)
            }
        }
        .onDisappear {
            if let resolvedPubkey {
                profileHost?.releaseProfileRef(
                    pubkey: resolvedPubkey,
                    consumerID: generatedConsumerID
                )
                self.resolvedPubkey = nil
            }
        }
    }

    private func label(for profile: ProfileWire) -> some View {
        Text(profile.display)
            .font(font)
            .foregroundStyle(color)
            .lineLimit(1)
            .accessibilityLabel("Display name: \(profile.display)")
    }
}
