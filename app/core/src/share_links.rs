//! Canonical Highlighter share-link construction.
//!
//! Native shells render share affordances, but Rust owns the protocol reference,
//! event kind, relay hint, and public route that make those links resolve.

use crate::errors::CoreError;
use crate::nostr_entities::encode_event_to_nevent;
use crate::relays::HIGHLIGHTER_RELAY;

const HIGHLIGHT_SHARE_BASE_URL: &str = "https://beta.highlighter.com/highlight/";
const HIGHLIGHT_EVENT_KIND: u32 = 9802;

pub fn highlight_share_url(
    event_id_hex: String,
    author_pubkey_hex: Option<String>,
) -> Result<String, CoreError> {
    let nevent = encode_event_to_nevent(
        event_id_hex,
        author_pubkey_hex,
        vec![HIGHLIGHTER_RELAY.to_owned()],
        Some(HIGHLIGHT_EVENT_KIND),
    )?;
    Ok(format!("{HIGHLIGHT_SHARE_BASE_URL}{nevent}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nostr_entities::{decode_nostr_entity, NostrEntityRef};

    fn event_id_hex() -> String {
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_owned()
    }

    fn author_hex() -> String {
        "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d".to_owned()
    }

    #[test]
    fn highlight_share_url_uses_canonical_route_and_nevent_hints() {
        let expected_event_id = event_id_hex();
        let expected_author = author_hex();
        let url = highlight_share_url(expected_event_id.clone(), Some(expected_author.clone()))
            .expect("share url");
        let nevent = url
            .strip_prefix(HIGHLIGHT_SHARE_BASE_URL)
            .expect("canonical route");

        let decoded = decode_nostr_entity(nevent).expect("decode nevent");
        match decoded {
            NostrEntityRef::Event {
                event_id_hex,
                author_hint_hex,
                kind_hint,
                relays,
            } => {
                assert_eq!(event_id_hex, expected_event_id);
                assert_eq!(author_hint_hex.as_deref(), Some(expected_author.as_str()));
                assert_eq!(kind_hint, Some(HIGHLIGHT_EVENT_KIND));
                assert_eq!(relays, vec![HIGHLIGHTER_RELAY.to_owned()]);
            }
            other => panic!("wrong variant: {other:?}"),
        }
    }

    #[test]
    fn highlight_share_url_rejects_bad_event_id() {
        assert!(highlight_share_url("not-hex".to_owned(), Some(author_hex())).is_err());
    }
}
