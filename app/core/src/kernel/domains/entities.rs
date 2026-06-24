//! Entity-ref domain — `refs.event` NRRD projection + `ViewId::EntityRef` snapshot.
//!
//! Receives `RefRowDeltaBatch` payloads from the `"refs.event"` typed sidecar
//! (ADR-0063 Lane H) and stores `ClaimedEventRow` objects in `AppState::claimed_events`.
//! The `project_entity_ref_snapshot` function assembles a `KernelEntitySnapshot`
//! from the cached row for an open `ViewId::EntityRef { key }` view.

use std::ffi::CString;
use std::os::raw::c_int;

use nmp_ffi::{nmp_app_release_ref, nmp_app_resolve_ref};

use crate::kernel::actor::NmpHandle;
use crate::kernel::app::AppState;
use crate::kernel::effect::Effect;
use crate::kernel::snapshot::{KernelEntitySnapshot, ViewSnapshot};

// NMP resolve_ref constants for event namespace (ADR-0063).
const EVENT_NAMESPACE: c_int = 1;
const EVENT_EMBED_SHAPE: c_int = 2;
const LIVENESS_LIVE: c_int = 1;
const CONSUMER_ID_PREFIX: &str = "hl.entity.";

/// Apply a `"refs.event"` NRRD batch to `AppState::claimed_events`.
///
/// Called from `projections::dispatch_typed_frame` when `schema_id == "refs.event"`.
/// D6: decode errors are silent no-ops.
pub(crate) fn apply_refs_event(state: &mut AppState, payload: &[u8]) {
    use nmp_core::refs::{decode_ref_row_delta_batch, RefRowState};
    use nmp_core::typed_projections::decode_claimed_events;
    let Ok(batch) = decode_ref_row_delta_batch(payload) else { return; };
    for row in &batch.rows {
        match row.state {
            RefRowState::Changed => {
                if let Ok(model) = decode_claimed_events(&row.payload) {
                    if let Some((_, event_row)) = model.entries.into_iter().next() {
                        state.claimed_events.insert(row.key.clone(), event_row);
                    }
                }
            }
            RefRowState::Cleared => {
                state.claimed_events.remove(&row.key);
            }
        }
    }
}

/// Assemble a `ViewSnapshot::EntityRef` for a `ViewId::EntityRef { key }` view.
///
/// Returns `None` if the event row has not arrived from NMP yet.
pub(crate) fn project_entity_ref_snapshot(state: &AppState, key: &str) -> Option<ViewSnapshot> {
    let row = state.claimed_events.get(key)?;
    let tags_json = serde_json::to_string(&row.tags).unwrap_or_default();
    Some(ViewSnapshot::EntityRef(KernelEntitySnapshot {
        key: key.to_string(),
        kind: row.kind,
        content: row.content.clone(),
        pubkey_hex: row.author_pubkey.clone(),
        tags_json,
        created_at: row.created_at,
    }))
}

/// Lifecycle effects when `ViewId::EntityRef { key }` is opened.
pub(crate) fn lifecycle_effects_for_view_open(id: &crate::kernel::view::ViewId) -> Vec<Effect> {
    if let crate::kernel::view::ViewId::EntityRef { key } = id {
        vec![Effect::ResolveEntityRef { key: key.clone() }]
    } else {
        Vec::new()
    }
}

/// Lifecycle effects when `ViewId::EntityRef { key }` is closed.
pub(crate) fn lifecycle_effects_for_view_close(id: &crate::kernel::view::ViewId) -> Vec<Effect> {
    if let crate::kernel::view::ViewId::EntityRef { key } = id {
        vec![Effect::ReleaseEntityRef { key: key.clone() }]
    } else {
        Vec::new()
    }
}

/// Run `Effect::ResolveEntityRef` — call `nmp_app_resolve_ref(namespace=1, ...)`.
pub(crate) fn run_effect_resolve_entity_ref(key: String, nmp: Option<&NmpHandle>) {
    let Some(handle) = nmp else { return };
    let Ok(key_c) = CString::new(key.as_str()) else { return; };
    let consumer_id = format!("{}{}", CONSUMER_ID_PREFIX, key);
    let Ok(consumer_c) = CString::new(consumer_id) else { return; };
    nmp_app_resolve_ref(
        handle.ptr.as_ptr(),
        EVENT_NAMESPACE,
        key_c.as_ptr(),
        consumer_c.as_ptr(),
        EVENT_EMBED_SHAPE,
        LIVENESS_LIVE,
    );
}

/// Run `Effect::ReleaseEntityRef` — call `nmp_app_release_ref(namespace=1, ...)`.
pub(crate) fn run_effect_release_entity_ref(key: String, nmp: Option<&NmpHandle>) {
    let Some(handle) = nmp else { return };
    let Ok(key_c) = CString::new(key.as_str()) else { return; };
    let consumer_id = format!("{}{}", CONSUMER_ID_PREFIX, key);
    let Ok(consumer_c) = CString::new(consumer_id) else { return; };
    nmp_app_release_ref(
        handle.ptr.as_ptr(),
        EVENT_NAMESPACE,
        key_c.as_ptr(),
        consumer_c.as_ptr(),
    );
}

// ─── Stateless uniffi helpers ────────────────────────────────────────────────

/// Tokenize Nostr event content and return a JSON-encoded `ContentTreeWire`.
///
/// Calls `nmp_content_tokenize_text` (stateless C-ABI, no NmpApp* needed).
/// Returns `{"ok":true,"tree":{...}}` on success or `{"ok":false,"error":"..."}` on failure.
/// `content` is the raw event content; mode=0 (plain text), kind=1 (note).
#[uniffi::export]
pub fn tokenize_nostr_content(content: String) -> String {
    use std::ffi::{CStr, CString};
    let Ok(content_c) = CString::new(content) else {
        return r#"{"ok":false,"error":"nul byte in content"}"#.to_string();
    };
    let tags_c = c"[]";
    unsafe {
        let raw = nmp_ffi::nmp_content_tokenize_text(
            content_c.as_ptr(),
            tags_c.as_ptr(),
            0,
            1,
        );
        if raw.is_null() {
            return r#"{"ok":false,"error":"null result"}"#.to_string();
        }
        let result = CStr::from_ptr(raw).to_string_lossy().into_owned();
        nmp_ffi::nmp_free_string(raw);
        result
    }
}
