//! Home feed composition.
//!
//! The native shells own observation and rendering; Rust owns how highlight
//! modules and following-read rows merge into a single bounded feed snapshot.

use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};

use crate::models::{HomeFeedItem, HydratedHighlight, ReadingFeedItem};

#[derive(Debug, Clone, uniffi::Record)]
pub struct HomeFeedSnapshot {
    pub items: Vec<HomeFeedItem>,
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct HomeFeedSnapshotApplyInput {
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct HomeFeedSnapshotApplyProjection {
    pub load_error: Option<String>,
}

pub fn snapshot(
    highlights: Vec<HydratedHighlight>,
    reads: Vec<ReadingFeedItem>,
) -> HomeFeedSnapshot {
    HomeFeedSnapshot {
        items: build_items(&highlights, &reads),
        error: String::new(),
    }
}

pub fn error_snapshot(error: impl ToString) -> HomeFeedSnapshot {
    HomeFeedSnapshot {
        items: Vec::new(),
        error: error.to_string(),
    }
}

pub fn snapshot_apply_projection(
    input: HomeFeedSnapshotApplyInput,
) -> HomeFeedSnapshotApplyProjection {
    let error_message = input.error.trim().to_string();
    HomeFeedSnapshotApplyProjection {
        load_error: if error_message.is_empty() {
            None
        } else {
            Some(error_message)
        },
    }
}

pub fn build_items(
    highlights: &[HydratedHighlight],
    reads: &[ReadingFeedItem],
) -> Vec<HomeFeedItem> {
    let mut group_map: HashMap<String, Vec<HydratedHighlight>> = HashMap::new();
    let mut group_order: Vec<String> = Vec::new();

    for highlight in highlights {
        let key = group_key(highlight)
            .unwrap_or_else(|| format!("solo:{}", highlight.highlight.event_id));
        if let Entry::Vacant(entry) = group_map.entry(key.clone()) {
            group_order.push(key.clone());
            entry.insert(Vec::new());
        }
        group_map
            .get_mut(&key)
            .expect("group inserted before push")
            .push(highlight.clone());
    }

    let highlighted_addresses: HashSet<String> = highlights
        .iter()
        .filter_map(|highlight| non_empty_trimmed(&highlight.highlight.artifact_address))
        .collect();

    let mut items: Vec<HomeFeedItem> = Vec::with_capacity(group_order.len() + reads.len());
    for key in group_order {
        let mut group = group_map.remove(&key).unwrap_or_default();
        group.sort_by(|a, b| {
            a.highlight
                .created_at
                .unwrap_or(0)
                .cmp(&b.highlight.created_at.unwrap_or(0))
        });
        let sort_key = group
            .iter()
            .filter_map(|highlight| highlight.highlight.created_at)
            .max()
            .unwrap_or(0);
        items.push(HomeFeedItem {
            stable_id: highlight_stable_id(&group),
            sort_key,
            highlights: group,
            read: None,
        });
    }

    for read in reads {
        if highlighted_addresses.contains(read.article.address.trim()) {
            continue;
        }
        items.push(HomeFeedItem {
            stable_id: format!("r:{}", read.article.address),
            sort_key: read.latest_activity_at,
            highlights: Vec::new(),
            read: Some(read.clone()),
        });
    }

    items.sort_by(|a, b| b.sort_key.cmp(&a.sort_key));
    items
}

fn group_key(highlight: &HydratedHighlight) -> Option<String> {
    non_empty_trimmed(&highlight.highlight.source_reference_key)
}

fn highlight_stable_id(group: &[HydratedHighlight]) -> String {
    let Some(first) = group.first() else {
        return "h:empty".to_string();
    };
    if let Some(address) = non_empty_trimmed(&first.highlight.artifact_address) {
        return format!("h:src:{address}");
    }
    if let Some(source_url) = non_empty_trimmed(&first.highlight.source_url) {
        return format!("h:src:{source_url}");
    }
    format!("h:evt:{}", first.highlight.event_id)
}

fn non_empty_trimmed(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ArticleRecord, HighlightRecord};

    #[test]
    fn build_items_groups_highlights_suppresses_reads_and_sorts() {
        let first = hydrated_highlight("h1", "ref:article", "addr1", "", Some(10));
        let second = hydrated_highlight("h2", "ref:article", "addr1", "", Some(20));
        let solo = hydrated_highlight("solo", "", "", "", Some(40));
        let duplicate_read = reading_item("addr1", 60);
        let read = reading_item("addr2", 15);

        let items = build_items(&[second, first, solo], &[duplicate_read, read]);

        assert_eq!(
            items
                .iter()
                .map(|item| item.stable_id.as_str())
                .collect::<Vec<_>>(),
            vec!["h:evt:solo", "h:src:addr1", "r:addr2"]
        );
        assert_eq!(
            items[1]
                .highlights
                .iter()
                .map(|item| item.highlight.event_id.as_str())
                .collect::<Vec<_>>(),
            vec!["h1", "h2"]
        );
        assert!(items[2].read.is_some());
    }

    #[test]
    fn build_items_uses_source_url_for_stable_id_without_address() {
        let highlight = hydrated_highlight("h1", "ref:url", "", "https://example.com", Some(5));

        let items = build_items(&[highlight], &[]);

        assert_eq!(items[0].stable_id, "h:src:https://example.com");
    }

    #[test]
    fn snapshot_apply_projection_normalizes_load_error() {
        let success = snapshot_apply_projection(HomeFeedSnapshotApplyInput {
            error: String::new(),
        });
        assert_eq!(success.load_error, None);

        let failure = snapshot_apply_projection(HomeFeedSnapshotApplyInput {
            error: " refresh failed ".into(),
        });
        assert_eq!(failure.load_error.as_deref(), Some("refresh failed"));
    }

    fn hydrated_highlight(
        event_id: &str,
        source_reference_key: &str,
        artifact_address: &str,
        source_url: &str,
        created_at: Option<u64>,
    ) -> HydratedHighlight {
        HydratedHighlight {
            highlight: HighlightRecord {
                event_id: event_id.into(),
                pubkey: "pubkey".into(),
                quote: String::new(),
                context: String::new(),
                note: String::new(),
                artifact_address: artifact_address.into(),
                event_reference: String::new(),
                external_reference: String::new(),
                source_url: source_url.into(),
                source_reference_key: source_reference_key.into(),
                clip_start_seconds: None,
                clip_end_seconds: None,
                clip_speaker: String::new(),
                clip_transcript_segment_ids: Vec::new(),
                image_url: String::new(),
                created_at,
            },
            artifact: None,
            shared_by_event_id: None,
            shared_by_pubkey: None,
        }
    }

    fn reading_item(address: &str, latest_activity_at: u64) -> ReadingFeedItem {
        ReadingFeedItem {
            article: ArticleRecord {
                event_id: format!("event-{address}"),
                address: address.into(),
                pubkey: "pubkey".into(),
                identifier: "identifier".into(),
                title: String::new(),
                summary: String::new(),
                image: String::new(),
                content: String::new(),
                hashtags: Vec::new(),
                published_at: None,
                created_at: Some(latest_activity_at),
            },
            author_followed: false,
            interactor_pubkeys: Vec::new(),
            latest_activity_at,
        }
    }
}
