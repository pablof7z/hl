//! Parsing + resolution for `nostr:` URI entities embedded in event
//! content (kind:1 notes, kind:30023 article bodies, kind:0 profile
//! about text, kind:11 discussions, kind:9 chat messages, …).
//!
//! Three primitives the iOS + web layers share:
//!
//! - [`decode_nostr_entity`] — classify a bech32 identifier (`npub1…`,
//!   `nprofile1…`, `note1…`, `nevent1…`, `naddr1…`) into a
//!   [`NostrEntityRef`] enum Swift can `switch` on. `nostr:` URI
//!   prefixes and whitespace are stripped.
//! - [`resolve_from_cache`] — if the referenced event is already in
//!   nostrdb, return a [`NostrEntityEvent`] the UI can render
//!   directly. Returns `None` when the cache is cold.
//! - The subscription side (`spawn_entity_backfill` over in
//!   `nostr_runtime`) installs a one-shot REQ on the indexer pool + any
//!   relay hints carried by the entity, so a cold cache warms up
//!   without blocking the first paint.

use nostr_sdk::nips::nip19::{FromBech32, Nip19, Nip19Coordinate, Nip19Event, Nip19Profile};
use nostr_sdk::prelude::*;
use nostrdb::{Filter as NdbFilter, Ndb, Transaction};

use crate::errors::CoreError;

/// Parsed reference to a Nostr entity encoded as a NIP-19 bech32.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum NostrEntityRef {
    /// `npub1…` / `nprofile1…` — reference to a user's profile.
    Profile {
        pubkey_hex: String,
        relays: Vec<String>,
    },
    /// `note1…` / `nevent1…` — reference to a specific event by id.
    Event {
        event_id_hex: String,
        relays: Vec<String>,
        /// `nevent` can carry a hinted author pubkey. `None` for `note1…`.
        author_hint_hex: Option<String>,
        /// `nevent` can carry a hinted kind so the UI can pick a
        /// renderer skeleton (article card vs. note card vs. highlight
        /// quote) before the actual event lands. `None` when absent.
        kind_hint: Option<u32>,
    },
    /// `naddr1…` — reference to a parameterised replaceable event
    /// (kind:30xxx with a `d` tag).
    Address {
        kind: u32,
        pubkey_hex: String,
        d_tag: String,
        relays: Vec<String>,
    },
}

/// Resolved event data for a [`NostrEntityRef`]. Returned by
/// [`resolve_from_cache`] when the underlying event is already in
/// nostrdb; the Swift layer switches on `kind` to pick the right
/// inline card (30023 → article, 1 → note, 9802 → highlight quote,
/// etc.) and falls back to a generic "Event <id>" rendering otherwise.
#[derive(Debug, Clone, uniffi::Record)]
pub struct NostrEntityEvent {
    pub event_id_hex: String,
    pub kind: u32,
    pub pubkey_hex: String,
    pub content: String,
    pub created_at: u64,
    /// Serialised `[["k", "v"], …]` so Swift can extract `title` /
    /// `image` etc. for an article card without needing a second FFI
    /// record schema per kind.
    pub tags_json: String,
}

/// Accept the bare bech32 (`npub1…`) and the `nostr:` URI form
/// (`nostr:npub1…`). Whitespace is trimmed. Returns the classified
/// entity ref.
pub fn decode_nostr_entity(input: &str) -> Result<NostrEntityRef, CoreError> {
    let trimmed = input
        .trim()
        .strip_prefix("nostr:")
        .unwrap_or(input.trim())
        .trim();
    if trimmed.is_empty() {
        return Err(CoreError::InvalidInput("empty nostr entity".into()));
    }

    let decoded = Nip19::from_bech32(trimmed)
        .map_err(|e| CoreError::InvalidInput(format!("bad nostr entity: {e}")))?;

    Ok(match decoded {
        Nip19::Pubkey(pk) => NostrEntityRef::Profile {
            pubkey_hex: pk.to_hex(),
            relays: Vec::new(),
        },
        Nip19::Profile(Nip19Profile {
            public_key, relays, ..
        }) => NostrEntityRef::Profile {
            pubkey_hex: public_key.to_hex(),
            relays: relays.into_iter().map(|u| u.to_string()).collect(),
        },
        Nip19::EventId(id) => NostrEntityRef::Event {
            event_id_hex: id.to_hex(),
            relays: Vec::new(),
            author_hint_hex: None,
            kind_hint: None,
        },
        Nip19::Event(Nip19Event {
            event_id,
            author,
            kind,
            relays,
        }) => NostrEntityRef::Event {
            event_id_hex: event_id.to_hex(),
            relays: relays.into_iter().map(|u| u.to_string()).collect(),
            author_hint_hex: author.map(|pk| pk.to_hex()),
            kind_hint: kind.map(|k| k.as_u16() as u32),
        },
        Nip19::Coordinate(Nip19Coordinate {
            coordinate, relays, ..
        }) => NostrEntityRef::Address {
            kind: coordinate.kind.as_u16() as u32,
            pubkey_hex: coordinate.public_key.to_hex(),
            d_tag: coordinate.identifier,
            relays: relays.into_iter().map(|u| u.to_string()).collect(),
        },
        // Secret/encrypted-secret should never appear in rendered content.
        // Treat as invalid so the renderer doesn't accidentally leak them.
        _ => {
            return Err(CoreError::InvalidInput(
                "nostr entity type not renderable".into(),
            ))
        }
    })
}

/// Look up a [`NostrEntityRef`] in nostrdb and return a renderable
/// [`NostrEntityEvent`] if present. `None` means the cache doesn't
/// have it yet — the caller should then subscribe via
/// `spawn_entity_backfill` and re-resolve on delta.
pub fn resolve_from_cache(
    ndb: &Ndb,
    entity: &NostrEntityRef,
) -> Result<Option<NostrEntityEvent>, CoreError> {
    match entity {
        NostrEntityRef::Profile { pubkey_hex, .. } => {
            resolve_replaceable(ndb, 0, pubkey_hex, None)
        }
        NostrEntityRef::Event { event_id_hex, .. } => resolve_by_event_id(ndb, event_id_hex),
        NostrEntityRef::Address {
            kind,
            pubkey_hex,
            d_tag,
            ..
        } => resolve_replaceable(ndb, *kind, pubkey_hex, Some(d_tag.as_str())),
    }
}

fn resolve_by_event_id(ndb: &Ndb, id_hex: &str) -> Result<Option<NostrEntityEvent>, CoreError> {
    let id = EventId::from_hex(id_hex)
        .map_err(|e| CoreError::InvalidInput(format!("bad event id: {e}")))?;
    let txn = Transaction::new(ndb)
        .map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;
    let id_bytes: [u8; 32] = id.to_bytes();
    let filter = NdbFilter::new().ids([&id_bytes]).build();
    let results = ndb
        .query(&txn, &[filter], 1)
        .map_err(|e| CoreError::Cache(format!("query event id: {e}")))?;
    for r in &results {
        if let Some(event) = event_from_note(ndb, &txn, r.note_key) {
            return Ok(Some(to_entity_event(&event)));
        }
    }
    Ok(None)
}

/// kind:0 or kind:30xxx (parameterised replaceable) lookup. For
/// addressable events `d_tag` must be `Some`; for kind:0 it's `None`.
fn resolve_replaceable(
    ndb: &Ndb,
    kind: u32,
    pubkey_hex: &str,
    d_tag: Option<&str>,
) -> Result<Option<NostrEntityEvent>, CoreError> {
    let author = PublicKey::from_hex(pubkey_hex)
        .map_err(|e| CoreError::InvalidInput(format!("bad pubkey: {e}")))?;
    let txn = Transaction::new(ndb)
        .map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;
    let pk_bytes: [u8; 32] = author.to_bytes();
    let mut builder = NdbFilter::new()
        .kinds([kind as u64])
        .authors([&pk_bytes]);
    if let Some(d) = d_tag {
        builder = builder.tags([d], 'd');
    }
    let filter = builder.build();
    let results = ndb
        .query(&txn, &[filter], 16)
        .map_err(|e| CoreError::Cache(format!("query replaceable: {e}")))?;
    // Parameterised replaceable events dedup on (kind, pubkey, d_tag) —
    // keep the newest.
    let mut newest: Option<Event> = None;
    for r in &results {
        let Some(event) = event_from_note(ndb, &txn, r.note_key) else {
            continue;
        };
        if let Some(d) = d_tag {
            if event_d_tag(&event).as_deref() != Some(d) {
                continue;
            }
        }
        newest = Some(match newest {
            Some(prev) if prev.created_at >= event.created_at => prev,
            _ => event,
        });
    }
    Ok(newest.as_ref().map(to_entity_event))
}

fn event_from_note(
    ndb: &Ndb,
    txn: &Transaction,
    key: nostrdb::NoteKey,
) -> Option<Event> {
    let note = ndb.get_note_by_key(txn, key).ok()?;
    let json = note.json().ok()?;
    Event::from_json(&json).ok()
}

fn event_d_tag(event: &Event) -> Option<String> {
    for tag in event.tags.iter() {
        let s = tag.as_slice();
        if s.first().map(String::as_str) == Some("d") {
            return s.get(1).cloned();
        }
    }
    None
}

fn to_entity_event(event: &Event) -> NostrEntityEvent {
    let tags_json = serde_json::to_string(
        &event
            .tags
            .iter()
            .map(|t| t.as_slice().to_vec())
            .collect::<Vec<_>>(),
    )
    .unwrap_or_else(|_| "[]".into());
    NostrEntityEvent {
        event_id_hex: event.id.to_hex(),
        kind: event.kind.as_u16() as u32,
        pubkey_hex: event.pubkey.to_hex(),
        content: event.content.clone(),
        created_at: event.created_at.as_secs(),
        tags_json,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_npub() {
        let out = decode_nostr_entity(
            "nostr:npub180cvv07tjdrrgpa0j7j7tmnyl2yr6yr7l8j4s3evf6u64th6gkwsyjh6w6",
        )
        .expect("decode");
        match out {
            NostrEntityRef::Profile { pubkey_hex, .. } => {
                assert_eq!(
                    pubkey_hex,
                    "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d"
                );
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn bare_form_works_without_nostr_prefix() {
        let out = decode_nostr_entity(
            "npub180cvv07tjdrrgpa0j7j7tmnyl2yr6yr7l8j4s3evf6u64th6gkwsyjh6w6",
        )
        .expect("decode");
        matches!(out, NostrEntityRef::Profile { .. });
    }

    #[test]
    fn rejects_nsec() {
        let err = decode_nostr_entity(
            "nsec1vl029mgpspedva04g90vltkh6fvh240zqtv9k0t9af8935ke9laqsnlfe5",
        );
        assert!(err.is_err());
    }

    #[test]
    fn empty_input_rejected() {
        assert!(decode_nostr_entity("").is_err());
        assert!(decode_nostr_entity("   nostr:  ").is_err());
    }

    #[test]
    fn garbage_input_rejected() {
        assert!(decode_nostr_entity("nostr:nope").is_err());
        assert!(decode_nostr_entity("literally not bech32").is_err());
    }
}
