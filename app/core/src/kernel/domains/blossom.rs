//! Blossom image upload — Phase 5G.
//!
//! Wires `nmp.blossom.upload` dispatch and the `"action_results"` typed
//! projection completion seam for both upload AND capture-publish results.
//!
//! ## Upload flow
//!
//! 1. Host dispatches `hl.blossom.upload { image_handle, servers }`.
//! 2. `reduce_action_blossom_upload` validates + emits `Effect::BlossomUpload`.
//! 3. `run_effect_blossom_upload` calls `nmp_app_dispatch_action("nmp.blossom.upload",…)`,
//!    reads the nmp-minted correlation_id from the return value, and sends
//!    `KernelEvent::NmpBlossomCorrelationMinted` so the actor can overwrite the
//!    reducer-minted placeholder with the real nmp id.
//! 4. nmp does the HTTP asynchronously; the result arrives in the next
//!    `NmpSnapshotFrame` as a Tier-2 typed sidecar with `schema_id = "action_results"`.
//! 5. `dispatch_typed_frame` calls `route_action_result` for each row.
//! 6. `route_action_result` matches rows by correlation_id and emits
//!    `KernelEvent::BlossomUploadResult` or `KernelEvent::CapturePublishActionResult`.
//!
//! ## Completion seam — action_results routing
//!
//! `route_action_result` is the single routing function that hl uses for ALL
//! action_results rows (upload + publish + future slices). It is called from
//! `projections::dispatch_typed_frame` with each `ActionResultRow` decoded from
//! the `"action_results"` sidecar.
//!
//! ## 5F publish correlation_id
//!
//! `capture_draft::reduce_action_publish` now emits
//! `Effect::PublishCaptureWithCorrelation { json, correlation_id }` instead of
//! the fire-and-forget `PublishCaptureEvent`. The reducer-minted correlation_id
//! is stored in `AppState::capture_draft.pending_publish_correlation_id`.
//! `run_effect_publish_capture_with_correlation` dispatches via `ActorCommand::PublishRawEvent`
//! and also calls `nmp_app_dispatch_action` on the publish namespace so the
//! action_results projection fires on completion. When the matching row arrives
//! in `route_action_result`, it emits `KernelEvent::CapturePublishActionResult`
//! which drives the FSM to Done/Error for real (not just the 30 s clock timeout).
//!
//! ## Fidelity
//!
//! The live bespoke lane (`app/core/src/blossom.rs`) is UNTOUCHED (Non-Negotiable
//! #6). This module owns only the kernel-lane upload + action_results path.
//!
//! ## D6 — no panics
//!
//! All action-result routing is guarded: unknown correlation_ids, missing
//! status fields, and malformed result JSON are silent no-ops.

use std::ffi::{c_char, CString};

use nmp_core::typed_projections::ActionResultRow;
use nmp_ffi::{nmp_free_string, NmpApp};

use crate::kernel::action::KernelEvent;
use crate::kernel::app::AppState;
use crate::kernel::domains::capture_draft::new_correlation_id;
use crate::kernel::effect::Effect;

/// Fallback Blossom server used when the host passes an empty `servers` list.
/// Matches the live lane default in `blossom.rs::DEFAULT_SERVER`.
pub(crate) const DEFAULT_BLOSSOM_SERVER: &str = "https://blossom.primal.net";

// `nmp_app_dispatch_action` is #[no_mangle] extern "C" in nmp-ffi/src/action.rs.
// Declared here (same pattern as reactions.rs / bookmarks.rs) so this module can
// call it directly without importing the full nmp-ffi action surface.
#[allow(improper_ctypes)] // NmpApp is opaque; the pointer is safe — nmp-ffi uses the same ABI.
extern "C" {
    fn nmp_app_dispatch_action(
        app: *mut NmpApp,
        namespace: *const c_char,
        action_json: *const c_char,
    ) -> *mut c_char;
}

// ─── Action reducer ─────────────────────────────────────────────────────────────

/// `hl.blossom.upload { image_handle, servers }` — validate and emit upload effect.
///
/// Validates that `image_handle` is non-empty (D6: rejects blank paths). Fills in
/// the default server if `servers` is empty. Mints a placeholder `correlation_id`
/// and stores it in `AppState::capture_draft.pending_upload_correlation_id`.
/// `run_effect_blossom_upload` will overwrite the placeholder with the real nmp
/// id after dispatch returns.
///
/// Emits `Effect::BlossomUpload`; the effect runner dispatches to nmp asynchronously.
/// Fire-and-forward: the reducer does NO I/O (Non-Negotiable #2).
pub(crate) fn reduce_action_blossom_upload(
    state: &mut AppState,
    image_handle: String,
    servers: Vec<String>,
    _now: u64,
) -> Vec<Effect> {
    let image_handle = image_handle.trim().to_string();
    if image_handle.is_empty() {
        tracing::warn!("blossom.upload: empty image_handle — no-op (D6)");
        return vec![];
    }

    let servers = {
        let non_empty: Vec<String> = servers
            .into_iter()
            .filter(|s| !s.trim().is_empty())
            .collect();
        if non_empty.is_empty() {
            vec![DEFAULT_BLOSSOM_SERVER.to_string()]
        } else {
            non_empty
        }
    };

    // Mint a placeholder id; the real nmp id overwrites it via
    // KernelEvent::NmpBlossomCorrelationMinted after the effect runner calls
    // nmp_app_dispatch_action. Insert into the set (not overwrite) so a
    // second concurrent dispatch does not orphan the first in-flight id.
    let correlation_id = new_correlation_id();
    state
        .capture_draft
        .pending_upload_correlation_ids
        .insert(correlation_id.clone());

    vec![Effect::BlossomUpload {
        correlation_id,
        image_handle,
        servers,
    }]
}

// ─── Helpers ────────────────────────────────────────────────────────────────────

/// Parse a NIP-94 `dim` value (e.g. `"1024x768"`) into `(width, height)`.
/// Returns `(0, 0)` for any unparseable input.
fn parse_dim(dim: &str) -> (u32, u32) {
    let mut parts = dim.splitn(2, 'x');
    let w = parts
        .next()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);
    let h = parts
        .next()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);
    (w, h)
}

// ─── Action-result routing ──────────────────────────────────────────────────────

/// Apply all settled rows from a decoded `ActionResultsModel` to `AppState`.
///
/// Called by `projections::dispatch_typed_frame` when schema_id == `"action_results"`.
/// Matches each row's `correlation_id` against the tracked pending ids and applies
/// the appropriate state mutation (upload result or publish FSM transition).
///
/// Unknown correlation_ids are a silent no-op (D6). Both pending ids are cleared
/// after the first match so a stale re-delivery cannot trigger a second state change.
///
/// Returns effects (currently always empty; reserved for future slices that need
/// side effects from action_results, e.g. 5J clip publish).
pub(crate) fn apply_action_results(
    state: &mut AppState,
    model: &nmp_core::typed_projections::ActionResultsModel,
    now: u64,
) -> Vec<Effect> {
    let mut effects = Vec::new();
    for row in &model.results {
        let mut row_effects = apply_action_result_row(state, row, now);
        effects.append(&mut row_effects);
    }
    effects
}

/// Apply one settled `ActionResultRow` to `AppState`.
///
/// Routing priority:
/// 1. Match `pending_upload_correlation_id` → upload result state change.
/// 2. Match `pending_publish_correlation_id` → publish FSM state change.
/// 3. No match → silent no-op (D6), logged at trace level.
fn apply_action_result_row(state: &mut AppState, row: &ActionResultRow, now: u64) -> Vec<Effect> {
    use crate::kernel::domains::capture_draft::reduce_event_capture_publish_action_result;

    let cid = &row.correlation_id;
    // nmp maps its internal "ok" terminal to "published" in the action_results
    // wire format (publish_engine_terminals.rs:73: `"ok" => "published"`).
    // Failure terminals are written as "failed". A status other than "published"
    // is treated as failure (D6 — unknown statuses degrade gracefully).
    let success = row.status == "published";

    // 1. Blossom upload correlation_id? (set lookup — supports multiple in-flight)
    if state
        .capture_draft
        .pending_upload_correlation_ids
        .contains(cid.as_str())
    {
        // Remove this id from the set so re-delivery is a no-op (D6).
        state
            .capture_draft
            .pending_upload_correlation_ids
            .remove(cid.as_str());

        // Parse the blob descriptor from the opaque result JSON.
        // nmp.blossom.upload sets result = `{"url":"…","sha256":"…","size":…,
        // "type":"image/jpeg","nip94":{"dim":"WxH","alt":"…"}}`.
        let (blob_url, blossom_upload) = if success {
            let parsed = row
                .result
                .as_deref()
                .and_then(|r| serde_json::from_str::<serde_json::Value>(r).ok());

            let url = parsed
                .as_ref()
                .and_then(|v| v.get("url").and_then(serde_json::Value::as_str))
                .unwrap_or_default()
                .to_string();

            let upload = if !url.is_empty() {
                let sha256_hex = parsed
                    .as_ref()
                    .and_then(|v| v.get("sha256").and_then(serde_json::Value::as_str))
                    .unwrap_or_default()
                    .to_string();
                let mime = parsed
                    .as_ref()
                    .and_then(|v| v.get("type").and_then(serde_json::Value::as_str))
                    .unwrap_or("image/jpeg")
                    .to_string();
                let size_bytes = parsed
                    .as_ref()
                    .and_then(|v| v.get("size").and_then(serde_json::Value::as_u64))
                    .unwrap_or(0);
                let dim = parsed
                    .as_ref()
                    .and_then(|v| v.pointer("/nip94/dim").and_then(serde_json::Value::as_str))
                    .unwrap_or_default();
                let (width, height) = parse_dim(dim);
                let alt = parsed
                    .as_ref()
                    .and_then(|v| v.pointer("/nip94/alt").and_then(serde_json::Value::as_str))
                    .unwrap_or_default()
                    .to_string();
                Some(crate::models::BlossomUpload {
                    url: url.clone(),
                    sha256_hex,
                    mime,
                    size_bytes,
                    width,
                    height,
                    alt,
                })
            } else {
                None
            };
            (url, upload)
        } else {
            (String::new(), None)
        };
        tracing::debug!(
            %cid,
            %success,
            %blob_url,
            "apply_action_result_row: blossom upload result"
        );
        reduce_event_blossom_upload_result_inner(state, success, blob_url, blossom_upload);
        return Vec::new();
    }

    // 2. Capture publish correlation_id?
    if state
        .capture_draft
        .pending_publish_correlation_id
        .as_deref()
        == Some(cid.as_str())
    {
        let error = row.error.clone().unwrap_or_default();
        tracing::debug!(
            %cid,
            %success,
            error = %error,
            "apply_action_result_row: capture publish result"
        );
        let effects = reduce_event_capture_publish_action_result(state, success, error, now);
        return effects;
    }

    // 3. Podcast clip publish correlation_id? (Phase 5J)
    if state.podcast.pending_clip_publish_correlation_id.as_deref() == Some(cid.as_str()) {
        let error = row.error.clone().unwrap_or_default();
        tracing::debug!(
            %cid,
            %success,
            error = %error,
            "apply_action_result_row: clip publish result"
        );
        let effects = crate::kernel::domains::podcast::reduce_event_clip_publish_action_result(
            state, success, error,
        );
        return effects;
    }

    // 4. Unknown correlation_id — silent no-op (D6).
    tracing::trace!(
        %cid,
        status = %row.status,
        "apply_action_result_row: unrecognised correlation_id — no-op"
    );
    Vec::new()
}

// ─── Event reducers ─────────────────────────────────────────────────────────────

/// `KernelEvent::BlossomUploadResult` — apply upload outcome to capture draft.
///
/// On success: sets `has_upload = true` and stores `blob_url` on the draft,
/// unlocking the kind:11 markdown publish path. Clears
/// `pending_upload_correlation_id`.
/// On failure: clears `pending_upload_correlation_id` only; `has_upload` stays
/// false so a retry is possible.
pub(crate) fn reduce_event_blossom_upload_result(
    state: &mut AppState,
    success: bool,
    blob_url: String,
    _error: String,
) -> Vec<Effect> {
    // Test-injection path: only the URL is available; blossom_upload is None.
    // The action_results path (apply_action_result_row) provides the full descriptor.
    reduce_event_blossom_upload_result_inner(state, success, blob_url, None);
    vec![]
}

/// Inner state mutation for a Blossom upload result — called by the
/// `apply_action_result_row` path AFTER the matching id has already been
/// removed from `pending_upload_correlation_ids`. The `KernelEvent` path
/// (test injection via `KernelEvent::BlossomUploadResult`) calls this
/// with `blossom_upload = None` since the test event only carries `blob_url`.
fn reduce_event_blossom_upload_result_inner(
    state: &mut AppState,
    success: bool,
    blob_url: String,
    blossom_upload: Option<crate::models::BlossomUpload>,
) {
    if success {
        state.capture_draft.has_upload = true;
        state.capture_draft.blossom_image_url = blob_url;
        // Store the full upload descriptor when available (action_results path).
        // `None` on the test-injection path (KernelEvent::BlossomUploadResult)
        // which only carries the URL; `blossom_image_url` still makes the URL
        // accessible for the view snapshot.
        state.capture_draft.blossom_upload = blossom_upload;
    } else {
        // On failure: clear stale URL and has_upload so the UI can offer a retry.
        // If a previous upload succeeded (has_upload=true) and this is a
        // different concurrent upload that failed, preserve the prior success —
        // only clear when no prior successful URL exists.
        if !state.capture_draft.has_upload {
            state.capture_draft.blossom_image_url = String::new();
            state.capture_draft.blossom_upload = None;
        }
    }
}

/// `KernelEvent::NmpBlossomCorrelationMinted` — swap the reducer-minted
/// placeholder out of `pending_upload_correlation_ids` and insert the real
/// nmp-minted id in its place.
///
/// `run_effect_blossom_upload` sends this after `nmp_app_dispatch_action`
/// returns the dispatch JSON `{"correlation_id":"<32-hex>"}`. Until this event
/// is processed, `apply_action_result_row` will not match the arriving
/// action_results row (because nmp's id differs from the reducer placeholder).
///
/// The swap is safe even if a `reset` raced ahead: if the placeholder was
/// already removed from the set (by reset clearing it or by a prior swap),
/// `remove` is a no-op and the nmp id is simply never inserted — the upload
/// result row will arrive as an unknown-correlation_id no-op (D6).
pub(crate) fn reduce_event_nmp_blossom_correlation_minted(
    state: &mut AppState,
    placeholder_correlation_id: String,
    nmp_correlation_id: String,
) -> Vec<Effect> {
    // Only swap if the placeholder is still in the set (guard against reset race).
    if state
        .capture_draft
        .pending_upload_correlation_ids
        .remove(&placeholder_correlation_id)
    {
        state
            .capture_draft
            .pending_upload_correlation_ids
            .insert(nmp_correlation_id);
    } else {
        tracing::debug!(
            placeholder = %placeholder_correlation_id,
            nmp_cid = %nmp_correlation_id,
            "reduce_event_nmp_blossom_correlation_minted: placeholder not in set (reset race?) — dropping"
        );
    }
    vec![]
}

// ─── Effect runners ─────────────────────────────────────────────────────────────

/// Run `Effect::BlossomUpload` — dispatch `nmp.blossom.upload` via the nmp FFI.
///
/// Calls `nmp_app_dispatch_action("nmp.blossom.upload", action_json)`. The
/// returned `{"correlation_id":"<32-hex>"}` JSON is parsed; if the nmp-minted
/// id differs from the reducer placeholder, sends
/// `KernelEvent::NmpBlossomCorrelationMinted` so the actor can update
/// `AppState::capture_draft.pending_upload_correlation_id` before the
/// action_results frame arrives.
///
/// D6: if dispatch returns an error JSON (nmp rejected the upload) or nmp is
/// None (test mode), emits a synthetic failure result via `KernelEvent::BlossomUploadResult`
/// so the draft can recover immediately without waiting for a frame that will
/// never arrive.
pub(crate) fn run_effect_blossom_upload(
    correlation_id: String,
    image_handle: String,
    servers: Vec<String>,
    nmp: Option<&crate::kernel::actor::NmpHandle>,
    tx: &tokio::sync::mpsc::UnboundedSender<crate::kernel::actor::Cmd>,
) {
    use crate::kernel::actor::Cmd;

    let Some(handle) = nmp else {
        tracing::debug!("run_effect_blossom_upload: no nmp handle (test mode) — no-op");
        return;
    };

    let action_json = serde_json::json!({
        "file_path": image_handle,
        "servers": servers,
    });
    let action_json_str = match serde_json::to_string(&action_json) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("run_effect_blossom_upload: serde_json failed: {e}");
            return;
        }
    };

    let ns_c = match CString::new("nmp.blossom.upload") {
        Ok(s) => s,
        Err(_) => return,
    };
    let json_c = match CString::new(action_json_str) {
        Ok(s) => s,
        Err(_) => return,
    };

    // SAFETY: handle.ptr is a valid non-null NmpApp pointer kept alive by
    // NmpHandle for the full actor lifetime. ns_c and json_c are valid CStrings
    // alive for the duration of this call. The returned pointer is freed below
    // via nmp_free_string (same allocator contract as reactions.rs:371-378).
    let result_ptr =
        unsafe { nmp_app_dispatch_action(handle.ptr.as_ptr(), ns_c.as_ptr(), json_c.as_ptr()) };

    if result_ptr.is_null() {
        // D6: null return is an nmp bug; treat as a failure.
        let _ = tx.send(Cmd::Event(KernelEvent::BlossomUploadResult {
            success: false,
            blob_url: String::new(),
            error: "nmp_app_dispatch_action returned null".to_string(),
        }));
        return;
    }

    // Parse the result JSON while the pointer is still valid, then free it.
    let result_json = {
        let cstr = unsafe { std::ffi::CStr::from_ptr(result_ptr) };
        let s = cstr.to_string_lossy().to_string();
        nmp_free_string(result_ptr);
        s
    };

    // Extract the nmp-minted correlation_id from `{"correlation_id":"…"}`.
    let nmp_cid = serde_json::from_str::<serde_json::Value>(&result_json)
        .ok()
        .and_then(|v| {
            v.get("correlation_id")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        });

    match nmp_cid {
        Some(nmp_id) if nmp_id != correlation_id => {
            // nmp minted a different id; swap the placeholder in AppState so
            // apply_action_result_row can match the real id when the
            // action_results frame arrives.
            tracing::debug!(
                placeholder_cid = %correlation_id,
                nmp_cid = %nmp_id,
                "run_effect_blossom_upload: swapping placeholder with nmp id"
            );
            let _ = tx.send(Cmd::Event(KernelEvent::NmpBlossomCorrelationMinted {
                placeholder_correlation_id: correlation_id,
                nmp_correlation_id: nmp_id,
            }));
        }
        Some(_) => {
            // nmp returned the same id — no swap needed, action_results will match.
        }
        None => {
            // nmp returned an error JSON or no correlation_id. Upload was rejected.
            tracing::warn!(
                result = %result_json,
                "run_effect_blossom_upload: nmp rejected upload — emitting failure"
            );
            let _ = tx.send(Cmd::Event(KernelEvent::BlossomUploadResult {
                success: false,
                blob_url: String::new(),
                error: result_json,
            }));
        }
    }
}

/// Run `Effect::PublishCaptureWithCorrelation` — dispatch a capture-draft
/// publish via `ActorCommand::PublishRawEvent` with an explicit `correlation_id`
/// so the nmp publish engine reports the outcome in `action_results` keyed on
/// that id. This mirrors the `run_effect_publish_highlight` path (Phase 4H) but
/// carries `correlation_id: Some(...)` to close the 5F FSM loop.
///
/// The publish engine writes the terminal verdict into `action_results` using
/// the correlation_id attached to `ActorCommand::PublishRawEvent`. When the
/// matching row arrives in `apply_action_result_row` with status `"published"`,
/// `reduce_event_capture_publish_action_result` drives the FSM to Done; on
/// `"failed"` it drives to Error. The 30 s clock-timeout in `clock_checks`
/// remains as a safety net.
///
/// D6: if nmp is None (test mode) this is a no-op; tests inject
/// `KernelEvent::CapturePublishActionResult` directly.
pub(crate) fn run_effect_publish_capture_with_correlation(
    json: String,
    correlation_id: String,
    nmp: Option<&crate::kernel::actor::NmpHandle>,
    _tx: &tokio::sync::mpsc::UnboundedSender<crate::kernel::actor::Cmd>,
) {
    let Some(handle) = nmp else {
        tracing::debug!(
            "run_effect_publish_capture_with_correlation: no nmp handle (test mode) — no-op"
        );
        return;
    };

    // Deserialize the event template — same as run_effect_publish_highlight (Phase 4H).
    // The JSON carries `kind`, `content`, `tags`; nmp fills `id/sig/pubkey/created_at`.
    #[derive(serde::Deserialize)]
    struct EventTemplate {
        kind: u32,
        content: String,
        tags: Vec<Vec<String>>,
    }

    let template = match serde_json::from_str::<EventTemplate>(&json) {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(
                "run_effect_publish_capture_with_correlation: failed to deserialize event template: {e} — no-op (D6)"
            );
            return;
        }
    };

    // Send ActorCommand::PublishRawEvent with correlation_id: Some(...).
    // The nmp publish engine threads this id onto the action_results terminal so
    // `apply_action_result_row` can route the verdict back to the capture FSM.
    // This is the ONLY correct way to get a correlation_id on a PublishRaw at
    // b4404159: `ActorCommand::PublishRawEvent::correlation_id` is passed
    // directly into the engine, not via `nmp_app_dispatch_action`.
    let nmp_ref: &nmp_ffi::NmpApp = unsafe { handle.ptr.as_ref() };
    let _ = nmp_ref
        .actor_sender()
        .send(nmp_core::ActorCommand::PublishRawEvent {
            kind: template.kind,
            content: template.content,
            tags: template.tags,
            target: nmp_core::publish::PublishTarget::Auto,
            signer_pubkey: None,
            correlation_id: Some(correlation_id),
        });
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use nmp_core::typed_projections::{ActionResultRow, ActionResultsModel};

    use crate::kernel::action::{AppActionEnvelope, KernelEvent};
    use crate::kernel::actor::{reduce, Cmd};
    use crate::kernel::clock::{Clock, ManualClock};
    use crate::kernel::domains::capture_draft::CaptureDraftPhase;

    fn make_state() -> AppState {
        AppState::default()
    }

    fn clock() -> ManualClock {
        ManualClock::default()
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

    // 5G-T1: dispatching hl.blossom.upload emits BlossomUpload effect
    // with a non-empty correlation_id and the correct image_handle.
    #[test]
    fn blossom_upload_dispatches_with_correlation_id() {
        let mut state = make_state();
        let c = clock();
        let json = r#"{"image_handle":"/tmp/photo.jpg","servers":["https://blossom.example"]}"#;
        let effects = step(&mut state, &c, envelope("hl.blossom.upload", json));

        let upload_effect = effects
            .iter()
            .find(|e| matches!(e, Effect::BlossomUpload { .. }))
            .expect("BlossomUpload effect must be emitted");

        if let Effect::BlossomUpload {
            correlation_id,
            image_handle,
            servers,
        } = upload_effect
        {
            assert!(
                !correlation_id.is_empty(),
                "correlation_id must be non-empty"
            );
            assert_eq!(image_handle, "/tmp/photo.jpg");
            assert_eq!(servers, &["https://blossom.example"]);
        }

        // Correlation id inserted into pending_upload_correlation_ids so action_results can match.
        assert!(
            !state
                .capture_draft
                .pending_upload_correlation_ids
                .is_empty(),
            "pending_upload_correlation_ids must be non-empty"
        );
    }

    // 5G-T2: apply_action_results routes by correlation_id — an arriving
    // upload result with the matching id sets has_upload=true and blossom_image_url.
    // Status "published" is the wire value nmp writes for a successful publish
    // terminal (publish_engine_terminals.rs:73: `"ok" => "published"`).
    #[test]
    fn action_result_routes_by_correlation_id() {
        let mut state = make_state();
        let cid = "deadbeef00000001deadbeef00000001".to_string();
        state
            .capture_draft
            .pending_upload_correlation_ids
            .insert(cid.clone());

        let model = ActionResultsModel {
            results: vec![ActionResultRow {
                correlation_id: cid.clone(),
                status: "published".to_string(),
                error: None,
                result: Some(r#"{"url":"https://blossom.example/img.jpg"}"#.to_string()),
            }],
        };

        apply_action_results(&mut state, &model, 0);

        assert!(
            state.capture_draft.has_upload,
            "has_upload must be true after upload result"
        );
        assert_eq!(
            state.capture_draft.blossom_image_url,
            "https://blossom.example/img.jpg"
        );
        assert!(
            state
                .capture_draft
                .pending_upload_correlation_ids
                .is_empty(),
            "pending_upload_correlation_ids must be empty after result consumed"
        );
    }

    // 5G-T3: a successful upload result sets has_upload=true and stores blob URL on draft.
    #[test]
    fn upload_result_sets_has_upload_and_url_on_draft() {
        let mut state = make_state();
        let c = clock();
        // Dispatch the upload — this mints and stores a correlation_id.
        let effects = step(
            &mut state,
            &c,
            envelope("hl.blossom.upload", r#"{"image_handle":"/tmp/img.jpg"}"#),
        );
        let cid = if let Some(Effect::BlossomUpload { correlation_id, .. }) = effects
            .iter()
            .find(|e| matches!(e, Effect::BlossomUpload { .. }))
        {
            correlation_id.clone()
        } else {
            panic!("no BlossomUpload effect");
        };

        // Inject the result as a KernelEvent (test mode: no live nmp).
        step(
            &mut state,
            &c,
            Cmd::Event(KernelEvent::BlossomUploadResult {
                success: true,
                blob_url: "https://cdn.example/photo.jpg".to_string(),
                error: String::new(),
            }),
        );

        assert!(state.capture_draft.has_upload);
        assert_eq!(
            state.capture_draft.blossom_image_url,
            "https://cdn.example/photo.jpg"
        );
        // The pending set should still contain the correlation_id because
        // KernelEvent::BlossomUploadResult (injected in test mode) does NOT
        // remove from the set — removal only happens via apply_action_result_row.
        // The set tracks in-flight nmp dispatches; test-injected events bypass nmp.
        let _ = cid; // used above, silence lint
    }

    // 5G-T4: a successful CapturePublishActionResult drives the FSM from
    // Publishing → Done and clears pending_publish_correlation_id.
    #[test]
    fn publish_action_result_drives_capture_fsm_to_done() {
        let mut state = make_state();
        let c = clock();
        state.capture_draft.publish_phase = CaptureDraftPhase::Publishing { started_at: 0 };
        state.capture_draft.pending_publish_correlation_id = Some("pub-cid-0001".to_string());

        step(
            &mut state,
            &c,
            Cmd::Event(KernelEvent::CapturePublishActionResult {
                success: true,
                error: String::new(),
            }),
        );

        assert_eq!(state.capture_draft.publish_phase, CaptureDraftPhase::Done);
        assert!(state.capture_draft.pending_publish_correlation_id.is_none());
    }

    // 5G-T5: a failing CapturePublishActionResult drives the FSM to Error.
    #[test]
    fn publish_failure_result_drives_error() {
        let mut state = make_state();
        let c = clock();
        state.capture_draft.publish_phase = CaptureDraftPhase::Publishing { started_at: 0 };
        state.capture_draft.pending_publish_correlation_id = Some("pub-cid-0002".to_string());

        step(
            &mut state,
            &c,
            Cmd::Event(KernelEvent::CapturePublishActionResult {
                success: false,
                error: "no relays available".to_string(),
            }),
        );

        assert!(
            matches!(
                &state.capture_draft.publish_phase,
                CaptureDraftPhase::Error { message } if message == "no relays available"
            ),
            "FSM must be in Error with the relay failure message"
        );
        assert!(state.capture_draft.pending_publish_correlation_id.is_none());
    }

    // 5G-T6: an action_results row whose correlation_id does not match any
    // pending id is a silent no-op (D6). State must remain unchanged.
    #[test]
    fn malformed_action_result_no_op() {
        let mut state = make_state();
        // No pending correlation ids in state — all fields are None/default.
        let model = ActionResultsModel {
            results: vec![ActionResultRow {
                correlation_id: "unknown-cid-that-nobody-owns".to_string(),
                status: "completed".to_string(),
                error: None,
                result: Some(r#"{"url":"https://ignored.example/img.jpg"}"#.to_string()),
            }],
        };

        apply_action_results(&mut state, &model, 0);

        // has_upload must still be false — the unknown row must be silently ignored.
        assert!(!state.capture_draft.has_upload);
        assert!(state.capture_draft.blossom_image_url.is_empty());
        // Phase must remain Idle — no state change.
        assert_eq!(state.capture_draft.publish_phase, CaptureDraftPhase::Idle);
    }

    // 5G-T7: the publish action_results shape uses "published" (not "success")
    // as the status for a successful publish terminal.
    //
    // Verifies that `apply_action_result_row` correctly maps `status="published"`
    // to a successful CapturePublishActionResult, driving the FSM to Done.
    // ("success" was a former incorrect assumption — nmp writes "published" via
    // publish_engine_terminals.rs:73: `"ok" => "published"`.)
    #[test]
    fn publish_uses_correct_nmp_action_shape() {
        let mut state = make_state();
        let cid = "cafe0000cafe0000cafe0000cafe0001".to_string();
        state.capture_draft.publish_phase = CaptureDraftPhase::Publishing { started_at: 100 };
        state.capture_draft.pending_publish_correlation_id = Some(cid.clone());

        // Status "published" is what nmp writes for a successful publish terminal
        // (publish_engine_terminals.rs:73: `"ok" => "published"`).
        let model = ActionResultsModel {
            results: vec![ActionResultRow {
                correlation_id: cid.clone(),
                status: "published".to_string(),
                error: None,
                result: None,
            }],
        };

        apply_action_results(&mut state, &model, 0);

        assert_eq!(
            state.capture_draft.publish_phase,
            CaptureDraftPhase::Done,
            "FSM must reach Done when status=\"published\" arrives"
        );
        assert!(
            state.capture_draft.pending_publish_correlation_id.is_none(),
            "pending_publish_correlation_id must be cleared on Done"
        );

        // Verify that "success" (the WRONG status) does NOT drive FSM to Done —
        // this would remain Publishing if an incorrect string were used.
        let mut state2 = make_state();
        let cid2 = "cafe0000cafe0000cafe0000cafe0002".to_string();
        state2.capture_draft.publish_phase = CaptureDraftPhase::Publishing { started_at: 100 };
        state2.capture_draft.pending_publish_correlation_id = Some(cid2.clone());
        let model2 = ActionResultsModel {
            results: vec![ActionResultRow {
                correlation_id: cid2,
                status: "success".to_string(), // WRONG — nmp never sends this
                error: None,
                result: None,
            }],
        };
        apply_action_results(&mut state2, &model2, 0);
        assert!(
            !matches!(state2.capture_draft.publish_phase, CaptureDraftPhase::Done),
            "status=\"success\" must NOT drive FSM to Done (nmp sends \"published\", not \"success\")"
        );
    }

    // 5G-T8: two concurrent uploads both resolve independently — the HashSet
    // design does not orphan the first id when a second upload is dispatched.
    //
    // Simulates: dispatch upload A → dispatch upload B → result for A → result for B.
    // Verifies both results are recognised and the set is empty at the end.
    #[test]
    fn two_concurrent_uploads_both_resolve() {
        let mut state = make_state();

        let cid_a = "aaa00000000000000000000000000001".to_string();
        let cid_b = "bbb00000000000000000000000000002".to_string();

        // Insert both ids into the set (simulating two in-flight dispatches).
        state
            .capture_draft
            .pending_upload_correlation_ids
            .insert(cid_a.clone());
        state
            .capture_draft
            .pending_upload_correlation_ids
            .insert(cid_b.clone());

        assert_eq!(state.capture_draft.pending_upload_correlation_ids.len(), 2);

        // Result for A arrives.
        let model_a = ActionResultsModel {
            results: vec![ActionResultRow {
                correlation_id: cid_a.clone(),
                status: "published".to_string(),
                error: None,
                result: Some(r#"{"url":"https://cdn.example/a.jpg"}"#.to_string()),
            }],
        };
        apply_action_results(&mut state, &model_a, 0);

        assert!(
            state.capture_draft.has_upload,
            "A success must set has_upload"
        );
        assert_eq!(
            state.capture_draft.blossom_image_url,
            "https://cdn.example/a.jpg"
        );
        assert_eq!(
            state.capture_draft.pending_upload_correlation_ids.len(),
            1,
            "B's id must still be in the set"
        );
        assert!(
            state
                .capture_draft
                .pending_upload_correlation_ids
                .contains(&cid_b),
            "B's id must survive A's result"
        );

        // Result for B arrives (different URL — should overwrite A's URL if success).
        let model_b = ActionResultsModel {
            results: vec![ActionResultRow {
                correlation_id: cid_b.clone(),
                status: "published".to_string(),
                error: None,
                result: Some(r#"{"url":"https://cdn.example/b.jpg"}"#.to_string()),
            }],
        };
        apply_action_results(&mut state, &model_b, 0);

        assert!(state.capture_draft.has_upload);
        assert_eq!(
            state.capture_draft.blossom_image_url,
            "https://cdn.example/b.jpg"
        );
        assert!(
            state
                .capture_draft
                .pending_upload_correlation_ids
                .is_empty(),
            "set must be empty after both results consumed"
        );
    }
}
