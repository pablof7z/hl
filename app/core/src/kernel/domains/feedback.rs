//! Feedback domain — NIP-22 kind:1111 comment threads scoped to the Highlighter
//! project root (Phase 7).
//!
//! ## Architecture
//!
//! Feedback is NIP-22 comments under a special feedback-project ROOT scope:
//!   - `root_tag_name = "A"` (addressable)
//!   - `root_tag_value = HIGHLIGHTER_PROJECT_COORDINATE`
//!   - `root_kind = 31933` (kind:31933 project definition)
//!
//! This module reuses the existing `comments.rs` / `nmp-nip22` infrastructure:
//! the boot-time `CommentObserver` already routes all kind:1111 events into
//! `AppState::comment_threads` keyed by `root_tag_value`. The feedback domain
//! reads `comment_threads[HIGHLIGHTER_PROJECT_COORDINATE]` and derives two views:
//!
//!   * **Thread list** (`ViewId::FeedbackThreads`) — top-level roots (records
//!     where `is_top_level()` is true), filtered to the active account, sorted
//!     by last-activity, capped at `FEEDBACK_LIST_CAP` (256).
//!   * **Thread detail** (`ViewId::FeedbackThread { root_event_id }`) — all
//!     records in the selected thread's ancestor chain, sorted oldest-first.
//!
//! ## Write path
//!
//! Both `hl.feedback.post_root` and `hl.feedback.post_reply` are dispatched as
//! `nmp.nip22.post_comment` with the fixed project-root scope, reusing
//! `comments::run_effect_dispatch_comment_action` for the C-ABI call.
//!
//! ## D-rules satisfied
//!
//! * D1 — `FeedbackThreadRow` and `FeedbackMessageRow` carry raw protocol fields
//!   only. No formatted timestamps, no byline strings, no fallback labels.
//!   `title`, `summary`, `status_label` are `None` unless an HL metadata source
//!   is explicitly wired (kind:513 → NIP-22 migration not yet in nmp `d16aea60`).
//! * D4 — no duplicate state: raw comment data lives in `AppState::comment_threads`;
//!   `AppState::feedback` holds only UI-lifecycle fields.
//! * D6 — empty content → no-op; malformed JSON → no-op; missing session → no
//!   threads visible; logout/identity-none → clear UI state.
//! * Non-Negotiable #3 — all write actions return `Vec<Effect>`, never `Result`.

use crate::kernel::app::{AppState, SessionState};
use crate::kernel::effect::Effect;
use crate::kernel::snapshot::{
    FeedbackMessageRow, FeedbackThreadRow, KernelFeedbackThreadSnapshot,
    KernelFeedbackThreadsSnapshot,
};

/// Maximum thread-root entries in `KernelFeedbackThreadsSnapshot`.
pub(crate) const FEEDBACK_LIST_CAP: usize = 256;

/// Preview body is truncated to 140 characters to match the live lane behavior.
const PREVIEW_CAP: usize = 140;

/// `show_header` gap threshold in seconds (matches chat/feedback live logic).
const SHOW_HEADER_GAP_SECS: u64 = 300;

// ─── Project-root constant ───────────────────────────────────────────────────

/// The Highlighter feedback project NIP-22 root scope value.
///
/// Used as `root_tag_value` for all feedback kind:1111 comments. This must
/// match `crate::feedback::HIGHLIGHTER_PROJECT_COORDINATE` in the live lane.
pub(crate) const HIGHLIGHTER_PROJECT_COORDINATE: &str =
    "31933:09d48a1a5dbe13404a729634f1d6ba722d40513468dd713c8ea38ca9b7b6f2c7:highlighter";

// ─── Feedback UI state ────────────────────────────────────────────────────────

/// View-lifecycle and publishing-FSM state for the feedback domain.
///
/// The actual feedback messages are NOT duplicated here — the source of truth is
/// `AppState::comment_threads[HIGHLIGHTER_PROJECT_COORDINATE]` (D4). This struct
/// holds only UI-lifecycle fields that have no protocol equivalent.
#[derive(Debug, Clone, Default)]
pub struct FeedbackState {
    /// Project root scope value (constant `HIGHLIGHTER_PROJECT_COORDINATE`).
    pub project_root_tag_value: String,
    /// Event id of the currently open thread detail view, if any.
    pub open_thread_root_event_id: Option<String>,
    /// `true` while a feedback post action is in flight.
    pub is_publishing: bool,
    /// Last publish error (D6: never panics on failure, surfaces as state).
    pub last_error: Option<String>,
}

impl FeedbackState {
    pub(crate) fn with_root() -> Self {
        Self {
            project_root_tag_value: HIGHLIGHTER_PROJECT_COORDINATE.to_string(),
            open_thread_root_event_id: None,
            is_publishing: false,
            last_error: None,
        }
    }
}

// ─── Payload structs ─────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub(crate) struct FeedbackOpenThreadPayload {
    pub root_event_id: String,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct FeedbackPostRootPayload {
    pub content: String,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct FeedbackPostReplyPayload {
    pub root_event_id: String,
    pub content: String,
    pub parent_author_pubkey: Option<String>,
}

// ─── Reducer helpers ─────────────────────────────────────────────────────────

/// `hl.feedback.open_list` — open the feedback thread list view.
///
/// No extra observer wiring: the global `CommentObserver` already ingests
/// all kind:1111 events into `AppState::comment_threads`. No-op if no session.
pub(crate) fn reduce_action_open_list(state: &mut AppState) -> Vec<Effect> {
    // Ensure project_root_tag_value is populated (idempotent).
    if state.feedback.project_root_tag_value.is_empty() {
        state.feedback.project_root_tag_value = HIGHLIGHTER_PROJECT_COORDINATE.to_string();
    }
    vec![]
}

/// `hl.feedback.close_list` — close the feedback thread list; clear UI flags.
pub(crate) fn reduce_action_close_list(state: &mut AppState) -> Vec<Effect> {
    state.feedback.is_publishing = false;
    state.feedback.last_error = None;
    vec![]
}

/// `hl.feedback.open_thread { root_event_id }` — record the open detail view.
pub(crate) fn reduce_action_open_thread(
    state: &mut AppState,
    root_event_id: String,
) -> Vec<Effect> {
    if root_event_id.trim().is_empty() {
        return vec![];
    }
    state.feedback.open_thread_root_event_id = Some(root_event_id);
    vec![]
}

/// `hl.feedback.close_thread { root_event_id }` — clear the open thread id.
pub(crate) fn reduce_action_close_thread(state: &mut AppState) -> Vec<Effect> {
    state.feedback.open_thread_root_event_id = None;
    vec![]
}

/// `hl.feedback.post_root { content }` — post a new top-level feedback thread.
///
/// Dispatches `nmp.nip22.post_comment` with:
///   - `root_tag_name = "A"`
///   - `root_tag_value = HIGHLIGHTER_PROJECT_COORDINATE`
///   - `root_kind = 31933`
///   - no `parent_event_id` (mirrors root, per NIP-22 top-level convention)
///
/// D6: empty content (trimmed) → no-op. No active session → no-op.
pub(crate) fn reduce_action_post_root(state: &mut AppState, content: String) -> Vec<Effect> {
    let content = content.trim().to_string();
    if content.is_empty() {
        return vec![];
    }

    // D6: require active session to publish.
    if !matches!(state.session, SessionState::Present { .. }) {
        return vec![];
    }

    let json_map = serde_json::json!({
        "root_tag_name": "A",
        "root_tag_value": HIGHLIGHTER_PROJECT_COORDINATE,
        "root_kind": 31933u32,
        "content": content,
    });

    let json = match serde_json::to_string(&json_map) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "feedback::reduce_action_post_root: serde error — no-op (D6)");
            return vec![];
        }
    };

    vec![Effect::DispatchFeedbackCommentAction { json }]
}

/// `hl.feedback.post_reply { root_event_id, content, parent_author_pubkey? }` —
/// post a reply to an existing feedback thread.
///
/// Dispatches `nmp.nip22.post_comment` with:
///   - `root_tag_name = "A"`, `root_tag_value = HIGHLIGHTER_PROJECT_COORDINATE`
///   - `parent_event_id = root_event_id`
///   - optional `parent_author_pubkey`
///
/// D6: empty content or empty root_event_id → no-op.
pub(crate) fn reduce_action_post_reply(
    state: &mut AppState,
    root_event_id: String,
    content: String,
    parent_author_pubkey: Option<String>,
) -> Vec<Effect> {
    let content = content.trim().to_string();
    let root_event_id = root_event_id.trim().to_string();

    if content.is_empty() || root_event_id.is_empty() {
        return vec![];
    }

    // D6: require active session to publish.
    if !matches!(state.session, SessionState::Present { .. }) {
        return vec![];
    }

    let mut json_map = serde_json::json!({
        "root_tag_name": "A",
        "root_tag_value": HIGHLIGHTER_PROJECT_COORDINATE,
        "root_kind": 31933u32,
        "parent_event_id": root_event_id,
        "content": content,
    });

    if let Some(pubkey) = parent_author_pubkey {
        if !pubkey.is_empty() {
            json_map["parent_author_pubkey"] = serde_json::Value::String(pubkey);
        }
    }

    let json = match serde_json::to_string(&json_map) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "feedback::reduce_action_post_reply: serde error — no-op (D6)");
            return vec![];
        }
    };

    vec![Effect::DispatchFeedbackCommentAction { json }]
}

/// Clear `FeedbackState` on logout or `IdentityChanged(None)`.
///
/// The underlying `comment_threads` entry for the project root is NOT cleared —
/// comment content is content-addressed and bounded by NMP. The list projection
/// will produce an empty result because there is no active viewer.
pub(crate) fn reduce_event_clear_on_logout(state: &mut AppState) -> Vec<Effect> {
    state.feedback = FeedbackState::with_root();
    vec![]
}

// ─── Effect runner ────────────────────────────────────────────────────────────

/// Execute `Effect::DispatchFeedbackCommentAction` — calls
/// `nmp_app_dispatch_action` with namespace `"nmp.nip22.post_comment"` and
/// the serialised JSON payload.
///
/// Delegates to `comments::run_effect_dispatch_comment_action` so the C-ABI
/// dispatch path is not duplicated. Fire-and-forget (D6, Non-Negotiable #3).
pub(crate) fn run_effect_dispatch_feedback_comment_action(
    json: String,
    nmp: Option<&crate::kernel::actor::NmpHandle>,
) {
    crate::kernel::domains::comments::run_effect_dispatch_comment_action(json, nmp);
}

// ─── Snapshot computation ─────────────────────────────────────────────────────

/// Compute `KernelFeedbackThreadsSnapshot` for `ViewId::FeedbackThreads`.
///
/// Sources from `AppState::comment_threads[HIGHLIGHTER_PROJECT_COORDINATE]`.
/// Filters to top-level records authored by the active account (matches live
/// lane behavior). Sorts by `last_activity_at` descending. Caps at 256.
///
/// D1: no formatted strings, no display fallbacks. `title`, `summary`,
/// `status_label` are `None` until an HL metadata source is added.
pub(crate) fn compute_feedback_threads_snapshot(state: &AppState) -> KernelFeedbackThreadsSnapshot {
    let (is_publishing, error) = (
        state.feedback.is_publishing,
        state.feedback.last_error.clone(),
    );

    let root_tag_value = HIGHLIGHTER_PROJECT_COORDINATE.to_string();

    // Derive active viewer pubkey (D6: empty list when no session).
    let viewer_pubkey = match &state.session {
        SessionState::Present { pubkey, .. } => pubkey.clone(),
        _ => {
            return KernelFeedbackThreadsSnapshot {
                root_tag_value,
                threads: vec![],
                is_publishing,
                error,
            };
        }
    };

    let snapshot = match state.comment_threads.get(HIGHLIGHTER_PROJECT_COORDINATE) {
        Some(s) => s,
        None => {
            return KernelFeedbackThreadsSnapshot {
                root_tag_value,
                threads: vec![],
                is_publishing,
                error,
            };
        }
    };

    // Collect top-level roots authored by the current viewer.
    let mut thread_rows: Vec<FeedbackThreadRow> = snapshot
        .records
        .iter()
        .filter(|r| r.is_top_level() && r.author_pubkey == viewer_pubkey)
        .map(|root_rec| {
            // Count replies + compute last_activity_at as max created_at across
            // root + all records whose ancestor chain reaches this root event.
            // For the first cut: replies are records whose parent_tag_value ==
            // root.event_id (direct replies). Nested replies not traversed here
            // — they still appear in the thread detail.
            let replies: Vec<_> = snapshot
                .records
                .iter()
                .filter(|r| r.parent_tag_value == root_rec.event_id)
                .collect();

            let last_activity_at = replies
                .iter()
                .map(|r| r.created_at)
                .chain(std::iter::once(root_rec.created_at))
                .max()
                .unwrap_or(root_rec.created_at);

            let reply_count = replies.len() as u32;

            // Preview: whitespace-collapsed first 140 chars of body (D1: raw text).
            let preview = build_preview(&root_rec.body);

            FeedbackThreadRow {
                root_event_id: root_rec.event_id.clone(),
                author_pubkey: root_rec.author_pubkey.clone(),
                created_at: root_rec.created_at,
                last_activity_at,
                title: None,
                summary: None,
                status_label: None,
                preview,
                reply_count,
            }
        })
        .collect();

    // Sort newest activity first; cap at 256.
    thread_rows.sort_by(|a, b| b.last_activity_at.cmp(&a.last_activity_at));
    thread_rows.truncate(FEEDBACK_LIST_CAP);

    KernelFeedbackThreadsSnapshot {
        root_tag_value,
        threads: thread_rows,
        is_publishing,
        error,
    }
}

/// Compute `KernelFeedbackThreadSnapshot` for `ViewId::FeedbackThread { root_event_id }`.
///
/// Includes the root record and all descendant records (ancestor chain reaches
/// the root event_id). Sorted oldest-first for chat-style display.
/// `show_header` follows the 300-second / author-change grouping rule.
///
/// D1: no formatted strings; `parent_event_id` is raw.
pub(crate) fn compute_feedback_thread_snapshot(
    state: &AppState,
    root_event_id: &str,
) -> KernelFeedbackThreadSnapshot {
    let (is_publishing, error) = (
        state.feedback.is_publishing,
        state.feedback.last_error.clone(),
    );

    let root_tag_value = HIGHLIGHTER_PROJECT_COORDINATE.to_string();

    let snapshot = match state.comment_threads.get(HIGHLIGHTER_PROJECT_COORDINATE) {
        Some(s) => s,
        None => {
            return KernelFeedbackThreadSnapshot {
                root_tag_value,
                root_event_id: root_event_id.to_string(),
                rows: vec![],
                is_publishing,
                error,
            };
        }
    };

    // Gather all records in this thread's ancestor chain.
    // Strategy: BFS from root_event_id — include root record and any record
    // whose parent_tag_value is in the set of included event_ids.
    let mut included_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    included_ids.insert(root_event_id.to_string());

    // First pass: find records whose parent is root_event_id or already included.
    // Iterate until stable (handles nested replies up to NMP projection depth).
    loop {
        let before = included_ids.len();
        for r in &snapshot.records {
            if included_ids.contains(&r.parent_tag_value) || r.event_id == root_event_id {
                included_ids.insert(r.event_id.clone());
            }
        }
        if included_ids.len() == before {
            break;
        }
    }

    // Collect matching records and sort oldest-first.
    let mut rows: Vec<_> = snapshot
        .records
        .iter()
        .filter(|r| included_ids.contains(&r.event_id))
        .collect();
    rows.sort_by_key(|r| r.created_at);

    // Map to FeedbackMessageRow with show_header computation.
    let mut message_rows: Vec<FeedbackMessageRow> = Vec::with_capacity(rows.len());
    for (i, rec) in rows.iter().enumerate() {
        let show_header = if i == 0 {
            true
        } else {
            let prev = &rows[i - 1];
            prev.author_pubkey != rec.author_pubkey
                || rec.created_at.saturating_sub(prev.created_at) > SHOW_HEADER_GAP_SECS
        };

        let parent_event_id = if rec.parent_tag_value == HIGHLIGHTER_PROJECT_COORDINATE
            || rec.parent_tag_value == rec.root_tag_value
        {
            None
        } else {
            Some(rec.parent_tag_value.clone())
        };

        message_rows.push(FeedbackMessageRow {
            event_id: rec.event_id.clone(),
            root_event_id: root_event_id.to_string(),
            author_pubkey: rec.author_pubkey.clone(),
            created_at: rec.created_at,
            content: rec.body.clone(),
            parent_event_id,
            show_header,
        });
    }

    KernelFeedbackThreadSnapshot {
        root_tag_value,
        root_event_id: root_event_id.to_string(),
        rows: message_rows,
        is_publishing,
        error,
    }
}

// ─── Preview helper ───────────────────────────────────────────────────────────

/// Build a whitespace-collapsed preview of `body`, capped at `PREVIEW_CAP` chars.
///
/// D1: no Rust-owned display labels. Raw text only.
fn build_preview(body: &str) -> String {
    let collapsed: String = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.len() <= PREVIEW_CAP {
        collapsed
    } else {
        // Trim to PREVIEW_CAP char boundary (Unicode-safe).
        let mut end = PREVIEW_CAP;
        while !collapsed.is_char_boundary(end) {
            end -= 1;
        }
        collapsed[..end].to_string()
    }
}

// ─── Unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::action::KernelEvent;
    use crate::kernel::actor::{reduce, Cmd};
    use crate::kernel::app::AppState;
    use crate::kernel::clock::{Clock, ManualClock};
    use crate::kernel::effect::Effect;

    const PROJ: &str = HIGHLIGHTER_PROJECT_COORDINATE;

    fn make_state() -> AppState {
        AppState::default()
    }

    /// Inject a `CommentThreadUpdated` event to populate `AppState::comment_threads`.
    fn inject_comment_thread(
        state: &mut AppState,
        clock: &ManualClock,
        records: Vec<nmp_nip22::CommentRecord>,
    ) {
        let snapshot = nmp_nip22::CommentThreadSnapshot {
            root_tag_value: PROJ.to_string(),
            tree: nmp_nip22::build_thread(&records, PROJ),
            records,
        };
        let now = clock.now_unix_seconds();
        reduce(
            state,
            Cmd::Event(KernelEvent::CommentThreadUpdated {
                root_tag_value: PROJ.to_string(),
                snapshot,
            }),
            now,
        );
    }

    fn make_record(
        event_id: &str,
        author: &str,
        parent_val: &str,
        created_at: u64,
        body: &str,
    ) -> nmp_nip22::CommentRecord {
        nmp_nip22::CommentRecord {
            event_id: event_id.to_string(),
            author_pubkey: author.to_string(),
            body: body.to_string(),
            root_tag_name: "A".to_string(),
            root_tag_value: PROJ.to_string(),
            root_kind: "31933".to_string(),
            root_author_pubkey: author.to_string(),
            parent_tag_name: "a".to_string(),
            parent_tag_value: parent_val.to_string(),
            parent_kind: "31933".to_string(),
            created_at,
        }
    }

    fn set_active_session(state: &mut AppState, pubkey: &str) {
        state.session = crate::kernel::app::SessionState::Present {
            pubkey: pubkey.to_string(),
            signer_kind: crate::kernel::action::SignerKind::LocalNsec,
        };
    }

    // ─── Test F-T1: feedback_list_empty_without_active_account ────────────────
    //
    // Project comments exist but no signed-in viewer yields no visible threads.
    #[test]
    fn feedback_list_empty_without_active_account() {
        let mut state = make_state();
        let clock = ManualClock::default();

        let rec = make_record(
            "aaaa0000000000000000000000000000000000000000000000000000000000a1",
            "bbbb000000000000000000000000000000000000000000000000000000000001",
            PROJ,
            1_000_000,
            "feedback body",
        );
        inject_comment_thread(&mut state, &clock, vec![rec]);

        let snap = compute_feedback_threads_snapshot(&state);
        assert!(
            snap.threads.is_empty(),
            "no threads without active session (D6)"
        );
    }

    // ─── Test F-T2: feedback_top_level_comments_become_threads ────────────────
    //
    // Project-root top-level NIP-22 records become FeedbackThreadRows.
    #[test]
    fn feedback_top_level_comments_become_threads() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let viewer = "bbbb000000000000000000000000000000000000000000000000000000000001";

        set_active_session(&mut state, viewer);

        let rec = make_record(
            "aaaa0000000000000000000000000000000000000000000000000000000000a1",
            viewer,
            PROJ, // parent == root → top-level
            1_000_000,
            "feedback body",
        );
        inject_comment_thread(&mut state, &clock, vec![rec.clone()]);

        let snap = compute_feedback_threads_snapshot(&state);
        assert_eq!(snap.threads.len(), 1, "one thread root");
        assert_eq!(snap.threads[0].root_event_id, rec.event_id);
        assert_eq!(snap.threads[0].author_pubkey, viewer);
        assert_eq!(snap.threads[0].reply_count, 0);
    }

    // ─── Test F-T3: feedback_list_filters_to_current_user_roots ───────────────
    //
    // Top-level roots by other authors are excluded from the list.
    #[test]
    fn feedback_list_filters_to_current_user_roots() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let viewer = "bbbb000000000000000000000000000000000000000000000000000000000001";
        let other = "cccc000000000000000000000000000000000000000000000000000000000002";

        set_active_session(&mut state, viewer);

        let viewer_rec = make_record(
            "aaaa0000000000000000000000000000000000000000000000000000000000a1",
            viewer,
            PROJ,
            1_000_000,
            "my feedback",
        );
        let other_rec = make_record(
            "aaaa0000000000000000000000000000000000000000000000000000000000a2",
            other,
            PROJ,
            1_000_001,
            "other feedback",
        );
        inject_comment_thread(&mut state, &clock, vec![viewer_rec.clone(), other_rec]);

        let snap = compute_feedback_threads_snapshot(&state);
        assert_eq!(snap.threads.len(), 1, "only viewer's thread shown");
        assert_eq!(snap.threads[0].root_event_id, viewer_rec.event_id);
    }

    // ─── Test F-T4: feedback_last_activity_uses_replies ───────────────────────
    //
    // A reply under a root updates that thread's last_activity_at.
    #[test]
    fn feedback_last_activity_uses_replies() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let viewer = "bbbb000000000000000000000000000000000000000000000000000000000001";
        let root_id = "aaaa0000000000000000000000000000000000000000000000000000000000a1";
        let reply_id = "dddd0000000000000000000000000000000000000000000000000000000000d1";

        set_active_session(&mut state, viewer);

        let root_rec = make_record(root_id, viewer, PROJ, 1_000_000, "root body");
        let reply_rec = nmp_nip22::CommentRecord {
            event_id: reply_id.to_string(),
            author_pubkey: "eeee000000000000000000000000000000000000000000000000000000000001"
                .to_string(),
            body: "a reply".to_string(),
            root_tag_name: "A".to_string(),
            root_tag_value: PROJ.to_string(),
            root_kind: "31933".to_string(),
            root_author_pubkey: viewer.to_string(),
            parent_tag_name: "e".to_string(),
            parent_tag_value: root_id.to_string(),
            parent_kind: "1111".to_string(),
            created_at: 1_999_999,
        };
        inject_comment_thread(&mut state, &clock, vec![root_rec, reply_rec]);

        let snap = compute_feedback_threads_snapshot(&state);
        assert_eq!(snap.threads.len(), 1);
        assert_eq!(
            snap.threads[0].last_activity_at, 1_999_999,
            "last_activity_at must use the reply's created_at"
        );
        assert_eq!(snap.threads[0].reply_count, 1, "one reply counted");
    }

    // ─── Test F-T5: feedback_thread_detail_includes_root_and_descendants_oldest_first
    //
    // Detail rows include root plus replies sorted ascending.
    #[test]
    fn feedback_thread_detail_includes_root_and_descendants_oldest_first() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let viewer = "bbbb000000000000000000000000000000000000000000000000000000000001";
        let root_id = "aaaa0000000000000000000000000000000000000000000000000000000000a1";
        let reply_id = "dddd0000000000000000000000000000000000000000000000000000000000d1";

        set_active_session(&mut state, viewer);

        let root_rec = make_record(root_id, viewer, PROJ, 1_000_000, "root body");
        let reply_rec = nmp_nip22::CommentRecord {
            event_id: reply_id.to_string(),
            author_pubkey: viewer.to_string(),
            body: "reply body".to_string(),
            root_tag_name: "A".to_string(),
            root_tag_value: PROJ.to_string(),
            root_kind: "31933".to_string(),
            root_author_pubkey: viewer.to_string(),
            parent_tag_name: "e".to_string(),
            parent_tag_value: root_id.to_string(),
            parent_kind: "1111".to_string(),
            created_at: 1_000_500,
        };
        inject_comment_thread(&mut state, &clock, vec![root_rec, reply_rec]);

        let snap = compute_feedback_thread_snapshot(&state, root_id);
        assert_eq!(snap.rows.len(), 2, "root + reply in detail");
        assert_eq!(snap.rows[0].event_id, root_id, "root is first (oldest)");
        assert_eq!(snap.rows[1].event_id, reply_id, "reply is second");
        assert!(
            snap.rows[0].created_at <= snap.rows[1].created_at,
            "sorted oldest-first"
        );
    }

    // ─── Test F-T6: feedback_show_header_matches_300s_author_grouping ─────────
    //
    // show_header is true on: first row, author change, or >300s gap.
    #[test]
    fn feedback_show_header_matches_300s_author_grouping() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let viewer = "bbbb000000000000000000000000000000000000000000000000000000000001";
        let other = "cccc000000000000000000000000000000000000000000000000000000000002";
        let root_id = "aaaa0000000000000000000000000000000000000000000000000000000000a1";

        set_active_session(&mut state, viewer);

        // root (viewer) → reply by same author within 300s → reply by other author
        let root_rec = make_record(root_id, viewer, PROJ, 1_000_000, "root");
        let reply_same = nmp_nip22::CommentRecord {
            event_id: "bbbb0000000000000000000000000000000000000000000000000000000000b1"
                .to_string(),
            author_pubkey: viewer.to_string(),
            body: "same author quick reply".to_string(),
            root_tag_name: "A".to_string(),
            root_tag_value: PROJ.to_string(),
            root_kind: "31933".to_string(),
            root_author_pubkey: viewer.to_string(),
            parent_tag_name: "e".to_string(),
            parent_tag_value: root_id.to_string(),
            parent_kind: "1111".to_string(),
            created_at: 1_000_100, // +100s — same author, no gap → no header
        };
        let reply_other = nmp_nip22::CommentRecord {
            event_id: "cccc0000000000000000000000000000000000000000000000000000000000c1"
                .to_string(),
            author_pubkey: other.to_string(),
            body: "other author reply".to_string(),
            root_tag_name: "A".to_string(),
            root_tag_value: PROJ.to_string(),
            root_kind: "31933".to_string(),
            root_author_pubkey: viewer.to_string(),
            parent_tag_name: "e".to_string(),
            parent_tag_value: root_id.to_string(),
            parent_kind: "1111".to_string(),
            created_at: 1_000_200, // +100s — author changed → header
        };

        inject_comment_thread(&mut state, &clock, vec![root_rec, reply_same, reply_other]);

        let snap = compute_feedback_thread_snapshot(&state, root_id);
        assert_eq!(snap.rows.len(), 3);
        assert!(snap.rows[0].show_header, "first row always has header");
        assert!(
            !snap.rows[1].show_header,
            "same author within 300s → no header"
        );
        assert!(snap.rows[2].show_header, "author change → header");
    }

    // ─── Test F-T7: feedback_post_root_dispatches_nip22_project_comment ───────
    //
    // hl.feedback.post_root emits DispatchFeedbackCommentAction with the
    // project coordinate, root_kind 31933, and no parent_event_id.
    #[test]
    fn feedback_post_root_dispatches_nip22_project_comment() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let viewer = "bbbb000000000000000000000000000000000000000000000000000000000001";
        set_active_session(&mut state, viewer);

        let payload = serde_json::json!({ "content": "new feedback thread" });
        let envelope = crate::kernel::action::AppActionEnvelope {
            namespace: "hl.feedback.post_root".to_string(),
            json: serde_json::to_string(&payload).unwrap(),
        };

        let now = clock.now_unix_seconds();
        let effects = reduce(&mut state, Cmd::ActionEnvelope(envelope), now);

        assert_eq!(effects.len(), 1, "must emit exactly one effect");
        match &effects[0] {
            Effect::DispatchFeedbackCommentAction { json } => {
                let parsed: serde_json::Value = serde_json::from_str(json).unwrap();
                assert_eq!(parsed["root_tag_name"].as_str().unwrap(), "A");
                assert_eq!(parsed["root_tag_value"].as_str().unwrap(), PROJ);
                assert_eq!(parsed["root_kind"].as_u64().unwrap(), 31933);
                assert_eq!(parsed["content"].as_str().unwrap(), "new feedback thread");
                assert!(
                    parsed.get("parent_event_id").is_none() || parsed["parent_event_id"].is_null(),
                    "top-level post must have no parent_event_id"
                );
            }
            _ => panic!("expected DispatchFeedbackCommentAction"),
        }
    }

    // ─── Test F-T8: feedback_post_reply_dispatches_parent_event_id ───────────
    //
    // hl.feedback.post_reply emits same project root plus parent_event_id.
    #[test]
    fn feedback_post_reply_dispatches_parent_event_id() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let viewer = "bbbb000000000000000000000000000000000000000000000000000000000001";
        let root_id = "aaaa0000000000000000000000000000000000000000000000000000000000a1";
        set_active_session(&mut state, viewer);

        let payload = serde_json::json!({
            "root_event_id": root_id,
            "content": "a reply to the thread",
        });
        let envelope = crate::kernel::action::AppActionEnvelope {
            namespace: "hl.feedback.post_reply".to_string(),
            json: serde_json::to_string(&payload).unwrap(),
        };

        let now = clock.now_unix_seconds();
        let effects = reduce(&mut state, Cmd::ActionEnvelope(envelope), now);

        assert_eq!(effects.len(), 1);
        match &effects[0] {
            Effect::DispatchFeedbackCommentAction { json } => {
                let parsed: serde_json::Value = serde_json::from_str(json).unwrap();
                assert_eq!(
                    parsed["parent_event_id"].as_str().unwrap(),
                    root_id,
                    "parent_event_id must be root_id"
                );
                assert_eq!(parsed["root_tag_value"].as_str().unwrap(), PROJ);
            }
            _ => panic!("expected DispatchFeedbackCommentAction"),
        }
    }

    // ─── Test F-T9: feedback_state_cleared_on_logout ─────────────────────────
    //
    // FeedbackState is cleared on logout; comment_threads remains.
    #[test]
    fn feedback_state_cleared_on_logout() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let viewer = "bbbb000000000000000000000000000000000000000000000000000000000001";
        set_active_session(&mut state, viewer);

        // Set some UI state.
        state.feedback.open_thread_root_event_id = Some("some_id".to_string());
        state.feedback.last_error = Some("prior error".to_string());

        // Inject a comment thread.
        let rec = make_record(
            "aaaa0000000000000000000000000000000000000000000000000000000000a1",
            viewer,
            PROJ,
            1_000_000,
            "body",
        );
        inject_comment_thread(&mut state, &clock, vec![rec]);
        assert!(state.comment_threads.contains_key(PROJ), "comment stored");

        // Simulate logout via the reduce pathway.
        let now = clock.now_unix_seconds();
        reduce(
            &mut state,
            Cmd::Action(crate::kernel::action::AppAction::Logout),
            now,
        );

        // UI state cleared.
        assert!(
            state.feedback.open_thread_root_event_id.is_none(),
            "open_thread cleared on logout"
        );
        assert!(
            state.feedback.last_error.is_none(),
            "last_error cleared on logout"
        );

        // Raw comment_threads NOT cleared (content-addressed, not per-account).
        assert!(
            state.comment_threads.contains_key(PROJ),
            "comment_threads remain after logout (content-addressed)"
        );

        // But list snapshot is empty because no active session.
        let snap = compute_feedback_threads_snapshot(&state);
        assert!(
            snap.threads.is_empty(),
            "thread list empty without active session"
        );
    }

    // ─── Test F-T10: feedback_kind513_metadata_not_required ──────────────────
    //
    // Absence of kind 513 yields raw preview and None metadata fields.
    #[test]
    fn feedback_kind513_metadata_not_required() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let viewer = "bbbb000000000000000000000000000000000000000000000000000000000001";
        set_active_session(&mut state, viewer);

        let rec = make_record(
            "aaaa0000000000000000000000000000000000000000000000000000000000a1",
            viewer,
            PROJ,
            1_000_000,
            "some raw feedback",
        );
        inject_comment_thread(&mut state, &clock, vec![rec]);

        let snap = compute_feedback_threads_snapshot(&state);
        assert_eq!(snap.threads.len(), 1);
        let row = &snap.threads[0];

        // Without kind:513 metadata these must all be None.
        assert!(row.title.is_none(), "title must be None without metadata");
        assert!(
            row.summary.is_none(),
            "summary must be None without metadata"
        );
        assert!(
            row.status_label.is_none(),
            "status_label must be None without metadata"
        );
        // Preview is derived from body.
        assert_eq!(row.preview, "some raw feedback", "preview from body");
    }

    // ─── Test F-T11: feedback_list_bounded_256 ────────────────────────────────
    //
    // Thread list is capped at FEEDBACK_LIST_CAP (256).
    #[test]
    fn feedback_list_bounded_256() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let viewer = "bbbb000000000000000000000000000000000000000000000000000000000001";
        set_active_session(&mut state, viewer);

        // Create 300 top-level records.
        let records: Vec<_> = (0u32..300)
            .map(|i| {
                let id = format!("{:064x}", i + 1);
                make_record(&id, viewer, PROJ, 1_000_000 + i as u64, "body")
            })
            .collect();
        inject_comment_thread(&mut state, &clock, records);

        let snap = compute_feedback_threads_snapshot(&state);
        assert_eq!(
            snap.threads.len(),
            FEEDBACK_LIST_CAP,
            "thread list capped at {FEEDBACK_LIST_CAP}"
        );
    }

    // ─── Test F-T12: cleared_on_logout_identity_none ─────────────────────────
    //
    // FeedbackState is cleared on both logout and IdentityChanged(None).
    #[test]
    fn cleared_on_logout_identity_none() {
        let mut state = make_state();
        let clock = ManualClock::default();

        state.feedback.open_thread_root_event_id = Some("some_id".to_string());
        state.feedback.is_publishing = true;

        let now = clock.now_unix_seconds();
        // Simulate IdentityChanged(None).
        reduce(
            &mut state,
            Cmd::Event(KernelEvent::IdentityChanged(None)),
            now,
        );

        assert!(state.feedback.open_thread_root_event_id.is_none());
        assert!(!state.feedback.is_publishing);
    }

    // ─── Test F-T13: malformed_post_no_op ────────────────────────────────────
    //
    // Empty content and bad JSON are no-ops (D6).
    #[test]
    fn malformed_post_no_op() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let viewer = "bbbb000000000000000000000000000000000000000000000000000000000001";
        set_active_session(&mut state, viewer);

        // Empty content for post_root.
        let payload = serde_json::json!({ "content": "" });
        let envelope = crate::kernel::action::AppActionEnvelope {
            namespace: "hl.feedback.post_root".to_string(),
            json: serde_json::to_string(&payload).unwrap(),
        };
        let now = clock.now_unix_seconds();
        let effects = reduce(&mut state, Cmd::ActionEnvelope(envelope), now);
        assert!(effects.is_empty(), "empty content → no effects (D6)");

        // Malformed JSON.
        let bad_envelope = crate::kernel::action::AppActionEnvelope {
            namespace: "hl.feedback.post_root".to_string(),
            json: "{bad json".to_string(),
        };
        let effects = reduce(&mut state, Cmd::ActionEnvelope(bad_envelope), now);
        // Bad JSON produces an invalid-action toast (not a panic) — effects may
        // include a toast effect, but must never panic (D6).
        let _ = effects; // just checking no panic
    }
}
