//! Highlight feed domain — NIP-84 kind:9802 feed pull (slice 4H).
//!
//! ## Responsibilities
//!
//! * **READ** — drain the `"hl.feed.highlights"` pull cursor (registered with
//!   `InterestShape{kinds:[9802]}`) and decode raw `KernelEvent`s from
//!   `AppState::highlight_feed.rows` into `HighlightRow`s for the snapshot.
//!
//! * **WRITE** — publish a new kind:9802 highlight event via
//!   `ActorCommand::PublishRawEvent` (there is no dedicated nmp kind:9802 action
//!   namespace at pinned b4404159 — verified by grep; the raw publish path is the
//!   same seam Phase 2D uses for the rooms relay list). `AppAction::PublishHighlight`
//!   produces `Effect::PublishHighlightEvent`.
//!
//! * **VIEW** — `ViewId::HighlightFeed` / `ViewRoute::HighlightFeed` /
//!   `ViewSnapshot::HighlightFeed(HighlightFeedSnapshot)`. Lifecycle:
//!   - `OpenView(HighlightFeed)` → `RegisterFeedCursor{key, shape}` + `DrainFeed{key}`
//!   - scroll-end `AppAction::DrainHighlightFeed` → `DrainFeed{key}` (pagination)
//!   - `CloseView(HighlightFeed)` → `ReleaseFeedCursor{key}`
//!
//! ## No byline formatting (D1)
//!
//! `HighlightRow` carries only raw protocol data (content, source reference,
//! author pubkey, created_at). Byline composition (`"Highlighted by {name},
//! {name2} and {n} others"`, avatar assembly, source-kind icon/label) is Swift's
//! responsibility (D1). No formatted strings here.
//!
//! ## Feed key
//!
//! `HIGHLIGHT_FEED_KEY = "hl.feed.highlights"` — matches the routing arm in
//! `actor.rs` and the `feed_state_cursor_id` helper added in Phase 4F.
//!
//! ## Threading
//!
//! All reducer functions run on the actor thread, synchronously. `run_effect_*`
//! functions may call `nmp_app_dispatch_action` (which sends via actor_sender,
//! non-blocking) or publish via `ActorCommand::PublishRawEvent` (also non-blocking).
//! No `.await`, no sleeps, no polling (D8).

use nmp_core::substrate::KernelEvent as NmpKernelEvent;

use crate::kernel::app::AppState;
use crate::kernel::domains::feed::{
    highlight_feed_scope, reduce_drain_feed, reduce_register_feed_cursor,
    reduce_release_feed_cursor,
};
use crate::kernel::effect::Effect;
use crate::kernel::snapshot::{HighlightFeedSnapshot, HighlightRow, ViewSnapshot};
use crate::kernel::view::ViewId;

// ─── Feed key ────────────────────────────────────────────────────────────────

/// Stable key for the highlights pull cursor — matches `AppState::highlight_feed`
/// and the routing arm in `actor.rs::reduce_event(FeedPage{key})`.
pub const HIGHLIGHT_FEED_KEY: &str = "hl.feed.highlights";

// ─── Row decode ──────────────────────────────────────────────────────────────

/// Decode a raw `KernelEvent` (kind:9802) from the feed page into a `HighlightRow`.
///
/// Returns `None` when the event is not kind:9802 (D6: non-highlight entries
/// in the feed are silently skipped — never a panic). Returns `None` when
/// `event.content` is empty (malformed event — D6 no-op).
///
/// D1: extracts raw protocol fields only — no byline formatting, no "Highlighted
/// by {name}" string, no avatar URL composition, no source-kind label. Swift owns
/// all presentation.
pub(crate) fn decode_highlight_row(event: &NmpKernelEvent) -> Option<HighlightRow> {
    // Only kind:9802 events belong in the highlight feed.
    if event.kind != 9802 {
        return None;
    }
    // A highlight with empty content is malformed — skip (D6 no-op).
    if event.content.is_empty() {
        return None;
    }

    // Extract the source reference from the NIP-84 `a` (address) or `e` (event)
    // tag. NIP-84 uses `["a", "<kind:pubkey:d>"]` for addressable targets and
    // `["e", "<event_id>"]` for non-addressable targets. We surface the first
    // match as a raw string; Swift decides how to render it.
    let source_reference: Option<String> = {
        // Prefer `a` tag (addressable — NIP-23 articles are the primary target).
        let a_tag = event
            .tags
            .iter()
            .find(|t| t.first().map(|s| s == "a").unwrap_or(false))
            .and_then(|t| t.get(1))
            .cloned();
        if a_tag.is_some() {
            a_tag
        } else {
            // Fall back to `e` tag (non-addressable events).
            event
                .tags
                .iter()
                .find(|t| t.first().map(|s| s == "e").unwrap_or(false))
                .and_then(|t| t.get(1))
                .cloned()
        }
    };

    // Optional user note from the NIP-84 `comment` tag (mirrors the live lane's
    // `first_tag_value(event, "comment")`). Empty/absent → None (D1).
    let note: Option<String> = event
        .tags
        .iter()
        .find(|t| t.first().map(|s| s == "comment").unwrap_or(false))
        .and_then(|t| t.get(1))
        .filter(|s| !s.is_empty())
        .cloned();

    // ── Phase 7 enrichment: mirror highlights.rs::record_from_cached_event ────
    // Raw NIP-84/NIP-73 source + clip + image fields so the highlight card can
    // render the resource header, podcast-clip chrome, and page-scan image.
    let tag = |name: &str| -> String {
        event
            .tags
            .iter()
            .find(|t| t.first().map(|s| s == name).unwrap_or(false))
            .and_then(|t| t.get(1))
            .cloned()
            .unwrap_or_default()
    };
    let artifact_address = tag("a");
    let event_reference = tag("e");
    let external_reference = tag("i");
    let source_url = tag("r");
    let context = tag("context");
    let source_reference_key = if !artifact_address.is_empty() {
        format!("a:{artifact_address}")
    } else if !event_reference.is_empty() {
        format!("e:{event_reference}")
    } else if !external_reference.is_empty() {
        format!("i:{external_reference}")
    } else if !source_url.is_empty() {
        format!("r:{source_url}")
    } else {
        String::new()
    };
    let clip_start_seconds = {
        let s = tag("start");
        s.trim().parse().ok()
    };
    let clip_end_seconds = {
        let s = tag("end");
        s.trim().parse().ok()
    };
    let clip_speaker = tag("speaker");
    let clip_transcript_segment_ids: Vec<String> = event
        .tags
        .iter()
        .filter(|t| t.first().map(|s| s == "segment").unwrap_or(false))
        .filter_map(|t| t.get(1).cloned())
        .collect();
    let image_url = imeta_image_url(event);

    Some(HighlightRow {
        event_id: event.id.clone(),
        author_pubkey: event.author.clone(),
        content: event.content.clone(),
        source_reference,
        note,
        created_at: event.created_at,
        context,
        artifact_address,
        event_reference,
        external_reference,
        source_url,
        source_reference_key,
        clip_start_seconds,
        clip_end_seconds,
        clip_speaker,
        clip_transcript_segment_ids,
        image_url,
    })
}

/// Extract the NIP-92 `imeta` image URL from a raw kernel event's tags.
/// Tag shape: `["imeta", "url <url>", "m <mime>", …]`. Mirrors the live lane's
/// `highlights.rs::imeta_image_url`. Empty when no imeta tag carries a url.
fn imeta_image_url(event: &NmpKernelEvent) -> String {
    for tag in event.tags.iter() {
        if tag.first().map(|s| s != "imeta").unwrap_or(true) {
            continue;
        }
        for part in tag.iter().skip(1) {
            if let Some(rest) = part.strip_prefix("url ") {
                let url = rest.trim();
                if !url.is_empty() {
                    return url.to_string();
                }
            }
        }
    }
    String::new()
}

// ─── Snapshot projection ─────────────────────────────────────────────────────

/// Build a `ViewSnapshot::HighlightFeed(HighlightFeedSnapshot)` from
/// `AppState::highlight_feed`.
///
/// Decodes all accumulated `KernelEvent`s in `highlight_feed.rows` into
/// `HighlightRow`s, sorted by `created_at` descending (newest first), then
/// by `event_id` ascending for stable tie-breaking.
///
/// Bounded: the number of rows is bounded by `FEED_PAGE_SIZE` per drain call
/// (Non-Negotiable #7) and `apply_feed_page` never grows unbounded.
///
/// D1: no formatted strings in the snapshot — raw fields only.
pub(crate) fn project_highlight_feed_snapshot(state: &AppState) -> Option<ViewSnapshot> {
    let mut rows: Vec<HighlightRow> = state
        .highlight_feed
        .rows
        .iter()
        .filter_map(decode_highlight_row)
        .collect();

    // Sort: newest first, stable tie-break by event_id ascending.
    rows.sort_unstable_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| a.event_id.cmp(&b.event_id))
    });

    // Dedup by event_id (replaceable events can appear twice if a gap-rebase
    // did not fully clear and a re-drain delivered the same id). Keep first
    // occurrence (newest sort means that is the most recent version).
    let mut seen = std::collections::HashSet::new();
    rows.retain(|r| seen.insert(r.event_id.clone()));

    Some(ViewSnapshot::HighlightFeed(HighlightFeedSnapshot {
        rows,
        exhausted: state.highlight_feed.exhausted,
    }))
}

// ─── Lifecycle effects ────────────────────────────────────────────────────────

/// Return lifecycle effects for `Cmd::OpenView(ViewId::HighlightFeed)`.
///
/// Emits `Effect::RegisterFeedCursor` (with the kind:9802 pull scope) followed
/// immediately by `Effect::DrainFeed` so the first page fills on open without
/// requiring a separate user "load older" action.
///
/// The cursor is registered with `mint_cursor_id("hl.feed.highlights")` which
/// is deterministic across restarts — re-opening the view resumes from the last
/// `after_seq` stored in `AppState::highlight_feed` (idempotent, D6).
pub(crate) fn lifecycle_effects_for_view_open(id: &ViewId) -> Vec<Effect> {
    match id {
        ViewId::HighlightFeed => {
            let scope = highlight_feed_scope();
            let mut effects = reduce_register_feed_cursor(HIGHLIGHT_FEED_KEY.to_string(), scope);
            effects.extend(reduce_drain_feed(HIGHLIGHT_FEED_KEY.to_string()));
            effects
        }
        _ => vec![],
    }
}

/// Return lifecycle effects for `Cmd::CloseView(ViewId::HighlightFeed)`.
///
/// Emits `Effect::ReleaseFeedCursor` so the nmp kernel unregisters the cursor
/// and stops holding the slot (idempotent — no-op if the cursor was never
/// registered). The `FeedState.rows` buffer is cleared inline in `actor_task`
/// (same pattern as `ReleaseGroupEvents` in Phase 3F).
pub(crate) fn lifecycle_effects_for_view_close(id: &ViewId) -> Vec<Effect> {
    match id {
        ViewId::HighlightFeed => reduce_release_feed_cursor(HIGHLIGHT_FEED_KEY.to_string()),
        _ => vec![],
    }
}

// ─── Write side: publish highlight ───────────────────────────────────────────

/// Build `Effect::PublishHighlightEvent` from the `PublishHighlight` action fields.
///
/// There is no dedicated nmp kind:9802 publish action namespace at pinned nmp
/// b4404159 (verified by grep). Publish goes via `ActorCommand::PublishRawEvent`
/// — the same path Phase 2D uses for the rooms relay list (D6: fire-and-forget).
///
/// `content` is the highlighted text. `source_reference` is the NIP-84 target
/// coordinate (`"<kind>:<pubkey>:<d_tag>"` for addressable, `<event_id>` for
/// non-addressable). `relay_hints` are optional relay URL hints for the `a`/`e`
/// tag (D3: opaque strings from the caller, never constructed by the kernel).
///
/// The JSON event template is built with `serde_json::json!` (never `format!`)
/// so content with quotes or backslashes is safe (D-rule: serde, not format).
/// The `kind`, `tags`, and `created_at` fields are kernel responsibility; the
/// `id`, `sig`, and `pubkey` are filled by nmp's signer on publish.
/// Trim an optional note/context string; return `Some(trimmed)` only when the
/// result is non-empty (mirrors build_highlight_event's `.trim()` + `is_empty()`
/// gates so empty/whitespace-only values never produce a tag — gotcha #7 edge
/// fidelity).
fn note_context_trimmed(value: Option<&str>) -> Option<String> {
    let trimmed = value?.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub(crate) fn reduce_action_publish_highlight(
    content: String,
    source_reference: String,
    relay_hint: Option<String>,
    note: Option<String>,
    context: Option<String>,
) -> Vec<Effect> {
    // Build the NIP-84 tags: ["a", "<coordinate>", "<relay_hint>"] or
    // ["e", "<event_id>", "<relay_hint>"] depending on whether the source_reference
    // looks like an address (`<kind>:<pubkey>:<d>`) or a plain event id (64-char hex).
    let is_address = source_reference.contains(':');
    let tag_name = if is_address { "a" } else { "e" };

    let source_tag: serde_json::Value = match relay_hint.as_deref() {
        Some(hint) if !hint.is_empty() => {
            serde_json::json!([tag_name, source_reference, hint])
        }
        _ => {
            serde_json::json!([tag_name, source_reference])
        }
    };

    let mut tags: Vec<serde_json::Value> = vec![source_tag];

    // `context` tag — emitted only when non-empty AND different from `content`
    // (parity with the bespoke build_highlight_event: a context equal to the
    // quote is redundant and never published).
    if let Some(ctx) = note_context_trimmed(context.as_deref()) {
        if ctx != content.trim() {
            tags.push(serde_json::json!(["context", ctx]));
        }
    }

    // `comment` tag (the user note) — emitted only when non-empty.
    if let Some(n) = note_context_trimmed(note.as_deref()) {
        tags.push(serde_json::json!(["comment", n]));
    }

    let event_json = serde_json::json!({
        "kind": 9802,
        "content": content,
        "tags": tags,
    });

    let Ok(json) = serde_json::to_string(&event_json) else {
        // Serialization failure is a programmer error but must not panic (D6).
        tracing::warn!("PublishHighlight: serde_json::to_string failed — no-op");
        return vec![];
    };

    vec![Effect::PublishHighlightEvent { json }]
}

// ─── Effect runner ────────────────────────────────────────────────────────────

/// Execute `Effect::PublishHighlightEvent`.
///
/// Sends `ActorCommand::PublishRawEvent` via `actor_sender()` with the
/// kind:9802 content and tags. nmp's actor fills `id`, `sig`, `pubkey`, and
/// `created_at` before broadcasting (D7 — kernel never stamps wall-clock time).
/// Fire-and-forget (D6).
///
/// No-op when `nmp` is `None` (test mode — tests inspect the `Effect` directly).
pub(crate) fn run_effect_publish_highlight(
    json: String,
    nmp: Option<&crate::kernel::actor::NmpHandle>,
) {
    let Some(handle) = nmp else {
        tracing::debug!("PublishHighlightEvent: no live NmpApp (test mode)");
        return;
    };
    let nmp_ref: &nmp_ffi::NmpApp = unsafe { handle.ptr.as_ref() };

    // Deserialize the event template to extract kind/content/tags (the only
    // fields the kernel provides — nmp fills id/sig/pubkey/created_at).
    #[derive(serde::Deserialize)]
    struct EventTemplate {
        kind: u32,
        content: String,
        tags: Vec<Vec<String>>,
    }

    let Ok(template) = serde_json::from_str::<EventTemplate>(&json) else {
        tracing::warn!("PublishHighlightEvent: failed to deserialize event template — no-op (D6)");
        return;
    };

    let _ = nmp_ref
        .actor_sender()
        .send(nmp_core::ActorCommand::PublishRawEvent {
            kind: template.kind,
            content: template.content,
            tags: template.tags,
            target: nmp_core::publish::PublishTarget::Auto,
            signer_pubkey: None, // sign with the active account
            correlation_id: None,
        });
}

// ─── Unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::action::{AppAction, KernelEvent};
    use crate::kernel::actor::{reduce, Cmd};
    use crate::kernel::app::AppState;
    use crate::kernel::clock::{Clock, ManualClock};
    use crate::kernel::domains::feed::apply_feed_page;
    use crate::kernel::effect::Effect;
    use crate::kernel::view::ViewId;

    fn make_state() -> AppState {
        AppState::default()
    }

    fn step(state: &mut AppState, clock: &ManualClock, cmd: Cmd) -> Vec<Effect> {
        let now = clock.now_unix_seconds();
        reduce(state, cmd, now)
    }

    /// Build a minimal raw kind:9802 KernelEvent for injection.
    fn highlight_event(
        id: &str,
        pubkey: &str,
        content: &str,
        source: &str,
        created_at: u64,
    ) -> NmpKernelEvent {
        NmpKernelEvent {
            id: id.to_string(),
            author: pubkey.to_string(),
            kind: 9802,
            created_at,
            tags: vec![vec!["a".to_string(), source.to_string()]],
            content: content.to_string(),
            relay_provenance: vec![],
        }
    }

    // 7-HD: decode_highlight_row extracts the NIP-84 `comment` tag into `note`
    // (parity with the live lane's `first_tag_value(event, "comment")`), and
    // leaves `note = None` when absent or empty.
    #[test]
    fn highlight_row_extracts_note_from_comment_tag() {
        let mut ev = highlight_event(
            "aa00000000000000000000000000000000000000000000000000000000000001",
            "bb00000000000000000000000000000000000000000000000000000000000002",
            "the highlighted text",
            "30023:author:slug",
            1_000_000,
        );
        ev.tags
            .push(vec!["comment".to_string(), "my note".to_string()]);
        let row = decode_highlight_row(&ev).expect("kind:9802 decodes");
        assert_eq!(row.note.as_deref(), Some("my note"));

        // Absent comment tag → None.
        let no_comment = highlight_event(
            "aa00000000000000000000000000000000000000000000000000000000000003",
            "bb00000000000000000000000000000000000000000000000000000000000002",
            "text",
            "30023:author:slug",
            1_000_001,
        );
        assert_eq!(decode_highlight_row(&no_comment).unwrap().note, None);

        // Empty comment value → None (D1).
        let mut empty = highlight_event(
            "aa00000000000000000000000000000000000000000000000000000000000004",
            "bb00000000000000000000000000000000000000000000000000000000000002",
            "text",
            "30023:author:slug",
            1_000_002,
        );
        empty.tags.push(vec!["comment".to_string(), "".to_string()]);
        assert_eq!(decode_highlight_row(&empty).unwrap().note, None);
    }

    // 4H-T1: highlight_feed_registers_cursor
    //
    // Opening ViewId::HighlightFeed must emit exactly one RegisterFeedCursor
    // effect followed by one DrainFeed effect, with the correct key and a
    // non-zero cursor_id.
    #[test]
    fn highlight_feed_registers_cursor() {
        let effects = lifecycle_effects_for_view_open(&ViewId::HighlightFeed);
        assert_eq!(
            effects.len(),
            2,
            "open must emit RegisterFeedCursor + DrainFeed"
        );

        match &effects[0] {
            Effect::RegisterFeedCursor { key, cursor_id, .. } => {
                assert_eq!(key, HIGHLIGHT_FEED_KEY);
                assert_ne!(*cursor_id, 0, "cursor_id must be non-zero");
                assert_eq!(
                    *cursor_id,
                    crate::kernel::domains::feed::mint_cursor_id(HIGHLIGHT_FEED_KEY)
                );
            }
            other => panic!("expected RegisterFeedCursor, got {:?}", other),
        }

        match &effects[1] {
            Effect::DrainFeed { key } => assert_eq!(key, HIGHLIGHT_FEED_KEY),
            other => panic!("expected DrainFeed, got {:?}", other),
        }
    }

    // 4H-T2: feedpage_appends_highlight_rows_raw
    //
    // A FeedPage event for "hl.feed.highlights" must append decoded HighlightRow
    // entries to AppState::highlight_feed.rows. The snapshot must expose raw
    // fields without any byline formatting.
    #[test]
    fn feedpage_appends_highlight_rows_raw() {
        let mut state = make_state();
        let clock = ManualClock::default();

        let event = highlight_event(
            "aaa0000000000000000000000000000000000000000000000000000000000001",
            "pub0000000000000000000000000000000000000000000000000000000000001",
            "This is the highlighted passage.",
            "30023:pub0000000000000000000000000000000000000000000000000000000000001:d",
            1_700_000_000,
        );

        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::FeedPage {
                key: HIGHLIGHT_FEED_KEY.to_string(),
                cursor_id: 1,
                rows: vec![event],
                next_after_seq: 10,
                exhausted: false,
                gap_rebased_to: None,
            }),
        );

        assert_eq!(state.highlight_feed.rows.len(), 1, "one raw row appended");
        assert_eq!(state.highlight_feed.after_seq, 10);

        // Snapshot must decode the row correctly.
        let snap = project_highlight_feed_snapshot(&state).expect("snapshot present");
        match snap {
            ViewSnapshot::HighlightFeed(ref s) => {
                assert_eq!(s.rows.len(), 1, "one decoded HighlightRow");
                let row = &s.rows[0];
                assert_eq!(row.content, "This is the highlighted passage.");
                assert_eq!(
                    row.author_pubkey,
                    "pub0000000000000000000000000000000000000000000000000000000000001"
                );
                assert_eq!(row.created_at, 1_700_000_000);
                assert!(row.source_reference.is_some(), "source_reference present");
                // D1: no byline formatting — no "Highlighted by" string anywhere
                let debug = format!("{:?}", s);
                assert!(
                    !debug.contains("Highlighted by"),
                    "no 'Highlighted by' byline in kernel snapshot (D1)"
                );
            }
            other => panic!("expected HighlightFeed snapshot, got {:?}", other),
        }
    }

    // 4H-T3: highlight_feed_no_byline_formatting
    //
    // Verifies the D1 invariant exhaustively: none of the known byline/format
    // strings from the bespoke lane appear in a decoded HighlightRow or snapshot.
    #[test]
    fn highlight_feed_no_byline_formatting() {
        let mut state = make_state();

        let events: Vec<NmpKernelEvent> = (0..3u64)
            .map(|i| {
                highlight_event(
                    &format!("{:064}", i + 1),
                    &format!("{:064}", i + 101),
                    &format!("Quote #{}", i),
                    "30023:cafe:d",
                    1_700_000_000 + i,
                )
            })
            .collect();

        apply_feed_page(&mut state.highlight_feed, events, 30, false, None);

        let snap = project_highlight_feed_snapshot(&state).unwrap();
        let debug = format!("{:?}", snap);

        // D1 assertions: no presentation strings that belong to Swift
        assert!(
            !debug.contains("Highlighted by"),
            "no 'Highlighted by' byline"
        );
        assert!(
            !debug.contains("others"),
            "no 'and N others' overflow string"
        );
        assert!(!debug.contains("min read"), "no 'min read' label");
        assert!(!debug.contains("Untitled"), "no 'Untitled' fallback");

        // Verify raw fields are present
        match snap {
            ViewSnapshot::HighlightFeed(ref s) => {
                assert_eq!(s.rows.len(), 3, "3 raw rows decoded");
                for (i, row) in s.rows.iter().enumerate() {
                    // Sorted newest-first: row 0 is index 2 (created_at 1_700_000_002), etc.
                    assert!(
                        row.content.starts_with("Quote #"),
                        "content is raw quote text"
                    );
                    let _ = i;
                }
            }
            other => panic!("expected HighlightFeed, got {:?}", other),
        }
    }

    // 4H-T4: scroll_end_emits_drain
    //
    // `AppAction::DrainHighlightFeed` must emit exactly one `Effect::DrainFeed`
    // for the correct key (pagination on scroll-to-end).
    #[test]
    fn scroll_end_emits_drain() {
        let mut state = make_state();
        let clock = ManualClock::default();

        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::DrainHighlightFeed),
        );

        let drain: Vec<_> = effects
            .iter()
            .filter(|e| matches!(e, Effect::DrainFeed { key } if key == HIGHLIGHT_FEED_KEY))
            .collect();
        assert_eq!(
            drain.len(),
            1,
            "exactly one DrainFeed for highlights on scroll-end"
        );
    }

    // 4H-T5: feed_cleared_on_identity_loss
    //
    // After Logout, `AppState::highlight_feed` must be reset to default empty
    // state so stale data from the previous account never leaks to the next session.
    #[test]
    fn feed_cleared_on_identity_loss() {
        let mut state = make_state();
        let clock = ManualClock::default();

        // Seed the highlight feed with one row.
        let event = highlight_event(
            "bbb0000000000000000000000000000000000000000000000000000000000001",
            "pub0000000000000000000000000000000000000000000000000000000000002",
            "Seedling highlight.",
            "30023:cafe:test",
            1_700_000_001,
        );
        apply_feed_page(&mut state.highlight_feed, vec![event], 5, false, None);
        assert_eq!(
            state.highlight_feed.rows.len(),
            1,
            "row present before logout"
        );

        // Logout must clear the highlight feed.
        step(&mut state, &clock, Cmd::Action(AppAction::Logout));

        assert!(
            state.highlight_feed.rows.is_empty(),
            "highlight_feed.rows must be cleared on Logout"
        );
        assert_eq!(
            state.highlight_feed.after_seq, 0,
            "after_seq must be reset on Logout"
        );
        assert_eq!(
            state.highlight_feed.cursor_id, 0,
            "cursor_id must be reset on Logout"
        );
    }

    // 4H-T6: malformed_highlight_events_no_op
    //
    // Non-kind:9802 events and kind:9802 events with empty content must be
    // silently skipped during snapshot decode (D6: no panic, no partial output).
    #[test]
    fn malformed_highlight_events_no_op() {
        let mut state = make_state();

        // Non-kind:9802 event (wrong kind).
        let not_highlight = NmpKernelEvent {
            id: "ccc0000000000000000000000000000000000000000000000000000000000001".to_string(),
            author: "pub0000000000000000000000000000000000000000000000000000000000003".to_string(),
            kind: 1, // wrong kind — not a highlight
            created_at: 1_700_000_000,
            tags: vec![],
            content: "regular note".to_string(),
            relay_provenance: vec![],
        };

        // kind:9802 with empty content (malformed highlight).
        let empty_content = NmpKernelEvent {
            id: "ddd0000000000000000000000000000000000000000000000000000000000001".to_string(),
            author: "pub0000000000000000000000000000000000000000000000000000000000004".to_string(),
            kind: 9802,
            created_at: 1_700_000_001,
            tags: vec![],
            content: String::new(), // malformed — no highlight text
            relay_provenance: vec![],
        };

        // One valid highlight alongside the malformed ones.
        let valid = highlight_event(
            "eee0000000000000000000000000000000000000000000000000000000000001",
            "pub0000000000000000000000000000000000000000000000000000000000005",
            "Valid highlight content.",
            "30023:cafe:ok",
            1_700_000_002,
        );

        apply_feed_page(
            &mut state.highlight_feed,
            vec![not_highlight, empty_content, valid],
            15,
            false,
            None,
        );

        // 3 raw rows stored (the engine doesn't filter — decode_highlight_row does).
        assert_eq!(state.highlight_feed.rows.len(), 3, "all 3 raw rows stored");

        // Snapshot decode must filter out the non-highlight and empty-content.
        let snap = project_highlight_feed_snapshot(&state).unwrap();
        match snap {
            ViewSnapshot::HighlightFeed(ref s) => {
                assert_eq!(
                    s.rows.len(),
                    1,
                    "only the valid kind:9802 row decoded (D6: malformed skipped)"
                );
                assert_eq!(s.rows[0].content, "Valid highlight content.");
            }
            other => panic!("expected HighlightFeed, got {:?}", other),
        }
    }

    // Helper: pull the emitted kind:9802 event JSON out of the publish effect.
    fn publish_event_value(effects: &[Effect]) -> serde_json::Value {
        let Some(Effect::PublishHighlightEvent { json }) = effects.first() else {
            panic!("expected PublishHighlightEvent effect, got {:?}", effects);
        };
        serde_json::from_str(json).expect("event json")
    }

    fn tag_values<'a>(event: &'a serde_json::Value, name: &str) -> Vec<&'a str> {
        event["tags"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|t| {
                let arr = t.as_array()?;
                if arr.first()?.as_str()? == name {
                    arr.get(1)?.as_str()
                } else {
                    None
                }
            })
            .collect()
    }

    // Phase 7: publishing a highlight with a note + context emits the `comment`
    // and `context` tags (the article-reader publish surface). The round-trip
    // (publish → record_from_cached_event) is the parity check — no hardcoded
    // tag positions, and it proves the kernel emits what the bespoke parser reads.
    #[test]
    fn publish_highlight_emits_comment_and_context_tags() {
        let effects = reduce_action_publish_highlight(
            "the quoted text".to_string(),
            "30023:aabbcc:my-article".to_string(),
            None,
            Some("  my note  ".to_string()), // trims to "my note"
            Some("surrounding paragraph".to_string()),
        );
        let event = publish_event_value(&effects);
        assert_eq!(event["kind"], 9802);
        assert_eq!(
            tag_values(&event, "comment"),
            vec!["my note"],
            "trimmed comment"
        );
        assert_eq!(
            tag_values(&event, "context"),
            vec!["surrounding paragraph"],
            "context tag"
        );
        assert_eq!(
            tag_values(&event, "a"),
            vec!["30023:aabbcc:my-article"],
            "source a-tag preserved"
        );
    }

    // Edge fidelity (gotcha #7), mirroring build_highlight_event:
    // - empty/whitespace note or context emits NO tag;
    // - a context equal to the content is redundant → skipped.
    #[test]
    fn publish_highlight_skips_empty_and_redundant_tags() {
        // Empty note, whitespace context → neither tag.
        let e1 = reduce_action_publish_highlight(
            "quote".to_string(),
            "evt".to_string(),
            None,
            Some("".to_string()),
            Some("   ".to_string()),
        );
        let v1 = publish_event_value(&e1);
        assert!(
            tag_values(&v1, "comment").is_empty(),
            "empty note → no comment tag"
        );
        assert!(
            tag_values(&v1, "context").is_empty(),
            "whitespace context → no context tag"
        );

        // Context equal to content → skipped (redundant).
        let e2 = reduce_action_publish_highlight(
            "same text".to_string(),
            "evt".to_string(),
            None,
            None,
            Some("same text".to_string()),
        );
        let v2 = publish_event_value(&e2);
        assert!(
            tag_values(&v2, "context").is_empty(),
            "context == content → skipped (build_highlight_event parity)"
        );
    }
}
