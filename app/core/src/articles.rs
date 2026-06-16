//! NIP-23 long-form articles (kind:30023) — the "Writing" tab on a user
//! profile.

use std::collections::BTreeMap;

use nostr_sdk::prelude::*;
use nostrdb::{Filter as NdbFilter, Ndb, Transaction};

use crate::errors::CoreError;
use crate::models::{ArticleReaderRoute, ArticleRecord, ArtifactPreview, ArtifactRecord};

pub const KIND_LONG_FORM: u16 = 30023;

#[derive(Debug, Clone, uniffi::Record)]
pub struct ArticleReaderHeaderProjectionInput {
    pub article: ArticleRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ArticleReaderHeaderProjection {
    pub title: String,
    pub hashtag_labels: Vec<String>,
    pub display_unix_seconds: Option<u64>,
    pub read_time_minutes: Option<u32>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ArticleProfileCardProjectionInput {
    pub article: ArticleRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ArticleProfileCardProjection {
    pub title: String,
    pub title_is_fallback: bool,
    pub display_unix_seconds: Option<u64>,
    pub hashtag_summary: Option<String>,
}

/// Presentation projection for the article reader header. Rust owns title
/// fallback, visible hashtag cap, timestamp source selection, and read-time
/// estimate; native shells render and apply localized date formatting.
pub fn article_reader_header_projection(
    input: ArticleReaderHeaderProjectionInput,
) -> ArticleReaderHeaderProjection {
    let article = input.article;
    ArticleReaderHeaderProjection {
        title: if article.title.is_empty() {
            "Untitled".to_string()
        } else {
            article.title
        },
        hashtag_labels: article
            .hashtags
            .iter()
            .take(12)
            .map(|tag| format!("#{tag}"))
            .collect(),
        display_unix_seconds: article
            .published_at
            .or(article.created_at)
            .filter(|seconds| *seconds > 0),
        read_time_minutes: read_time_minutes(&article.content),
    }
}

/// Presentation projection for the profile Writing tab article row. Rust owns
/// fallback title state, timestamp source selection, and two-tag summary.
pub fn article_profile_card_projection(
    input: ArticleProfileCardProjectionInput,
) -> ArticleProfileCardProjection {
    let article = input.article;
    let title_is_fallback = article.title.is_empty();
    ArticleProfileCardProjection {
        title: if title_is_fallback {
            "Untitled".to_string()
        } else {
            article.title
        },
        title_is_fallback,
        display_unix_seconds: article
            .published_at
            .or(article.created_at)
            .filter(|seconds| *seconds > 0),
        hashtag_summary: if article.hashtags.is_empty() {
            None
        } else {
            Some(
                article
                    .hashtags
                    .iter()
                    .take(2)
                    .map(|tag| format!("#{tag}"))
                    .collect::<Vec<_>>()
                    .join(" "),
            )
        },
    }
}

fn read_time_minutes(content: &str) -> Option<u32> {
    let words = content.split_whitespace().count();
    if words > 60 {
        Some(((words / 240).max(1)) as u32)
    } else {
        None
    }
}

/// Read a single NIP-23 article by its NIP-33 addressable id (`pubkey:d`).
/// Returns the newest `created_at` event with a matching `d` tag. `None` if
/// nostrdb has no matching event cached — the reader view spawns a relay
/// subscription on the article's address to backfill, at which point a later
/// call returns `Some`.
pub fn query_article(
    ndb: &Ndb,
    pubkey_hex: &str,
    d_tag: &str,
) -> Result<Option<ArticleRecord>, CoreError> {
    let pubkey_hex = pubkey_hex.trim();
    let d_tag = d_tag.trim();
    if pubkey_hex.is_empty() || d_tag.is_empty() {
        return Ok(None);
    }
    let author = PublicKey::from_hex(pubkey_hex)
        .map_err(|e| CoreError::InvalidInput(format!("invalid pubkey: {e}")))?;

    let txn = Transaction::new(ndb).map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;

    let pk_bytes: [u8; 32] = author.to_bytes();
    let filter = NdbFilter::new()
        .kinds([KIND_LONG_FORM as u64])
        .authors([&pk_bytes])
        .tags([d_tag], 'd')
        .build();

    let results = ndb
        .query(&txn, &[filter], 32)
        .map_err(|e| CoreError::Cache(format!("query article: {e}")))?;

    let mut events: Vec<Event> = Vec::with_capacity(results.len());
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        events.push(event);
    }

    Ok(build_articles(&events, 1).into_iter().next())
}

/// Read a single NIP-23 article by its full NIP-33 address
/// (`30023:<pubkey>:<d>`). Malformed or non-article addresses resolve
/// to `None`; cache/storage errors still surface.
pub fn query_article_by_address(
    ndb: &Ndb,
    address: &str,
) -> Result<Option<ArticleRecord>, CoreError> {
    let Some((pubkey_hex, d_tag)) = article_address_parts(address) else {
        return Ok(None);
    };
    query_article(ndb, &pubkey_hex, &d_tag)
}

/// Return the author pubkey from a valid NIP-23 article address.
pub fn article_author_from_address(address: &str) -> Option<String> {
    article_address_parts(address).map(|(pubkey_hex, _)| pubkey_hex)
}

/// Build a native reader route from a full NIP-23 article address. Malformed
/// or non-article addresses do not produce a route.
pub fn article_reader_route_from_address(address: &str) -> Option<ArticleReaderRoute> {
    let (pubkey_hex, d_tag) = article_address_parts(address)?;
    article_reader_route(&pubkey_hex, &d_tag)
}

/// Build a native reader route from an article author + `d` tag. Rust owns the
/// canonical NIP-33 address construction so native shells never synthesize it.
pub fn article_reader_route(pubkey_hex: &str, d_tag: &str) -> Option<ArticleReaderRoute> {
    let pubkey = pubkey_hex.trim();
    let d_tag = d_tag.trim();
    if pubkey.is_empty() || d_tag.is_empty() {
        return None;
    }
    Some(ArticleReaderRoute {
        address: article_address(pubkey, d_tag),
        pubkey: pubkey.to_string(),
        d_tag: d_tag.to_string(),
    })
}

/// Project a NIP-23 article into the artifact preview shape used by kind:11
/// shares and kind:9802 highlight references. Rust owns the protocol tag
/// semantics so native shells do not synthesize `a`/`k`/reference fields.
pub fn article_artifact_preview(article: &ArticleRecord) -> ArtifactPreview {
    article_artifact_preview_parts(
        &article.identifier,
        &article.address,
        &article.title,
        &article.image,
        &article.summary,
        article.published_at,
    )
}

/// Project an article address into a minimal article artifact preview. Used
/// when a highlight has only the article address cached, not the full article
/// metadata.
pub fn article_artifact_preview_from_address(address: &str) -> Option<ArtifactPreview> {
    let route = article_reader_route_from_address(address)?;
    Some(article_artifact_preview_parts(
        &route.d_tag,
        &route.address,
        "",
        "",
        "",
        None,
    ))
}

/// Project a NIP-23 article into the artifact record shape expected by
/// highlight publishing. The shell supplies the user's selected quote/note;
/// Rust owns the source artifact reference.
pub fn article_artifact_record(article: &ArticleRecord) -> ArtifactRecord {
    ArtifactRecord {
        preview: article_artifact_preview(article),
        group_id: String::new(),
        share_event_id: String::new(),
        pubkey: article.pubkey.clone(),
        created_at: article.created_at,
        note: String::new(),
    }
}

fn article_artifact_preview_parts(
    id: &str,
    address: &str,
    title: &str,
    image: &str,
    summary: &str,
    published_at: Option<u64>,
) -> ArtifactPreview {
    ArtifactPreview {
        id: id.to_string(),
        url: String::new(),
        title: title.to_string(),
        author: String::new(),
        image: image.to_string(),
        description: summary.to_string(),
        source: "article".to_string(),
        domain: String::new(),
        catalog_id: String::new(),
        catalog_kind: String::new(),
        podcast_guid: String::new(),
        podcast_item_guid: String::new(),
        podcast_show_title: String::new(),
        audio_url: String::new(),
        audio_preview_url: String::new(),
        transcript_url: String::new(),
        feed_url: String::new(),
        published_at: published_at
            .map(|seconds| seconds.to_string())
            .unwrap_or_default(),
        duration_seconds: None,
        reference_tag_name: "a".to_string(),
        reference_tag_value: address.to_string(),
        reference_kind: KIND_LONG_FORM.to_string(),
        highlight_tag_name: "a".to_string(),
        highlight_tag_value: address.to_string(),
        highlight_reference_key: format!("a:{address}"),
        chapters: Vec::new(),
    }
}

/// Resolve the article addresses stored on a bookmark/curation set into cached
/// NIP-23 records, newest first. Malformed or non-article addresses are
/// ignored; cache and storage errors still surface to the caller.
pub fn query_articles_for_addresses(
    ndb: &Ndb,
    addresses: &[String],
) -> Result<Vec<ArticleRecord>, CoreError> {
    let mut articles = Vec::new();
    for address in addresses {
        let Some((pubkey_hex, d_tag)) = article_address_parts(address) else {
            continue;
        };
        match query_article(ndb, &pubkey_hex, &d_tag) {
            Ok(Some(article)) => articles.push(article),
            Ok(None) => {}
            Err(CoreError::InvalidInput(_)) => {}
            Err(error) => return Err(error),
        }
    }
    sort_articles_newest_first(&mut articles);
    Ok(articles)
}

/// Read a pubkey's long-form articles from nostrdb, deduped by `d` tag
/// (newest wins, matching NIP-33 parameterized replaceable semantics) and
/// sorted desc by `published_at` (falling back to `created_at`).
pub fn query_articles_by_author(
    ndb: &Ndb,
    pubkey_hex: &str,
    limit: u32,
) -> Result<Vec<ArticleRecord>, CoreError> {
    if pubkey_hex.is_empty() {
        return Ok(Vec::new());
    }
    let author = PublicKey::from_hex(pubkey_hex)
        .map_err(|e| CoreError::InvalidInput(format!("invalid pubkey: {e}")))?;

    let txn = Transaction::new(ndb).map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;

    let pk_bytes: [u8; 32] = author.to_bytes();
    // Fetch generously so the dedupe step has enough history to pick newest
    // per `d`; the final slice honors `limit`.
    let ndb_cap = limit.saturating_mul(4).max(64) as i32;
    let filter = NdbFilter::new()
        .kinds([KIND_LONG_FORM as u64])
        .authors([&pk_bytes])
        .build();

    let results = ndb
        .query(&txn, &[filter], ndb_cap)
        .map_err(|e| CoreError::Cache(format!("query articles: {e}")))?;

    let mut events: Vec<Event> = Vec::with_capacity(results.len());
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        events.push(event);
    }

    Ok(build_articles(&events, limit as usize))
}

/// Pure: dedupe by `d`, keep newest per `d`, sort desc by `published_at ?? created_at`.
pub fn build_articles(events: &[Event], limit: usize) -> Vec<ArticleRecord> {
    // Keep newest event per `d` identifier. Events missing `d` are skipped —
    // they're not conformant NIP-23 articles.
    let mut latest_by_d: BTreeMap<String, &Event> = BTreeMap::new();
    for event in events {
        let Some(d) = first_tag_value(event, "d") else {
            continue;
        };
        let key = d.trim();
        if key.is_empty() {
            continue;
        }
        match latest_by_d.get(key) {
            Some(prev) if prev.created_at >= event.created_at => {}
            _ => {
                latest_by_d.insert(key.to_string(), event);
            }
        }
    }

    let mut records: Vec<ArticleRecord> =
        latest_by_d.into_values().map(record_from_event).collect();
    records.sort_by(|a, b| {
        b.published_at
            .unwrap_or(b.created_at.unwrap_or(0))
            .cmp(&a.published_at.unwrap_or(a.created_at.unwrap_or(0)))
    });
    records.truncate(limit);
    records
}

fn record_from_event(event: &Event) -> ArticleRecord {
    let identifier = first_tag_value(event, "d").unwrap_or("").trim().to_string();
    let title = first_tag_value(event, "title")
        .unwrap_or("")
        .trim()
        .to_string();
    let summary = first_tag_value(event, "summary")
        .unwrap_or("")
        .trim()
        .to_string();
    let image = first_tag_value(event, "image")
        .unwrap_or("")
        .trim()
        .to_string();
    let published_at =
        first_tag_value(event, "published_at").and_then(|s| s.trim().parse::<u64>().ok());
    let hashtags: Vec<String> = event
        .tags
        .iter()
        .filter_map(|tag| {
            let s = tag.as_slice();
            if s.first().map(String::as_str) == Some("t") {
                s.get(1)
                    .map(|v| v.trim().to_string())
                    .filter(|v| !v.is_empty())
            } else {
                None
            }
        })
        .collect();

    ArticleRecord {
        event_id: event.id.to_hex(),
        address: article_address(&event.pubkey.to_hex(), &identifier),
        pubkey: event.pubkey.to_hex(),
        identifier,
        title,
        summary,
        image,
        content: event.content.clone(),
        hashtags,
        published_at,
        created_at: Some(event.created_at.as_secs()),
    }
}

pub fn article_address(pubkey_hex: &str, d_tag: &str) -> String {
    format!("{KIND_LONG_FORM}:{}:{}", pubkey_hex.trim(), d_tag.trim())
}

fn article_address_parts(address: &str) -> Option<(String, String)> {
    let mut parts = address.trim().splitn(3, ':');
    let kind = parts.next()?;
    let pubkey = parts.next()?.trim();
    let d_tag = parts.next()?.trim();
    if kind.parse::<u16>().ok() != Some(KIND_LONG_FORM) || pubkey.is_empty() || d_tag.is_empty() {
        return None;
    }
    Some((pubkey.to_string(), d_tag.to_string()))
}

fn sort_articles_newest_first(articles: &mut [ArticleRecord]) {
    articles.sort_by(|a, b| {
        b.published_at
            .unwrap_or(b.created_at.unwrap_or(0))
            .cmp(&a.published_at.unwrap_or(a.created_at.unwrap_or(0)))
    });
}

fn first_tag_value<'a>(event: &'a Event, name: &str) -> Option<&'a str> {
    for tag in event.tags.iter() {
        let slice = tag.as_slice();
        if slice.first().map(String::as_str) == Some(name) {
            return slice.get(1).map(String::as_str);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_ndb::process_event_and_wait;

    fn sign_article(keys: &Keys, d: &str, tags: Vec<Tag>, ts: u64, content: &str) -> Event {
        let mut all = vec![Tag::identifier(d)];
        all.extend(tags);
        EventBuilder::new(Kind::Custom(KIND_LONG_FORM), content)
            .tags(all)
            .custom_created_at(Timestamp::from(ts))
            .sign_with_keys(keys)
            .expect("sign")
    }

    fn named(name: &str, value: &str) -> Tag {
        Tag::parse(vec![name.to_string(), value.to_string()]).expect("named tag")
    }

    fn article_record(title: &str, content: String, hashtags: Vec<String>) -> ArticleRecord {
        ArticleRecord {
            event_id: "event".into(),
            address: article_address("author", "d"),
            pubkey: "author".into(),
            identifier: "d".into(),
            title: title.into(),
            summary: String::new(),
            image: String::new(),
            content,
            hashtags,
            published_at: Some(123),
            created_at: Some(100),
        }
    }

    fn repeated_words(count: usize) -> String {
        vec!["word"; count].join(" ")
    }

    #[test]
    fn dedupes_by_d_keeping_newest() {
        let keys = Keys::generate();
        let older = sign_article(
            &keys,
            "post-1",
            vec![named("title", "Old")],
            1_000,
            "old body",
        );
        let newer = sign_article(
            &keys,
            "post-1",
            vec![named("title", "New")],
            2_000,
            "new body",
        );
        let distinct = sign_article(&keys, "post-2", vec![named("title", "Other")], 1_500, "x");
        let out = build_articles(&[older, newer, distinct], 10);
        assert_eq!(out.len(), 2);
        let p1 = out.iter().find(|a| a.identifier == "post-1").unwrap();
        assert_eq!(p1.title, "New");
        assert_eq!(p1.content, "new body");
    }

    #[test]
    fn skips_events_missing_d_tag() {
        let keys = Keys::generate();
        let good = sign_article(&keys, "ok", vec![named("title", "Ok")], 1_000, "body");
        let orphan = EventBuilder::new(Kind::Custom(KIND_LONG_FORM), "orphan")
            .tags(vec![named("title", "Orphan")])
            .custom_created_at(Timestamp::from(1_500))
            .sign_with_keys(&keys)
            .expect("sign");
        let out = build_articles(&[good, orphan], 10);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].identifier, "ok");
    }

    #[test]
    fn sorts_desc_by_published_at_then_created_at() {
        let keys = Keys::generate();
        // a published earlier but created later → should sort after b
        let a = sign_article(
            &keys,
            "a",
            vec![named("title", "A"), named("published_at", "1000")],
            9_000,
            "",
        );
        let b = sign_article(
            &keys,
            "b",
            vec![named("title", "B"), named("published_at", "2000")],
            8_000,
            "",
        );
        // c has no published_at → falls back to created_at
        let c = sign_article(&keys, "c", vec![named("title", "C")], 10_000, "");
        let out = build_articles(&[a, b, c], 10);
        let order: Vec<_> = out.iter().map(|r| r.identifier.as_str()).collect();
        assert_eq!(order, vec!["c", "b", "a"]);
    }

    #[test]
    fn extracts_hashtags_from_t_tags() {
        let keys = Keys::generate();
        let event = sign_article(
            &keys,
            "post",
            vec![
                named("title", "T"),
                named("t", "nostr"),
                named("t", "rust"),
                named("t", ""),
            ],
            1_000,
            "",
        );
        let out = build_articles(&[event], 10);
        assert_eq!(out[0].hashtags, vec!["nostr", "rust"]);
    }

    #[test]
    fn article_reader_header_projection_matches_reader_header_policy() {
        let hashtags: Vec<String> = (0..14).map(|idx| format!("tag{idx}")).collect();
        let projection = article_reader_header_projection(ArticleReaderHeaderProjectionInput {
            article: article_record("", repeated_words(480), hashtags),
        });

        assert_eq!(projection.title, "Untitled");
        assert_eq!(projection.hashtag_labels.len(), 12);
        assert_eq!(projection.hashtag_labels[0], "#tag0");
        assert_eq!(projection.hashtag_labels[11], "#tag11");
        assert_eq!(projection.display_unix_seconds, Some(123));
        assert_eq!(projection.read_time_minutes, Some(2));
    }

    #[test]
    fn article_reader_header_projection_hides_invalid_time_and_short_read_time() {
        let mut article = article_record("A title", repeated_words(60), vec!["nostr".into()]);
        article.published_at = None;
        article.created_at = Some(0);

        let projection =
            article_reader_header_projection(ArticleReaderHeaderProjectionInput { article });

        assert_eq!(projection.title, "A title");
        assert_eq!(projection.hashtag_labels, vec!["#nostr".to_string()]);
        assert_eq!(projection.display_unix_seconds, None);
        assert_eq!(projection.read_time_minutes, None);
    }

    #[test]
    fn article_profile_card_projection_preserves_row_policy() {
        let projection = article_profile_card_projection(ArticleProfileCardProjectionInput {
            article: article_record(
                "",
                String::new(),
                vec!["nostr".into(), "rust".into(), "swift".into()],
            ),
        });

        assert_eq!(projection.title, "Untitled");
        assert!(projection.title_is_fallback);
        assert_eq!(projection.display_unix_seconds, Some(123));
        assert_eq!(projection.hashtag_summary, Some("#nostr #rust".into()));
    }

    #[test]
    fn article_profile_card_projection_hides_empty_tags_and_invalid_time() {
        let mut article = article_record("A title", String::new(), Vec::new());
        article.published_at = None;
        article.created_at = Some(0);

        let projection =
            article_profile_card_projection(ArticleProfileCardProjectionInput { article });

        assert_eq!(projection.title, "A title");
        assert!(!projection.title_is_fallback);
        assert_eq!(projection.display_unix_seconds, None);
        assert_eq!(projection.hashtag_summary, None);
    }

    #[test]
    fn article_address_parts_accepts_nip23_addresses_only() {
        let keys = Keys::generate();
        let valid = format!("30023:{}:essay", keys.public_key().to_hex());
        assert_eq!(
            article_address_parts(&valid),
            Some((keys.public_key().to_hex(), "essay".to_string()))
        );
        assert_eq!(
            article_author_from_address(&valid),
            Some(keys.public_key().to_hex())
        );
        assert_eq!(
            article_reader_route_from_address(&valid),
            Some(ArticleReaderRoute {
                address: valid.clone(),
                pubkey: keys.public_key().to_hex(),
                d_tag: "essay".to_string()
            })
        );
        assert!(article_address_parts("30023:missing-d").is_none());
        assert!(article_address_parts("30023::essay").is_none());
        assert!(article_address_parts("1:abcdef:note").is_none());
        assert!(article_author_from_address("1:abcdef:note").is_none());
        assert!(article_reader_route_from_address("1:abcdef:note").is_none());
    }

    #[test]
    fn article_artifact_preview_projects_protocol_reference_fields() {
        let article = ArticleRecord {
            event_id: "event-1".into(),
            address: "30023:pk:essay".into(),
            pubkey: "pk".into(),
            identifier: "essay".into(),
            title: "Essay".into(),
            summary: "A short summary".into(),
            image: "https://example.com/cover.jpg".into(),
            content: "body".into(),
            hashtags: vec!["nostr".into()],
            published_at: Some(1_700),
            created_at: Some(1_800),
        };

        let preview = article_artifact_preview(&article);
        assert_eq!(preview.id, "essay");
        assert_eq!(preview.title, "Essay");
        assert_eq!(preview.description, "A short summary");
        assert_eq!(preview.image, "https://example.com/cover.jpg");
        assert_eq!(preview.source, "article");
        assert_eq!(preview.published_at, "1700");
        assert_eq!(preview.reference_tag_name, "a");
        assert_eq!(preview.reference_tag_value, "30023:pk:essay");
        assert_eq!(preview.reference_kind, "30023");
        assert_eq!(preview.highlight_tag_name, "a");
        assert_eq!(preview.highlight_tag_value, "30023:pk:essay");
        assert_eq!(preview.highlight_reference_key, "a:30023:pk:essay");

        let record = article_artifact_record(&article);
        assert_eq!(
            record.preview.highlight_reference_key,
            preview.highlight_reference_key
        );
        assert_eq!(
            record.preview.reference_tag_value,
            preview.reference_tag_value
        );
        assert_eq!(record.pubkey, "pk");
        assert_eq!(record.created_at, Some(1_800));
        assert!(record.group_id.is_empty());
        assert!(record.share_event_id.is_empty());
    }

    #[test]
    fn article_artifact_preview_from_address_is_minimal_and_validated() {
        let preview = article_artifact_preview_from_address("30023:pk:essay").expect("preview");
        assert_eq!(preview.id, "essay");
        assert_eq!(preview.source, "article");
        assert_eq!(preview.reference_tag_name, "a");
        assert_eq!(preview.reference_tag_value, "30023:pk:essay");
        assert_eq!(preview.reference_kind, "30023");
        assert_eq!(preview.highlight_tag_name, "a");
        assert_eq!(preview.highlight_tag_value, "30023:pk:essay");
        assert_eq!(preview.highlight_reference_key, "a:30023:pk:essay");
        assert!(preview.title.is_empty());
        assert!(article_artifact_preview_from_address("1:pk:note").is_none());
    }

    #[test]
    fn sort_articles_newest_first_matches_collection_order() {
        let mut articles = vec![
            ArticleRecord {
                event_id: "old".into(),
                address: article_address("pk", "old"),
                pubkey: "pk".into(),
                identifier: "old".into(),
                title: "Old".into(),
                summary: String::new(),
                image: String::new(),
                content: String::new(),
                hashtags: Vec::new(),
                published_at: Some(1_000),
                created_at: Some(9_000),
            },
            ArticleRecord {
                event_id: "new".into(),
                address: article_address("pk", "new"),
                pubkey: "pk".into(),
                identifier: "new".into(),
                title: "New".into(),
                summary: String::new(),
                image: String::new(),
                content: String::new(),
                hashtags: Vec::new(),
                published_at: None,
                created_at: Some(2_000),
            },
            ArticleRecord {
                event_id: "middle".into(),
                address: article_address("pk", "middle"),
                pubkey: "pk".into(),
                identifier: "middle".into(),
                title: "Middle".into(),
                summary: String::new(),
                image: String::new(),
                content: String::new(),
                hashtags: Vec::new(),
                published_at: Some(1_500),
                created_at: Some(1_500),
            },
        ];

        sort_articles_newest_first(&mut articles);
        let order: Vec<_> = articles
            .iter()
            .map(|article| article.identifier.as_str())
            .collect();
        assert_eq!(order, vec!["new", "middle", "old"]);
    }

    #[test]
    fn query_article_returns_newest_event_for_d_tag() {
        use nostrdb::{Config as NdbConfig, Ndb};
        use tempfile::tempdir;

        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().to_str().unwrap();
        let ndb =
            Ndb::new(db_path, &NdbConfig::new().set_mapsize(64 * 1024 * 1024)).expect("open ndb");

        let keys = Keys::generate();
        let older = sign_article(&keys, "post-x", vec![named("title", "Older")], 1_000, "v1");
        let newer = sign_article(&keys, "post-x", vec![named("title", "Newer")], 2_000, "v2");
        let other = sign_article(&keys, "post-y", vec![named("title", "Other")], 1_500, "w");

        for event in [&older, &newer, &other] {
            process_event_and_wait(&ndb, event);
        }

        let got = query_article(&ndb, &keys.public_key().to_hex(), "post-x")
            .expect("query_article")
            .expect("found article");
        assert_eq!(got.title, "Newer");
        assert_eq!(got.content, "v2");

        let missing = query_article(&ndb, &keys.public_key().to_hex(), "does-not-exist")
            .expect("query_article");
        assert!(missing.is_none());
    }

    #[test]
    fn limit_is_applied_after_dedup_and_sort() {
        let keys = Keys::generate();
        let mut events = Vec::new();
        for i in 0..5 {
            events.push(sign_article(
                &keys,
                &format!("p{i}"),
                vec![named("title", &format!("T{i}"))],
                1_000 + i,
                "",
            ));
        }
        let out = build_articles(&events, 2);
        assert_eq!(out.len(), 2);
        // newest two identifiers
        assert_eq!(out[0].identifier, "p4");
        assert_eq!(out[1].identifier, "p3");
    }
}
