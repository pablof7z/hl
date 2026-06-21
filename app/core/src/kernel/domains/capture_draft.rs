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

    // ── kind:16 group-share additions (append-only) ──────────────────────────
    /// When a highlight is published into a NIP-29 group (quote path with a
    /// `target_group_id`), holds the group id so a kind:16 generic repost can be
    /// emitted AFTER the highlight publish succeeds. Mirrors the bespoke
    /// `publish_and_share` (highlights.rs:788) two-step share. Cleared once the
    /// repost is emitted. `None` when the highlight is not group-targeted.
    pub pending_group_repost_group_id: Option<String>,
    /// Active author pubkey (hex) captured at publish time for the kind:16 repost
    /// `p` tag. Paired with `pending_group_repost_group_id`.
    pub pending_group_repost_author_pubkey_hex: Option<String>,

    // ── pending-book FSM ordering (append-only) ──────────────────────────────
    /// When a pending-book artifact publish is in-flight, holds the highlight
    /// event JSON to emit once the artifact publish succeeds. The highlight
    /// publish is gated on this: if artifact fails, FSM goes to Error without
    /// publishing the highlight.
    pub pending_highlight_json: Option<String>,
}

impl CaptureDraftState {
    /// The publish gate — mirrors `publish_projection` from the live lane
    /// (`capture.rs:188`: `can_publish = phase_allows_publish && has_upload`).
    ///
    /// The iOS capture flow is always image-based (photo-always invariant), so
    /// BOTH paths require `phase == Reviewing` AND a completed Blossom upload
    /// (`has_upload == true`). No path publishes without an image.
    ///
    /// * **Quote/highlight path** (kind:9802): additionally requires a non-empty
    ///   `quote` after trim.
    ///
    /// * **Picture/kind:20 path**: additionally requires non-empty markdown AND a
    ///   target group.
    pub fn can_publish(&self, ocr_markdown: &str) -> bool {
        if self.publish_phase != CaptureDraftPhase::Reviewing {
            return false;
        }
        // The iOS capture flow is always image-based (photo-always invariant).
        // Both paths require a completed Blossom upload before publish, mirroring
        // the bespoke capture.rs:188: `can_publish = phase_allows_publish && has_upload`.
        if !self.has_upload {
            return false;
        }
        // Quote/highlight path: kind:9802.
        if !self.quote.is_empty() {
            return true;
        }
        // Picture/kind:20 path: also requires OCR markdown and a target group.
        !ocr_markdown.is_empty() && self.target_group_id.is_some()
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
///   absent, with a host group) the kind:11 artifact share is published FIRST
///   as the correlated primary; the kind:9802 highlight is deferred and emitted
///   only once the artifact publish succeeds (so a failed artifact cannot let
///   the FSM lie — Issue 3). A group-targeted highlight additionally schedules a
///   kind:16 generic repost into the group on success (Issue 1).
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

    // Active author pubkey — captured now for a possible kind:16 group repost
    // (mirrors the bespoke `publish_and_share` two-step: highlight then repost).
    let active_pubkey =
        if let crate::kernel::app::SessionState::Present { pubkey, .. } = &state.session {
            Some(pubkey.clone())
        } else {
            None
        };

    // ── Build phase (immutable borrow of the draft) ───────────────────────────
    // Outputs computed while borrowing the draft, then committed below:
    //  * `primary_json`            — event published with `correlation_id` now.
    //  * `deferred_highlight_json` — Some on the pending-book path: the kind:9802
    //    highlight to publish ONLY AFTER the artifact (the primary) succeeds, so
    //    a silent artifact failure cannot let the FSM lie (Issue 3).
    //  * `repost_group_id`         — Some when a group-targeted highlight should
    //    emit a kind:16 generic repost into the group on success (Issue 1).
    let mut deferred_highlight_json: Option<String> = None;
    let mut repost_group_id: Option<String> = None;

    let primary_json_result: Result<String, &'static str> = {
        let draft = &state.capture_draft;

        if !draft.quote.is_empty() {
            // ── Quote/highlight path (kind:9802) ─────────────────────────────────
            // A group-targeted highlight schedules a kind:16 repost on success.
            let group_id = draft.target_group_id.as_deref().filter(|g| !g.is_empty());
            if let Some(gid) = group_id {
                repost_group_id = Some(gid.to_string());
            }

            // Resolve the artifact record:
            //  - existing `artifact_record` (already published kind:11), OR
            //  - `artifact_preview` (pending book — the artifact is published
            //    first as the correlated primary, then the highlight follows).
            let artifact = draft.artifact_record.clone().or_else(|| {
                draft
                    .artifact_preview
                    .as_ref()
                    .map(|p| crate::artifacts::unpublished_record(p.clone()))
            });

            match artifact {
                Some(artifact_rec) => {
                    let is_pending_book =
                        draft.artifact_record.is_none() && draft.artifact_preview.is_some();
                    match (is_pending_book, &draft.artifact_preview, group_id) {
                        (true, Some(preview), Some(gid)) => {
                            // Pending-book with a host group: publish the artifact
                            // (kind:11) as the correlated primary; defer the
                            // highlight until that publish succeeds (Issue 3).
                            match build_capture_highlight_event_json(draft, &artifact_rec) {
                                Ok(highlight_json) => {
                                    deferred_highlight_json = Some(highlight_json);
                                    build_artifact_share_event_json(preview, gid)
                                }
                                Err(msg) => Err(msg),
                            }
                        }
                        // Published artifact, or pending-book without a host group:
                        // single correlated highlight publish.
                        _ => build_capture_highlight_event_json(draft, &artifact_rec),
                    }
                }
                None => {
                    // No artifact: minimal kind:9802 text highlight (NIP-84). The
                    // kernel supports a text highlight without a book/article ref;
                    // imeta is attached when a Blossom upload is present.
                    build_capture_minimal_highlight_event_json(draft)
                }
            }
        } else {
            // ── Picture path (kind:20 NIP-68) ────────────────────────────────────
            // No quote. `target_group_id` is guaranteed `Some` AND `has_upload`
            // is `true` here (enforced by `can_publish`). kind:20 carries the
            // `h` tag inline, so there is NO kind:16 group repost on this path.
            build_capture_picture_event_json(draft)
        }
    };

    let primary_json = match primary_json_result {
        Ok(j) => j,
        Err(msg) => {
            tracing::warn!("capture.publish: {} — no-op (D6)", msg);
            return vec![];
        }
    };

    // ── Commit phase (mutable) ────────────────────────────────────────────────
    state.capture_draft.pending_publish_correlation_id = Some(correlation_id.clone());
    // Record `now` for the clock-driven timeout fallback (D8).
    state.capture_draft.publish_phase = CaptureDraftPhase::Publishing { started_at: now };
    state.capture_draft.pending_highlight_json = deferred_highlight_json;
    if let Some(gid) = repost_group_id {
        state.capture_draft.pending_group_repost_group_id = Some(gid);
        state.capture_draft.pending_group_repost_author_pubkey_hex = active_pubkey;
    }

    vec![Effect::PublishCaptureWithCorrelation {
        json: primary_json,
        correlation_id,
    }]
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

/// Build a kind:11 artifact share event JSON template for the pending-book
/// path. Complete mirror of `artifacts::build_share_event` (artifacts.rs:839):
/// h, d, title, source, reference (with secondary podcast feed `i`), r, author,
/// image, summary, and the podcast-specific tags (podcast_guid,
/// podcast_show_title, audio, audio_preview, transcript, feed, published_at,
/// duration). Published via `Effect::PublishCaptureWithCorrelation` as the
/// pending-book primary before the deferred kind:9802 highlight.
fn build_artifact_share_event_json(
    preview: &crate::kernel::models::ArtifactPreview,
    group_id: &str,
) -> Result<String, &'static str> {
    let mut tags: Vec<serde_json::Value> = vec![
        serde_json::json!(["h", group_id]),
        serde_json::json!(["d", &preview.id]),
        serde_json::json!(["title", &preview.title]),
        serde_json::json!(["source", &preview.source]),
    ];

    // Reference tag — mirrors build_share_event reference_tag_name branch.
    let ref_name = preview.reference_tag_name.trim();
    let ref_value = preview.reference_tag_value.trim();
    if !ref_name.is_empty() && !ref_value.is_empty() {
        if ref_name == "i" && !preview.url.is_empty() {
            tags.push(serde_json::json!([ref_name, ref_value, &preview.url]));
        } else {
            tags.push(serde_json::json!([ref_name, ref_value]));
        }
        if !preview.reference_kind.is_empty() {
            tags.push(serde_json::json!(["k", &preview.reference_kind]));
        }
        // Secondary podcast feed i-tag: for podcast episodes, also emit the
        // feed-level NIP-73 identifier (mirrors build_share_event:876).
        if ref_name == "i" {
            let ref_is_item = ref_value.starts_with("podcast:item:guid:");
            let has_feed_guid = !preview.podcast_guid.is_empty();
            let feed_catalog = format!("podcast:guid:{}", preview.podcast_guid);
            let ref_is_feed = ref_value == feed_catalog;
            if ref_is_item && has_feed_guid && !ref_is_feed {
                tags.push(serde_json::json!(["i", &feed_catalog]));
            }
        }
    }

    if !preview.url.is_empty() {
        tags.push(serde_json::json!(["r", &preview.url]));
    }
    if !preview.author.is_empty() {
        tags.push(serde_json::json!(["author", &preview.author]));
    }
    if !preview.image.is_empty() {
        tags.push(serde_json::json!(["image", &preview.image]));
    }
    if !preview.description.is_empty() {
        tags.push(serde_json::json!(["summary", &preview.description]));
    }
    if !preview.podcast_guid.is_empty() {
        tags.push(serde_json::json!(["podcast_guid", &preview.podcast_guid]));
    }
    if !preview.podcast_show_title.is_empty() {
        tags.push(serde_json::json!([
            "podcast_show_title",
            &preview.podcast_show_title
        ]));
    }
    if !preview.audio_url.is_empty() {
        tags.push(serde_json::json!(["audio", &preview.audio_url]));
    }
    if !preview.audio_preview_url.is_empty() {
        tags.push(serde_json::json!([
            "audio_preview",
            &preview.audio_preview_url
        ]));
    }
    if !preview.transcript_url.is_empty() {
        tags.push(serde_json::json!(["transcript", &preview.transcript_url]));
    }
    if !preview.feed_url.is_empty() {
        tags.push(serde_json::json!(["feed", &preview.feed_url]));
    }
    if !preview.published_at.is_empty() {
        tags.push(serde_json::json!(["published_at", &preview.published_at]));
    }
    if let Some(d) = preview.duration_seconds {
        if d >= 0 {
            tags.push(serde_json::json!(["duration", d.to_string()]));
        }
    }

    let event_json = serde_json::json!({
        "kind": 11,
        "content": "",
        "tags": tags,
    });
    serde_json::to_string(&event_json).map_err(|_| "serde_json failed (11 artifact share)")
}

/// Build a kind:16 generic-repost event JSON for sharing a highlight into a
/// NIP-29 group. Mirrors `highlights::build_repost_event` (highlights.rs:1674)
/// but omits the `e` tag: the highlight event_id is not returned through the
/// nmp action_results mechanism (known architectural limit — nmp rev d16aea60;
/// tracked at https://github.com/pablof7z/nostr-multi-platform/issues/1702).
/// Carries h, k (9802), p (author_pubkey_hex) tags. Fire-and-forget via
/// `Effect::PublishCaptureEvent`.
fn build_group_repost_event_json(
    group_id: &str,
    author_pubkey_hex: &str,
) -> Result<String, &'static str> {
    if group_id.is_empty() {
        return Err("group repost requires non-empty group_id");
    }
    let event_json = serde_json::json!({
        "kind": 16,
        "content": "",
        "tags": [
            ["k", "9802"],
            ["p", author_pubkey_hex],
            ["h", group_id],
        ],
    });
    serde_json::to_string(&event_json).map_err(|_| "serde_json failed (16 group repost)")
}

/// `hl.capture.reset` — reset all draft state to defaults (phase back to Idle).
pub(crate) fn reduce_action_reset(state: &mut AppState) -> Vec<Effect> {
    state.capture_draft = CaptureDraftState::default();
    vec![]
}

/// `hl.capture.set_artifact_record` — the highlight/picture publish references an
/// EXISTING (already-published kind:11) artifact. Clears any pending preview so the
/// two artifact paths can't both be set. Device-local scratch until publish.
pub(crate) fn reduce_action_set_artifact_record(
    state: &mut AppState,
    artifact: crate::kernel::models::ArtifactRecord,
) -> Vec<Effect> {
    state.capture_draft.artifact_record = Some(artifact);
    state.capture_draft.artifact_preview = None;
    vec![]
}

/// `hl.capture.set_artifact_preview` — the publish references a PENDING book
/// (the kind:11 artifact is published first on this path). Clears any existing record.
pub(crate) fn reduce_action_set_artifact_preview(
    state: &mut AppState,
    preview: crate::kernel::models::ArtifactPreview,
) -> Vec<Effect> {
    state.capture_draft.artifact_preview = Some(preview);
    state.capture_draft.artifact_record = None;
    vec![]
}

/// `hl.capture.clear_artifact` — drop any selected book (a standalone capture with
/// no artifact: quote-only kind:9802 highlight, or a kind:20 picture).
pub(crate) fn reduce_action_clear_artifact(state: &mut AppState) -> Vec<Effect> {
    state.capture_draft.artifact_record = None;
    state.capture_draft.artifact_preview = None;
    vec![]
}

// ─── uniffi serialize helpers (Swift book selection → set-artifact actions) ───
// The Swift book-picker holds a typed `ArtifactRecord`/`ArtifactPreview` (uniffi
// structs, which aren't Codable), so the kernel does the serde to build the
// `artifact_json`/`preview_json` the set-artifact actions carry — symmetric with
// the actor-side parse.

/// Serialize an `ArtifactRecord` for `hl.capture.set_artifact_record`.
#[uniffi::export]
pub fn capture_artifact_record_json(artifact: crate::kernel::models::ArtifactRecord) -> String {
    serde_json::to_string(&artifact).unwrap_or_default()
}

/// Serialize an `ArtifactPreview` for `hl.capture.set_artifact_preview`.
#[uniffi::export]
pub fn capture_artifact_preview_json(preview: crate::kernel::models::ArtifactPreview) -> String {
    serde_json::to_string(&preview).unwrap_or_default()
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
    now: u64,
) -> Vec<Effect> {
    state.capture_draft.pending_publish_correlation_id = None;

    if !success {
        // The artifact OR the highlight publish failed. Drop any deferred
        // highlight so a failed artifact never lets the highlight (and FSM) lie
        // (Issue 3), and clear any scheduled group repost.
        state.capture_draft.pending_highlight_json = None;
        state.capture_draft.pending_group_repost_group_id = None;
        state.capture_draft.pending_group_repost_author_pubkey_hex = None;
        state.capture_draft.publish_phase = CaptureDraftPhase::Error { message: error };
        return vec![];
    }

    // Success. Multi-step publish ordering:
    //
    // Step 1 (pending-book, Issue 3) — the artifact just published. Now publish
    // the deferred highlight under a fresh correlation_id. Reset `started_at`
    // to `now` so the clock-driven timeout runs from the highlight dispatch,
    // not from when the artifact publish started (prevents premature timeout
    // when the artifact took most of the budget).
    if let Some(highlight_json) = state.capture_draft.pending_highlight_json.take() {
        let cid = new_correlation_id();
        state.capture_draft.pending_publish_correlation_id = Some(cid.clone());
        state.capture_draft.publish_phase = CaptureDraftPhase::Publishing { started_at: now };
        return vec![Effect::PublishCaptureWithCorrelation {
            json: highlight_json,
            correlation_id: cid,
        }];
    }

    // Step 2 (group share, Issue 1) — the highlight (or single-step event) just
    // published. If it was group-targeted, emit the kind:16 generic repost into
    // the NIP-29 group (fire-and-forget) and settle the FSM to Done.
    if let Some(group_id) = state.capture_draft.pending_group_repost_group_id.take() {
        let author = state
            .capture_draft
            .pending_group_repost_author_pubkey_hex
            .take()
            .unwrap_or_default();
        state.capture_draft.publish_phase = CaptureDraftPhase::Done;
        return match build_group_repost_event_json(&group_id, &author) {
            Ok(json) => vec![Effect::PublishCaptureEvent { json }],
            Err(msg) => {
                tracing::warn!(
                    "capture.publish: group repost build failed: {} — skipping repost",
                    msg
                );
                vec![]
            }
        };
    }

    // Terminal: nothing deferred → Done.
    state.capture_draft.publish_phase = CaptureDraftPhase::Done;
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

        // Photo-always invariant: every publish requires a completed upload.
        state.capture_draft.has_upload = true;
        state.capture_draft.blossom_upload = Some(fixture_blossom());

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
        // Photo-always invariant: can_publish requires a completed upload, so set
        // one to keep the can_publish projection assertion below meaningful.
        state.capture_draft.has_upload = true;
        state.capture_draft.blossom_upload = Some(fixture_blossom());

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
        // Photo-always invariant: every publish requires a completed upload.
        state.capture_draft.has_upload = true;
        state.capture_draft.blossom_upload = Some(fixture_blossom());
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
        // Photo-always invariant: every publish requires a completed upload.
        state.capture_draft.has_upload = true;
        state.capture_draft.blossom_upload = Some(fixture_blossom());
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
        // Photo-always invariant: every publish requires a completed upload.
        state.capture_draft.has_upload = true;
        state.capture_draft.blossom_upload = Some(fixture_blossom());
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

    // 7-Cap-Setter-T: hl.capture.set_artifact_record / set_artifact_preview
    // populate the draft (mutually exclusive); clear_artifact drops both. This is
    // the wiring that lets the Swift book-picker reach the full-parity publish
    // (publish-with-artifact itself is covered by the PP-T parity tests).
    #[test]
    fn capture_set_artifact_actions_populate_draft_mutually_exclusive() {
        let mut state = make_state();
        let clock = ManualClock::default();

        let rec_json = serde_json::to_string(&fixture_artifact()).unwrap();
        step(
            &mut state,
            &clock,
            envelope(
                "hl.capture.set_artifact_record",
                &serde_json::json!({ "artifact_json": rec_json }).to_string(),
            ),
        );
        assert!(state.capture_draft.artifact_record.is_some());
        assert!(state.capture_draft.artifact_preview.is_none());

        // Mutual exclusion: setting a pending preview clears the record.
        let prev_json = serde_json::to_string(&fixture_podcast_preview()).unwrap();
        step(
            &mut state,
            &clock,
            envelope(
                "hl.capture.set_artifact_preview",
                &serde_json::json!({ "preview_json": prev_json }).to_string(),
            ),
        );
        assert!(state.capture_draft.artifact_preview.is_some());
        assert!(state.capture_draft.artifact_record.is_none());

        step(
            &mut state,
            &clock,
            envelope("hl.capture.clear_artifact", "{}"),
        );
        assert!(state.capture_draft.artifact_record.is_none());
        assert!(state.capture_draft.artifact_preview.is_none());
    }

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

    // ── Bespoke parity helpers ───────────────────────────────────────────────
    //
    // These call the now-`pub(crate)` bespoke builders in the UNTOUCHED live-lane
    // files (highlights.rs / pictures.rs / artifacts.rs), sign with throwaway
    // keys, and compare the resulting tags byte-for-byte against the kernel's
    // emitted event-template JSON. This is REAL parity — not a re-implementation
    // of the tag logic.

    /// Sign a bespoke `EventBuilder` with throwaway keys and return the Event so
    /// tests can read its canonical `content` + `tags`. Mirrors the `tag_pairs`
    /// inspection helper in pictures.rs tests.
    fn bespoke_event(builder: nostr_sdk::EventBuilder) -> nostr_sdk::Event {
        let keys = nostr_sdk::Keys::generate();
        builder
            .sign_with_keys(&keys)
            .expect("sign bespoke event for inspection")
    }

    /// A bespoke event's tags as `Vec<Vec<String>>`.
    fn bespoke_tags(event: &nostr_sdk::Event) -> Vec<Vec<String>> {
        event.tags.iter().map(|t| t.as_slice().to_vec()).collect()
    }

    /// Parse a kernel event-template JSON string's `tags` into `Vec<Vec<String>>`.
    fn kernel_tags(json: &str) -> Vec<Vec<String>> {
        let v: serde_json::Value = serde_json::from_str(json).expect("valid kernel json");
        v["tags"]
            .as_array()
            .expect("tags array")
            .iter()
            .map(|t| {
                t.as_array()
                    .expect("tag is array")
                    .iter()
                    .map(|p| p.as_str().expect("tag part is string").to_string())
                    .collect()
            })
            .collect()
    }

    /// Sort tags for order-independent set equality.
    fn sorted(mut tags: Vec<Vec<String>>) -> Vec<Vec<String>> {
        tags.sort();
        tags
    }

    /// Fixture: a podcast episode `ArtifactPreview` with ALL optional fields
    /// populated. Used by PP-T4 to exercise every optional tag in
    /// `build_artifact_share_event_json` / `build_share_event`:
    /// image, summary, podcast_guid, podcast_show_title, audio, audio_preview,
    /// transcript, feed, published_at, duration, and the secondary podcast i-tag
    /// (emitted because `reference_tag_value` starts with "podcast:item:guid:"
    /// and `podcast_guid` is non-empty).
    fn fixture_podcast_preview() -> crate::kernel::models::ArtifactPreview {
        crate::kernel::models::ArtifactPreview {
            id: "podcast-preview-ep1".into(),
            url: "https://example.com/episodes/ep1".into(),
            title: "Episode 1: Systems Thinking".into(),
            author: "Jane Host".into(),
            image: "https://cdn.example/ep1-art.jpg".into(),
            description: "A deep dive into systems thinking.".into(),
            source: "podcast-episode".into(),
            domain: "example.com".into(),
            catalog_id: "podcast:item:guid:abc-123-guid".into(),
            catalog_kind: "podcast:item:guid".into(),
            podcast_guid: "show-guid-xyz".into(),
            podcast_item_guid: "abc-123-guid".into(),
            podcast_show_title: "The Tech Show".into(),
            audio_url: "https://cdn.example/ep1.mp3".into(),
            audio_preview_url: "https://cdn.example/ep1-preview.mp3".into(),
            transcript_url: "https://cdn.example/ep1-transcript.vtt".into(),
            feed_url: "https://feeds.example/tech-show.rss".into(),
            published_at: "2024-01-15T10:00:00Z".into(),
            duration_seconds: Some(3600),
            reference_tag_name: "i".into(),
            reference_tag_value: "podcast:item:guid:abc-123-guid".into(),
            reference_kind: "podcast:item:guid".into(),
            highlight_tag_name: "i".into(),
            highlight_tag_value: "podcast:item:guid:abc-123-guid".into(),
            highlight_reference_key: "i:podcast:item:guid:abc-123-guid".into(),
            chapters: Vec::new(),
        }
    }

    /// Build expected kind:16 repost tags from the bespoke
    /// `highlights::build_repost_event`, then drop the `e` tag (omitted in the
    /// kernel path — nmp rev d16aea60 does not surface the published event_id
    /// through `ActionResultRow`, so the kernel cannot include the `e` reference).
    /// Returns sorted tags for order-independent comparison.
    fn expected_repost_tags_minus_e(group_id: &str, author_pubkey_hex: &str) -> Vec<Vec<String>> {
        // Sign a dummy event to obtain a valid EventId for the helper call.
        // The id value is irrelevant; we immediately drop the `e` tag before
        // comparing with the kernel output.
        let dummy = bespoke_event(nostr_sdk::EventBuilder::new(
            nostr_sdk::Kind::TextNote,
            "dummy",
        ));
        let repost = bespoke_event(
            crate::highlights::build_repost_event(dummy.id, author_pubkey_hex, group_id, "")
                .expect("build_repost_event for test fixture"),
        );
        let mut tags = bespoke_tags(&repost);
        // Drop the `e` tag — the kernel omits it (see build_group_repost_event_json
        // and the nmp architectural limit comment there).
        tags.retain(|t| t.first().map(|s| s.as_str()) != Some("e"));
        sorted(tags)
    }

    /// PP-T1 — Path 1: highlight + EXISTING artifact → kind:9802. Real parity:
    /// the expected event is built by the bespoke `highlights::build_highlight_event`,
    /// signed, and its tags compared against the kernel JSON. Also asserts the
    /// kind:16 group repost is emitted once the publish settles.
    #[test]
    fn parity_highlight_with_artifact_kind9802() {
        let artifact = fixture_artifact();
        let blossom = fixture_blossom();

        // Bespoke expected event — same inputs the kernel will see.
        let bespoke_draft = crate::models::HighlightDraft {
            quote: "A profound insight from the book.".into(),
            context: "The chapter about design patterns.".into(),
            note: "This changed my mind.".into(),
            clip_start_seconds: None,
            clip_end_seconds: None,
            clip_speaker: String::new(),
            clip_transcript_segment_ids: Vec::new(),
            image: Some(blossom.clone()),
        };
        let expected = bespoke_event(
            crate::highlights::build_highlight_event(&bespoke_draft, &artifact)
                .expect("bespoke build_highlight_event"),
        );
        let expected_tags = sorted(bespoke_tags(&expected));

        // Drive the kernel path.
        let mut state = make_state();
        // A real session pubkey so the kind:16 repost carries a valid `p` tag.
        let author_keys = nostr_sdk::Keys::generate();
        let author_pubkey_hex = author_keys.public_key().to_hex();
        state.session = crate::kernel::app::SessionState::Present {
            pubkey: author_pubkey_hex.clone(),
            signer_kind: crate::kernel::action::SignerKind::LocalNsec,
        };
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

        let Effect::PublishCaptureWithCorrelation { json, .. } = effects
            .iter()
            .find(|e| matches!(e, Effect::PublishCaptureWithCorrelation { .. }))
            .expect("must emit PublishCaptureWithCorrelation")
        else {
            unreachable!()
        };
        let v: serde_json::Value = serde_json::from_str(json).expect("valid json");
        assert_eq!(
            v["kind"], 9802,
            "highlight+artifact path must emit kind:9802"
        );
        assert_eq!(v["content"], expected.content, "content must equal bespoke");
        assert_eq!(
            sorted(kernel_tags(json)),
            expected_tags,
            "kernel highlight tags must match bespoke build_highlight_event"
        );

        // The kind:16 group repost is emitted AFTER the highlight publish settles
        // (the highlight event_id is not returned through nmp action_results, so
        // the repost omits the `e` tag — see build_group_repost_event_json).
        let result_effects = step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::CapturePublishActionResult {
                success: true,
                error: String::new(),
            }),
        );
        let Effect::PublishCaptureEvent { json: repost_json } = result_effects
            .iter()
            .find(|e| matches!(e, Effect::PublishCaptureEvent { .. }))
            .expect("must emit kind:16 group repost on success")
        else {
            unreachable!()
        };
        let rv: serde_json::Value = serde_json::from_str(repost_json).expect("valid repost json");
        assert_eq!(rv["kind"], 16, "group repost must be kind:16");
        // Field-for-field tag comparison against the bespoke builder (minus the `e` tag
        // which is omitted because nmp rev d16aea60 does not return the published event_id).
        let actual_repost_tags = sorted(kernel_tags(repost_json));
        assert!(
            !actual_repost_tags
                .iter()
                .any(|t| t.first().map(|s| s.as_str()) == Some("e")),
            "kind:16 repost must NOT carry an e-tag \
             (nmp rev d16aea60 does not return event_id through ActionResultRow); \
             tags: {actual_repost_tags:?}"
        );
        assert_eq!(
            actual_repost_tags,
            expected_repost_tags_minus_e("group-a", &author_pubkey_hex),
            "kind:16 repost tags (h/k/p) must match bespoke build_repost_event minus e-tag"
        );
        assert_eq!(
            state.capture_draft.publish_phase,
            CaptureDraftPhase::Done,
            "FSM must settle to Done after the repost is scheduled"
        );
    }

    /// PP-T2 — Path 2: no quote → kind:20 NIP-68 picture. Real parity against the
    /// bespoke `pictures::build_picture_event`.
    #[test]
    fn parity_picture_no_quote_kind20() {
        let blossom = fixture_blossom();

        // Bespoke expected event (no artifact reference on this path).
        let expected = bespoke_event(
            crate::pictures::build_picture_event(
                Some("group-a"),
                &blossom,
                None,
                "Page capture note.".trim(),
            )
            .expect("bespoke build_picture_event"),
        );
        let expected_tags = sorted(bespoke_tags(&expected));

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

        let Effect::PublishCaptureWithCorrelation { json, .. } = effects
            .iter()
            .find(|e| matches!(e, Effect::PublishCaptureWithCorrelation { .. }))
            .expect("must emit PublishCaptureWithCorrelation")
        else {
            unreachable!()
        };
        let v: serde_json::Value = serde_json::from_str(json).expect("valid json");
        assert_eq!(v["kind"], 20, "picture path must emit kind:20, not kind:11");
        assert_eq!(
            v["content"], expected.content,
            "content must equal the note"
        );
        assert_eq!(
            sorted(kernel_tags(json)),
            expected_tags,
            "kernel picture tags must match bespoke build_picture_event"
        );

        // Picture path is single-step (no kind:16 repost — h carried inline).
        assert!(
            !effects
                .iter()
                .any(|e| matches!(e, Effect::PublishCaptureEvent { .. })),
            "kind:20 picture path must NOT emit a kind:16 repost"
        );
    }

    /// PP-T3 — Path 3: pending-book (artifact_preview + quote, no artifact_record).
    /// Real parity: artifact (kind:11) publishes FIRST as the correlated primary;
    /// the kind:9802 highlight is deferred until the artifact succeeds (Issue 3).
    #[test]
    fn parity_pending_book_artifact_published_then_referenced() {
        let artifact = fixture_artifact();
        let preview = artifact.preview.clone();
        let blossom = fixture_blossom();

        // Bespoke expected artifact-share event (kind:11).
        let expected_artifact = bespoke_event(
            crate::artifacts::build_share_event("group-a", &preview, None)
                .expect("bespoke build_share_event"),
        );
        let expected_artifact_tags = sorted(bespoke_tags(&expected_artifact));

        // Bespoke expected highlight event (kind:9802) referencing the same
        // artifact record the kernel synthesises from the preview.
        let artifact_from_preview = crate::artifacts::unpublished_record(preview.clone());
        let bespoke_draft = crate::models::HighlightDraft {
            quote: "Key insight from unpublished book.".into(),
            context: "The introduction chapter.".into(),
            note: "Must revisit.".into(),
            clip_start_seconds: None,
            clip_end_seconds: None,
            clip_speaker: String::new(),
            clip_transcript_segment_ids: Vec::new(),
            image: Some(blossom.clone()),
        };
        let expected_highlight = bespoke_event(
            crate::highlights::build_highlight_event(&bespoke_draft, &artifact_from_preview)
                .expect("bespoke build_highlight_event"),
        );
        let expected_highlight_tags = sorted(bespoke_tags(&expected_highlight));

        // Drive the kernel pending-book path.
        let mut state = make_state();
        let clock = ManualClock::default();
        // A real session pubkey so the kind:16 repost carries a valid `p` tag.
        let author_keys = nostr_sdk::Keys::generate();
        let author_pubkey_hex = author_keys.public_key().to_hex();
        state.session = crate::kernel::app::SessionState::Present {
            pubkey: author_pubkey_hex.clone(),
            signer_kind: crate::kernel::action::SignerKind::LocalNsec,
        };
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
        state.capture_draft.artifact_preview = Some(preview.clone());
        state.capture_draft.artifact_record = None;

        // Step 1 — publish emits exactly ONE correlated artifact (kind:11).
        let effects = step(&mut state, &clock, envelope("hl.capture.publish", "{}"));
        let correlated: Vec<_> = effects
            .iter()
            .filter(|e| matches!(e, Effect::PublishCaptureWithCorrelation { .. }))
            .collect();
        assert_eq!(
            correlated.len(),
            1,
            "pending-book publish must emit exactly one correlated artifact effect; got: {effects:?}"
        );
        let Effect::PublishCaptureWithCorrelation {
            json: artifact_json,
            ..
        } = correlated[0]
        else {
            unreachable!()
        };
        let av: serde_json::Value =
            serde_json::from_str(artifact_json).expect("valid artifact json");
        assert_eq!(av["kind"], 11, "artifact primary must be kind:11");
        assert_eq!(
            sorted(kernel_tags(artifact_json)),
            expected_artifact_tags,
            "kernel artifact tags must match bespoke build_share_event"
        );
        assert!(
            matches!(
                state.capture_draft.publish_phase,
                CaptureDraftPhase::Publishing { .. }
            ),
            "phase must be Publishing after the artifact publish"
        );

        // Step 2 — artifact succeeds → exactly ONE correlated highlight (kind:9802).
        let result_effects = step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::CapturePublishActionResult {
                success: true,
                error: String::new(),
            }),
        );
        let correlated2: Vec<_> = result_effects
            .iter()
            .filter(|e| matches!(e, Effect::PublishCaptureWithCorrelation { .. }))
            .collect();
        assert_eq!(
            correlated2.len(),
            1,
            "artifact success must emit exactly one correlated highlight; got: {result_effects:?}"
        );
        let Effect::PublishCaptureWithCorrelation {
            json: highlight_json,
            ..
        } = correlated2[0]
        else {
            unreachable!()
        };
        let hv: serde_json::Value =
            serde_json::from_str(highlight_json).expect("valid highlight json");
        assert_eq!(hv["kind"], 9802, "deferred highlight must be kind:9802");
        assert_eq!(hv["content"], expected_highlight.content);
        assert_eq!(
            sorted(kernel_tags(highlight_json)),
            expected_highlight_tags,
            "kernel highlight tags must match bespoke build_highlight_event"
        );
        // Still Publishing — the highlight publish is now in flight.
        assert!(
            matches!(
                state.capture_draft.publish_phase,
                CaptureDraftPhase::Publishing { .. }
            ),
            "phase must remain Publishing until the highlight settles"
        );

        // Step 3 — highlight succeeds → kind:16 group repost + Done.
        let final_effects = step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::CapturePublishActionResult {
                success: true,
                error: String::new(),
            }),
        );
        let Effect::PublishCaptureEvent {
            json: final_repost_json,
        } = final_effects
            .iter()
            .find(|e| matches!(e, Effect::PublishCaptureEvent { .. }))
            .expect("highlight success must emit the kind:16 group repost")
        else {
            unreachable!()
        };
        let final_actual_tags = sorted(kernel_tags(final_repost_json));
        // e-tag must be absent (same nmp d16aea60 limit as in PP-T1).
        assert!(
            !final_actual_tags
                .iter()
                .any(|t| t.first().map(|s| s.as_str()) == Some("e")),
            "kind:16 group repost must NOT carry an e-tag (nmp rev d16aea60 limit); \
             tags: {final_actual_tags:?}"
        );
        assert_eq!(
            final_actual_tags,
            expected_repost_tags_minus_e("group-a", &author_pubkey_hex),
            "kind:16 repost tags (h/k/p) must match bespoke build_repost_event minus e-tag"
        );
        assert_eq!(state.capture_draft.publish_phase, CaptureDraftPhase::Done);
    }

    /// PP-T4 — Podcast artifact: all optional tags present. Exercises every
    /// optional field in `build_artifact_share_event_json` / `build_share_event`:
    /// image, summary, podcast_guid, podcast_show_title, audio, audio_preview,
    /// transcript, feed, published_at, duration, AND the secondary podcast i-tag
    /// (emitted when the primary reference is a podcast:item:guid and the show-level
    /// podcast:guid is also present). Uses `fixture_podcast_preview()` which
    /// populates all of these. If any optional tag is dropped from the kernel builder,
    /// this test fails.
    #[test]
    fn parity_podcast_artifact_all_optional_tags() {
        let preview = fixture_podcast_preview();
        let blossom = fixture_blossom();

        // Bespoke expected artifact-share event (kind:11) with ALL optional tags.
        let expected_artifact = bespoke_event(
            crate::artifacts::build_share_event("group-a", &preview, None)
                .expect("bespoke build_share_event"),
        );
        let expected_artifact_tags = sorted(bespoke_tags(&expected_artifact));

        // Drive the kernel pending-book path with the podcast preview.
        let mut state = make_state();
        let clock = ManualClock::default();
        state.communities = vec![community("group-a", "Group A")];
        state.capture_draft.publish_phase = CaptureDraftPhase::Reviewing;
        state.capture_draft.quote = "Key insight from the podcast episode.".into();
        state.capture_draft.target_group_id = Some("group-a".into());
        state.capture_draft.has_upload = true;
        state.capture_draft.blossom_upload = Some(blossom.clone());
        state.capture_draft.blossom_image_url = blossom.url.clone();
        // Pending podcast episode: preview present, no artifact_record.
        state.capture_draft.artifact_preview = Some(preview.clone());
        state.capture_draft.artifact_record = None;

        // Publish → kernel emits the artifact (kind:11) as the correlated primary.
        let effects = step(&mut state, &clock, envelope("hl.capture.publish", "{}"));
        let correlated: Vec<_> = effects
            .iter()
            .filter(|e| matches!(e, Effect::PublishCaptureWithCorrelation { .. }))
            .collect();
        assert_eq!(
            correlated.len(),
            1,
            "pending podcast publish must emit exactly one correlated artifact; got: {effects:?}"
        );
        let Effect::PublishCaptureWithCorrelation {
            json: artifact_json,
            ..
        } = correlated[0]
        else {
            unreachable!()
        };

        let av: serde_json::Value =
            serde_json::from_str(artifact_json).expect("valid artifact json");
        assert_eq!(av["kind"], 11, "podcast artifact must be kind:11");
        // Full tag-set comparison including all optional tags and secondary
        // podcast i-tag. This assertion fails if any optional tag is dropped.
        assert_eq!(
            sorted(kernel_tags(artifact_json)),
            expected_artifact_tags,
            "podcast artifact tags must match bespoke build_share_event \
             (including image/summary/podcast_guid/show_title/audio/audio_preview/\
             transcript/feed/published_at/duration + secondary podcast i-tag)"
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
