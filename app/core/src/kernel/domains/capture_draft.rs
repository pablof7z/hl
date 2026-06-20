//! Capture draft FSM domain — Phase 5F.
//!
//! Builds on the Phase 5D OCR domain (`kernel/domains/ocr.rs`). 5D added the
//! device-local OCR scan state (`AppState::ocr`, projected as
//! `ViewSnapshot::Capture(KernelCaptureSnapshot)`). 5F adds the capture DRAFT
//! state on top of the SAME `Capture` view:
//!
//! * **Draft fields** — quote / context / note / selected words / target
//!   community / publish-phase FSM (`AppState::capture_draft`).
//! * **Publish decision** — the publish reducer routes to the existing Phase 4H
//!   raw publish path (kind:9802 highlight) when a quote is present, or to the
//!   new `Effect::PublishCaptureEvent` (kind:11 plain capture) when only OCR
//!   markdown is available. NO new nmp.publish namespace — both routes use
//!   `ActorCommand::PublishRawEvent` (the kernel is the sole writer).
//!
//! ## FSM
//!
//! ```text
//!   Idle ──set_quote(non-empty)──▶ Reviewing ──publish(can_publish)──▶ Publishing
//!                                                                          │
//!                                            CaptureDraftPublishResult ────┤
//!                                                                          ▼
//!                                                              Done | Error{message}
//! ```
//!
//! `reset` returns to `Idle` and clears all draft fields.
//!
//! ## Fidelity reference
//!
//! The live bespoke lane (`app/core/src/capture.rs`) is the fidelity reference
//! for the phase vocabulary (`CapturePublishPhase`, can-publish predicate, text
//! trimming). The live module is UNTOUCHED (Non-Negotiable #6). 5F mirrors its
//! `CapturePublishPhase` as the kernel-owned `CaptureDraftPhase`.
//!
//! ## Raw doctrine (D1)
//!
//! The snapshot carries raw fields only — no formatted strings, no community
//! name fallbacks ("Optional"), no markdown preview labels. Swift owns all
//! presentation.

use crate::kernel::app::AppState;
use crate::kernel::effect::Effect;
use crate::kernel::snapshot::{KernelCaptureDraftPhase, KernelCaptureSnapshot, ViewSnapshot};

// ─── Publish-phase FSM ─────────────────────────────────────────────────────────

/// Publish-phase FSM for a capture draft.
///
/// Mirrors `CapturePublishPhase` in the live bespoke lane (`capture.rs`), minus
/// the `Processing` (upload-in-flight) phase, which the nmp-lane does not model
/// at slice 5F (image upload is deferred). `Error { message }` carries the raw
/// publish error so the snapshot can surface it (D1: Swift formats the copy).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum CaptureDraftPhase {
    /// No draft in progress.
    #[default]
    Idle,
    /// A non-empty quote (or markdown source) is staged and editable.
    Reviewing,
    /// A publish effect has been emitted; awaiting the result.
    Publishing,
    /// The publish completed successfully.
    Done,
    /// The publish failed. `message` is the raw error (D1).
    Error { message: String },
}

// ─── AppState::capture_draft ────────────────────────────────────────────────────

/// Capture draft state — quote / context / note + target community + publish FSM.
///
/// Device-local until publish: the draft fields are scratch state, not nostr
/// facts (`hl-app-state-vs-nostr-facts`). Only `reduce_action_publish` turns the
/// draft into a real nostr event (kind:9802 or kind:11) via the raw publish path.
///
/// Bounded: a single fixed-overhead struct. `selected_word_indices` is bounded by
/// the OCR scan's word count, not by the event store (Non-Negotiable #7).
#[derive(Debug, Clone, Default)]
pub struct CaptureDraftState {
    /// The highlighted/selected quote text.
    pub quote: String,
    /// The surrounding source context (paragraph the quote was lifted from).
    pub context: String,
    /// A user-authored note attached to the capture.
    pub note: String,
    /// Indices into `AppState::ocr.selectable_words` for the current drag selection.
    pub selected_word_indices: Vec<usize>,
    /// The NIP-29 group id this capture targets, validated against
    /// `AppState::communities`. `None` for a standalone capture.
    pub target_group_id: Option<String>,
    /// Publish-phase FSM state.
    pub publish_phase: CaptureDraftPhase,
    /// `true` once an image upload has completed (deferred in 5F — always false).
    pub has_upload: bool,
}

impl CaptureDraftState {
    /// The publish gate.
    ///
    /// `can_publish` ⟺ phase is `Reviewing` AND
    /// (quote is non-empty) OR (OCR markdown is non-empty AND a target group is set).
    ///
    /// The quote path publishes a free-standing kind:9802 highlight; the
    /// markdown path publishes a kind:11 capture into a group.
    pub fn can_publish(&self, ocr_markdown: &str) -> bool {
        if self.publish_phase != CaptureDraftPhase::Reviewing {
            return false;
        }
        if !self.quote.is_empty() {
            return true;
        }
        !ocr_markdown.is_empty() && self.target_group_id.is_some()
    }
}

// ─── Reducers (action envelope) ─────────────────────────────────────────────────

/// `hl.capture.set_quote { quote }` — set the draft quote.
///
/// A non-empty quote transitions `Idle → Reviewing` (the draft becomes
/// publishable). An empty quote is a no-op when the phase is `Idle` (D6: nothing
/// to review) — the phase is NOT forced backward, mirroring the live lane's
/// `should_stash` rejection of blank quotes.
pub(crate) fn reduce_action_set_quote(
    state: &mut AppState,
    quote: String,
    _now: u64,
) -> Vec<Effect> {
    if quote.is_empty() {
        // Blank quote: no-op (live lane rejects blank stashes). Do not advance
        // the FSM and do not clobber a prior quote with an empty one.
        return vec![];
    }
    state.capture_draft.quote = quote;
    if state.capture_draft.publish_phase == CaptureDraftPhase::Idle {
        state.capture_draft.publish_phase = CaptureDraftPhase::Reviewing;
    }
    vec![]
}

/// `hl.capture.set_context { context }` — set the surrounding source context.
pub(crate) fn reduce_action_set_context(state: &mut AppState, context: String) -> Vec<Effect> {
    state.capture_draft.context = context;
    vec![]
}

/// `hl.capture.set_note { note }` — set the user-authored note.
pub(crate) fn reduce_action_set_note(state: &mut AppState, note: String) -> Vec<Effect> {
    state.capture_draft.note = note;
    vec![]
}

/// `hl.capture.select_word { word_index }` — append a word index to the selection.
///
/// `word_index` is `u64` over FFI; stored as `usize`. Duplicate appends are
/// permitted (Swift owns drag-select de-dup geometry; the kernel just records).
pub(crate) fn reduce_action_select_word(state: &mut AppState, word_index: u64) -> Vec<Effect> {
    state
        .capture_draft
        .selected_word_indices
        .push(word_index as usize);
    vec![]
}

/// `hl.capture.clear_selection` — clear the current word selection.
pub(crate) fn reduce_action_clear_selection(state: &mut AppState) -> Vec<Effect> {
    state.capture_draft.selected_word_indices.clear();
    vec![]
}

/// `hl.capture.set_target_group { group_id }` — set the target community.
///
/// Validated against `AppState::communities` (Phase 3B `CommunityRow`). When the
/// `group_id` is not a joined community, the action is a no-op (D6) — the target
/// is left unchanged so a stale/unknown id cannot leak into a publish.
pub(crate) fn reduce_action_set_target_group(
    state: &mut AppState,
    group_id: String,
    _now: u64,
) -> Vec<Effect> {
    let known = state.communities.iter().any(|c| c.group_id == group_id);
    if !known {
        tracing::debug!(%group_id, "set_target_group: unknown community — no-op (D6)");
        return vec![];
    }
    state.capture_draft.target_group_id = Some(group_id);
    vec![]
}

/// `hl.capture.clear_target_group` — clear the target community.
pub(crate) fn reduce_action_clear_target_group(state: &mut AppState) -> Vec<Effect> {
    state.capture_draft.target_group_id = None;
    vec![]
}

/// `hl.capture.publish` — emit the publish effect if the draft is publishable.
///
/// Publish decision (no new nmp.publish namespace — both routes reuse the Phase
/// 4H raw publish path via `PublishRawEvent`):
///
/// * `quote` non-empty → `Effect::PublishHighlightEvent { json }` (kind:9802),
///   the same effect Phase 4H emits. Tags carry the context as `["source", ..]`
///   and the note as `["alt", ..]`.
/// * `quote` empty but OCR markdown present → `Effect::PublishCaptureEvent { json }`
///   (kind:11). Same `PublishRawEvent` runner.
///
/// When `can_publish` is false the action is a no-op (D6 — no event, no phase
/// change). On a successful emit the phase advances to `Publishing`; the result
/// arrives via `KernelEvent::CaptureDraftPublishResult`.
///
/// All event JSON is built with `serde_json::json!` (never `format!`) so quotes
/// and backslashes in user text are safe (D-rule: serde, not format).
pub(crate) fn reduce_action_publish(state: &mut AppState, _now: u64) -> Vec<Effect> {
    let markdown = state.ocr.markdown.clone();
    if !state.capture_draft.can_publish(&markdown) {
        tracing::debug!("capture.publish: not publishable — no-op (D6)");
        return vec![];
    }

    let draft = &state.capture_draft;

    let effect = if !draft.quote.is_empty() {
        // Quote path → kind:9802 highlight via the Phase 4H raw publish path.
        let event_json = serde_json::json!({
            "kind": 9802,
            "content": draft.quote,
            "tags": [
                ["source", draft.context],
                ["alt", draft.note],
            ],
        });
        match serde_json::to_string(&event_json) {
            Ok(json) => Effect::PublishHighlightEvent { json },
            Err(_) => {
                tracing::warn!("capture.publish: serde_json failed (9802) — no-op (D6)");
                return vec![];
            }
        }
    } else {
        // Markdown path → kind:11 plain capture via the raw publish path.
        // `target_group_id` is guaranteed `Some` here by `can_publish`.
        let group_id = draft.target_group_id.clone().unwrap_or_default();
        let event_json = serde_json::json!({
            "kind": 11,
            "content": markdown,
            "tags": [
                ["h", group_id],
                ["alt", draft.note],
            ],
        });
        match serde_json::to_string(&event_json) {
            Ok(json) => Effect::PublishCaptureEvent { json },
            Err(_) => {
                tracing::warn!("capture.publish: serde_json failed (11) — no-op (D6)");
                return vec![];
            }
        }
    };

    state.capture_draft.publish_phase = CaptureDraftPhase::Publishing;
    vec![effect]
}

/// `hl.capture.reset` — reset all draft state to defaults (phase back to Idle).
pub(crate) fn reduce_action_reset(state: &mut AppState) -> Vec<Effect> {
    state.capture_draft = CaptureDraftState::default();
    vec![]
}

// ─── Reducer (kernel event) ─────────────────────────────────────────────────────

/// `KernelEvent::CaptureDraftPublishResult` — apply the publish outcome.
///
/// Mirrors the live lane's `publish_result_projection`: success → `Done`,
/// failure → `Error { message }` (the raw error, D1). The event is produced by
/// the publish round-trip; in tests it is injectable directly via `Cmd::Event`.
pub(crate) fn reduce_event_publish_result(
    state: &mut AppState,
    success: bool,
    _event_id: String,
    error: String,
) -> Vec<Effect> {
    state.capture_draft.publish_phase = if success {
        CaptureDraftPhase::Done
    } else {
        CaptureDraftPhase::Error { message: error }
    };
    vec![]
}

// ─── Snapshot projection ────────────────────────────────────────────────────────

/// Project `ViewId::Capture` from BOTH `AppState::ocr` (Phase 5D OCR fields) and
/// `AppState::capture_draft` (Phase 5F draft fields).
///
/// `ViewId::Capture` maps to `ViewSnapshot::Capture(KernelCaptureSnapshot)`. This
/// is now the authoritative projector for that view (the actor routes
/// `ViewId::Capture` here instead of `ocr::project_capture_snapshot`). The OCR
/// fields are sourced via `ocr::project_capture_snapshot` to avoid duplicating
/// 5D logic; the draft fields are appended.
///
/// D1: raw fields only — no formatted strings, no community name fallbacks.
pub(crate) fn project_capture_snapshot(state: &AppState) -> Option<ViewSnapshot> {
    // Reuse the 5D OCR projector to fill the OCR fields, then layer on the draft.
    let Some(ViewSnapshot::Capture(ocr_fields)) =
        crate::kernel::domains::ocr::project_capture_snapshot(state)
    else {
        return None;
    };

    let draft = &state.capture_draft;
    let phase = match &draft.publish_phase {
        CaptureDraftPhase::Idle => KernelCaptureDraftPhase::Idle,
        CaptureDraftPhase::Reviewing => KernelCaptureDraftPhase::Reviewing,
        CaptureDraftPhase::Publishing => KernelCaptureDraftPhase::Publishing,
        CaptureDraftPhase::Done => KernelCaptureDraftPhase::Done,
        CaptureDraftPhase::Error { .. } => KernelCaptureDraftPhase::Error,
    };
    let publish_error = match &draft.publish_phase {
        CaptureDraftPhase::Error { message } => message.clone(),
        _ => String::new(),
    };
    let can_publish = draft.can_publish(&ocr_fields.markdown);

    Some(ViewSnapshot::Capture(KernelCaptureSnapshot {
        // Phase 5D OCR fields (sourced from ocr::project_capture_snapshot).
        image_handle: ocr_fields.image_handle,
        markdown: ocr_fields.markdown,
        selectable_words: ocr_fields.selectable_words,
        raw_lines: ocr_fields.raw_lines,
        pending: ocr_fields.pending,
        // Phase 5F draft fields.
        draft_quote: draft.quote.clone(),
        draft_context: draft.context.clone(),
        draft_note: draft.note.clone(),
        selected_word_indices: draft
            .selected_word_indices
            .iter()
            .map(|&i| i as u64)
            .collect(),
        target_group_id: draft.target_group_id.clone(),
        publish_phase: phase,
        can_publish,
        publish_error,
    }))
}

// ─── Unit tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::ocr::{OcrLine, OcrRect, OcrResult};
    use crate::capabilities::CapabilityResult;
    use crate::kernel::action::{AppActionEnvelope, KernelEvent};
    use crate::kernel::actor::{reduce, Cmd};
    use crate::kernel::clock::{Clock, ManualClock};
    use crate::kernel::snapshot::{CommunityRow, ViewSnapshot};
    use crate::kernel::view::{ViewId, ViewRoute};

    fn make_state() -> AppState {
        AppState::default()
    }

    fn step(state: &mut AppState, clock: &ManualClock, cmd: Cmd) -> Vec<Effect> {
        let now = clock.now_unix_seconds();
        reduce(state, cmd, now)
    }

    fn envelope(ns: &str, json: &str) -> Cmd {
        Cmd::ActionEnvelope(AppActionEnvelope {
            namespace: ns.to_string(),
            json: json.to_string(),
        })
    }

    fn line(text: &str, x: f64, y: f64, w: f64, h: f64) -> OcrLine {
        OcrLine {
            text: text.to_string(),
            bbox: OcrRect { x, y, w, h },
            confidence: 0.9,
            words: Vec::new(),
        }
    }

    fn inject_ocr(state: &mut AppState, clock: &ManualClock) {
        state.ocr.pending = true;
        step(
            state,
            clock,
            Cmd::ProvideCapabilityResult(CapabilityResult::Ocr(OcrResult::Lines(vec![
                line(
                    "This is a captured paragraph with enough words to stay body",
                    0.1,
                    0.80,
                    0.7,
                    0.03,
                ),
                line(
                    "and it wraps across the next recognized line.",
                    0.1,
                    0.765,
                    0.65,
                    0.03,
                ),
            ]))),
        );
    }

    fn community(group_id: &str, name: &str) -> CommunityRow {
        CommunityRow {
            group_id: group_id.to_string(),
            host_relay_url: "wss://relay.example".to_string(),
            name: Some(name.to_string()),
            picture: None,
            about: None,
            member_count: 0,
            public: true,
            open: true,
            is_admin: false,
        }
    }

    // 5F-T1: capture draft from OCR text — set_quote stores + transitions to Reviewing.
    #[test]
    fn capture_draft_from_ocr_text() {
        let mut state = make_state();
        let clock = ManualClock::default();
        inject_ocr(&mut state, &clock);

        assert_eq!(state.capture_draft.publish_phase, CaptureDraftPhase::Idle);

        step(
            &mut state,
            &clock,
            envelope(
                "hl.capture.set_quote",
                r#"{"quote":"a captured paragraph"}"#,
            ),
        );

        assert_eq!(state.capture_draft.quote, "a captured paragraph");
        assert_eq!(
            state.capture_draft.publish_phase,
            CaptureDraftPhase::Reviewing
        );
    }

    // 5F-T2: select quote + context + note — all stored, phase is Reviewing.
    #[test]
    fn select_quote_and_context() {
        let mut state = make_state();
        let clock = ManualClock::default();

        step(
            &mut state,
            &clock,
            envelope("hl.capture.set_quote", r#"{"quote":"the quote"}"#),
        );
        step(
            &mut state,
            &clock,
            envelope(
                "hl.capture.set_context",
                r#"{"context":"the surrounding context"}"#,
            ),
        );
        step(
            &mut state,
            &clock,
            envelope("hl.capture.set_note", r#"{"note":"my note"}"#),
        );

        assert_eq!(state.capture_draft.quote, "the quote");
        assert_eq!(state.capture_draft.context, "the surrounding context");
        assert_eq!(state.capture_draft.note, "my note");
        assert_eq!(
            state.capture_draft.publish_phase,
            CaptureDraftPhase::Reviewing
        );
    }

    // 5F-T3: set target community from joined; unknown id is a no-op.
    #[test]
    fn set_target_community_from_joined() {
        let mut state = make_state();
        let clock = ManualClock::default();
        state.communities = vec![community("group-a", "Group A")];

        step(
            &mut state,
            &clock,
            envelope("hl.capture.set_target_group", r#"{"group_id":"group-a"}"#),
        );
        assert_eq!(
            state.capture_draft.target_group_id.as_deref(),
            Some("group-a")
        );

        // Unknown group_id → no-op (target unchanged).
        step(
            &mut state,
            &clock,
            envelope(
                "hl.capture.set_target_group",
                r#"{"group_id":"does-not-exist"}"#,
            ),
        );
        assert_eq!(
            state.capture_draft.target_group_id.as_deref(),
            Some("group-a"),
            "unknown group_id must not overwrite a valid target"
        );

        // From a fresh state, an unknown group stays None.
        let mut fresh = make_state();
        step(
            &mut fresh,
            &clock,
            envelope(
                "hl.capture.set_target_group",
                r#"{"group_id":"does-not-exist"}"#,
            ),
        );
        assert!(fresh.capture_draft.target_group_id.is_none());

        // clear_target_group resets it.
        step(
            &mut state,
            &clock,
            envelope("hl.capture.clear_target_group", "{}"),
        );
        assert!(state.capture_draft.target_group_id.is_none());
    }

    // 5F-T4: publish routes to the existing Phase 4H highlight path (quote path),
    // reusing Effect::PublishHighlightEvent (no new publish lane).
    #[test]
    fn publish_routes_to_existing_highlight_or_artifact_path() {
        let mut state = make_state();
        let clock = ManualClock::default();

        // Reviewing state with a non-empty quote.
        step(
            &mut state,
            &clock,
            envelope("hl.capture.set_quote", r#"{"quote":"highlight me"}"#),
        );
        step(
            &mut state,
            &clock,
            envelope(
                "hl.capture.set_context",
                r#"{"context":"source paragraph"}"#,
            ),
        );
        step(
            &mut state,
            &clock,
            envelope("hl.capture.set_note", r#"{"note":"a thought"}"#),
        );

        let effects = step(&mut state, &clock, envelope("hl.capture.publish", "{}"));

        // Exactly one effect, and it is the Phase 4H highlight publish path.
        let highlight: Vec<_> = effects
            .iter()
            .filter(|e| matches!(e, Effect::PublishHighlightEvent { .. }))
            .collect();
        assert_eq!(
            highlight.len(),
            1,
            "quote path must reuse Effect::PublishHighlightEvent; got: {effects:?}"
        );

        // No kind:11 capture effect on the quote path.
        let capture: Vec<_> = effects
            .iter()
            .filter(|e| matches!(e, Effect::PublishCaptureEvent { .. }))
            .collect();
        assert!(
            capture.is_empty(),
            "quote path must not emit PublishCaptureEvent"
        );

        // The event template is kind:9802 with the quote as content.
        if let Effect::PublishHighlightEvent { json } = highlight[0] {
            let v: serde_json::Value = serde_json::from_str(json).expect("valid json");
            assert_eq!(v["kind"], 9802);
            assert_eq!(v["content"], "highlight me");
            assert_eq!(v["tags"][0][0], "source");
            assert_eq!(v["tags"][0][1], "source paragraph");
            assert_eq!(v["tags"][1][0], "alt");
            assert_eq!(v["tags"][1][1], "a thought");
        }

        // Phase advanced to Publishing.
        assert_eq!(
            state.capture_draft.publish_phase,
            CaptureDraftPhase::Publishing
        );

        // Result event → Done.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::CaptureDraftPublishResult {
                success: true,
                event_id: "abc".to_string(),
                error: String::new(),
            }),
        );
        assert_eq!(state.capture_draft.publish_phase, CaptureDraftPhase::Done);
    }

    // 5F-T4b: markdown-only path routes to Effect::PublishCaptureEvent (kind:11).
    #[test]
    fn publish_markdown_path_routes_to_capture_event() {
        let mut state = make_state();
        let clock = ManualClock::default();
        state.communities = vec![community("group-a", "Group A")];
        inject_ocr(&mut state, &clock);

        // No quote, but markdown present + a target group → markdown path.
        // Force Reviewing (no quote set, so set_quote can't advance it).
        state.capture_draft.publish_phase = CaptureDraftPhase::Reviewing;
        step(
            &mut state,
            &clock,
            envelope("hl.capture.set_target_group", r#"{"group_id":"group-a"}"#),
        );

        let effects = step(&mut state, &clock, envelope("hl.capture.publish", "{}"));

        let capture: Vec<_> = effects
            .iter()
            .filter(|e| matches!(e, Effect::PublishCaptureEvent { .. }))
            .collect();
        assert_eq!(
            capture.len(),
            1,
            "markdown path must emit Effect::PublishCaptureEvent; got: {effects:?}"
        );

        if let Effect::PublishCaptureEvent { json } = capture[0] {
            let v: serde_json::Value = serde_json::from_str(json).expect("valid json");
            assert_eq!(v["kind"], 11);
            assert!(!v["content"].as_str().unwrap_or("").is_empty());
            assert_eq!(v["tags"][0][0], "h");
            assert_eq!(v["tags"][0][1], "group-a");
        }
        assert_eq!(
            state.capture_draft.publish_phase,
            CaptureDraftPhase::Publishing
        );
    }

    // 5F-T5: capture snapshot raw — all raw fields present, no formatted strings.
    #[test]
    fn capture_snapshot_raw() {
        let mut state = make_state();
        let clock = ManualClock::default();

        step(
            &mut state,
            &clock,
            Cmd::OpenView(ViewId::Capture, ViewRoute::Capture),
        );
        inject_ocr(&mut state, &clock);

        step(
            &mut state,
            &clock,
            envelope("hl.capture.set_quote", r#"{"quote":"raw quote"}"#),
        );
        step(
            &mut state,
            &clock,
            envelope("hl.capture.set_context", r#"{"context":"raw context"}"#),
        );
        step(
            &mut state,
            &clock,
            envelope("hl.capture.select_word", r#"{"word_index":2}"#),
        );

        let snap = project_capture_snapshot(&state).expect("snapshot must be Some");
        let ViewSnapshot::Capture(s) = snap else {
            panic!("expected ViewSnapshot::Capture");
        };

        // OCR fields.
        assert!(!s.markdown.is_empty(), "markdown present from OCR");
        assert!(!s.pending, "not pending after result");
        // Draft fields (raw — no formatting).
        assert_eq!(s.draft_quote, "raw quote");
        assert_eq!(s.draft_context, "raw context");
        assert_eq!(s.selected_word_indices, vec![2u64]);
        assert_eq!(s.publish_phase, KernelCaptureDraftPhase::Reviewing);
        assert!(s.can_publish, "Reviewing + non-empty quote ⇒ can_publish");
        assert!(s.publish_error.is_empty());
        assert!(s.target_group_id.is_none());
    }

    // 5F-T6: malformed / empty input is a no-op (no panic, no phase change).
    #[test]
    fn malformed_empty_no_op() {
        let mut state = make_state();
        let clock = ManualClock::default();

        // Empty quote → no state change, phase stays Idle.
        step(
            &mut state,
            &clock,
            envelope("hl.capture.set_quote", r#"{"quote":""}"#),
        );
        assert_eq!(state.capture_draft.quote, "");
        assert_eq!(state.capture_draft.publish_phase, CaptureDraftPhase::Idle);

        // Unknown target group → no-op (still None).
        step(
            &mut state,
            &clock,
            envelope("hl.capture.set_target_group", r#"{"group_id":"unknown"}"#),
        );
        assert!(state.capture_draft.target_group_id.is_none());

        // publish with nothing to publish → no effect, phase stays Idle.
        let effects = step(&mut state, &clock, envelope("hl.capture.publish", "{}"));
        let publish: Vec<_> = effects
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Effect::PublishHighlightEvent { .. } | Effect::PublishCaptureEvent { .. }
                )
            })
            .collect();
        assert!(publish.is_empty(), "no publish effect when not publishable");
        assert_eq!(state.capture_draft.publish_phase, CaptureDraftPhase::Idle);

        // Malformed JSON → invalid-action toast, no panic.
        step(
            &mut state,
            &clock,
            envelope("hl.capture.set_quote", "{not json"),
        );
        // (no assertion beyond "did not panic" — the router emits a toast)
    }

    // 5F-T7: reset clears all draft state back to Idle.
    #[test]
    fn reset_clears_draft() {
        let mut state = make_state();
        let clock = ManualClock::default();
        state.communities = vec![community("group-a", "Group A")];

        step(
            &mut state,
            &clock,
            envelope("hl.capture.set_quote", r#"{"quote":"q"}"#),
        );
        step(
            &mut state,
            &clock,
            envelope("hl.capture.set_target_group", r#"{"group_id":"group-a"}"#),
        );
        step(
            &mut state,
            &clock,
            envelope("hl.capture.select_word", r#"{"word_index":1}"#),
        );

        step(&mut state, &clock, envelope("hl.capture.reset", "{}"));

        assert_eq!(state.capture_draft.quote, "");
        assert!(state.capture_draft.target_group_id.is_none());
        assert!(state.capture_draft.selected_word_indices.is_empty());
        assert_eq!(state.capture_draft.publish_phase, CaptureDraftPhase::Idle);
    }
}
