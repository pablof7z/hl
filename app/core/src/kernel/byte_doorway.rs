//! Shared helper for NMP's typed **byte** write doorway (ADR-0064 / S4).
//!
//! Every hl write action used to cross the FFI through the JSON doorway
//! (`nmp_app_dispatch_action(app, namespace, action_json)`). That JSON twin is
//! retired here ahead of NMP's S9 Cut B: hl now builds a typed per-crate
//! [`ActionPayload`] (FlatBuffers), wraps it in a
//! [`nmp_core::dispatch_envelope::DispatchEnvelope`], and crosses the FFI
//! through [`nmp_app_dispatch_action_bytes`].
//!
//! The envelope carries a HOST-SUPPLIED `correlation_id` that the doorway
//! echoes back verbatim (ADR-0064 §4) — so a caller that needs to track the
//! operation passes its own id and reads it straight back, rather than parsing
//! a kernel-minted id out of the return JSON.
//!
//! # Returned JSON shape (unchanged from the JSON twin)
//!
//! * `{"correlation_id":"<id>"}` — accepted + enqueued (the id is the one the
//!   caller stamped into the envelope).
//! * `{"error":"<message>"}` — rejected (fail-closed): unknown namespace, a
//!   not-typed-capable module, a malformed/oversize/wrong-version envelope, …

use std::os::raw::c_char;

use nmp_core::dispatch_envelope::{encode_dispatch_envelope, DISPATCH_ENVELOPE_SCHEMA_VERSION};
use nmp_core::substrate::ActionPayload;
use nmp_ffi::{nmp_free_string, NmpApp};

// `nmp_app_dispatch_action_bytes` is #[no_mangle] extern "C" in
// nmp-ffi/src/action/bytes.rs. It carries the bytes of a finished
// `DispatchEnvelope` (`ptr`/`len`) and returns a heap-allocated NUL-terminated
// JSON C string the caller MUST release via `nmp_free_string`.
#[allow(improper_ctypes)]
extern "C" {
    fn nmp_app_dispatch_action_bytes(
        app: *mut NmpApp,
        ptr: *const u8,
        len: usize,
    ) -> *mut c_char;
}

/// Build a finished `DispatchEnvelope` carrying the typed FlatBuffers encoding
/// of `payload` under `namespace`, stamped with the host-supplied
/// `correlation_id` and the single recognised envelope schema version.
///
/// The typed payload encode (`ActionPayload::encode`) self-stamps the per-crate
/// payload `schema_version`; the envelope `schema_version` is the transport's
/// own (`DISPATCH_ENVELOPE_SCHEMA_VERSION`). The two are distinct gates.
#[must_use]
pub(crate) fn build_envelope<P: ActionPayload>(
    correlation_id: &str,
    namespace: &str,
    payload: &P,
) -> Vec<u8> {
    let payload_bytes = payload.encode();
    encode_dispatch_envelope(
        correlation_id,
        namespace,
        DISPATCH_ENVELOPE_SCHEMA_VERSION,
        &payload_bytes,
    )
}

/// Dispatch a finished envelope through the byte doorway and return the
/// doorway's JSON verdict string (`{"correlation_id":…}` / `{"error":…}`), or
/// `None` if the doorway returned a null pointer (D6 null-safety contract).
///
/// The returned C string is freed here through `nmp_free_string` (the same Rust
/// allocator that allocated it) — callers receive an owned `String`.
///
/// # Safety
/// `app` must be a valid non-null `NmpApp` pointer kept alive for the call.
pub(crate) fn dispatch_envelope_bytes(app: *mut NmpApp, envelope: &[u8]) -> Option<String> {
    // SAFETY: `app` is a valid non-null NmpApp pointer kept alive by the caller
    // for the duration of this call. `envelope` is a valid readable byte range
    // (a finished DispatchEnvelope); the doorway reads but never retains it.
    let result_ptr =
        unsafe { nmp_app_dispatch_action_bytes(app, envelope.as_ptr(), envelope.len()) };
    if result_ptr.is_null() {
        return None;
    }
    // SAFETY: the doorway returns a NUL-terminated CString::into_raw pointer.
    let s = unsafe { std::ffi::CStr::from_ptr(result_ptr) }
        .to_string_lossy()
        .to_string();
    // Free through the same Rust allocator the doorway allocated with.
    nmp_free_string(result_ptr);
    Some(s)
}

/// Fire-and-forget convenience: build the envelope for `payload` under
/// `namespace` (with `correlation_id`), dispatch it, and discard the verdict.
///
/// The returned JSON is freed and dropped — used by the write paths whose
/// authoritative result arrives back through a projection tick, not the
/// dispatch return value.
pub(crate) fn dispatch_action_bytes<P: ActionPayload>(
    app: *mut NmpApp,
    namespace: &str,
    correlation_id: &str,
    payload: &P,
) {
    let envelope = build_envelope(correlation_id, namespace, payload);
    let _ = dispatch_envelope_bytes(app, &envelope);
}

/// Build a finished `DispatchEnvelope` from pre-encoded typed payload bytes.
/// Useful when the payload was encoded in the reducer and carried as `Vec<u8>`
/// through the `Effect`.
#[must_use]
pub(crate) fn build_envelope_from_bytes(
    correlation_id: &str,
    namespace: &str,
    payload_bytes: &[u8],
) -> Vec<u8> {
    encode_dispatch_envelope(
        correlation_id,
        namespace,
        DISPATCH_ENVELOPE_SCHEMA_VERSION,
        payload_bytes,
    )
}

/// A fresh 32-hex correlation id for a fire-and-forget action that does not
/// track the operation. The byte doorway requires a non-empty correlation_id
/// (it rejects `MissingCorrelationId`); a random id satisfies that without the
/// caller having to thread one through.
#[must_use]
pub(crate) fn fresh_correlation_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}
