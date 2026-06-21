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
//!                       clock_check (PUBLISH_TIMEOUT_SECS) → Error ───────┤
//!                                            CaptureDraftPublishResult ────┤
//!                                                                          ▼
//!                                                              Done | Error{message}
//! ```
//!
//! `reset` returns to `Idle` and clears all draft fields.
//!
//! ## Publishing → Done/Error completion path
//!
//! `run_effect_publish_highlight_event` (Phase 4H) dispatches via
//! `ActorCommand::PublishRawEvent` which is **fire-and-forget** — it does not
//! return a completion event. The full `action_results` typed-projection wiring
//! (correlation_id → Done/Error) is 5G's responsibility. Until 5G ships, the
//! reducer accepts `KernelEvent::CaptureDraftPublishResult` injected directly
//! (works in tests and from any future completion wiring), AND a clock-driven
//! timeout transitions `Publishing → Error` after `PUBLISH_TIMEOUT_SECS`
//! (mirroring the Phase 2A sign-in timeout pattern). This prevents the capture
//! screen from hanging on a spinner indefinitely in production.
//!
//! ## Text normalization
//!
//! All draft text (quote, context, note) is trimmed and blank-whitespace-only
//! strings are rejected as empty, matching the live lane's `should_stash`
//! rejection logic (`capture.rs::stash_projection`, `:170`) and note/context
//! trimming (`:235,:256`).
//!
//! ## `has_upload` and the cross-slice dependency on 5G
//!
//! The live `capture.rs::publish_projection` (`:182`) gates `can_publish` on
//! `phase_allows_publish && has_upload`. In the live lane every capture is
//! image-based (camera → Blossom upload). The kernel lane decouples the two
//! paths:
//!
//! * **Quote path** (kind:9802 text highlight): text-only publish is valid
//!   without a Blossom image; `has_upload` is NOT required here. The image
//!   becomes an optional enhancement once 5G/5E ship.
//! * **Markdown/kind:11 path**: `has_upload` IS required because a kind:11
//!   capture is an image share — publishing without the image descriptor is
//!   semantically wrong. 5G sets `has_upload = true` via
//!   `KernelEvent::BlossomUploadResult`; until 5G ships this path will not
//!   satisfy `can_publish` and is effectively gated closed.
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

/// Generate a random hex correlation id for action dispatch tracking.
///
/// Uses 16 random bytes (128 bits of entropy) encoded as a lowercase hex string
/// (32 chars). Matches the nmp correlation_id alphabet: the registry mints 32-hex
/// ids; hl mints the same shape so the action_results projection can compare by
/// string equality without length or charset checks.
///
/// Uses `uuid::Uuid::new_v4()` for true cryptographic randomness via the OS
/// random source (not clock-based XOR). Collision probability across all
/// in-flight uploads within a session is negligible at 128 bits.
pub(crate) fn new_correlation_id() -> String {
    // simple() encodes as 32 lowercase hex chars with no dashes — matches the
    // nmp correlation_id alphabet exactly (see nmp correlation id registry).
    uuid::Uuid::new_v4().simple().to_string()
}

/// Seconds after the publish effect is emitted before the FSM times out to
/// `Error`. Mirrors the Phase 2A sign-in timeout pattern (D8: clock-driven,
/// no sleeps). 5G closes the loop via action_results; this timeout is the
/// safety net for cases where nmp never posts a result (relay/network failure).
pub(crate) const PUBLISH_TIMEOUT_SECS: u64 = 30;

/// Clock-driven timeout: if the phase has been `Publishing` for longer than
/// `PUBLISH_TIMEOUT_SECS`, advance to `Error` so the capture screen does not
/// hang on a spinner indefinitely.
///
/// Called from `clock_checks` in `actor.rs` on every reduce pass (D8 / D9).
pub(crate) fn clock_check_publish_timeout(state: &mut AppState, now: u64) {
    if let CaptureDraftPhase::Publishing { started_at } = state.capture_draft.publish_phase {
        if now.saturating_sub(started_at) >= PUBLISH_TIMEOUT_SECS {
            state.capture_draft.publish_phase = CaptureDraftPhase::Error {
                message: "publish timed out".to_string(),
            };
        }
    }
}

// ─── Publish-phase FSM ─────────────────────────────────────────────────────────

/// Publish-phase FSM for a capture draft.
///
/// Mirrors `CapturePublishPhase` in the live bespoke lane (`capture.rs`), minus
/// the `Processing` (upload-in-flight) phase, which the nmp-lane does not model
/// at slice 5F (image upload is deferred). `Error { message }` carries the raw
/// publish error so the snapshot can surface it (D1: Swift formats the copy).
///
/// `Publishing { started_at }` records the UNIX second the effect was emitted so
/// `clock_check_publish_timeout` can drive a terminal transition to `Error` when
/// the completion event never arrives (fire-and-forget path; 5G will close the
/// loop properly via the action_results typed projection).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum CaptureDraftPhase {
    /// No draft in progress.
    #[default]
    Idle,
    /// A non-empty quote (or markdown source) is staged and editable.
    Reviewing,
    /// A publish effect has been emitted; awaiting the result.
    ///
    /// `started_at` is the UNIX second of the transition — used by
    /// `clock_check_publish_timeout` to drive a terminal `Error` if the result
    /// never arrives within `PUBLISH_TIMEOUT_SECS`.
    Publishing {
        /// UNIX second when `reduce_action_publish` transitioned into this phase.
        started_at: u64,
    },
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
    /// `true` once an image upload has completed successfully.
    /// Set by `reduce_event_blossom_upload_result(success=true)` in the blossom domain.
    /// Until set, the kind:11 markdown publish path stays gated (see `can_publish`).
    pub has_upload: bool,

    // ── Phase 5G additions (append-only) ─────────────────────────────────────
    /// Canonical Blossom blob URL, populated when `has_upload` becomes `true`.
    /// Empty until a successful upload result arrives. D1 — raw URL only.
    pub blossom_image_url: String,
    /// Set of correlation ids for in-flight Blossom upload actions.
    /// Each dispatched `hl.blossom.upload` adds one id (initially a placeholder;
    /// overwritten by the nmp-minted id via `NmpBlossomCorrelationMinted`). An
    /// arriving action_results row clears the matching id from the set.
    /// Using a set (not a single Option) supports two concurrent uploads (e.g.
    /// user re-taps while the first upload is in flight) without the second
    /// dispatch silently orphaning the first id.
    pub pending_upload_correlation_ids: std::collections::HashSet<String>,
    /// Correlation id of the in-flight capture-publish action (set when
    /// `Effect::PublishCaptureWithCorrelation` is emitted; cleared after the
    /// result arrives). The action_results projection matches arriving results
    /// against this id. `None` when no publish is in flight.
    pub pending_publish_correlation_id: Option<String>,
}

impl CaptureDraftState {
    /// The publish gate — mirrors `publish_projection` from the live lane
    /// (`capture.rs:182`) with per-path `has_upload` semantics.
    ///
    /// * **Quote path** (kind:9802 text highlight): `phase == Reviewing` AND
    ///   `quote` is non-empty after trim. A Blossom image is NOT required —
    ///   NIP-84 text highlights are valid without an image URL. `has_upload`
    ///   becomes optional metadata that 5G attaches when the camera was used.
    ///
    /// * **Markdown/kind:11 path**: `phase == Reviewing` AND markdown is
    ///   non-empty AND a target group is set AND `has_upload == true`. The live
    ///   lane requires `has_upload` for every capture because every capture is
    ///   image-based. This path is gated closed until 5G sets `has_upload`.
    pub fn can_publish(&self, ocr_markdown: &str) -> bool {
        if self.publish_phase != CaptureDraftPhase::Reviewing {
            return false;
        }
        // Quote path: text highlight, no image upload required.
        if !self.quote.is_empty() {
            return true;
        }
        // Markdown/kind:11 path: requires an uploaded image (5G wires this).
        self.has_upload && !ocr_markdown.is_empty() && self.target_group_id.is_some()
    }
}

// ─── Reducers (action envelope) ─────────────────────────────────────────────────

/// `hl.capture.set_quote { quote }` — set the draft quote.
///
/// Trims whitespace before storing (mirrors live `stash_projection` `:170`).
/// A blank-whitespace-only quote is rejected as a no-op (D6): the phase is NOT
/// forced backward, and a prior non-empty quote is not clobbered. A non-empty
/// trimmed quote transitions `Idle → Reviewing`.
pub(crate) fn reduce_action_set_quote(
    state: &mut AppState,
    quote: String,
    _now: u64,
) -> Vec<Effect> {
    let trimmed = quote.trim().to_string();
    if trimmed.is_empty() {
        // Blank / whitespace-only: no-op (live lane rejects blank stashes).
        return vec![];
    }
    state.capture_draft.quote = trimmed;
    if state.capture_draft.publish_phase == CaptureDraftPhase::Idle {
        state.capture_draft.publish_phase = CaptureDraftPhase::Reviewing;
    }
    vec![]
}

/// `hl.capture.set_context { context }` — set the surrounding source context.
///
/// Trims whitespace (mirrors live `stash_projection` `:235`). Blank-whitespace
/// is stored as the empty string (no FSM effect).
pub(crate) fn reduce_action_set_context(state: &mut AppState, context: String) -> Vec<Effect> {
    state.capture_draft.context = context.trim().to_string();
    vec![]
}

/// `hl.capture.set_note { note }` — set the user-authored note.
///
/// Trims whitespace (mirrors live `highlight_draft_projection` `:256`). Blank
/// note is stored as the empty string.
pub(crate) fn reduce_action_set_note(state: &mut AppState, note: String) -> Vec<Effect> {
    state.capture_draft.note = note.trim().to_string();
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
/// * `quote` non-empty → `Effect::PublishCaptureWithCorrelation { json, correlation_id }`
///   (kind:9802 highlight). Tags carry the context as `["source", ..]` and the
///   note as `["alt", ..]`. The correlation_id is stored in `pending_publish_correlation_id`
///   so the action_results projection can route the result back.
/// * `quote` empty but OCR markdown present → `Effect::PublishCaptureWithCorrelation`
///   (kind:11). Same correlation_id wiring.
///
/// When `can_publish` is false the action is a no-op (D6 — no event, no phase
/// change). On a successful emit the phase advances to `Publishing`; the result
/// arrives via `KernelEvent::CapturePublishActionResult` (action_results live path)
/// or `KernelEvent::CaptureDraftPublishResult` (test injection path).
///
/// All event JSON is built with `serde_json::json!` (never `format!`) so quotes
/// and backslashes in user text are safe (D-rule: serde, not format).
pub(crate) fn reduce_action_publish(state: &mut AppState, now: u64) -> Vec<Effect> {
    let markdown = state.ocr.markdown.clone();
    if !state.capture_draft.can_publish(&markdown) {
        tracing::debug!("capture.publish: not publishable — no-op (D6)");
        return vec![];
    }

    // Mint a correlation_id so the action_results projection can route the
    // publish outcome back to this draft. 5G closes the loop that 5F left open.
    let correlation_id = new_correlation_id();

    let draft = &state.capture_draft;

    let event_json_result = if !draft.quote.is_empty() {
        // Quote path → kind:9802 highlight via the Phase 4H raw publish path.
        // Content and tags are already trimmed (reduced in set_quote/set_context/
        // set_note), so `draft.quote` etc. are guaranteed non-empty / trimmed here.
        let event_json = serde_json::json!({
            "kind": 9802,
            "content": draft.quote,
            "tags": [
                ["source", draft.context],
                ["alt", draft.note],
            ],
        });
        serde_json::to_string(&event_json).map_err(|_| "serde_json failed (9802)")
    } else {
        // Markdown path → kind:11 plain capture via the raw publish path.
        // `target_group_id` is guaranteed `Some` AND `has_upload` is `true`
        // here (enforced by `can_publish`).
        let group_id = draft.target_group_id.clone().unwrap_or_default();
        let event_json = serde_json::json!({
            "kind": 11,
            "content": markdown,
            "tags": [
                ["h", group_id],
                ["alt", draft.note],
            ],
        });
        serde_json::to_string(&event_json).map_err(|_| "serde_json failed (11)")
    };

    let json = match event_json_result {
        Ok(j) => j,
        Err(msg) => {
            tracing::warn!("capture.publish: {} — no-op (D6)", msg);
            return vec![];
        }
    };

    // Store the correlation_id so the action_results arm can look it up.
    state.capture_draft.pending_publish_correlation_id = Some(correlation_id.clone());
    // Record `now` for the clock-driven timeout fallback (D8).
    state.capture_draft.publish_phase = CaptureDraftPhase::Publishing { started_at: now };
    vec![Effect::PublishCaptureWithCorrelation {
        json,
        correlation_id,
    }]
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

// ─── Phase 5G event reducers ────────────────────────────────────────────────────

/// `KernelEvent::CapturePublishActionResult` — live action_results completion.
///
/// Routed from `blossom::route_action_result` when the correlation_id matches
/// `AppState::capture_draft.pending_publish_correlation_id`. Drives
/// `CaptureDraftPhase::Publishing → Done | Error` for real (not just the
/// clock-timeout fallback). Clears `pending_publish_correlation_id` after settling.
pub(crate) fn reduce_event_capture_publish_action_result(
    state: &mut AppState,
    success: bool,
    error: String,
) -> Vec<Effect> {
    state.capture_draft.pending_publish_correlation_id = None;
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
        CaptureDraftPhase::Publishing { .. } => KernelCaptureDraftPhase::Publishing,
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

    // 5G-updated-T4: publish routes to PublishCaptureWithCorrelation for the quote
    // (kind:9802 highlight) path. 5G replaced the fire-and-forget effects with the
    // correlation-id-tracked variant so the action_results sidecar can close the loop.
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

        // 5G: quote path now emits PublishCaptureWithCorrelation (tracked, not fire-and-forget).
        let tracked: Vec<_> = effects
            .iter()
            .filter(|e| matches!(e, Effect::PublishCaptureWithCorrelation { .. }))
            .collect();
        assert_eq!(
            tracked.len(),
            1,
            "quote path must emit Effect::PublishCaptureWithCorrelation; got: {effects:?}"
        );

        // The event template is kind:9802 with the quote as content.
        if let Effect::PublishCaptureWithCorrelation {
            json,
            correlation_id,
        } = tracked[0]
        {
            let v: serde_json::Value = serde_json::from_str(json).expect("valid json");
            assert_eq!(v["kind"], 9802);
            assert_eq!(v["content"], "highlight me");
            assert_eq!(v["tags"][0][0], "source");
            assert_eq!(v["tags"][0][1], "source paragraph");
            assert_eq!(v["tags"][1][0], "alt");
            assert_eq!(v["tags"][1][1], "a thought");
            assert!(
                !correlation_id.is_empty(),
                "correlation_id must be non-empty"
            );
        }

        // Phase advanced to Publishing (carries started_at).
        assert!(
            matches!(
                state.capture_draft.publish_phase,
                CaptureDraftPhase::Publishing { .. }
            ),
            "expected Publishing after publish; got: {:?}",
            state.capture_draft.publish_phase
        );

        // Result event via 5G completion seam → Done.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::CapturePublishActionResult {
                success: true,
                error: String::new(),
            }),
        );
        assert_eq!(state.capture_draft.publish_phase, CaptureDraftPhase::Done);
    }

    // 5G-updated-T4b: markdown-only path routes to PublishCaptureWithCorrelation (kind:11).
    // 5G replaced PublishCaptureEvent with the tracked variant for the action_results seam.
    #[test]
    fn publish_markdown_path_routes_to_capture_event() {
        let mut state = make_state();
        let clock = ManualClock::default();
        state.communities = vec![community("group-a", "Group A")];
        inject_ocr(&mut state, &clock);

        // No quote, but markdown present + a target group + has_upload → markdown path.
        // Force Reviewing (no quote set, so set_quote can't advance it).
        state.capture_draft.publish_phase = CaptureDraftPhase::Reviewing;
        // 5G sets has_upload when the Blossom upload completes; simulate that here.
        state.capture_draft.has_upload = true;
        step(
            &mut state,
            &clock,
            envelope("hl.capture.set_target_group", r#"{"group_id":"group-a"}"#),
        );

        let effects = step(&mut state, &clock, envelope("hl.capture.publish", "{}"));

        // 5G: markdown path also emits PublishCaptureWithCorrelation, not PublishCaptureEvent.
        let tracked: Vec<_> = effects
            .iter()
            .filter(|e| matches!(e, Effect::PublishCaptureWithCorrelation { .. }))
            .collect();
        assert_eq!(
            tracked.len(),
            1,
            "markdown path must emit Effect::PublishCaptureWithCorrelation; got: {effects:?}"
        );

        if let Effect::PublishCaptureWithCorrelation { json, .. } = tracked[0] {
            let v: serde_json::Value = serde_json::from_str(json).expect("valid json");
            assert_eq!(v["kind"], 11);
            assert!(!v["content"].as_str().unwrap_or("").is_empty());
            assert_eq!(v["tags"][0][0], "h");
            assert_eq!(v["tags"][0][1], "group-a");
        }
        assert!(
            matches!(
                state.capture_draft.publish_phase,
                CaptureDraftPhase::Publishing { .. }
            ),
            "expected Publishing after markdown publish; got: {:?}",
            state.capture_draft.publish_phase
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
                // 5G: fire-and-forget effects replaced by the tracked variant.
                matches!(
                    e,
                    Effect::PublishCaptureWithCorrelation { .. }
                        | Effect::PublishHighlightEvent { .. }
                        | Effect::PublishCaptureEvent { .. }
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

    // 5F-T8: publish_advances_to_done_on_result — the completion event drives Done.
    #[test]
    fn publish_advances_to_done_on_result() {
        let mut state = make_state();
        let clock = ManualClock::default();

        step(
            &mut state,
            &clock,
            envelope("hl.capture.set_quote", r#"{"quote":"the quote"}"#),
        );
        step(&mut state, &clock, envelope("hl.capture.publish", "{}"));
        assert!(
            matches!(
                state.capture_draft.publish_phase,
                CaptureDraftPhase::Publishing { .. }
            ),
            "should be Publishing after publish dispatch"
        );

        // Inject success result (simulates the completion path wired by 5G).
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::CaptureDraftPublishResult {
                success: true,
                event_id: "deadbeef".to_string(),
                error: String::new(),
            }),
        );
        assert_eq!(
            state.capture_draft.publish_phase,
            CaptureDraftPhase::Done,
            "CaptureDraftPublishResult(success=true) must advance to Done"
        );
    }

    // 5F-T9: publish_advances_to_error_on_failure — the failure result drives Error.
    #[test]
    fn publish_advances_to_error_on_failure() {
        let mut state = make_state();
        let clock = ManualClock::default();

        step(
            &mut state,
            &clock,
            envelope("hl.capture.set_quote", r#"{"quote":"the quote"}"#),
        );
        step(&mut state, &clock, envelope("hl.capture.publish", "{}"));

        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::CaptureDraftPublishResult {
                success: false,
                event_id: String::new(),
                error: "relay rejected".to_string(),
            }),
        );
        assert!(
            matches!(
                &state.capture_draft.publish_phase,
                CaptureDraftPhase::Error { message } if message == "relay rejected"
            ),
            "CaptureDraftPublishResult(success=false) must advance to Error with message"
        );
    }

    // 5F-T10: clock timeout drives Publishing → Error when result never arrives.
    #[test]
    fn publish_timeout_drives_error() {
        let mut state = make_state();
        let clock = ManualClock::default();

        step(
            &mut state,
            &clock,
            envelope("hl.capture.set_quote", r#"{"quote":"the quote"}"#),
        );
        let effects = step(&mut state, &clock, envelope("hl.capture.publish", "{}"));
        assert!(!effects.is_empty(), "publish must emit an effect");
        assert!(matches!(
            state.capture_draft.publish_phase,
            CaptureDraftPhase::Publishing { .. }
        ));

        // Advance clock past the timeout threshold.
        clock.advance(PUBLISH_TIMEOUT_SECS);
        // Any subsequent reduce pass runs clock_checks which fires the timeout.
        step(&mut state, &clock, Cmd::Event(KernelEvent::ClockTick));

        assert!(
            matches!(
                &state.capture_draft.publish_phase,
                CaptureDraftPhase::Error { message } if message.contains("timed out")
            ),
            "clock timeout must drive Publishing → Error; got: {:?}",
            state.capture_draft.publish_phase
        );
    }

    // 5F-T11: blank-whitespace quote is rejected (mirrors live stash_projection).
    #[test]
    fn blank_whitespace_quote_rejected() {
        let mut state = make_state();
        let clock = ManualClock::default();

        // Pure whitespace.
        step(
            &mut state,
            &clock,
            envelope("hl.capture.set_quote", r#"{"quote":"   \t\n  "}"#),
        );
        assert_eq!(
            state.capture_draft.quote, "",
            "whitespace-only quote must be rejected"
        );
        assert_eq!(
            state.capture_draft.publish_phase,
            CaptureDraftPhase::Idle,
            "blank quote must not advance to Reviewing"
        );

        // Context and note are also trimmed.
        step(
            &mut state,
            &clock,
            envelope("hl.capture.set_context", r#"{"context":"  ctx  "}"#),
        );
        assert_eq!(
            state.capture_draft.context, "ctx",
            "context must be trimmed"
        );

        step(
            &mut state,
            &clock,
            envelope("hl.capture.set_note", r#"{"note":"  note  "}"#),
        );
        assert_eq!(state.capture_draft.note, "note", "note must be trimmed");

        // A real non-blank quote IS accepted.
        step(
            &mut state,
            &clock,
            envelope("hl.capture.set_quote", r#"{"quote":"  real quote  "}"#),
        );
        assert_eq!(
            state.capture_draft.quote, "real quote",
            "non-blank quote must be trimmed and stored"
        );
        assert_eq!(
            state.capture_draft.publish_phase,
            CaptureDraftPhase::Reviewing,
            "non-blank trimmed quote must advance to Reviewing"
        );
    }

    // 5G-updated-T12: can_publish_requires_upload for the markdown/kind:11 path.
    // 5G replaced PublishCaptureEvent with PublishCaptureWithCorrelation.
    #[test]
    fn can_publish_requires_upload_for_markdown_path() {
        let mut state = make_state();
        let clock = ManualClock::default();
        state.communities = vec![community("group-a", "Group A")];
        inject_ocr(&mut state, &clock);

        // Force Reviewing, set target group — but no has_upload yet.
        state.capture_draft.publish_phase = CaptureDraftPhase::Reviewing;
        step(
            &mut state,
            &clock,
            envelope("hl.capture.set_target_group", r#"{"group_id":"group-a"}"#),
        );

        // Without has_upload the markdown path must NOT publish.
        let effects = step(&mut state, &clock, envelope("hl.capture.publish", "{}"));
        let publish_effects: Vec<_> = effects
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Effect::PublishCaptureWithCorrelation { .. }
                        | Effect::PublishHighlightEvent { .. }
                        | Effect::PublishCaptureEvent { .. }
                )
            })
            .collect();
        assert!(
            publish_effects.is_empty(),
            "markdown path must not publish without has_upload; got: {effects:?}"
        );
        // Phase must not have advanced.
        assert_eq!(
            state.capture_draft.publish_phase,
            CaptureDraftPhase::Reviewing,
            "phase must stay Reviewing when publish was blocked by !has_upload"
        );

        // Now set has_upload (simulates 5G Blossom result).
        state.capture_draft.has_upload = true;
        let effects2 = step(&mut state, &clock, envelope("hl.capture.publish", "{}"));
        // 5G: now emits PublishCaptureWithCorrelation.
        let tracked_effects: Vec<_> = effects2
            .iter()
            .filter(|e| matches!(e, Effect::PublishCaptureWithCorrelation { .. }))
            .collect();
        assert_eq!(
            tracked_effects.len(),
            1,
            "markdown path must publish once has_upload is true; got: {effects2:?}"
        );
    }

    // Phase 7 E2E: full capability round-trip stitched into one chain —
    // camera → OCR → blossom → publish → Done. Each link is unit-tested above
    // (and in camera.rs / blossom.rs); this is the single regression guard that
    // the whole capture flow holds together end to end, with NO device (camera +
    // OCR + blossom results are injected). The live camera-VC presentation is the
    // only device-only piece and is exercised by `CapturePresenter` at runtime.
    #[test]
    fn capture_round_trip_camera_ocr_blossom_publish_to_done() {
        use crate::capabilities::camera::{CameraOp, CameraResult};
        use crate::capabilities::ocr::OcrOp;
        use crate::capabilities::CapabilityRequest;

        let mut state = make_state();
        let clock = ManualClock::default();

        // 1. capture_page → emits a Camera(CapturePage) request, marks pending.
        let effects = step(&mut state, &clock, envelope("hl.camera.capture_page", "{}"));
        assert!(
            effects.iter().any(|e| matches!(
                e,
                Effect::EmitCapabilityRequest(CapabilityRequest::Camera(CameraOp::CapturePage))
            )),
            "capture_page must emit a Camera(CapturePage) request; got: {effects:?}"
        );
        assert!(
            state.camera.pending,
            "capture_page must mark camera pending"
        );

        // 2. CameraResult::PageImage auto-chains to an OCR RecognizeText request
        //    on the same handle (camera::reduce_capability_camera).
        let effects = step(
            &mut state,
            &clock,
            Cmd::ProvideCapabilityResult(CapabilityResult::Camera(CameraResult::PageImage {
                image_handle: "/tmp/page.jpg".to_string(),
                width: 1200,
                height: 1600,
            })),
        );
        assert!(
            effects.iter().any(|e| matches!(
                e,
                Effect::EmitCapabilityRequest(CapabilityRequest::Ocr(OcrOp::RecognizeText {
                    image_handle,
                })) if image_handle == "/tmp/page.jpg"
            )),
            "PageImage must chain to an OCR RecognizeText on the same handle; got: {effects:?}"
        );

        // 3. OcrResult::Lines reconstructs the draft markdown.
        step(
            &mut state,
            &clock,
            Cmd::ProvideCapabilityResult(CapabilityResult::Ocr(OcrResult::Lines(vec![line(
                "A captured paragraph long enough to read as body text here.",
                0.1,
                0.80,
                0.7,
                0.03,
            )]))),
        );
        assert!(
            !state.ocr.markdown.is_empty(),
            "OCR result must populate the draft markdown"
        );

        // 4. hl.blossom.upload (native-annotated handle) → emits a BlossomUpload effect.
        let effects = step(
            &mut state,
            &clock,
            envelope(
                "hl.blossom.upload",
                r#"{"image_handle":"/tmp/annotated.jpg","servers":["https://blossom.example"]}"#,
            ),
        );
        assert!(
            effects.iter().any(|e| matches!(
                e,
                Effect::BlossomUpload { image_handle, .. } if image_handle == "/tmp/annotated.jpg"
            )),
            "blossom.upload must emit a BlossomUpload effect; got: {effects:?}"
        );

        // 5. BlossomUploadResult(success) sets has_upload on the draft.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::BlossomUploadResult {
                success: true,
                blob_url: "https://blossom.example/abc.jpg".to_string(),
                error: String::new(),
            }),
        );
        assert!(
            state.capture_draft.has_upload,
            "successful upload result must set has_upload"
        );

        // 6. set_quote → Reviewing (quote path = kind:9802 highlight).
        step(
            &mut state,
            &clock,
            envelope(
                "hl.capture.set_quote",
                r#"{"quote":"A captured paragraph"}"#,
            ),
        );
        assert_eq!(
            state.capture_draft.publish_phase,
            CaptureDraftPhase::Reviewing
        );

        // 7. publish → emits the publish effect, advances to Publishing.
        let effects = step(&mut state, &clock, envelope("hl.capture.publish", "{}"));
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::PublishCaptureWithCorrelation { .. })),
            "publish must emit a PublishCaptureWithCorrelation effect; got: {effects:?}"
        );
        assert!(matches!(
            state.capture_draft.publish_phase,
            CaptureDraftPhase::Publishing { .. }
        ));

        // 8. publish result(success) → terminal Done.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::CaptureDraftPublishResult {
                success: true,
                event_id: "evt-roundtrip".to_string(),
                error: String::new(),
            }),
        );
        assert_eq!(state.capture_draft.publish_phase, CaptureDraftPhase::Done);
    }
}
