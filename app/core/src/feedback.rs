//! In-app feedback threads scoped to a single project (a kind:31933 event).
//!
//! Each thread is rooted in a kind:1 note that `a`-tags the project's
//! addressable coordinate and `p`-tags the project's first registered agent.
//! Replies are kind:1 events `e`-tagged to the root (NIP-10 marked `root`).
//! A kind:513 metadata event (with an `e` tag matching the root) carries an
//! optional title/summary/status-label rendered in the conversation list.

use crate::models::{FeedbackEventRecord, FeedbackThreadRecord, ProfileMetadata};

pub const HIGHLIGHTER_PROJECT_COORDINATE: &str =
    "31933:09d48a1a5dbe13404a729634f1d6ba722d40513468dd713c8ea38ca9b7b6f2c7:highlighter";

pub const KIND_FEEDBACK_NOTE: u16 = 1;
pub const KIND_FEEDBACK_THREAD_META: u16 = 513;
pub const KIND_PROJECT_DEFINITION: u16 = 31933;
pub const FEEDBACK_THREAD_LIMIT: i32 = 256;
pub const FEEDBACK_THREAD_META_LIMIT: i32 = 512;
pub const FEEDBACK_THREAD_EVENT_LIMIT: i32 = 4096;

#[derive(Debug, Clone, uniffi::Record)]
pub struct FeedbackThreadsSnapshot {
    pub threads: Vec<FeedbackThreadRecord>,
    pub error: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FeedbackMessageRowProjection {
    pub event: FeedbackEventRecord,
    pub show_header: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FeedbackThreadSnapshot {
    pub rows: Vec<FeedbackMessageRowProjection>,
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FeedbackComposerProjectionInput {
    pub body: String,
    pub is_publishing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FeedbackComposerProjection {
    pub submit_body: String,
    pub can_send: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FeedbackPublishResultInput {
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FeedbackPublishResultProjection {
    pub did_publish: bool,
    pub error_message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FeedbackSnapshotApplyInput {
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FeedbackSnapshotApplyProjection {
    pub should_apply_snapshot: bool,
    pub load_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FeedbackThreadPresentationProjection {
    pub navigation_title: String,
    pub row_title: String,
    pub row_secondary_text: Option<String>,
    pub detail_summary: Option<String>,
    pub status_label: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct FeedbackMessagePresentationInput {
    pub event: FeedbackEventRecord,
    pub show_header: bool,
    pub current_user_pubkey: Option<String>,
    pub profile: Option<ProfileMetadata>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FeedbackMessagePresentationProjection {
    pub is_from_me: bool,
    pub show_header: bool,
    pub display_name: String,
    pub display_initial: String,
    pub picture_url: String,
}

pub fn feedback_composer_projection(
    input: FeedbackComposerProjectionInput,
) -> FeedbackComposerProjection {
    let submit_body = input.body.trim().to_string();
    FeedbackComposerProjection {
        can_send: !submit_body.is_empty() && !input.is_publishing,
        submit_body,
    }
}

pub fn feedback_publish_result_projection(
    input: FeedbackPublishResultInput,
) -> FeedbackPublishResultProjection {
    let error_message = input.error.trim().to_string();
    FeedbackPublishResultProjection {
        did_publish: error_message.is_empty(),
        error_message,
    }
}

pub fn feedback_snapshot_apply_projection(
    input: FeedbackSnapshotApplyInput,
) -> FeedbackSnapshotApplyProjection {
    let error_message = input.error.trim().to_string();
    let load_error = if error_message.is_empty() {
        None
    } else {
        Some(error_message)
    };
    FeedbackSnapshotApplyProjection {
        should_apply_snapshot: load_error.is_none(),
        load_error,
    }
}

pub fn feedback_message_presentation(
    input: FeedbackMessagePresentationInput,
) -> FeedbackMessagePresentationProjection {
    let is_from_me = input
        .current_user_pubkey
        .as_deref()
        .is_some_and(|pubkey| pubkey == input.event.author_pubkey);
    let display_name = profile_display_name(input.profile.as_ref(), &input.event.author_pubkey);
    let display_initial = display_initial(&display_name);
    let picture_url = input
        .profile
        .as_ref()
        .map(|profile| profile.picture.clone())
        .unwrap_or_default();

    FeedbackMessagePresentationProjection {
        is_from_me,
        show_header: input.show_header,
        display_name,
        display_initial,
        picture_url,
    }
}

pub fn feedback_thread_presentation(
    thread: FeedbackThreadRecord,
) -> FeedbackThreadPresentationProjection {
    let row_title = thread
        .title
        .clone()
        .unwrap_or_else(|| thread.preview.clone());
    let navigation_title = thread.title.clone().unwrap_or_else(|| "Feedback".into());
    let row_secondary_text = renderable_text(thread.summary.clone()).or_else(|| {
        if thread.title.is_some() && !thread.preview.is_empty() {
            Some(thread.preview.clone())
        } else {
            None
        }
    });
    let detail_summary = renderable_text(thread.summary);
    let status_label = renderable_text(thread.status_label);

    FeedbackThreadPresentationProjection {
        navigation_title,
        row_title,
        row_secondary_text,
        detail_summary,
        status_label,
    }
}

pub fn threads_snapshot_with_root(
    snapshot: FeedbackThreadsSnapshot,
    root_event: &FeedbackEventRecord,
) -> FeedbackThreadsSnapshot {
    FeedbackThreadsSnapshot {
        threads: optimistically_insert_root_thread(&snapshot.threads, root_event),
        error: snapshot.error,
    }
}

pub fn thread_snapshot_with_event(
    snapshot: FeedbackThreadSnapshot,
    event: &FeedbackEventRecord,
) -> FeedbackThreadSnapshot {
    let events: Vec<FeedbackEventRecord> = snapshot.rows.into_iter().map(|row| row.event).collect();
    FeedbackThreadSnapshot {
        rows: rows_for_events(upsert_thread_event(&events, event)),
        error: snapshot.error,
    }
}

/// Insert a freshly-published root event into a thread list before the relay
/// echo is indexed. Rust owns the root-only guard, preview policy, dedupe, and
/// newest-activity ordering.
pub fn optimistically_insert_root_thread(
    threads: &[FeedbackThreadRecord],
    root_event: &FeedbackEventRecord,
) -> Vec<FeedbackThreadRecord> {
    let mut out = threads.to_vec();
    if root_event.root_event_id != root_event.event_id {
        out.sort_by(|a, b| b.last_activity_at.cmp(&a.last_activity_at));
        return out;
    }
    if !out
        .iter()
        .any(|thread| thread.root_event_id == root_event.event_id)
    {
        out.push(FeedbackThreadRecord {
            root_event_id: root_event.event_id.clone(),
            author_pubkey: root_event.author_pubkey.clone(),
            created_at: root_event.created_at,
            last_activity_at: root_event.created_at,
            title: None,
            summary: None,
            status_label: None,
            preview: trim_preview(&root_event.content),
        });
    }
    out.sort_by(|a, b| b.last_activity_at.cmp(&a.last_activity_at));
    out
}

/// Upsert a streamed feedback-thread event into a bounded view snapshot.
/// Rust owns replacement identity and oldest-first chat ordering.
pub fn upsert_thread_event(
    events: &[FeedbackEventRecord],
    event: &FeedbackEventRecord,
) -> Vec<FeedbackEventRecord> {
    let mut out = Vec::with_capacity(events.len() + 1);
    let mut replaced = false;
    for existing in events {
        if existing.event_id == event.event_id {
            out.push(event.clone());
            replaced = true;
        } else {
            out.push(existing.clone());
        }
    }
    if !replaced {
        out.push(event.clone());
    }
    out.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    out
}

// --- helpers ---------------------------------------------------------------

fn rows_for_events(events: Vec<FeedbackEventRecord>) -> Vec<FeedbackMessageRowProjection> {
    events
        .iter()
        .enumerate()
        .map(|(index, event)| {
            let show_header = index == 0
                || events[index - 1].author_pubkey != event.author_pubkey
                || event.created_at > events[index - 1].created_at.saturating_add(300);
            FeedbackMessageRowProjection {
                event: event.clone(),
                show_header,
            }
        })
        .collect()
}

fn trim_preview(content: &str) -> String {
    let collapsed: String = content.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= 140 {
        collapsed
    } else {
        let mut truncated: String = collapsed.chars().take(139).collect();
        truncated.push('…');
        truncated
    }
}

fn renderable_text(value: Option<String>) -> Option<String> {
    value.filter(|text| !text.is_empty())
}

fn profile_display_name(profile: Option<&ProfileMetadata>, fallback_pubkey: &str) -> String {
    if let Some(profile) = profile {
        if !profile.display_name.is_empty() {
            return profile.display_name.clone();
        }
        if !profile.name.is_empty() {
            return profile.name.clone();
        }
    }
    fallback_pubkey.chars().take(8).collect()
}

fn display_initial(display_name: &str) -> String {
    display_name
        .chars()
        .next()
        .map(|ch| ch.to_uppercase().collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trim_preview_collapses_whitespace_and_truncates() {
        assert_eq!(trim_preview("hello   world"), "hello world");
        let long = "x".repeat(200);
        let out = trim_preview(&long);
        assert_eq!(out.chars().count(), 140);
        assert!(out.ends_with('…'));
    }

    #[test]
    fn feedback_composer_projection_trims_submit_body_and_allows_send() {
        let projection = feedback_composer_projection(FeedbackComposerProjectionInput {
            body: "  hello feedback  \n".into(),
            is_publishing: false,
        });

        assert_eq!(projection.submit_body, "hello feedback");
        assert!(projection.can_send);
    }

    #[test]
    fn feedback_composer_projection_blocks_blank_or_publishing_body() {
        let blank = feedback_composer_projection(FeedbackComposerProjectionInput {
            body: "  \n\t ".into(),
            is_publishing: false,
        });
        let publishing = feedback_composer_projection(FeedbackComposerProjectionInput {
            body: "ready".into(),
            is_publishing: true,
        });

        assert_eq!(blank.submit_body, "");
        assert!(!blank.can_send);
        assert_eq!(publishing.submit_body, "ready");
        assert!(!publishing.can_send);
    }

    #[test]
    fn feedback_publish_result_projection_classifies_success_and_error() {
        let success = feedback_publish_result_projection(FeedbackPublishResultInput {
            error: String::new(),
        });
        assert!(success.did_publish);
        assert_eq!(success.error_message, "");

        let failed = feedback_publish_result_projection(FeedbackPublishResultInput {
            error: " publish failed ".into(),
        });
        assert!(!failed.did_publish);
        assert_eq!(failed.error_message, "publish failed");
    }

    #[test]
    fn feedback_snapshot_apply_projection_blocks_error_snapshots() {
        let success = feedback_snapshot_apply_projection(FeedbackSnapshotApplyInput {
            error: String::new(),
        });
        assert!(success.should_apply_snapshot);
        assert_eq!(success.load_error, None);

        let failed = feedback_snapshot_apply_projection(FeedbackSnapshotApplyInput {
            error: " refresh failed ".into(),
        });
        assert!(!failed.should_apply_snapshot);
        assert_eq!(failed.load_error.as_deref(), Some("refresh failed"));
    }

    #[test]
    fn feedback_thread_presentation_uses_title_summary_status_and_detail_title() {
        let projection = feedback_thread_presentation(FeedbackThreadRecord {
            root_event_id: "root".into(),
            author_pubkey: "pubkey".into(),
            created_at: 1,
            last_activity_at: 2,
            title: Some("Title".into()),
            summary: Some("Summary".into()),
            status_label: Some("waiting".into()),
            preview: "Preview".into(),
        });

        assert_eq!(projection.navigation_title, "Title");
        assert_eq!(projection.row_title, "Title");
        assert_eq!(projection.row_secondary_text.as_deref(), Some("Summary"));
        assert_eq!(projection.detail_summary.as_deref(), Some("Summary"));
        assert_eq!(projection.status_label.as_deref(), Some("waiting"));
    }

    #[test]
    fn feedback_thread_presentation_preserves_preview_fallbacks() {
        let no_title = feedback_thread_presentation(FeedbackThreadRecord {
            root_event_id: "root".into(),
            author_pubkey: "pubkey".into(),
            created_at: 1,
            last_activity_at: 2,
            title: None,
            summary: None,
            status_label: Some(String::new()),
            preview: "Preview".into(),
        });
        let titled_without_summary = feedback_thread_presentation(FeedbackThreadRecord {
            root_event_id: "root".into(),
            author_pubkey: "pubkey".into(),
            created_at: 1,
            last_activity_at: 2,
            title: Some("Title".into()),
            summary: Some(String::new()),
            status_label: None,
            preview: "Preview".into(),
        });

        assert_eq!(no_title.navigation_title, "Feedback");
        assert_eq!(no_title.row_title, "Preview");
        assert_eq!(no_title.row_secondary_text, None);
        assert_eq!(no_title.detail_summary, None);
        assert_eq!(no_title.status_label, None);
        assert_eq!(
            titled_without_summary.row_secondary_text.as_deref(),
            Some("Preview")
        );
    }

    #[test]
    fn feedback_message_presentation_marks_current_user_and_profile_name() {
        let event = feedback_event("event", "root", 100, "body");
        let projection = feedback_message_presentation(FeedbackMessagePresentationInput {
            event: FeedbackEventRecord {
                author_pubkey: "me".into(),
                ..event
            },
            show_header: true,
            current_user_pubkey: Some("me".into()),
            profile: Some(profile("alice", "Alice Smith", "https://example.com/a.png")),
        });

        assert!(projection.is_from_me);
        assert!(projection.show_header);
        assert_eq!(projection.display_name, "Alice Smith");
        assert_eq!(projection.display_initial, "A");
        assert_eq!(projection.picture_url, "https://example.com/a.png");
    }

    #[test]
    fn feedback_message_presentation_groups_adjacent_messages() {
        let previous = FeedbackEventRecord {
            author_pubkey: "agent".into(),
            ..feedback_event("previous", "root", 100, "previous")
        };
        let current = FeedbackEventRecord {
            author_pubkey: "agent".into(),
            ..feedback_event("current", "root", 399, "current")
        };
        let later = FeedbackEventRecord {
            author_pubkey: "agent".into(),
            ..feedback_event("later", "root", 401, "later")
        };

        let grouped = feedback_message_presentation(FeedbackMessagePresentationInput {
            event: current.clone(),
            show_header: rows_for_events(vec![previous.clone(), current])[1].show_header,
            current_user_pubkey: Some("me".into()),
            profile: Some(profile("agent-name", "", "")),
        });
        let separated = feedback_message_presentation(FeedbackMessagePresentationInput {
            event: later.clone(),
            show_header: rows_for_events(vec![previous, later])[1].show_header,
            current_user_pubkey: Some("me".into()),
            profile: None,
        });

        assert!(!grouped.is_from_me);
        assert!(!grouped.show_header);
        assert_eq!(grouped.display_name, "agent-name");
        assert_eq!(grouped.display_initial, "A");
        assert!(separated.show_header);
        assert_eq!(separated.display_name, "agent");
        assert_eq!(separated.picture_url, "");
    }

    #[test]
    fn feedback_message_presentation_shows_header_when_author_changes() {
        let previous = FeedbackEventRecord {
            author_pubkey: "agent".into(),
            ..feedback_event("previous", "root", 100, "previous")
        };
        let current = FeedbackEventRecord {
            author_pubkey: "user".into(),
            ..feedback_event("current", "root", 120, "current")
        };

        let projection = feedback_message_presentation(FeedbackMessagePresentationInput {
            event: current.clone(),
            show_header: rows_for_events(vec![previous, current])[1].show_header,
            current_user_pubkey: None,
            profile: None,
        });

        assert!(projection.show_header);
        assert_eq!(projection.display_name, "user");
        assert_eq!(projection.display_initial, "U");
    }

    #[test]
    fn optimistically_insert_root_thread_dedupes_previews_and_sorts() {
        let older = feedback_thread("older", 10);
        let root = feedback_event("new", "new", 30, "hello   world");

        let out = optimistically_insert_root_thread(&[older], &root);

        assert_eq!(
            out.iter()
                .map(|thread| thread.root_event_id.as_str())
                .collect::<Vec<_>>(),
            vec!["new", "older"]
        );
        assert_eq!(out[0].preview, "hello world");

        let duplicate = optimistically_insert_root_thread(&out, &root);
        assert_eq!(
            duplicate
                .iter()
                .filter(|thread| thread.root_event_id == "new")
                .count(),
            1
        );
    }

    #[test]
    fn optimistically_insert_root_thread_ignores_replies() {
        let older = feedback_thread("older", 10);
        let reply = feedback_event("reply", "root", 30, "reply");

        let out = optimistically_insert_root_thread(&[older], &reply);

        assert_eq!(out.len(), 1);
        assert_eq!(out[0].root_event_id, "older");
    }

    #[test]
    fn upsert_thread_event_replaces_and_orders_oldest_first() {
        let older = feedback_event("older", "root", 10, "older");
        let newer = feedback_event("newer", "root", 30, "newer");
        let replacement = feedback_event("newer", "root", 5, "replacement");

        let out = upsert_thread_event(&[older, newer], &replacement);

        assert_eq!(
            out.iter()
                .map(|event| event.event_id.as_str())
                .collect::<Vec<_>>(),
            vec!["newer", "older"]
        );
        assert_eq!(out[0].content, "replacement");
    }

    fn feedback_thread(root_event_id: &str, last_activity_at: u64) -> FeedbackThreadRecord {
        FeedbackThreadRecord {
            root_event_id: root_event_id.into(),
            author_pubkey: "pubkey".into(),
            created_at: last_activity_at,
            last_activity_at,
            title: None,
            summary: None,
            status_label: None,
            preview: String::new(),
        }
    }

    fn feedback_event(
        event_id: &str,
        root_event_id: &str,
        created_at: u64,
        content: &str,
    ) -> FeedbackEventRecord {
        FeedbackEventRecord {
            event_id: event_id.into(),
            root_event_id: root_event_id.into(),
            author_pubkey: "pubkey".into(),
            created_at,
            content: content.into(),
        }
    }

    fn profile(name: &str, display_name: &str, picture: &str) -> ProfileMetadata {
        ProfileMetadata {
            pubkey: "profile-pubkey".into(),
            name: name.into(),
            display_name: display_name.into(),
            about: String::new(),
            picture: picture.into(),
            banner: String::new(),
            nip05: String::new(),
            website: String::new(),
            lud16: String::new(),
            created_at: None,
        }
    }
}
