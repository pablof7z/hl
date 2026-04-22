//! Mirrors `web/src/lib/ndk/config.ts`.

pub const HIGHLIGHTER_RELAY: &str = "wss://relay.highlighter.com";

pub const DEFAULT_RELAYS: &[&str] = &[
    HIGHLIGHTER_RELAY,
    "wss://relay.damus.io",
    "wss://purplepag.es",
    "wss://relay.primal.net",
];

pub const GROUP_RELAYS: &[&str] = &[HIGHLIGHTER_RELAY];

/// Relay used for outgoing `nostrconnect://` pairing. Matches Olas's choice —
/// Primal's bunker relay is the lowest-friction option because it's what
/// Primal's built-in signer expects.
pub const NOSTR_CONNECT_RELAY: &str = "wss://relay.primal.net";

/// Perms string included in our `nostrconnect://` URI. We request only the
/// kinds Highlighter actually publishes plus encryption for NIP-46 transport.
pub const DEFAULT_NOSTR_CONNECT_PERMS: &str =
    "sign_event:11,sign_event:1111,sign_event:9802,sign_event:16,nip04_encrypt,nip04_decrypt,nip44_encrypt,nip44_decrypt";
