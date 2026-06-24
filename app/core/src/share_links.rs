//! Canonical Highlighter share-link construction.
//!
//! Native shells render share affordances, but Rust owns the protocol reference,
//! event kind, relay hint, and public route that make those links resolve.

use crate::errors::CoreError;
use crate::relays::highlighter_relay;
use nostr_sdk::nips::nip19::{Nip19Coordinate, Nip19Event, ToBech32};
use nostr_sdk::prelude::*;

const HIGHLIGHT_SHARE_BASE_URL: &str = "https://beta.highlighter.com/highlight/";
const ARTICLE_SHARE_BASE_URL: &str = "https://highlighter.com/a/";
const HIGHLIGHT_EVENT_KIND: u32 = 9802;
const ARTICLE_EVENT_KIND: u16 = 30023;

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ArticleShareUrlSnapshot {
    pub url: String,
    pub error: String,
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
    let (kind, public_key, identifier) = article_address_parts(&address)?;
    let coordinate = Coordinate {
        kind,
        public_key,
        identifier,
    };
    let relay = RelayUrl::parse(highlighter_relay())
        .map_err(|e| CoreError::InvalidInput(format!("bad relay hint: {e}")))?;
    let naddr = Nip19Coordinate::new(coordinate, [relay])
        .to_bech32()
        .map_err(|e| CoreError::InvalidInput(format!("encode naddr: {e}")))?;

    Ok(format!("{ARTICLE_SHARE_BASE_URL}{naddr}"))
}

pub fn highlight_share_url(
    event_id_hex: String,
    author_pubkey_hex: Option<String>,
) -> Result<String, CoreError> {
    let id = EventId::from_hex(&event_id_hex)
        .map_err(|e| CoreError::InvalidInput(format!("bad event id: {e}")))?;
    let mut nevent = Nip19Event::new(id);
    if let Some(pk_hex) = author_pubkey_hex {
        let trimmed = pk_hex.trim();
        if !trimmed.is_empty() {
            let author = PublicKey::from_hex(trimmed)
                .map_err(|e| CoreError::InvalidInput(format!("bad author pubkey: {e}")))?;
            nevent = nevent.author(author);
        }
    }
    nevent = nevent.kind(Kind::from(HIGHLIGHT_EVENT_KIND as u16));
    let relay = RelayUrl::parse(highlighter_relay())
        .map_err(|e| CoreError::InvalidInput(format!("bad relay hint: {e}")))?;
    nevent = nevent.relays([relay]);
    let encoded = nevent
        .to_bech32()
        .map_err(|e| CoreError::InvalidInput(format!("encode nevent: {e}")))?;
    Ok(format!("{HIGHLIGHT_SHARE_BASE_URL}{encoded}"))
}

fn article_address_parts(address: &str) -> Result<(Kind, PublicKey, String), CoreError> {
    let mut parts = address.trim().splitn(3, ':');
    let kind = parts
        .next()
        .ok_or_else(|| CoreError::InvalidInput("article address missing kind".into()))?
        .parse::<u16>()
        .map_err(|e| CoreError::InvalidInput(format!("bad article kind: {e}")))?;
    if kind != ARTICLE_EVENT_KIND {
        return Err(CoreError::InvalidInput(
            "address is not a NIP-23 article".into(),
        ));
    }

    let pubkey_hex = parts
        .next()
        .ok_or_else(|| CoreError::InvalidInput("article address missing pubkey".into()))?
        .trim();
    let public_key = PublicKey::from_hex(pubkey_hex)
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

    Ok((Kind::from(kind), public_key, identifier))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::NostrEntityRef;
    use nostr_sdk::nips::nip19::{FromBech32, Nip19, Nip19Coordinate, Nip19Profile};

    fn decode_nostr_entity(input: &str) -> Result<NostrEntityRef, crate::errors::CoreError> {
        let trimmed = input
            .trim()
            .strip_prefix("nostr:")
            .unwrap_or(input.trim())
            .trim();
        let decoded = Nip19::from_bech32(trimmed).map_err(|e| {
            crate::errors::CoreError::InvalidInput(format!("bad nostr entity: {e}"))
        })?;
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
            Nip19::Event(nostr_sdk::nips::nip19::Nip19Event {
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
}
