//! Canonical Highlighter share-link construction.
//!
//! Native shells render share affordances, but Rust owns the protocol reference,
//! event kind, relay hint, and public route that make those links resolve.

use crate::errors::CoreError;
use crate::relays::highlighter_relay;

const HIGHLIGHT_SHARE_BASE_URL: &str = "https://beta.highlighter.com/highlight/";
const ARTICLE_SHARE_BASE_URL: &str = "https://highlighter.com/a/";
/// Curation sets (kind:30004) resolve via the web app's canonical `/note/<naddr>`
/// route. Verified against `web/src/routes/`: there is NO `/a/` route; every
/// naddr link-builder targets `/note/<naddr>` (server/nostr.ts:535,
/// artifact-preview.ts:675, ndk/artifacts.ts:788). The naddr encoding carries
/// kind=30004 so the generic `/note/[id]` handler decodes the set correctly.
const CURATION_SET_SHARE_BASE_URL: &str = "https://highlighter.com/note/";
const HIGHLIGHT_EVENT_KIND: u32 = 9802;
const ARTICLE_EVENT_KIND: u16 = 30023;
const CURATION_SET_EVENT_KIND: u16 = 30004;

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ArticleShareUrlSnapshot {
    pub url: String,
    pub error: String,
}

/// Snapshot type for `curation_set_share_url_snapshot` — mirrors
/// `ArticleShareUrlSnapshot`. On success `url` is set and `error` is empty;
/// on failure `url` is empty and `error` carries the reason.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CurationSetShareUrlSnapshot {
    pub url: String,
    pub error: String,
}

/// FFI-exported wrapper around `curation_set_share_url`. Returns a
/// `CurationSetShareUrlSnapshot` with the resolved URL or an error string.
#[uniffi::export]
pub fn curation_set_share_url_snapshot(coordinate: String) -> CurationSetShareUrlSnapshot {
    match curation_set_share_url(coordinate) {
        Ok(url) => CurationSetShareUrlSnapshot {
            url,
            error: String::new(),
        },
        Err(error) => CurationSetShareUrlSnapshot {
            url: String::new(),
            error: error.to_string(),
        },
    }
}

/// Build a canonical share URL for a kind:30004 curation set from its
/// `"30004:<pubkey_hex>:<d_tag>"` coordinate.
///
/// Encodes the coordinate as an `naddr` bech32 with a relay hint pointing to
/// `relay.highlighter.com`, then prepends the canonical base URL. The naddr
/// encoding carries kind=30004 so any nostr-aware client can resolve the set.
///
/// Returns `Err(CoreError::InvalidInput)` when:
/// - the coordinate does not have exactly three colon-delimited parts,
/// - the kind is not 30004,
/// - the pubkey is not valid hex, or
/// - the d_tag is empty.
pub fn curation_set_share_url(coordinate: String) -> Result<String, CoreError> {
    let (kind, pubkey, identifier) = curation_set_address_parts(&coordinate)?;
    let naddr = nmp_nostr_id::encode_naddr(&nmp_nostr_id::NaddrData {
        identifier,
        pubkey,
        kind,
        relays: vec![highlighter_relay().to_owned()],
    })
    .map_err(|e| CoreError::InvalidInput(format!("encode naddr: {e}")))?;

    Ok(format!("{CURATION_SET_SHARE_BASE_URL}{naddr}"))
}

/// Parse and validate a `"30004:<pubkey_hex>:<d_tag>"` coordinate string.
/// Returns `(kind, pubkey_hex, identifier)` on success.
fn curation_set_address_parts(coordinate: &str) -> Result<(u32, String, String), CoreError> {
    let mut parts = coordinate.trim().splitn(3, ':');
    let kind_str = parts
        .next()
        .ok_or_else(|| CoreError::InvalidInput("coordinate missing kind".into()))?;
    let kind_num = kind_str
        .parse::<u32>()
        .map_err(|e| CoreError::InvalidInput(format!("bad coordinate kind: {e}")))?;
    if kind_num != u32::from(CURATION_SET_EVENT_KIND) {
        return Err(CoreError::InvalidInput(
            "coordinate is not a kind:30004 curation set".into(),
        ));
    }

    let pubkey_hex = parts
        .next()
        .ok_or_else(|| CoreError::InvalidInput("coordinate missing pubkey".into()))?
        .trim();
    nmp_nostr_id::encode_npub(pubkey_hex)
        .map_err(|e| CoreError::InvalidInput(format!("bad coordinate pubkey: {e}")))?;

    let identifier = parts
        .next()
        .ok_or_else(|| CoreError::InvalidInput("coordinate missing d_tag".into()))?
        .trim()
        .to_owned();
    if identifier.is_empty() {
        return Err(CoreError::InvalidInput(
            "coordinate d_tag must not be empty".into(),
        ));
    }

    Ok((kind_num, pubkey_hex.to_owned(), identifier))
}

pub fn article_share_url_snapshot(address: String) -> ArticleShareUrlSnapshot {
    match article_share_url(address) {
        Ok(url) => ArticleShareUrlSnapshot {
            url,
            error: String::new(),
        },
        Err(error) => ArticleShareUrlSnapshot {
            url: String::new(),
            error: error.to_string(),
        },
    }
}

pub fn article_share_url(address: String) -> Result<String, CoreError> {
    let (kind, pubkey, identifier) = article_address_parts(&address)?;
    let naddr = nmp_nostr_id::encode_naddr(&nmp_nostr_id::NaddrData {
        identifier,
        pubkey,
        kind,
        relays: vec![highlighter_relay().to_owned()],
    })
    .map_err(|e| CoreError::InvalidInput(format!("encode naddr: {e}")))?;

    Ok(format!("{ARTICLE_SHARE_BASE_URL}{naddr}"))
}

pub fn highlight_share_url(
    event_id_hex: String,
    author_pubkey_hex: Option<String>,
) -> Result<String, CoreError> {
    let author = author_pubkey_hex.and_then(|pk_hex| {
        let trimmed = pk_hex.trim().to_owned();
        (!trimmed.is_empty()).then_some(trimmed)
    });
    let encoded = nmp_nostr_id::encode_nevent(&nmp_nostr_id::NeventData {
        event_id: event_id_hex,
        relays: vec![highlighter_relay().to_owned()],
        author,
        kind: Some(HIGHLIGHT_EVENT_KIND),
    })
    .map_err(|e| CoreError::InvalidInput(format!("encode nevent: {e}")))?;
    Ok(format!("{HIGHLIGHT_SHARE_BASE_URL}{encoded}"))
}

fn article_address_parts(address: &str) -> Result<(u32, String, String), CoreError> {
    let mut parts = address.trim().splitn(3, ':');
    let kind = parts
        .next()
        .ok_or_else(|| CoreError::InvalidInput("article address missing kind".into()))?
        .parse::<u32>()
        .map_err(|e| CoreError::InvalidInput(format!("bad article kind: {e}")))?;
    if kind != u32::from(ARTICLE_EVENT_KIND) {
        return Err(CoreError::InvalidInput(
            "address is not a NIP-23 article".into(),
        ));
    }

    let pubkey_hex = parts
        .next()
        .ok_or_else(|| CoreError::InvalidInput("article address missing pubkey".into()))?
        .trim();
    nmp_nostr_id::encode_npub(pubkey_hex)
        .map_err(|e| CoreError::InvalidInput(format!("bad article pubkey: {e}")))?;

    let identifier = parts
        .next()
        .ok_or_else(|| CoreError::InvalidInput("article address missing identifier".into()))?
        .trim()
        .to_owned();
    if identifier.is_empty() {
        return Err(CoreError::InvalidInput(
            "article address missing identifier".into(),
        ));
    }

    Ok((kind, pubkey_hex.to_owned(), identifier))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::NostrEntityRef;

    fn decode_nostr_entity(input: &str) -> Result<NostrEntityRef, crate::errors::CoreError> {
        let trimmed = input
            .trim()
            .strip_prefix("nostr:")
            .unwrap_or(input.trim())
            .trim();
        let decoded = nmp_nostr_id::parse(trimmed).map_err(|e| {
            crate::errors::CoreError::InvalidInput(format!("bad nostr entity: {e}"))
        })?;
        Ok(match decoded {
            nmp_nostr_id::Nip19Entity::Npub(pubkey_hex) => NostrEntityRef::Profile {
                pubkey_hex,
                relays: Vec::new(),
            },
            nmp_nostr_id::Nip19Entity::Nprofile(data) => NostrEntityRef::Profile {
                pubkey_hex: data.pubkey,
                relays: data.relays,
            },
            nmp_nostr_id::Nip19Entity::Note(event_id_hex) => NostrEntityRef::Event {
                event_id_hex,
                relays: Vec::new(),
                author_hint_hex: None,
                kind_hint: None,
            },
            nmp_nostr_id::Nip19Entity::Nevent(data) => NostrEntityRef::Event {
                event_id_hex: data.event_id,
                relays: data.relays,
                author_hint_hex: data.author,
                kind_hint: data.kind,
            },
            nmp_nostr_id::Nip19Entity::Naddr(data) => NostrEntityRef::Address {
                kind: data.kind,
                pubkey_hex: data.pubkey,
                d_tag: data.identifier,
                relays: data.relays,
            },
            _ => {
                return Err(crate::errors::CoreError::InvalidInput(
                    "nostr entity type not renderable".into(),
                ))
            }
        })
    }

    fn event_id_hex() -> String {
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_owned()
    }

    fn author_hex() -> String {
        "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d".to_owned()
    }

    #[test]
    fn article_share_url_uses_canonical_route_and_naddr_hint() {
        let expected_author = author_hex();
        let expected_d_tag = "article:with:colons".to_owned();
        let url = article_share_url(format!("30023:{expected_author}:{expected_d_tag}"))
            .expect("share url");
        let naddr = url
            .strip_prefix(ARTICLE_SHARE_BASE_URL)
            .expect("canonical article route");

        let decoded = decode_nostr_entity(naddr).expect("decode naddr");
        match decoded {
            NostrEntityRef::Address {
                kind,
                pubkey_hex,
                d_tag,
                relays,
            } => {
                assert_eq!(kind, ARTICLE_EVENT_KIND as u32);
                assert_eq!(pubkey_hex, expected_author);
                assert_eq!(d_tag, expected_d_tag);
                assert_eq!(relays, vec![highlighter_relay().to_owned()]);
            }
            other => panic!("wrong variant: {other:?}"),
        }
    }

    #[test]
    fn article_share_url_rejects_non_article_address() {
        assert!(article_share_url(format!("1:{}:note", author_hex())).is_err());
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
                assert_eq!(relays, vec![highlighter_relay().to_owned()]);
            }
            other => panic!("wrong variant: {other:?}"),
        }
    }

    #[test]
    fn highlight_share_url_rejects_bad_event_id() {
        assert!(highlight_share_url("not-hex".to_owned(), Some(author_hex())).is_err());
    }

    // ── Issue #63: curation set share URL ────────────────────────────────────

    #[test]
    fn curation_set_share_url_encodes_naddr_kind_30004() {
        let expected_author = author_hex();
        let expected_d_tag = "my-reading-list".to_owned();
        let coordinate = format!("30004:{expected_author}:{expected_d_tag}");
        let url = curation_set_share_url(coordinate).expect("share url");

        // Route fix (B2): web app has no `/a/` route; naddr resolves via `/note/`.
        assert!(
            url.starts_with("https://highlighter.com/note/naddr1"),
            "curation set URL must use the canonical /note/<naddr> route, got: {url}"
        );

        let naddr = url
            .strip_prefix(CURATION_SET_SHARE_BASE_URL)
            .expect("canonical curation set route prefix must match");

        let decoded = decode_nostr_entity(naddr).expect("decode naddr");
        match decoded {
            NostrEntityRef::Address {
                kind,
                pubkey_hex,
                d_tag,
                relays,
            } => {
                assert_eq!(kind, 30004u32, "naddr kind must be 30004");
                assert_eq!(pubkey_hex, expected_author, "pubkey must match");
                assert_eq!(d_tag, expected_d_tag, "d_tag must match");
                assert_eq!(
                    relays,
                    vec![crate::relays::highlighter_relay().to_owned()],
                    "relay hint must be present"
                );
            }
            other => panic!("expected Address variant, got: {other:?}"),
        }
    }

    #[test]
    fn curation_set_share_url_rejects_non_30004() {
        // kind:30003 (bookmark set, not curation set) must be rejected
        assert!(
            curation_set_share_url(format!("30003:{}:my-set", author_hex())).is_err(),
            "non-30004 coordinate must be rejected"
        );
        // kind:30023 (article) must be rejected
        assert!(
            curation_set_share_url(format!("30023:{}:my-article", author_hex())).is_err(),
            "non-30004 coordinate must be rejected"
        );
    }

    #[test]
    fn curation_set_share_url_rejects_malformed_coordinate() {
        assert!(
            curation_set_share_url("not-a-coordinate".to_owned()).is_err(),
            "malformed coordinate must be rejected"
        );
        assert!(
            curation_set_share_url("30004:bad-pubkey:my-set".to_owned()).is_err(),
            "bad pubkey must be rejected"
        );
        assert!(
            curation_set_share_url(format!("30004:{}:", author_hex())).is_err(),
            "empty d_tag must be rejected"
        );
    }
}
