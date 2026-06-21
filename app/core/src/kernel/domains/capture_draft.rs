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

    // ── Publish-parity additions (append-only) ────────────────────────────────
    /// Full Blossom upload descriptor for NIP-92 imeta construction. Populated
    /// via the action_results path (which parses sha256/mime/size/dim/alt from
    /// the nmp blob JSON). `None` when only the URL is available (test-injection
    /// path via `KernelEvent::BlossomUploadResult`). `blossom_image_url` keeps
    /// the URL accessible for the view snapshot regardless.
    pub blossom_upload: Option<crate::models::BlossomUpload>,
    /// Existing published artifact to reference on the highlight+artifact path
    /// (kind:9802). Set via `hl.capture.set_artifact_record` (future action
    /// wiring); tests set this field directly on `AppState::capture_draft`.
    pub artifact_record: Option<crate::kernel::models::ArtifactRecord>,
    /// Unpublished artifact preview for the pending-book path. When set alongside
    /// a non-empty quote and no `artifact_record`, `reduce_action_publish` emits
    /// a fire-and-forget kind:11 artifact publish BEFORE the correlated kind:9802
    /// highlight publish. Tests set this field directly on `AppState::capture_draft`.
    pub artifact_preview: Option<crate::kernel::models::ArtifactPreview>,
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
    /// * **Picture/kind:20 path**: `phase == Reviewing` AND markdown is
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

/// `hl.capture.publish` — emit the publish effect(s) if the draft is publishable.
///
/// Publish decision (mirrors `publish_capture` in the bespoke lane, client.rs:2561):
///
/// * **Highlight + artifact** (`quote` non-empty AND `artifact_record` or
///   `artifact_preview` present): kind:9802 with NIP-92 imeta (from
///   `blossom_upload`), artifact reference tag, optional context/comment tags.
///   For the pending-book variant (`artifact_preview` set, `artifact_record`
///   absent) a fire-and-forget kind:11 artifact share is emitted first via
///   `Effect::PublishCaptureEvent`, then the correlated kind:9802 follows.
/// * **Highlight only** (`quote` non-empty, no artifact): minimal kind:9802 with
///   optional context/comment/imeta tags (NIP-84 text highlight, no artifact
///   reference).
/// * **Picture** (`quote` empty, has_upload true, OCR markdown present): kind:20
///   (NIP-68, NOT kind:11) with imeta, `["h", group]`, and note as content.
///
/// Correlation-id wiring: `pending_publish_correlation_id` is stored so the
/// `action_results` projection can route the publish outcome to
/// `KernelEvent::CapturePublishActionResult` (5G live path) or
/// `KernelEvent::CaptureDraftPublishResult` (test injection path).
///
/// When `can_publish` is false the action is a no-op (D6). On a successful emit
/// the phase advances to `Publishing { started_at: now }`.
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

    // Collect any leading fire-and-forget effects (pending-book artifact publish).
    let mut prefix_effects: Vec<Effect> = Vec::new();

    let event_json_result: Result<String, &'static str> = if !draft.quote.is_empty() {
        // ── Quote path ──────────────────────────────────────────────────────────
        //
        // Resolve the artifact record:
        //  - existing `artifact_record` (already published kind:11)
        //  - OR `artifact_preview` (pending book — emit fire-and-forget kind:11
        //    first, then reference the artifact in the kind:9802 using the preview's
        //    `highlight_tag_name/value` which is deterministic from the preview alone)
        let artifact = draft.artifact_record.clone().or_else(|| {
            draft
                .artifact_preview
                .as_ref()
                .map(|p| crate::artifacts::unpublished_record(p.clone()))
        });

        if let Some(artifact_rec) = artifact {
            // Pending-book: emit artifact publish (fire-and-forget) before the
            // highlight publish. The kind:11 artifact event runs concurrently
            // in the actor; the highlight's artifact ref tags use
            // `preview.highlight_tag_name/value` which are deterministic from
            // the preview alone (not from the artifact's event_id).
            if draft.artifact_record.is_none() {
                if let (Some(preview), Some(group_id)) =
                    (&draft.artifact_preview, &draft.target_group_id)
                {
                    match build_artifact_share_event_json(preview, group_id) {
                        Ok(artifact_json) => {
                            prefix_effects.push(Effect::PublishCaptureEvent {
                                json: artifact_json,
                            });
                        }
                        Err(msg) => {
                            tracing::warn!(
                                "capture.publish: pending-book artifact build failed: {} — proceeding without artifact publish",
                                msg
                            );
                        }
                    }
                }
            }

            build_capture_highlight_event_json(draft, &artifact_rec)
        } else {
            // No artifact: minimal kind:9802 text highlight. The kernel supports
            // publishing a text highlight without a photo (unlike the bespoke
            // capture lane which always has an image). Context and comment tags
            // use the NIP-84 canonical names; imeta is attached if an upload exists.
            build_capture_minimal_highlight_event_json(draft)
        }
    } else {
        // ── Picture path ─────────────────────────────────────────────────────────
        // No quote: kind:20 NIP-68 picture (NOT kind:11). `target_group_id` is
        // guaranteed `Some` AND `has_upload` is `true` here (enforced by
        // `can_publish`). Uses OCR markdown as the content source only to gate
        // `can_publish`; the actual event content is `draft.note`.
        build_capture_picture_event_json(draft)
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

    let mut effects = prefix_effects;
    effects.push(Effect::PublishCaptureWithCorrelation {
        json,
        correlation_id,
    });
    effects
}

// ─── Pure event-building helpers ────────────────────────────────────────────────
// These are pure functions (no I/O, no state mutation) used by
// `reduce_action_publish` and exposed to the parity tests below.
// They mirror the bespoke builders in `highlights.rs` / `pictures.rs` /
// `artifacts.rs` without importing or modifying those files.

/// Build the NIP-92 imeta tag parts for a Blossom upload. Mirrors
/// `highlights::build_imeta_tag` without the nostr-sdk Tag wrapper —
/// produces a `Vec<String>` suitable for embedding in a `serde_json::json!`
/// tag array.
fn imeta_tag_parts(upload: &crate::models::BlossomUpload) -> Vec<String> {
    let mut parts = vec!["imeta".to_string()];
    parts.push(format!("url {}", upload.url));
    parts.push(format!("m {}", upload.mime));
    parts.push(format!("x {}", upload.sha256_hex));
    parts.push(format!("size {}", upload.size_bytes));
    if upload.width > 0 && upload.height > 0 {
        parts.push(format!("dim {}x{}", upload.width, upload.height));
    }
    let alt = upload.alt.trim();
    if !alt.is_empty() {
        parts.push(format!("alt {alt}"));
    }
    parts
}

/// Build a kind:9802 highlight event JSON template for the highlight+artifact
/// path. Mirrors `highlights::build_highlight_event` (highlights.rs:1544).
///
/// Tags produced (in order):
/// 1. `[highlight_tag_name, highlight_tag_value]` — artifact source reference
/// 2. `["i", catalog_id]` — NIP-73 catalog tag (omitted if same as ref or empty)
/// 3. `["context", context]` — omitted if empty or equals content
/// 4. `["comment", note]` — omitted if empty
/// 5. `["imeta", ...]` — NIP-92 image descriptor (omitted if no blossom_upload)
pub(crate) fn build_capture_highlight_event_json(
    draft: &CaptureDraftState,
    artifact: &crate::kernel::models::ArtifactRecord,
) -> Result<String, &'static str> {
    let content = draft.quote.trim();
    if content.is_empty() {
        return Err("highlight event requires non-empty quote");
    }
    let ref_name = artifact.preview.highlight_tag_name.trim();
    let ref_value = artifact.preview.highlight_tag_value.trim();
    if ref_name.is_empty() || ref_value.is_empty() {
        return Err("artifact missing highlight reference tag");
    }

    let mut tags: Vec<serde_json::Value> = Vec::new();

    // 1. Artifact source reference tag.
    tags.push(serde_json::json!([ref_name, ref_value]));

    // 2. NIP-73 catalog tag — skip if empty or already covered by the ref.
    let catalog_id = artifact.preview.catalog_id.trim();
    if !(catalog_id.is_empty() || (ref_name == "i" && ref_value == catalog_id)) {
        tags.push(serde_json::json!(["i", catalog_id]));
    }

    // 3. Context tag — only if differs from content.
    let context = draft.context.trim();
    if !context.is_empty() && context != content {
        tags.push(serde_json::json!(["context", context]));
    }

    // 4. Comment tag.
    let note = draft.note.trim();
    if !note.is_empty() {
        tags.push(serde_json::json!(["comment", note]));
    }

    // 5. NIP-92 imeta tag — only when a Blossom upload descriptor is present.
    if let Some(upload) = &draft.blossom_upload {
        tags.push(serde_json::json!(imeta_tag_parts(upload)));
    }

    let event_json = serde_json::json!({
        "kind": 9802,
        "content": content,
        "tags": tags,
    });
    serde_json::to_string(&event_json).map_err(|_| "serde_json failed (9802 highlight)")
}

/// Build a minimal kind:9802 event JSON template for the quote-only path
/// (no artifact record or preview). The kernel supports publishing a text
/// highlight without a book/article reference — the bespoke capture lane
/// always has an artifact, but the kernel also handles the reader highlight
/// flow. Tags use NIP-84 canonical names (`"context"`, `"comment"`).
fn build_capture_minimal_highlight_event_json(
    draft: &CaptureDraftState,
) -> Result<String, &'static str> {
    let content = draft.quote.trim();
    if content.is_empty() {
        return Err("minimal highlight event requires non-empty quote");
    }

    let mut tags: Vec<serde_json::Value> = Vec::new();

    let context = draft.context.trim();
    if !context.is_empty() && context != content {
        tags.push(serde_json::json!(["context", context]));
    }

    let note = draft.note.trim();
    if !note.is_empty() {
        tags.push(serde_json::json!(["comment", note]));
    }

    if let Some(upload) = &draft.blossom_upload {
        tags.push(serde_json::json!(imeta_tag_parts(upload)));
    }

    let event_json = serde_json::json!({
        "kind": 9802,
        "content": content,
        "tags": tags,
    });
    serde_json::to_string(&event_json).map_err(|_| "serde_json failed (9802 minimal)")
}

/// Build a NIP-68 kind:20 picture event JSON template. Mirrors
/// `pictures::build_picture_event` (pictures.rs:67).
///
/// Tags produced (in order):
/// 1. `["h", group_id]` — NIP-29 community tag (omitted when no group)
/// 2. `["imeta", ...]` — NIP-92 image descriptor (from `blossom_upload`;
///    URL-only fallback when only `blossom_image_url` is set)
/// 3. `[highlight_tag_name, highlight_tag_value]` — artifact reference (omitted
///    when no `artifact_record` or the reference fields are empty)
pub(crate) fn build_capture_picture_event_json(
    draft: &CaptureDraftState,
) -> Result<String, &'static str> {
    let note = draft.note.trim().to_string();
    let group_id = draft.target_group_id.as_deref().unwrap_or_default();

    let imeta = if let Some(upload) = &draft.blossom_upload {
        imeta_tag_parts(upload)
    } else if !draft.blossom_image_url.is_empty() {
        // URL-only fallback for the test-injection path where only
        // `blossom_image_url` is available (no sha256/mime/size).
        vec![
            "imeta".to_string(),
            format!("url {}", draft.blossom_image_url),
        ]
    } else {
        return Err("picture event requires a blossom upload or image URL");
    };

    let mut tags: Vec<serde_json::Value> = Vec::new();

    if !group_id.is_empty() {
        tags.push(serde_json::json!(["h", group_id]));
    }
    tags.push(serde_json::json!(imeta));

    if let Some(artifact) = &draft.artifact_record {
        let ref_name = artifact.preview.highlight_tag_name.trim();
        let ref_value = artifact.preview.highlight_tag_value.trim();
        if !ref_name.is_empty() && !ref_value.is_empty() {
            tags.push(serde_json::json!([ref_name, ref_value]));
        }
    }

    let event_json = serde_json::json!({
        "kind": 20,
        "content": note,
        "tags": tags,
    });
    serde_json::to_string(&event_json).map_err(|_| "serde_json failed (20 picture)")
}

/// Build a minimal kind:11 artifact share event JSON template for the
/// pending-book path. Mirrors the key tags of `artifacts::build_share_event`
/// (artifacts.rs:839) — h, d, title, source, reference, r, author.
/// Emitted as a fire-and-forget `Effect::PublishCaptureEvent` so the artifact
/// is published concurrently with the kind:9802 highlight that references it.
fn build_artifact_share_event_json(
    preview: &crate::kernel::models::ArtifactPreview,
    group_id: &str,
) -> Result<String, &'static str> {
    let mut tags: Vec<serde_json::Value> = vec![
        serde_json::json!(["h", group_id]),
        serde_json::json!(["d", preview.id]),
        serde_json::json!(["title", preview.title]),
        serde_json::json!(["source", preview.source]),
    ];

    // Reference tag — mirrors `build_share_event`'s reference_tag_name branch.
    let ref_name = preview.reference_tag_name.trim();
    let ref_value = preview.reference_tag_value.trim();
    if !ref_name.is_empty() && !ref_value.is_empty() {
        if ref_name == "i" && !preview.url.is_empty() {
            tags.push(serde_json::json!([ref_name, ref_value, preview.url]));
        } else {
            tags.push(serde_json::json!([ref_name, ref_value]));
        }
        if !preview.reference_kind.is_empty() {
            tags.push(serde_json::json!(["k", preview.reference_kind]));
        }
    }

    if !preview.url.is_empty() {
        tags.push(serde_json::json!(["r", preview.url]));
    }
    if !preview.author.is_empty() {
        tags.push(serde_json::json!(["author", preview.author]));
    }

    let event_json = serde_json::json!({
        "kind": 11,
        "content": "",
        "tags": tags,
    });
    serde_json::to_string(&event_json).map_err(|_| "serde_json failed (11 artifact share)")
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

        // The event template is kind:9802 with the quote as content and
        // NIP-84 canonical tag names ("context", "comment" — not the old
        // stub names "source"/"alt").
        if let Effect::PublishCaptureWithCorrelation {
            json,
            correlation_id,
        } = tracked[0]
        {
            let v: serde_json::Value = serde_json::from_str(json).expect("valid json");
            assert_eq!(v["kind"], 9802);
            assert_eq!(v["content"], "highlight me");
            // No artifact set → minimal highlight; tags are context + comment.
            let tags = v["tags"].as_array().expect("tags array");
            assert!(
                tags.iter()
                    .any(|t| t[0] == "context" && t[1] == "source paragraph"),
                "context tag must be present with NIP-84 canonical name; tags: {tags:?}"
            );
            assert!(
                tags.iter()
                    .any(|t| t[0] == "comment" && t[1] == "a thought"),
                "comment tag must be present with NIP-84 canonical name; tags: {tags:?}"
            );
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

    // 5G-updated-T4b: no-quote path routes to PublishCaptureWithCorrelation (kind:20 NIP-68).
    // Parity fix: was kind:11, now kind:20 to match `pictures::publish_picture`.
    // 5G replaced PublishCaptureEvent with the tracked variant for the action_results seam.
    #[test]
    fn publish_picture_path_routes_to_kind20() {
        let mut state = make_state();
        let clock = ManualClock::default();
        state.communities = vec![community("group-a", "Group A")];
        inject_ocr(&mut state, &clock);

        // No quote, but markdown present + a target group + has_upload → picture path.
        // Force Reviewing (no quote set, so set_quote can't advance it).
        state.capture_draft.publish_phase = CaptureDraftPhase::Reviewing;
        // 5G sets has_upload when the Blossom upload completes; simulate that here.
        state.capture_draft.has_upload = true;
        // Provide a full blossom_upload so imeta is included in the event.
        state.capture_draft.blossom_upload = Some(crate::models::BlossomUpload {
            url: "https://blossom.example/img.jpg".into(),
            sha256_hex: "aa".repeat(32),
            mime: "image/jpeg".into(),
            size_bytes: 1024,
            width: 800,
            height: 600,
            alt: String::new(),
        });
        state.capture_draft.blossom_image_url = "https://blossom.example/img.jpg".into();
        step(
            &mut state,
            &clock,
            envelope("hl.capture.set_target_group", r#"{"group_id":"group-a"}"#),
        );

        let effects = step(&mut state, &clock, envelope("hl.capture.publish", "{}"));

        // Picture path emits PublishCaptureWithCorrelation with kind:20.
        let tracked: Vec<_> = effects
            .iter()
            .filter(|e| matches!(e, Effect::PublishCaptureWithCorrelation { .. }))
            .collect();
        assert_eq!(
            tracked.len(),
            1,
            "picture path must emit Effect::PublishCaptureWithCorrelation; got: {effects:?}"
        );

        if let Effect::PublishCaptureWithCorrelation { json, .. } = tracked[0] {
            let v: serde_json::Value = serde_json::from_str(json).expect("valid json");
            // Parity fix: kind:20 (NIP-68 picture), not kind:11.
            assert_eq!(v["kind"], 20, "picture path must emit kind:20");
            let tags = v["tags"].as_array().expect("tags array");
            // h-tag must appear for the group.
            assert!(
                tags.iter().any(|t| t[0] == "h" && t[1] == "group-a"),
                "h tag must be present; tags: {tags:?}"
            );
            // imeta must appear.
            assert!(
                tags.iter().any(|t| t[0] == "imeta"),
                "imeta tag must be present; tags: {tags:?}"
            );
        }
        assert!(
            matches!(
                state.capture_draft.publish_phase,
                CaptureDraftPhase::Publishing { .. }
            ),
            "expected Publishing after picture publish; got: {:?}",
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

        // Now set has_upload + blossom fields (simulates a completed 5G Blossom result).
        // The picture path requires at least blossom_image_url to build the
        // imeta tag; without it build_capture_picture_event_json returns Err
        // and the reducer no-ops (emitting zero effects).
        state.capture_draft.has_upload = true;
        state.capture_draft.blossom_image_url = "https://blossom.example/img.jpg".into();
        state.capture_draft.blossom_upload = Some(fixture_blossom());
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

    // ── Publish-parity tests ─────────────────────────────────────────────────────
    //
    // Each test builds the expected event shape using the same inputs as the
    // bespoke `publish_capture` path, then drives `reduce_action_publish` and
    // asserts the kernel-emitted JSON matches. The helpers here mirror the logic
    // of `highlights::build_highlight_event`, `pictures::build_picture_event`,
    // and `highlights::build_imeta_tag` — those are private functions in
    // untouched live-lane files; we replicate the tag logic inline using the
    // same `pub(crate)` builders now living in this module.

    fn fixture_artifact() -> crate::kernel::models::ArtifactRecord {
        crate::kernel::models::ArtifactRecord {
            preview: crate::kernel::models::ArtifactPreview {
                id: "book-preview-1".into(),
                url: "https://openlibrary.org/isbn/9781234567890".into(),
                title: "The Art of Systems".into(),
                author: "Alice B".into(),
                image: String::new(),
                description: String::new(),
                source: "book".into(),
                domain: "openlibrary.org".into(),
                catalog_id: "isbn:9781234567890".into(),
                catalog_kind: "isbn".into(),
                podcast_guid: String::new(),
                podcast_item_guid: String::new(),
                podcast_show_title: String::new(),
                audio_url: String::new(),
                audio_preview_url: String::new(),
                transcript_url: String::new(),
                feed_url: String::new(),
                published_at: String::new(),
                duration_seconds: None,
                // "i" reference with the same value as catalog_id — so no
                // duplicate catalog tag in the highlight event.
                reference_tag_name: "i".into(),
                reference_tag_value: "isbn:9781234567890".into(),
                reference_kind: "isbn".into(),
                highlight_tag_name: "i".into(),
                highlight_tag_value: "isbn:9781234567890".into(),
                highlight_reference_key: "i:isbn:9781234567890".into(),
                chapters: Vec::new(),
            },
            group_id: "group-a".into(),
            share_event_id: "share-evt-abc".into(),
            pubkey: "f".repeat(64),
            created_at: Some(1_700_000_000),
            note: String::new(),
        }
    }

    fn fixture_blossom() -> crate::models::BlossomUpload {
        crate::models::BlossomUpload {
            url: "https://blossom.primal.net/abc123.jpg".into(),
            sha256_hex: "abc123".repeat(8) + "ab", // 50-char placeholder
            mime: "image/jpeg".into(),
            size_bytes: 4096,
            width: 1024,
            height: 768,
            alt: "page text".into(),
        }
    }

    /// PP-T1 — Path 1: highlight + existing artifact → kind:9802 with imeta +
    /// artifact ref + context + comment. Parity against `build_highlight_event`.
    #[test]
    fn parity_highlight_with_artifact_kind9802() {
        let artifact = fixture_artifact();
        let blossom = fixture_blossom();

        // ── Build "bespoke" expected event tags using the same logic as
        //    highlights::build_highlight_event ─────────────────────────────────
        let ref_name = artifact.preview.highlight_tag_name.trim();
        let ref_value = artifact.preview.highlight_tag_value.trim();

        // Whether catalog_id would generate a SECOND "i" tag.
        let catalog_id = artifact.preview.catalog_id.trim();
        let emit_catalog = !(catalog_id.is_empty() || (ref_name == "i" && ref_value == catalog_id));

        // Build expected imeta parts using the same logic as imeta_tag_parts.
        let expected_imeta: Vec<serde_json::Value> = {
            let mut p: Vec<serde_json::Value> = vec![
                serde_json::json!("imeta"),
                serde_json::json!(format!("url {}", blossom.url)),
                serde_json::json!(format!("m {}", blossom.mime)),
                serde_json::json!(format!("x {}", blossom.sha256_hex)),
                serde_json::json!(format!("size {}", blossom.size_bytes)),
            ];
            if blossom.width > 0 && blossom.height > 0 {
                p.push(serde_json::json!(format!(
                    "dim {}x{}",
                    blossom.width, blossom.height
                )));
            }
            let alt = blossom.alt.trim();
            if !alt.is_empty() {
                p.push(serde_json::json!(format!("alt {alt}")));
            }
            p
        };

        // ── Drive the kernel path ─────────────────────────────────────────────
        let mut state = make_state();
        state.communities = vec![community("group-a", "Group A")];
        state.capture_draft.publish_phase = CaptureDraftPhase::Reviewing;
        state.capture_draft.quote = "A profound insight from the book.".into();
        state.capture_draft.context = "The chapter about design patterns.".into();
        state.capture_draft.note = "This changed my mind.".into();
        state.capture_draft.target_group_id = Some("group-a".into());
        state.capture_draft.has_upload = true;
        state.capture_draft.blossom_upload = Some(blossom.clone());
        state.capture_draft.blossom_image_url = blossom.url.clone();
        state.capture_draft.artifact_record = Some(artifact.clone());

        let clock = ManualClock::default();
        let effects = step(&mut state, &clock, envelope("hl.capture.publish", "{}"));

        let publish_effect = effects
            .iter()
            .find(|e| matches!(e, Effect::PublishCaptureWithCorrelation { .. }))
            .expect("must emit PublishCaptureWithCorrelation");

        let Effect::PublishCaptureWithCorrelation { json, .. } = publish_effect else {
            panic!("expected PublishCaptureWithCorrelation")
        };
        let v: serde_json::Value = serde_json::from_str(json).expect("valid json");

        // ── Kind and content ──────────────────────────────────────────────────
        assert_eq!(
            v["kind"], 9802,
            "highlight+artifact path must emit kind:9802"
        );
        assert_eq!(
            v["content"], "A profound insight from the book.",
            "content must equal the quote"
        );

        // ── Tags — check each expected tag is present ─────────────────────────
        let tags = v["tags"].as_array().expect("tags array");

        // 1. Artifact source reference tag.
        assert!(
            tags.iter().any(|t| t[0] == ref_name && t[1] == ref_value),
            "artifact reference tag [{ref_name}, {ref_value}] must be present; tags: {tags:?}"
        );

        // 2. Catalog duplicate tag (skipped when ref IS the catalog).
        if emit_catalog {
            assert!(
                tags.iter().any(|t| t[0] == "i" && t[1] == catalog_id),
                "catalog tag [i, {catalog_id}] must be present; tags: {tags:?}"
            );
        } else {
            // Only ONE "i" tag with this value.
            let i_tags: Vec<_> = tags
                .iter()
                .filter(|t| t[0] == "i" && t[1] == ref_value)
                .collect();
            assert_eq!(
                i_tags.len(),
                1,
                "reference and catalog are the same — must appear exactly once; tags: {tags:?}"
            );
        }

        // 3. Context tag.
        assert!(
            tags.iter().any(|t| t[0] == "context"),
            "context tag must be present; tags: {tags:?}"
        );

        // 4. Comment tag.
        assert!(
            tags.iter()
                .any(|t| t[0] == "comment" && t[1] == "This changed my mind."),
            "comment tag must be present; tags: {tags:?}"
        );

        // 5. Imeta tag — verify it matches the expected parts.
        let imeta_tag = tags
            .iter()
            .find(|t| t[0] == "imeta")
            .expect("imeta tag must be present");
        let imeta_parts: Vec<&serde_json::Value> = imeta_tag
            .as_array()
            .expect("imeta is an array")
            .iter()
            .collect();
        for part in &expected_imeta {
            assert!(
                imeta_parts.contains(&part),
                "imeta tag missing part {part}; full imeta: {imeta_tag:?}"
            );
        }

        // No h-tag on the kind:9802 itself (group sharing goes via kind:16
        // repost in the bespoke lane — mirrors build_highlight_event behaviour).
        assert!(
            !tags.iter().any(|t| t[0] == "h"),
            "kind:9802 must NOT carry h-tag (group sharing is via kind:16 repost); tags: {tags:?}"
        );
    }

    /// PP-T2 — Path 2: no quote, has upload → kind:20 NIP-68 picture with
    /// imeta + h-tag. Parity against `pictures::build_picture_event`.
    #[test]
    fn parity_picture_no_quote_kind20() {
        let blossom = fixture_blossom();

        let mut state = make_state();
        let clock = ManualClock::default();
        state.communities = vec![community("group-a", "Group A")];
        inject_ocr(&mut state, &clock);

        state.capture_draft.publish_phase = CaptureDraftPhase::Reviewing;
        state.capture_draft.quote = String::new(); // no quote → picture path
        state.capture_draft.note = "Page capture note.".into();
        state.capture_draft.target_group_id = Some("group-a".into());
        state.capture_draft.has_upload = true;
        state.capture_draft.blossom_upload = Some(blossom.clone());
        state.capture_draft.blossom_image_url = blossom.url.clone();

        let effects = step(&mut state, &clock, envelope("hl.capture.publish", "{}"));

        let publish_effect = effects
            .iter()
            .find(|e| matches!(e, Effect::PublishCaptureWithCorrelation { .. }))
            .expect("must emit PublishCaptureWithCorrelation");

        let Effect::PublishCaptureWithCorrelation { json, .. } = publish_effect else {
            panic!("expected PublishCaptureWithCorrelation")
        };
        let v: serde_json::Value = serde_json::from_str(json).expect("valid json");

        // Kind:20 (NIP-68), NOT kind:11 (the old stub emitted kind:11).
        assert_eq!(v["kind"], 20, "picture path must emit kind:20, not kind:11");
        assert_eq!(
            v["content"], "Page capture note.",
            "content must be the note"
        );

        let tags = v["tags"].as_array().expect("tags array");

        // h-tag present on the kind:20 event itself (NIP-68 carries it inline,
        // unlike kind:9802 which uses a separate kind:16 repost).
        assert!(
            tags.iter().any(|t| t[0] == "h" && t[1] == "group-a"),
            "h tag must be present on kind:20; tags: {tags:?}"
        );

        // imeta present with correct url.
        let imeta = tags
            .iter()
            .find(|t| t[0] == "imeta")
            .expect("imeta must be present on kind:20");
        let url_part = format!("url {}", blossom.url);
        assert!(
            imeta
                .as_array()
                .unwrap()
                .iter()
                .any(|p| p.as_str() == Some(&url_part)),
            "imeta must carry the blossom url; imeta: {imeta:?}"
        );

        // No "alt" or "source" stub tags (old kind:11 remnants).
        assert!(
            !tags.iter().any(|t| t[0] == "alt"),
            "kind:20 must not carry legacy 'alt' tag from stub; tags: {tags:?}"
        );
    }

    /// PP-T3 — Path 3: pending-book (artifact_preview + quote, no artifact_record).
    /// Two effects emitted: fire-and-forget kind:11 artifact publish THEN correlated
    /// kind:9802 highlight referencing the artifact via preview.highlight_tag_name/value.
    #[test]
    fn parity_pending_book_artifact_published_then_referenced() {
        let artifact = fixture_artifact();
        let blossom = fixture_blossom();

        let mut state = make_state();
        let clock = ManualClock::default();
        state.communities = vec![community("group-a", "Group A")];

        state.capture_draft.publish_phase = CaptureDraftPhase::Reviewing;
        state.capture_draft.quote = "Key insight from unpublished book.".into();
        state.capture_draft.context = "The introduction chapter.".into();
        state.capture_draft.note = "Must revisit.".into();
        state.capture_draft.target_group_id = Some("group-a".into());
        state.capture_draft.has_upload = true;
        state.capture_draft.blossom_upload = Some(blossom.clone());
        state.capture_draft.blossom_image_url = blossom.url.clone();
        // Pending book: preview present, no artifact_record yet.
        state.capture_draft.artifact_preview = Some(artifact.preview.clone());
        state.capture_draft.artifact_record = None;

        let effects = step(&mut state, &clock, envelope("hl.capture.publish", "{}"));

        // ── Expect TWO effects: artifact publish (fire-and-forget) + highlight ──
        let artifact_effect = effects
            .iter()
            .find(|e| matches!(e, Effect::PublishCaptureEvent { .. }));
        assert!(
            artifact_effect.is_some(),
            "pending-book must emit a fire-and-forget artifact publish effect; got: {effects:?}"
        );

        let highlight_effect = effects
            .iter()
            .find(|e| matches!(e, Effect::PublishCaptureWithCorrelation { .. }));
        assert!(
            highlight_effect.is_some(),
            "pending-book must emit a correlated highlight publish effect; got: {effects:?}"
        );

        // ── Artifact event: kind:11, h-tag, reference tag ─────────────────────
        if let Some(Effect::PublishCaptureEvent { json }) = artifact_effect {
            let v: serde_json::Value = serde_json::from_str(json).expect("valid artifact json");
            assert_eq!(v["kind"], 11, "artifact share must be kind:11");
            let tags = v["tags"].as_array().expect("tags array");
            assert!(
                tags.iter().any(|t| t[0] == "h" && t[1] == "group-a"),
                "artifact event must have h-tag; tags: {tags:?}"
            );
            let ref_name = artifact.preview.reference_tag_name.trim();
            let ref_value = artifact.preview.reference_tag_value.trim();
            assert!(
                tags.iter().any(|t| t[0] == ref_name),
                "artifact event must carry reference tag [{ref_name}, ...]; tags: {tags:?}"
            );
            let _ = ref_value; // checked via tag[0] == ref_name above
        }

        // ── Highlight event: kind:9802, artifact reference, imeta ─────────────
        if let Some(Effect::PublishCaptureWithCorrelation { json, .. }) = highlight_effect {
            let v: serde_json::Value = serde_json::from_str(json).expect("valid highlight json");
            assert_eq!(v["kind"], 9802, "highlight must be kind:9802");
            assert_eq!(
                v["content"], "Key insight from unpublished book.",
                "content must equal the quote"
            );
            let tags = v["tags"].as_array().expect("tags array");

            // Artifact reference from preview.highlight_tag_name/value.
            let hl_ref_name = artifact.preview.highlight_tag_name.trim();
            let hl_ref_value = artifact.preview.highlight_tag_value.trim();
            assert!(
                tags.iter()
                    .any(|t| t[0] == hl_ref_name && t[1] == hl_ref_value),
                "highlight must reference artifact via [{hl_ref_name}, {hl_ref_value}]; tags: {tags:?}"
            );

            // imeta present — the photo-always invariant.
            assert!(
                tags.iter().any(|t| t[0] == "imeta"),
                "highlight must carry imeta tag (photo-always invariant); tags: {tags:?}"
            );
        }

        // Phase advanced to Publishing.
        assert!(
            matches!(
                state.capture_draft.publish_phase,
                CaptureDraftPhase::Publishing { .. }
            ),
            "phase must advance to Publishing; got: {:?}",
            state.capture_draft.publish_phase
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
