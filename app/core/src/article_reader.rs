//! Article reader read model. The native shell owns the scroll/rendering
//! surface; Rust owns the cache queries, section limits, and partial-failure
//! fallback for the reader's data dependencies.

use nostrdb::Ndb;

use crate::errors::CoreError;
use crate::models::{ArticleRecord, HighlightRecord, ProfileMetadata};
use crate::{articles, highlights, profile};

pub const ARTICLE_READER_HIGHLIGHT_LIMIT: u32 = 128;

#[derive(Debug, Clone, uniffi::Record)]
pub struct ArticleReaderSnapshot {
    pub article: Option<ArticleRecord>,
    pub author_profile: Option<ProfileMetadata>,
    pub highlights: Vec<HighlightRecord>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ArticleReaderHighlightPublishSnapshot {
    pub snapshot: ArticleReaderSnapshot,
    pub published_highlight_id: String,
    pub error: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ArticleReaderSnapshotApplyInput {
    pub snapshot: ArticleReaderSnapshot,
    pub current_article: Option<ArticleRecord>,
    pub current_author_profile: Option<ProfileMetadata>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ArticleReaderSnapshotProjection {
    pub article: Option<ArticleRecord>,
    pub author_profile: Option<ProfileMetadata>,
    pub highlights: Vec<HighlightRecord>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ArticleReaderPublishResultInput {
    pub error: String,
    pub published_highlight_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ArticleReaderPublishResultProjection {
    pub should_apply_snapshot: bool,
    pub last_published_highlight_id: String,
}

impl ArticleReaderSnapshot {
    fn empty() -> Self {
        Self {
            article: None,
            author_profile: None,
            highlights: Vec::new(),
        }
    }
}

pub fn article_reader_snapshot_projection(
    input: ArticleReaderSnapshotApplyInput,
) -> ArticleReaderSnapshotProjection {
    ArticleReaderSnapshotProjection {
        article: input.snapshot.article.or(input.current_article),
        author_profile: input
            .snapshot
            .author_profile
            .or(input.current_author_profile),
        highlights: input.snapshot.highlights,
    }
}

pub fn article_reader_publish_result_projection(
    input: ArticleReaderPublishResultInput,
) -> ArticleReaderPublishResultProjection {
    let should_apply_snapshot = input.error.trim().is_empty();
    ArticleReaderPublishResultProjection {
        should_apply_snapshot,
        last_published_highlight_id: if should_apply_snapshot {
            input.published_highlight_id
        } else {
            String::new()
        },
    }
}

pub fn snapshot_with_published_highlight(
    snapshot: ArticleReaderSnapshot,
    highlight: &HighlightRecord,
) -> ArticleReaderSnapshot {
    ArticleReaderSnapshot {
        article: snapshot.article,
        author_profile: snapshot.author_profile,
        highlights: highlights::insert_unique_front(&snapshot.highlights, highlight),
    }
}

/// Full reader snapshot for one NIP-23 article. Individual cache failures are
/// non-fatal: a missing or failed article/profile becomes `None`, and a failed
/// highlight query becomes an empty overlay list. That matches the reader's
/// network-backfill behavior while keeping the fallback policy in Rust.
pub fn query_article_reader_snapshot(
    ndb: &Ndb,
    pubkey_hex: &str,
    d_tag: &str,
) -> ArticleReaderSnapshot {
    let pubkey = pubkey_hex.trim();
    let d_tag = d_tag.trim();
    if pubkey.is_empty() || d_tag.is_empty() {
        return ArticleReaderSnapshot::empty();
    }
    let address = articles::article_address(pubkey, d_tag);

    ArticleReaderSnapshot {
        article: optional_section_or_none("article", articles::query_article(ndb, pubkey, d_tag)),
        author_profile: optional_section_or_none(
            "author_profile",
            profile::query_profile_from_ndb(ndb, pubkey),
        ),
        highlights: list_section_or_empty(
            "highlights",
            highlights::query_for_article(ndb, &address, ARTICLE_READER_HIGHLIGHT_LIMIT),
        ),
    }
}

fn optional_section_or_none<T>(
    section: &'static str,
    result: Result<Option<T>, CoreError>,
) -> Option<T> {
    match result {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(section, error = %error, "article reader snapshot section failed");
            None
        }
    }
}

fn list_section_or_empty<T>(section: &'static str, result: Result<Vec<T>, CoreError>) -> Vec<T> {
    match result {
        Ok(values) => values,
        Err(error) => {
            tracing::warn!(section, error = %error, "article reader snapshot section failed");
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr_sdk::prelude::*;
    use tempfile::TempDir;

    fn fresh_ndb() -> (Ndb, TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = nostrdb::Config::new().set_mapsize(64 * 1024 * 1024);
        let ndb = Ndb::new(tmp.path().to_str().unwrap(), &cfg).unwrap();
        (ndb, tmp)
    }

    fn ndb_with_events(events: &[&Event]) -> (Ndb, TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = nostrdb::Config::new().set_mapsize(64 * 1024 * 1024);
        let db_path = tmp.path().to_str().unwrap().to_owned();
        {
            let ndb = Ndb::new(&db_path, &cfg).unwrap();
            for event in events {
                let line = format!("[\"EVENT\",\"sub\",{}]", event.as_json());
                ndb.process_event(&line).unwrap();
            }
        }
        let ndb = Ndb::new(&db_path, &cfg).unwrap();
        (ndb, tmp)
    }

    fn named(name: &str, value: &str) -> Tag {
        Tag::parse(vec![name.to_string(), value.to_string()]).unwrap()
    }

    fn article(title: &str) -> ArticleRecord {
        ArticleRecord {
            event_id: format!("event-{title}"),
            address: format!("30023:author:{title}"),
            pubkey: "author".into(),
            identifier: title.into(),
            title: title.into(),
            summary: String::new(),
            image: String::new(),
            content: String::new(),
            hashtags: Vec::new(),
            published_at: None,
            created_at: None,
        }
    }

    fn profile(name: &str) -> ProfileMetadata {
        ProfileMetadata {
            pubkey: "author".into(),
            name: name.into(),
            display_name: name.into(),
            about: String::new(),
            picture: String::new(),
            banner: String::new(),
            nip05: String::new(),
            website: String::new(),
            lud16: String::new(),
            created_at: None,
        }
    }

    fn highlight(event_id: &str) -> HighlightRecord {
        HighlightRecord {
            event_id: event_id.into(),
            pubkey: "reader".into(),
            quote: event_id.into(),
            context: String::new(),
            note: String::new(),
            artifact_address: "30023:author:essay".into(),
            event_reference: String::new(),
            external_reference: String::new(),
            source_url: String::new(),
            source_reference_key: "a:30023:author:essay".into(),
            clip_start_seconds: None,
            clip_end_seconds: None,
            clip_speaker: String::new(),
            clip_transcript_segment_ids: Vec::new(),
            image_url: String::new(),
            created_at: None,
        }
    }

    #[test]
    fn article_reader_snapshot_hydrates_reader_dependencies() {
        let author = Keys::generate();
        let highlighter = Keys::generate();
        let address = articles::article_address(&author.public_key().to_hex(), "essay");

        let article = EventBuilder::new(Kind::Custom(articles::KIND_LONG_FORM), "body")
            .tags(vec![
                Tag::identifier("essay"),
                named("title", "Reader title"),
                named("published_at", "1000"),
            ])
            .custom_created_at(Timestamp::from(1_100))
            .sign_with_keys(&author)
            .unwrap();
        let profile = EventBuilder::new(
            Kind::Custom(0),
            r#"{"display_name":"Reader Author","name":"author"}"#,
        )
        .custom_created_at(Timestamp::from(1_200))
        .sign_with_keys(&author)
        .unwrap();
        let highlight = EventBuilder::new(Kind::Custom(9802), "quote")
            .tags(vec![named("a", &address), named("comment", "note")])
            .custom_created_at(Timestamp::from(1_300))
            .sign_with_keys(&highlighter)
            .unwrap();

        let (ndb, _tmp) = ndb_with_events(&[&article, &profile, &highlight]);

        let snapshot = query_article_reader_snapshot(&ndb, &author.public_key().to_hex(), "essay");

        assert_eq!(
            snapshot.article.as_ref().map(|a| a.title.as_str()),
            Some("Reader title")
        );
        assert_eq!(
            snapshot
                .author_profile
                .as_ref()
                .map(|profile| profile.display_name.as_str()),
            Some("Reader Author")
        );
        assert_eq!(snapshot.highlights.len(), 1);
        assert_eq!(snapshot.highlights[0].quote, "quote");
        assert_eq!(snapshot.highlights[0].note, "note");
    }

    #[test]
    fn article_reader_snapshot_uses_empty_state_for_blank_target() {
        let (ndb, _tmp) = fresh_ndb();
        let snapshot = query_article_reader_snapshot(&ndb, "", "");

        assert!(snapshot.article.is_none());
        assert!(snapshot.author_profile.is_none());
        assert!(snapshot.highlights.is_empty());
    }

    #[test]
    fn article_reader_snapshot_projection_preserves_seed_fallbacks() {
        let seed_article = article("seed");
        let seed_profile = profile("Seed Author");
        let loaded_highlight = highlight("highlight-1");
        let projection = article_reader_snapshot_projection(ArticleReaderSnapshotApplyInput {
            snapshot: ArticleReaderSnapshot {
                article: None,
                author_profile: None,
                highlights: vec![loaded_highlight.clone()],
            },
            current_article: Some(seed_article.clone()),
            current_author_profile: Some(seed_profile.clone()),
        });

        assert_eq!(
            projection
                .article
                .as_ref()
                .map(|article| article.title.as_str()),
            Some("seed")
        );
        assert_eq!(
            projection
                .author_profile
                .as_ref()
                .map(|profile| profile.display_name.as_str()),
            Some("Seed Author")
        );
        assert_eq!(projection.highlights[0].event_id, loaded_highlight.event_id);

        let loaded_article = article("loaded");
        let loaded_profile = profile("Loaded Author");
        let projection = article_reader_snapshot_projection(ArticleReaderSnapshotApplyInput {
            snapshot: ArticleReaderSnapshot {
                article: Some(loaded_article.clone()),
                author_profile: Some(loaded_profile.clone()),
                highlights: Vec::new(),
            },
            current_article: Some(seed_article),
            current_author_profile: Some(seed_profile),
        });

        assert_eq!(
            projection
                .article
                .as_ref()
                .map(|article| article.title.as_str()),
            Some("loaded")
        );
        assert_eq!(
            projection
                .author_profile
                .as_ref()
                .map(|profile| profile.display_name.as_str()),
            Some("Loaded Author")
        );
    }

    #[test]
    fn article_reader_publish_result_projection_applies_success_only() {
        let success = article_reader_publish_result_projection(ArticleReaderPublishResultInput {
            error: String::new(),
            published_highlight_id: "highlight-1".into(),
        });
        assert!(success.should_apply_snapshot);
        assert_eq!(success.last_published_highlight_id, "highlight-1");

        let failure = article_reader_publish_result_projection(ArticleReaderPublishResultInput {
            error: "publish failed".into(),
            published_highlight_id: "highlight-1".into(),
        });
        assert!(!failure.should_apply_snapshot);
        assert_eq!(failure.last_published_highlight_id, "");
    }

    #[test]
    fn article_reader_snapshot_with_published_highlight_dedupes_front() {
        let existing = HighlightRecord {
            event_id: "existing".into(),
            pubkey: "reader".into(),
            quote: "old".into(),
            context: String::new(),
            note: String::new(),
            artifact_address: "30023:author:essay".into(),
            event_reference: String::new(),
            external_reference: String::new(),
            source_url: String::new(),
            source_reference_key: "a:30023:author:essay".into(),
            clip_start_seconds: None,
            clip_end_seconds: None,
            clip_speaker: String::new(),
            clip_transcript_segment_ids: Vec::new(),
            image_url: String::new(),
            created_at: Some(10),
        };
        let published = HighlightRecord {
            event_id: "published".into(),
            quote: "new".into(),
            ..existing.clone()
        };
        let snapshot = ArticleReaderSnapshot {
            article: None,
            author_profile: None,
            highlights: vec![existing.clone()],
        };

        let projected = snapshot_with_published_highlight(snapshot, &published);
        let duplicate = snapshot_with_published_highlight(projected.clone(), &published);

        assert_eq!(
            projected
                .highlights
                .iter()
                .map(|highlight| highlight.event_id.as_str())
                .collect::<Vec<_>>(),
            vec!["published", "existing"]
        );
        assert_eq!(duplicate.highlights.len(), 2);
    }
}
