//! NIP-84 highlights (kind:9802) + cross-community shares (kind:16). Ports
//! `web/src/lib/ndk/highlights.ts`.

use nostr_sdk::prelude::*;
use nostrdb::{Filter as NdbFilter, Ndb, Transaction};

use crate::errors::CoreError;
use crate::models::{ArtifactRecord, HighlightDraft, HighlightRecord, HydratedHighlight};
use crate::nostr_runtime::NostrRuntime;
use crate::relays::HIGHLIGHTER_RELAY;

/// NIP-84 highlight event.
const KIND_HIGHLIGHT: u16 = 9802;
/// NIP-18 generic repost used to share a highlight into a NIP-29 community.
const KIND_GENERIC_REPOST: u16 = 16;

/// Port of `publishAndShareHighlight` (`highlights.ts:288-319`).
/// 1. Publishes the canonical kind:9802 highlight on the user's write relays.
/// 2. Publishes a kind:16 repost tagged `h=target_group_id` on the group's relay.
///
/// Returns records in the same order as `drafts`.
pub async fn publish_and_share(
    runtime: &NostrRuntime,
    artifact: ArtifactRecord,
    drafts: Vec<HighlightDraft>,
    target_group_id: &str,
) -> Result<Vec<HighlightRecord>, CoreError> {
    if target_group_id.trim().is_empty() {
        return Err(CoreError::InvalidInput(
            "target_group_id must not be empty".into(),
        ));
    }

    let client = runtime.client();

    // Resolve author pubkey once from the installed signer. We need it for the
    // repost's `p` tag and for the returned `HighlightRecord.pubkey`.
    let signer = client
        .signer()
        .await
        .map_err(|e| CoreError::Signer(format!("no signer installed: {e}")))?;
    let author_pubkey = signer
        .get_public_key()
        .await
        .map_err(|e| CoreError::Signer(format!("get_public_key failed: {e}")))?;
    let author_pubkey_hex = author_pubkey.to_hex();

    let mut records: Vec<HighlightRecord> = Vec::with_capacity(drafts.len());

    for draft in drafts {
        // 1. Build + sign + publish the canonical highlight.
        let builder = build_highlight_event(&draft, &artifact)?;
        let highlight_event = client
            .sign_event_builder(builder)
            .await
            .map_err(|e| CoreError::Signer(format!("sign highlight: {e}")))?;
        client
            .send_event(&highlight_event)
            .await
            .map_err(|e| CoreError::Relay(format!("publish highlight: {e}")))?;

        // 2. Build + sign + publish the kind:16 repost into the target group.
        let repost_builder = build_repost_event(
            highlight_event.id,
            &author_pubkey_hex,
            target_group_id,
            HIGHLIGHTER_RELAY,
        )?;
        let repost_event = client
            .sign_event_builder(repost_builder)
            .await
            .map_err(|e| CoreError::Signer(format!("sign repost: {e}")))?;
        client
            .send_event(&repost_event)
            .await
            .map_err(|e| CoreError::Relay(format!("publish repost: {e}")))?;

        // 3. Build the HighlightRecord to return.
        records.push(record_from_event(&highlight_event, &draft, &artifact));
    }

    Ok(records)
}

/// Port of `shareHighlightToCommunity` (`highlights.ts:321-357`).
/// Publishes a kind:16 repost referencing an existing highlight into another group.
pub async fn share_to_community(
    runtime: &NostrRuntime,
    highlight_id: &str,
    highlight_author_pubkey: &str,
    highlight_relay_url: &str,
    target_group_id: &str,
) -> Result<(), CoreError> {
    if target_group_id.trim().is_empty() {
        return Err(CoreError::InvalidInput(
            "target_group_id must not be empty".into(),
        ));
    }
    let event_id = EventId::from_hex(highlight_id)
        .map_err(|e| CoreError::InvalidInput(format!("invalid highlight id: {e}")))?;
    // Validate (but don't retain) the author pubkey.
    PublicKey::from_hex(highlight_author_pubkey)
        .map_err(|e| CoreError::InvalidInput(format!("invalid author pubkey: {e}")))?;

    let relay_hint = if highlight_relay_url.trim().is_empty() {
        HIGHLIGHTER_RELAY
    } else {
        highlight_relay_url
    };

    let client = runtime.client();
    let builder = build_repost_event(
        event_id,
        highlight_author_pubkey,
        target_group_id,
        relay_hint,
    )?;
    let event = client
        .sign_event_builder(builder)
        .await
        .map_err(|e| CoreError::Signer(format!("sign repost: {e}")))?;
    client
        .send_event(&event)
        .await
        .map_err(|e| CoreError::Relay(format!("publish repost: {e}")))?;
    Ok(())
}

/// Port of `hydrateHighlights`. Given a set of highlights, look up the
/// artifact each references and return joined records.
pub fn hydrate(
    _highlights: Vec<HighlightRecord>,
) -> Result<Vec<HydratedHighlight>, CoreError> {
    todo!()
}

/// Read highlights referencing the given NIP-23 article address
/// (`30023:<pubkey>:<d>`) from nostrdb, newest first. Used by the article
/// reader to overlay existing highlights on the body.
pub fn query_for_article(
    ndb: &Ndb,
    address: &str,
    limit: u32,
) -> Result<Vec<HighlightRecord>, CoreError> {
    let address = address.trim();
    if address.is_empty() {
        return Ok(Vec::new());
    }
    let txn = Transaction::new(ndb)
        .map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;

    let ndb_cap = limit.max(32) as i32;
    let filter = NdbFilter::new()
        .kinds([KIND_HIGHLIGHT as u64])
        .tags([address], 'a')
        .build();

    let results = ndb
        .query(&txn, &[filter], ndb_cap)
        .map_err(|e| CoreError::Cache(format!("query highlights for article: {e}")))?;

    let mut records: Vec<HighlightRecord> = Vec::with_capacity(results.len());
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        if let Some(rec) = record_from_cached_event(&event) {
            records.push(rec);
        }
    }
    records.sort_by(|a, b| b.created_at.unwrap_or(0).cmp(&a.created_at.unwrap_or(0)));
    records.truncate(limit as usize);
    Ok(records)
}

/// Publish a solo NIP-84 highlight without any NIP-29 repost. Variant of
/// `publish_and_share` for reader flows that save to the user's vault only —
/// sharing into a community is a later, explicit action.
pub async fn publish(
    runtime: &NostrRuntime,
    draft: HighlightDraft,
    artifact: ArtifactRecord,
) -> Result<HighlightRecord, CoreError> {
    let client = runtime.client();
    let builder = build_highlight_event(&draft, &artifact)?;
    let event = client
        .sign_event_builder(builder)
        .await
        .map_err(|e| CoreError::Signer(format!("sign highlight: {e}")))?;
    client
        .send_event(&event)
        .await
        .map_err(|e| CoreError::Relay(format!("publish highlight: {e}")))?;
    Ok(record_from_event(&event, &draft, &artifact))
}

/// Read highlights authored by `pubkey_hex` from nostrdb, newest first.
/// Used both for the profile page's Highlights tab and for the vault view.
pub fn query_highlights_by_author(
    ndb: &Ndb,
    pubkey_hex: &str,
    limit: u32,
) -> Result<Vec<HighlightRecord>, CoreError> {
    if pubkey_hex.is_empty() {
        return Ok(Vec::new());
    }
    let author = PublicKey::from_hex(pubkey_hex)
        .map_err(|e| CoreError::InvalidInput(format!("invalid pubkey: {e}")))?;

    let txn = Transaction::new(ndb)
        .map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;

    let pk_bytes: [u8; 32] = author.to_bytes();
    let ndb_cap = limit.max(32) as i32;
    let filter = NdbFilter::new()
        .kinds([KIND_HIGHLIGHT as u64])
        .authors([&pk_bytes])
        .build();

    let results = ndb
        .query(&txn, &[filter], ndb_cap)
        .map_err(|e| CoreError::Cache(format!("query highlights: {e}")))?;

    let mut records: Vec<HighlightRecord> = Vec::with_capacity(results.len());
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        if let Some(rec) = record_from_cached_event(&event) {
            records.push(rec);
        }
    }

    records.sort_by(|a, b| b.created_at.unwrap_or(0).cmp(&a.created_at.unwrap_or(0)));
    records.truncate(limit as usize);
    Ok(records)
}

/// Pure: build a `HighlightRecord` from an already-cached kind:9802 event.
/// Separate from `record_from_event` above, which relies on the draft for
/// clip fields known up front.
fn record_from_cached_event(event: &Event) -> Option<HighlightRecord> {
    if event.kind.as_u16() != KIND_HIGHLIGHT {
        return None;
    }
    let artifact_address = first_tag_value(event, "a").unwrap_or("").to_string();
    let event_reference = first_tag_value(event, "e").unwrap_or("").to_string();
    let source_url = first_tag_value(event, "r").unwrap_or("").to_string();
    let context = first_tag_value(event, "context").unwrap_or("").to_string();
    let comment = first_tag_value(event, "comment").unwrap_or("").to_string();

    let source_reference_key = if !artifact_address.is_empty() {
        format!("a:{artifact_address}")
    } else if !event_reference.is_empty() {
        format!("e:{event_reference}")
    } else if !source_url.is_empty() {
        format!("r:{source_url}")
    } else {
        String::new()
    };

    let clip_start_seconds = first_tag_value(event, "start").and_then(|s| s.trim().parse().ok());
    let clip_end_seconds = first_tag_value(event, "end").and_then(|s| s.trim().parse().ok());
    let clip_speaker = first_tag_value(event, "speaker").unwrap_or("").to_string();
    let clip_transcript_segment_ids: Vec<String> = event
        .tags
        .iter()
        .filter_map(|tag| {
            let s = tag.as_slice();
            if s.first().map(String::as_str) == Some("segment") {
                s.get(1).map(|v| v.to_string())
            } else {
                None
            }
        })
        .collect();

    Some(HighlightRecord {
        event_id: event.id.to_hex(),
        pubkey: event.pubkey.to_hex(),
        quote: event.content.clone(),
        context,
        note: comment,
        artifact_address,
        event_reference,
        source_url,
        source_reference_key,
        clip_start_seconds,
        clip_end_seconds,
        clip_speaker,
        clip_transcript_segment_ids,
        created_at: Some(event.created_at.as_secs()),
    })
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

// -- Builders (pure: no IO, unit-testable) --

/// Build the kind:9802 highlight `EventBuilder`. Pure — safe to unit test.
/// Matches `publishCanonicalHighlight` (highlights.ts:359-423).
fn build_highlight_event(
    draft: &HighlightDraft,
    artifact: &ArtifactRecord,
) -> Result<EventBuilder, CoreError> {
    let quote = draft.quote.trim();
    let has_clip = draft.clip_start_seconds.is_some() && draft.clip_end_seconds.is_some();
    if quote.is_empty() && !has_clip {
        return Err(CoreError::InvalidInput(
            "highlight must have a quote or a clip".into(),
        ));
    }

    let content = if quote.is_empty() {
        build_clip_fallback_quote(
            draft.clip_start_seconds.unwrap_or(0.0),
            draft.clip_end_seconds.unwrap_or(0.0),
        )
    } else {
        quote.to_string()
    };

    let mut tags: Vec<Tag> = Vec::new();

    // Source reference tag: one of ("a", addr), ("e", id), or ("r", url).
    let ref_name = artifact.preview.highlight_tag_name.trim();
    let ref_value = artifact.preview.highlight_tag_value.trim();
    if ref_name.is_empty() || ref_value.is_empty() {
        return Err(CoreError::InvalidInput(
            "artifact missing highlight reference tag".into(),
        ));
    }
    tags.push(
        Tag::parse(vec![ref_name.to_string(), ref_value.to_string()])
            .map_err(|e| CoreError::Other(format!("build reference tag: {e}")))?,
    );

    // Context tag: only if differs from content.
    let context = draft.context.trim();
    if !context.is_empty() && context != content {
        tags.push(
            Tag::parse(vec!["context".to_string(), context.to_string()])
                .map_err(|e| CoreError::Other(format!("build context tag: {e}")))?,
        );
    }

    // Comment tag.
    let note = draft.note.trim();
    if !note.is_empty() {
        tags.push(
            Tag::parse(vec!["comment".to_string(), note.to_string()])
                .map_err(|e| CoreError::Other(format!("build comment tag: {e}")))?,
        );
    }

    // Clip tags. Start/end always appear together (both Some) or not at all.
    // The TS code emits them with `.toFixed(3)` — 3 decimal places, rounded.
    if let (Some(start), Some(end)) = (draft.clip_start_seconds, draft.clip_end_seconds) {
        tags.push(
            Tag::parse(vec!["start".to_string(), format!("{:.3}", start)])
                .map_err(|e| CoreError::Other(format!("build start tag: {e}")))?,
        );
        tags.push(
            Tag::parse(vec!["end".to_string(), format!("{:.3}", end)])
                .map_err(|e| CoreError::Other(format!("build end tag: {e}")))?,
        );

        let speaker = draft.clip_speaker.trim();
        if !speaker.is_empty() {
            tags.push(
                Tag::parse(vec!["speaker".to_string(), speaker.to_string()])
                    .map_err(|e| CoreError::Other(format!("build speaker tag: {e}")))?,
            );
        }

        for segment_id in &draft.clip_transcript_segment_ids {
            let segment_id = segment_id.trim();
            if segment_id.is_empty() {
                continue;
            }
            tags.push(
                Tag::parse(vec!["segment".to_string(), segment_id.to_string()])
                    .map_err(|e| CoreError::Other(format!("build segment tag: {e}")))?,
            );
        }
    }

    Ok(EventBuilder::new(Kind::Custom(KIND_HIGHLIGHT), content).tags(tags))
}

/// Build the kind:16 repost `EventBuilder` that shares a highlight into a
/// NIP-29 community. Pure — safe to unit test.
fn build_repost_event(
    highlight_event_id: EventId,
    highlight_author_pubkey_hex: &str,
    target_group_id: &str,
    relay_hint: &str,
) -> Result<EventBuilder, CoreError> {
    let author_pk = PublicKey::from_hex(highlight_author_pubkey_hex)
        .map_err(|e| CoreError::InvalidInput(format!("invalid author pubkey: {e}")))?;

    let e_tag = Tag::parse(vec![
        "e".to_string(),
        highlight_event_id.to_hex(),
        relay_hint.to_string(),
    ])
    .map_err(|e| CoreError::Other(format!("build e tag: {e}")))?;

    let k_tag = Tag::parse(vec!["k".to_string(), KIND_HIGHLIGHT.to_string()])
        .map_err(|e| CoreError::Other(format!("build k tag: {e}")))?;

    let p_tag = Tag::public_key(author_pk);

    let h_tag = Tag::parse(vec!["h".to_string(), target_group_id.to_string()])
        .map_err(|e| CoreError::Other(format!("build h tag: {e}")))?;

    Ok(EventBuilder::new(Kind::Custom(KIND_GENERIC_REPOST), "")
        .tags(vec![e_tag, k_tag, p_tag, h_tag]))
}

/// Build a `HighlightRecord` from the signed highlight event + the draft we
/// sent. Mirrors `highlightFromEvent` (highlights.ts:56-82) but uses the
/// draft directly (no re-parsing) for clip fields that are known up front.
fn record_from_event(
    event: &Event,
    draft: &HighlightDraft,
    artifact: &ArtifactRecord,
) -> HighlightRecord {
    let ref_name = artifact.preview.highlight_tag_name.as_str();
    let ref_value = artifact.preview.highlight_tag_value.as_str();

    let (artifact_address, event_reference, source_url) = match ref_name {
        "a" => (ref_value.to_string(), String::new(), String::new()),
        "e" => (String::new(), ref_value.to_string(), String::new()),
        "r" => (String::new(), String::new(), ref_value.to_string()),
        _ => (String::new(), String::new(), String::new()),
    };

    let source_reference_key = if !artifact_address.is_empty() {
        format!("a:{artifact_address}")
    } else if !event_reference.is_empty() {
        format!("e:{event_reference}")
    } else if !source_url.is_empty() {
        format!("r:{source_url}")
    } else {
        String::new()
    };

    HighlightRecord {
        event_id: event.id.to_hex(),
        pubkey: event.pubkey.to_hex(),
        quote: event.content.clone(),
        context: draft.context.trim().to_string(),
        note: draft.note.trim().to_string(),
        artifact_address,
        event_reference,
        source_url,
        source_reference_key,
        clip_start_seconds: draft.clip_start_seconds,
        clip_end_seconds: draft.clip_end_seconds,
        clip_speaker: draft.clip_speaker.trim().to_string(),
        clip_transcript_segment_ids: draft
            .clip_transcript_segment_ids
            .iter()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        created_at: Some(event.created_at.as_secs()),
    }
}

fn build_clip_fallback_quote(start: f64, end: f64) -> String {
    format!("Clip {}-{}", format_clip_time(start), format_clip_time(end))
}

fn format_clip_time(value: f64) -> String {
    let total_seconds = if value.is_finite() && value > 0.0 {
        value.round() as u64
    } else {
        0
    };
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ArtifactPreview, ArtifactRecord, HighlightDraft};

    fn preview_for_podcast(url: &str) -> ArtifactPreview {
        ArtifactPreview {
            id: "id1".into(),
            url: url.into(),
            title: "Episode 1".into(),
            author: "Alice".into(),
            image: String::new(),
            description: String::new(),
            source: "podcast".into(),
            domain: "example.com".into(),
            catalog_id: String::new(),
            catalog_kind: String::new(),
            podcast_guid: "guid-1".into(),
            podcast_show_title: "Show".into(),
            audio_url: url.into(),
            audio_preview_url: String::new(),
            transcript_url: String::new(),
            feed_url: String::new(),
            published_at: String::new(),
            duration_seconds: Some(3600),
            reference_tag_name: "i".into(),
            reference_tag_value: format!("podcast:guid:{}", "guid-1"),
            reference_kind: "30054".into(),
            highlight_tag_name: "r".into(),
            highlight_tag_value: url.into(),
            highlight_reference_key: format!("r:{url}"),
        }
    }

    fn artifact_for_podcast(url: &str) -> ArtifactRecord {
        ArtifactRecord {
            preview: preview_for_podcast(url),
            group_id: "group-a".into(),
            share_event_id: "share-1".into(),
            pubkey: "f".repeat(64),
            created_at: Some(1_700_000_000),
            note: String::new(),
        }
    }

    fn draft_with_clip() -> HighlightDraft {
        HighlightDraft {
            quote: "the quote".into(),
            context: String::new(),
            note: String::new(),
            clip_start_seconds: Some(12.5),
            clip_end_seconds: Some(34.5678),
            clip_speaker: String::new(),
            clip_transcript_segment_ids: vec![],
        }
    }

    /// Collect tags into a Vec<Vec<String>> for easy assertion.
    fn tags_as_vec(builder: &EventBuilder) -> Vec<Vec<String>> {
        // EventBuilder doesn't expose its tag list. Sign with a throwaway
        // key to inspect the resulting event.
        let keys = Keys::generate();
        let event = builder
            .clone()
            .sign_with_keys(&keys)
            .expect("sign for inspection");
        event
            .tags
            .iter()
            .map(|t| t.as_slice().to_vec())
            .collect()
    }

    #[test]
    fn audio_clip_tags_use_3_decimal_format() {
        let artifact = artifact_for_podcast("https://example.com/ep1");
        let draft = draft_with_clip();
        let builder =
            build_highlight_event(&draft, &artifact).expect("build highlight event");
        let tags = tags_as_vec(&builder);

        let starts: Vec<_> = tags
            .iter()
            .filter(|t| t.first().map(String::as_str) == Some("start"))
            .collect();
        let ends: Vec<_> = tags
            .iter()
            .filter(|t| t.first().map(String::as_str) == Some("end"))
            .collect();

        assert_eq!(starts.len(), 1, "exactly one start tag");
        assert_eq!(ends.len(), 1, "exactly one end tag");
        assert_eq!(starts[0], &vec!["start".to_string(), "12.500".to_string()]);
        assert_eq!(ends[0], &vec!["end".to_string(), "34.568".to_string()]);
    }

    #[test]
    fn empty_speaker_produces_no_speaker_tag() {
        let artifact = artifact_for_podcast("https://example.com/ep1");
        let mut draft = draft_with_clip();
        draft.clip_speaker = String::new();
        let builder =
            build_highlight_event(&draft, &artifact).expect("build highlight event");
        let tags = tags_as_vec(&builder);

        assert!(
            !tags
                .iter()
                .any(|t| t.first().map(String::as_str) == Some("speaker")),
            "no speaker tag when speaker is empty, got: {tags:?}"
        );
    }

    #[test]
    fn multiple_segment_ids_produce_multiple_tags() {
        let artifact = artifact_for_podcast("https://example.com/ep1");
        let mut draft = draft_with_clip();
        draft.clip_transcript_segment_ids =
            vec!["a".into(), "b".into(), "c".into()];
        let builder =
            build_highlight_event(&draft, &artifact).expect("build highlight event");
        let tags = tags_as_vec(&builder);

        let segments: Vec<_> = tags
            .iter()
            .filter(|t| t.first().map(String::as_str) == Some("segment"))
            .collect();
        assert_eq!(segments.len(), 3, "one tag per segment id");
        assert_eq!(segments[0], &vec!["segment".to_string(), "a".to_string()]);
        assert_eq!(segments[1], &vec!["segment".to_string(), "b".to_string()]);
        assert_eq!(segments[2], &vec!["segment".to_string(), "c".to_string()]);
    }

    #[test]
    fn highlight_for_podcast_uses_r_tag() {
        let url = "https://example.com/ep";
        let artifact = artifact_for_podcast(url);
        let draft = HighlightDraft {
            quote: "hello".into(),
            context: String::new(),
            note: String::new(),
            clip_start_seconds: None,
            clip_end_seconds: None,
            clip_speaker: String::new(),
            clip_transcript_segment_ids: vec![],
        };
        let builder =
            build_highlight_event(&draft, &artifact).expect("build highlight event");
        let tags = tags_as_vec(&builder);

        // Source reference tags are only `a`, `e`, or `r` — there must be
        // exactly one reference tag and it must be ["r", url].
        let refs: Vec<_> = tags
            .iter()
            .filter(|t| {
                matches!(
                    t.first().map(String::as_str),
                    Some("a") | Some("e") | Some("r")
                )
            })
            .collect();
        assert_eq!(refs.len(), 1, "exactly one reference tag, got: {tags:?}");
        assert_eq!(refs[0], &vec!["r".to_string(), url.to_string()]);
    }

    #[test]
    fn comment_tag_emitted_for_note() {
        let artifact = artifact_for_podcast("https://example.com/ep1");
        let draft = HighlightDraft {
            quote: "q".into(),
            context: String::new(),
            note: "a note".into(),
            clip_start_seconds: None,
            clip_end_seconds: None,
            clip_speaker: String::new(),
            clip_transcript_segment_ids: vec![],
        };
        let builder =
            build_highlight_event(&draft, &artifact).expect("build highlight event");
        let tags = tags_as_vec(&builder);
        assert!(
            tags.iter().any(|t| t.as_slice()
                == ["comment".to_string(), "a note".to_string()]),
            "comment tag missing: {tags:?}"
        );
    }

    #[test]
    fn context_tag_omitted_when_equal_to_content() {
        let artifact = artifact_for_podcast("https://example.com/ep1");
        let draft = HighlightDraft {
            quote: "same".into(),
            context: "same".into(),
            note: String::new(),
            clip_start_seconds: None,
            clip_end_seconds: None,
            clip_speaker: String::new(),
            clip_transcript_segment_ids: vec![],
        };
        let builder =
            build_highlight_event(&draft, &artifact).expect("build highlight event");
        let tags = tags_as_vec(&builder);
        assert!(
            !tags
                .iter()
                .any(|t| t.first().map(String::as_str) == Some("context")),
            "context tag should be omitted when equal to content: {tags:?}"
        );
    }

    #[test]
    fn repost_event_has_required_tags() {
        // Use two distinct keys: `reposter` signs the kind:16 event, `author`
        // is the original highlight creator. EventBuilder auto-strips `p` tags
        // matching the signer, so self-references get filtered out — we want
        // to see the `p` tag survive.
        let reposter = Keys::generate();
        let author = Keys::generate();
        let highlight_id = EventId::all_zeros();
        let builder = build_repost_event(
            highlight_id,
            &author.public_key().to_hex(),
            "group-a",
            "wss://relay.highlighter.com",
        )
        .expect("build repost");
        let event = builder.sign_with_keys(&reposter).expect("sign");

        let tags: Vec<Vec<String>> =
            event.tags.iter().map(|t| t.as_slice().to_vec()).collect();

        assert_eq!(event.kind, Kind::Custom(KIND_GENERIC_REPOST));
        assert_eq!(event.content, "");

        let e = tags
            .iter()
            .find(|t| t.first().map(String::as_str) == Some("e"))
            .expect("e tag");
        assert_eq!(e.len(), 3);
        assert_eq!(e[1], highlight_id.to_hex());
        assert_eq!(e[2], "wss://relay.highlighter.com");

        let k = tags
            .iter()
            .find(|t| t.first().map(String::as_str) == Some("k"))
            .expect("k tag");
        assert_eq!(k[1], "9802");

        let p = tags
            .iter()
            .find(|t| t.first().map(String::as_str) == Some("p"))
            .expect("p tag");
        assert_eq!(p[1], author.public_key().to_hex());

        let h = tags
            .iter()
            .find(|t| t.first().map(String::as_str) == Some("h"))
            .expect("h tag");
        assert_eq!(h[1], "group-a");
    }

    #[test]
    fn query_for_article_returns_only_matching_a_tag() {
        use nostrdb::{Config as NdbConfig, Ndb};
        use tempfile::tempdir;

        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().to_str().unwrap();
        let ndb = Ndb::new(db_path, &NdbConfig::new().set_mapsize(64 * 1024 * 1024))
            .expect("open ndb");

        let keys = Keys::generate();
        let target_address = "30023:aabb:post-1";
        let other_address = "30023:aabb:post-2";

        let matching = EventBuilder::new(Kind::Custom(KIND_HIGHLIGHT), "matching quote")
            .tags(vec![
                Tag::parse(vec!["a".to_string(), target_address.to_string()]).unwrap(),
            ])
            .sign_with_keys(&keys)
            .expect("sign");
        let other = EventBuilder::new(Kind::Custom(KIND_HIGHLIGHT), "other quote")
            .tags(vec![
                Tag::parse(vec!["a".to_string(), other_address.to_string()]).unwrap(),
            ])
            .sign_with_keys(&keys)
            .expect("sign");

        for e in [&matching, &other] {
            let relay_line = format!("[\"EVENT\",\"s\",{}]", e.as_json());
            ndb.process_event(&relay_line).expect("process event");
        }
        std::thread::sleep(std::time::Duration::from_millis(100));

        let hits = query_for_article(&ndb, target_address, 32).expect("query");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].quote, "matching quote");
        assert_eq!(hits[0].artifact_address, target_address);
    }

    #[test]
    fn clip_fallback_quote_formats_hms() {
        assert_eq!(build_clip_fallback_quote(0.0, 65.0), "Clip 0:00-1:05");
        assert_eq!(
            build_clip_fallback_quote(3_600.0, 3_665.0),
            "Clip 1:00:00-1:01:05"
        );
    }
}
