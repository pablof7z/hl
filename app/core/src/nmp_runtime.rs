//! Highlighter's NMP composition root.
//!
//! Native shells must not drive NMP directly. This module owns the one live
//! `nmp_ffi::NmpApp`, wires the canonical `nmp-defaults` composition before
//! start, and exposes the app-core operations Highlighter still needs while
//! feature projections are being rendered from the existing Rust read model.

use std::borrow::Cow;
use std::collections::{HashMap, VecDeque};
use std::ffi::{CStr, CString};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use nmp_core::planner::{
    InterestId, InterestLifecycle, InterestScope, InterestShape, LogicalInterest,
};
use nmp_core::publish::{PublishAction, PublishTarget};
use nmp_core::substrate::{
    ActionRegistrar, AppHost, SignedEvent as NmpSignedEvent, UnsignedEvent as NmpUnsignedEvent,
};
use nmp_core::typed_projections::{
    decode_action_results, decode_relay_diagnostics, ActionResultRow, ACTION_RESULTS_SCHEMA_ID,
    RELAY_DIAGNOSTICS_SCHEMA_ID,
};
use nmp_core::{
    decode_snapshot_typed_projections, ActorCommand, KindFilter, RawEventObserver,
    RawEventObserverId, SignContinuation, SignerSource, TypedProjectionData,
};
use nmp_defaults::{NmpAppBuilder, RunConfig};
use nmp_ffi::{
    nmp_app_add_relay, nmp_app_deliver_external_signer_response, nmp_app_dispatch_action,
    nmp_app_free, nmp_app_lifecycle_background, nmp_app_lifecycle_foreground,
    nmp_app_nostrconnect_uri, nmp_app_remove_relay, nmp_app_set_capability_callback,
    nmp_app_set_update_callback, nmp_app_signin_nip55, nmp_external_signer_init, nmp_free_string,
    nmp_signer_broker_init, NmpApp,
};
use nostr_sdk::prelude::*;
use nostrdb::Ndb;
use parking_lot::{Mutex, RwLock};
use tokio::sync::oneshot;
use zeroize::Zeroizing;

use crate::errors::CoreError;
use crate::events::{DataChangeType, Delta, EventCallback};
use crate::models::{Nip11Document, NostrConnectOptions, RelayDiagnostic, RelayStatus};
use crate::relays::{nostr_connect_relay, RelayConfig};

const NMP_PUBLISH_NAMESPACE: &str = "nmp.publish";
const NMP_SIGN_TIMEOUT: Duration = Duration::from_secs(65);
/// Interactive signer-pairing budget — shared by NIP-46 (bunker approval)
/// and NIP-55 (Amber approval dialog). Backend-neutral on purpose (V-78).
const NMP_SIGNER_PAIR_TIMEOUT: Duration = Duration::from_secs(300);
const NMP_PROTOCOL_ACTION_TIMEOUT: Duration = Duration::from_secs(360);
const NMP_ACTION_RESULT_CACHE_LIMIT: usize = 128;

type EventCallbackSlot = Arc<RwLock<Option<Arc<dyn EventCallback>>>>;

/// NIP-55 external-signer request channel capacity. Deep enough that the
/// Kotlin reader never drops a request under normal interaction latencies
/// (one request per user action), tight enough to backpressure a runaway
/// dispatch loop.
const NMP_SIGNER_REQUEST_CAPACITY: usize = 16;

/// Drain-tick budget for [`HighlighterNmpRuntime::next_signer_request`].
/// The Kotlin reader parks INSIDE the channel's `recv_timeout` (D8 — never a
/// sleep+check poll); the timeout exists only so the reader can observe its
/// shutdown flag with bounded latency. Matches the 250 ms tick used by
/// `nmp-android-ffi`'s `nativeNextSignerRequest`.
const NMP_SIGNER_DRAIN_TICK: Duration = Duration::from_millis(250);

/// Wire constant — matches `nmp_signer_iface::EXTERNAL_SIGNER_NAMESPACE`.
/// Used in the capability trampoline without importing `nmp-signer-iface`.
const EXTERNAL_SIGNER_NAMESPACE: &str = "external_signer";

/// Result of one [`HighlighterNmpRuntime::next_signer_request`] drain tick
/// (ADR-0048 Stage 2). Mirrors `nmp-android-ffi`'s `NextSignerRequest`:
/// `Idle` is a normal timeout tick (the reader parked in the channel wait),
/// `Closed` means the sender side is gone (session teardown) and the Kotlin
/// reader must stop.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum SignerRequestDrain {
    Request(String),
    Idle,
    Closed,
}

/// Process-wide registry of live [`ExternalSignerContext`]s, keyed by the
/// opaque handle id passed as the capability-callback `context` pointer.
///
/// The trampoline cannot receive a raw `Arc` pointer safely:
/// `nmp_app_set_capability_callback(None)` does NOT quiesce an in-flight
/// dispatch (the kernel's `dispatch_capability` clones the registration out
/// and drops the slot lock before invoking, and the capability worker thread
/// is detached — never joined by `nmp_app_free`). A raw pointer therefore has
/// a use-after-free window during teardown. With a handle id, an in-flight
/// dispatch either upgrades the registry entry to a live `Arc` (kept alive
/// for the duration of the call) or misses and degrades to an error envelope
/// (D6). Same design as `nmp-android-ffi`'s session registry.
fn external_signer_registry(
) -> &'static parking_lot::Mutex<HashMap<usize, Arc<ExternalSignerContext>>> {
    static REGISTRY: OnceLock<parking_lot::Mutex<HashMap<usize, Arc<ExternalSignerContext>>>> =
        OnceLock::new();
    REGISTRY.get_or_init(|| parking_lot::Mutex::new(HashMap::new()))
}

static NEXT_EXTERNAL_SIGNER_HANDLE: AtomicUsize = AtomicUsize::new(1);

/// Thin owner for the actor-backed NMP runtime.
pub(crate) struct HighlighterNmpRuntime {
    app: NonNull<NmpApp>,
    storage_dir: PathBuf,
    applied_relays: RwLock<HashMap<String, String>>,
    identity: Arc<NmpIdentityState>,
    diagnostics: Arc<NmpRelayDiagnosticsState>,
    action_results: Arc<NmpActionResultsState>,
    action_result_stop: Arc<AtomicBool>,
    action_result_wake: SyncSender<()>,
    action_result_worker: Option<JoinHandle<()>>,
    /// Keep-alive for the C update callback context pointer. The callback
    /// reads this Arc via raw pointer; Drop clears the registration before
    /// releasing it.
    _update_frames: Arc<UpdateFrameSidecar>,
    typed_projection_drain_lock: Arc<Mutex<()>>,
    raw_mirror_id: RawEventObserverId,
    raw_mirror_worker: Option<JoinHandle<()>>,
    /// Outbound channel for NIP-55 `ExternalSignerRequest` JSON payloads.
    ///
    /// The C capability trampoline (registered with the kernel's capability
    /// socket) pushes the `payload_json` of every `external_signer`
    /// `CapabilityRequest` here. The Kotlin side drains it via
    /// `next_signer_request` (blocking timed recv — D8, no polling) and
    /// routes each payload through `ExternalSignerCapabilityBridge.handleJson`.
    /// The `SyncSender` is owned by the [`ExternalSignerContext`] held in the
    /// process-wide registry (see [`external_signer_registry`]); this receiver
    /// is the Kotlin-drain end. When the registry entry is removed in `Drop`
    /// the sender drops and the reader observes `Closed`.
    signer_request_rx: Mutex<Receiver<String>>,
    /// Registry handle id for this runtime's [`ExternalSignerContext`]. The
    /// value (cast to `*mut c_void`) is the `context` passed to
    /// `nmp_app_set_capability_callback`; `Drop` removes the entry.
    external_signer_handle: usize,
}

// SAFETY: `NmpApp` is actor-backed and explicitly designed for cross-thread
// host calls. This wrapper only sends commands through the NMP public API and
// frees the pointer once in Drop.
unsafe impl Send for HighlighterNmpRuntime {}
unsafe impl Sync for HighlighterNmpRuntime {}

impl Drop for HighlighterNmpRuntime {
    fn drop(&mut self) {
        // Clear the update callback first: the setter's quiescence contract
        // guarantees no in-flight invocation after it returns, so the context
        // pointer (`update_frames`) can never be used past this point.
        nmp_app_set_update_callback(self.app.as_ptr(), std::ptr::null_mut(), None);
        // Unregister the capability callback (NIP-55 trampoline). NOTE:
        // unlike the update-callback gate above, this does NOT quiesce an
        // in-flight capability dispatch (`dispatch_capability` clones the
        // registration out before invoking, and the capability worker thread
        // is detached). UAF safety comes from the registry indirection
        // instead: the trampoline context is a handle id, not a pointer, and
        // an in-flight dispatch either holds its own `Arc` clone (valid for
        // the duration of the call) or misses the registry lookup and
        // degrades to an error envelope (D6).
        nmp_app_set_capability_callback(self.app.as_ptr(), std::ptr::null_mut(), None);
        // Remove the registry entry: drops the channel sender (the drain
        // reader observes `Closed`) once any in-flight dispatch's Arc clone
        // is released.
        external_signer_registry()
            .lock()
            .remove(&self.external_signer_handle);
        self.action_result_stop.store(true, Ordering::Release);
        let _ = self.action_result_wake.try_send(());
        if let Some(worker) = self.action_result_worker.take() {
            let _ = worker.join();
        }

        self.app_ref()
            .unregister_raw_event_observer(self.raw_mirror_id);
        if let Some(worker) = self.raw_mirror_worker.take() {
            let _ = worker.join();
        }
        nmp_app_free(self.app.as_ptr());
    }
}

/// Context for the NIP-55 capability trampoline.
///
/// The trampoline `extern "C"` fn cannot capture state, so it resolves this
/// struct through [`external_signer_registry`] keyed by the handle id passed
/// as the `context` value (never a raw pointer — see the registry doc for the
/// teardown-race rationale).
struct ExternalSignerContext {
    /// Bounded channel into which the trampoline pushes `ExternalSignerRequest`
    /// JSON payloads. The Kotlin side drains via `next_signer_request`.
    tx: SyncSender<String>,
}

/// Capability trampoline registered with the NMP kernel for NIP-55.
///
/// Runs on whichever Rust thread dispatches the capability (D6: never
/// panics, never returns NULL for non-NULL inputs; errors are data).
/// Non-`external_signer` namespaces return an error envelope — hl does
/// not use any other capability namespace today (no Android keyring yet).
///
/// `context` is a registry handle id (see [`external_signer_registry`]), NOT
/// a pointer — nothing is dereferenced, so a dispatch racing teardown
/// resolves to a missed lookup and an error envelope rather than a UAF.
extern "C" fn on_capability_request(
    context: *mut std::ffi::c_void,
    request_json: *const std::ffi::c_char,
) -> *mut std::ffi::c_char {
    use std::ffi::CString;

    if request_json.is_null() {
        return std::ptr::null_mut();
    }

    // Resolve the handle to a live context. The Arc clone taken here keeps
    // the channel sender alive for the duration of this call even if the
    // runtime's Drop removes the registry entry concurrently.
    let handle = context as usize;
    let ctx: Option<Arc<ExternalSignerContext>> =
        external_signer_registry().lock().get(&handle).cloned();

    // SAFETY: caller (nmp-ffi dispatcher) guarantees valid NUL-terminated
    // string for the duration of the call.
    let request = unsafe { std::ffi::CStr::from_ptr(request_json) }
        .to_string_lossy()
        .into_owned();

    let parsed: serde_json::Value = serde_json::from_str(&request).unwrap_or_default();
    let namespace = parsed
        .get("namespace")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let correlation_id = parsed
        .get("correlation_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if namespace != EXTERNAL_SIGNER_NAMESPACE {
        // hl has no other capability namespace today — report the gap as an
        // error envelope so Rust surfaces it as `signer_state: unavailable`
        // (D6: errors are data, not exceptions).
        let envelope = serde_json::json!({
            "namespace": namespace,
            "correlation_id": correlation_id,
            "result_json": r#"{"status":"error","reason":"no-capability-handler"}"#,
        });
        return CString::new(envelope.to_string())
            .unwrap_or_else(|_| c"{}".to_owned())
            .into_raw();
    }

    let Some(payload) = parsed.get("payload_json").and_then(|v| v.as_str()) else {
        let envelope = serde_json::json!({
            "namespace": EXTERNAL_SIGNER_NAMESPACE,
            "correlation_id": correlation_id,
            "result_json": r#"{"status":"error","reason":"missing-payload"}"#,
        });
        return CString::new(envelope.to_string())
            .unwrap_or_else(|_| c"{}".to_owned())
            .into_raw();
    };

    // A missed registry lookup means the runtime is (or has finished) tearing
    // down — report it as data (D6), exactly like a disconnected channel.
    let Some(ctx) = ctx else {
        let envelope = serde_json::json!({
            "namespace": EXTERNAL_SIGNER_NAMESPACE,
            "correlation_id": correlation_id,
            "result_json": r#"{"status":"error","reason":"session-closed"}"#,
        });
        return CString::new(envelope.to_string())
            .unwrap_or_else(|_| c"{}".to_owned())
            .into_raw();
    };

    // Push onto the outbound channel (best-effort; a full channel — more than
    // NMP_SIGNER_REQUEST_CAPACITY concurrent requests — is a backpressure
    // signal and degrades to timeout on the Rust side, D6).
    match ctx.tx.try_send(payload.to_string()) {
        Ok(()) | Err(TrySendError::Full(_)) => {}
        Err(TrySendError::Disconnected(_)) => {
            // Receiver dropped (runtime shutting down) — error envelope.
            let envelope = serde_json::json!({
                "namespace": EXTERNAL_SIGNER_NAMESPACE,
                "correlation_id": correlation_id,
                "result_json": r#"{"status":"error","reason":"session-closed"}"#,
            });
            return CString::new(envelope.to_string())
                .unwrap_or_else(|_| c"{}".to_owned())
                .into_raw();
        }
    }

    // Ack: the dispatch is queued; the actual IPC result comes later via
    // `deliver_external_signer_response` (D7: the host fires and reports).
    let ack = serde_json::json!({
        "namespace": EXTERNAL_SIGNER_NAMESPACE,
        "correlation_id": correlation_id,
        "result_json": r#"{"status":"dispatched"}"#,
    });
    CString::new(ack.to_string())
        .unwrap_or_else(|_| c"{}".to_owned())
        .into_raw()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NmpInterestHandle {
    id: u64,
}

struct RawMirrorMessage {
    source: String,
    json: String,
}

struct RawMirrorObserver {
    tx: SyncSender<RawMirrorMessage>,
}

struct NmpIdentityState {
    inner: Mutex<NmpIdentitySnapshot>,
}

#[derive(Default)]
struct NmpIdentitySnapshot {
    active: Option<String>,
    waiters: Vec<oneshot::Sender<String>>,
}

struct NmpRelayDiagnosticsState {
    relays: RwLock<HashMap<String, RelayDiagnostic>>,
    /// Per-relay NIP-11 documents, fetched and parsed entirely inside NMP
    /// (ADR-0051) and carried on the `relay_diagnostics` projection's `info`
    /// child. Keyed by the projection's relay URL. Highlighter never issues
    /// an HTTP request or parses JSON for these.
    infos: RwLock<HashMap<String, Nip11Document>>,
    callback_slot: RwLock<Option<EventCallbackSlot>>,
}

#[derive(Default)]
struct NmpActionResultsState {
    inner: Mutex<NmpActionResultsSnapshot>,
}

#[derive(Default)]
struct NmpActionResultsSnapshot {
    waiters: HashMap<String, oneshot::Sender<ActionResultRow>>,
    completed: HashMap<String, ActionResultRow>,
    order: VecDeque<String>,
}

/// `nostr_sdk::Client` still provides event-builder ergonomics for feature
/// modules, but every actual signature resolves through NMP's identity actor.
#[derive(Clone)]
pub(crate) struct NmpNostrSigner {
    nmp: Arc<HighlighterNmpRuntime>,
}

impl std::fmt::Debug for NmpNostrSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NmpNostrSigner")
    }
}

impl RawEventObserver for RawMirrorObserver {
    fn on_raw_event(&self, kind: u32, json: &str) {
        self.on_raw_event_with_source(kind, json, None);
    }

    fn on_raw_event_with_source(&self, _kind: u32, json: &str, source_relay_url: Option<&str>) {
        let message = RawMirrorMessage {
            source: source_relay_url.unwrap_or("nmp").to_string(),
            json: json.to_string(),
        };
        match self.tx.try_send(message) {
            Ok(()) | Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {}
        }
    }
}

impl NmpIdentityState {
    fn new(active: Option<String>) -> Self {
        Self {
            inner: Mutex::new(NmpIdentitySnapshot {
                active,
                waiters: Vec::new(),
            }),
        }
    }

    fn active(&self) -> Option<String> {
        self.inner.lock().active.clone()
    }

    fn apply(&self, active: Option<String>) {
        let mut guard = self.inner.lock();
        if guard.active == active {
            return;
        }
        guard.active = active.clone();
        if let Some(pubkey) = active {
            for waiter in guard.waiters.drain(..) {
                let _ = waiter.send(pubkey.clone());
            }
        }
    }

    async fn wait_for_change(
        &self,
        previous: Option<String>,
        timeout: Duration,
    ) -> Result<String, CoreError> {
        let rx = {
            let mut guard = self.inner.lock();
            if let Some(active) = guard.active.clone() {
                if previous.as_deref() != Some(active.as_str()) {
                    return Ok(active);
                }
            }
            let (tx, rx) = oneshot::channel();
            guard.waiters.push(tx);
            rx
        };
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(pubkey)) => Ok(pubkey),
            Ok(Err(_)) => Err(CoreError::Signer("NMP identity waiter dropped".into())),
            Err(_) => Err(CoreError::Signer(
                "NMP identity activation timed out".into(),
            )),
        }
    }
}

impl Default for NmpRelayDiagnosticsState {
    fn default() -> Self {
        Self {
            relays: RwLock::new(HashMap::new()),
            infos: RwLock::new(HashMap::new()),
            callback_slot: RwLock::new(None),
        }
    }
}

impl NmpRelayDiagnosticsState {
    fn set_callback(&self, callback_slot: EventCallbackSlot) {
        *self.callback_slot.write() = Some(callback_slot);
    }

    fn snapshot(&self) -> Vec<RelayDiagnostic> {
        self.relays.read().values().cloned().collect()
    }

    fn info_snapshot(&self) -> Vec<Nip11Document> {
        self.infos.read().values().cloned().collect()
    }

    fn info_for(&self, url: &str) -> Option<Nip11Document> {
        let infos = self.infos.read();
        if let Some(doc) = infos.get(url) {
            return Some(doc.clone());
        }
        // NMP normalises relay URLs (e.g. strips a trailing slash); accept
        // either spelling so the add-relay preview hits the cache too.
        let trimmed = url.trim_end_matches('/');
        infos.get(trimmed).cloned()
    }

    fn apply_infos(&self, docs: Vec<Nip11Document>) {
        let mut arrived = Vec::new();
        {
            let mut infos = self.infos.write();
            for doc in docs {
                let url = doc.url.clone();
                if infos.get(&url) != Some(&doc) {
                    arrived.push(url.clone());
                }
                infos.insert(url, doc);
            }
        }
        if arrived.is_empty() {
            return;
        }
        // A document arriving (or changing) after the relay connected does
        // not move the connection state, so it would never re-emit through
        // `apply`. Re-announce the relay's current state so the app layer
        // re-reads the row and picks up the new document.
        let Some(slot) = self.callback_slot.read().clone() else {
            return;
        };
        let Some(callback) = slot.read().clone() else {
            return;
        };
        let relays = self.relays.read();
        for url in arrived {
            let state = relays
                .get(&url)
                .map(|diag| diag.state)
                .unwrap_or(RelayStatus::Connected);
            callback.on_data_changed(Delta {
                subscription_id: 0,
                change: DataChangeType::RelayStatusChanged { url, state },
            });
        }
    }

    fn apply(&self, rows: Vec<RelayDiagnostic>) {
        let next: HashMap<String, RelayDiagnostic> =
            rows.into_iter().map(|row| (row.url.clone(), row)).collect();
        let mut changed = Vec::new();
        {
            let previous = self.relays.read();
            for (url, diag) in next.iter() {
                match previous.get(url) {
                    Some(prev) if prev.state == diag.state => {}
                    _ => changed.push((url.clone(), diag.state)),
                }
            }
            for url in previous.keys() {
                if !next.contains_key(url) {
                    changed.push((url.clone(), RelayStatus::Terminated));
                }
            }
        }
        *self.relays.write() = next;

        if changed.is_empty() {
            return;
        }
        let Some(slot) = self.callback_slot.read().clone() else {
            return;
        };
        let Some(callback) = slot.read().clone() else {
            return;
        };
        for (url, state) in changed {
            callback.on_data_changed(Delta {
                subscription_id: 0,
                change: DataChangeType::RelayStatusChanged { url, state },
            });
        }
    }
}

impl NmpActionResultsState {
    fn apply(&self, rows: Vec<ActionResultRow>) {
        for row in rows {
            self.apply_row(row);
        }
    }

    fn apply_row(&self, row: ActionResultRow) {
        let correlation_id = row.correlation_id.trim();
        if correlation_id.is_empty() {
            return;
        }

        let correlation_id = correlation_id.to_string();
        let mut guard = self.inner.lock();
        if let Some(waiter) = guard.waiters.remove(&correlation_id) {
            let _ = waiter.send(row);
            return;
        }

        if !guard.completed.contains_key(&correlation_id) {
            guard.order.push_back(correlation_id.clone());
        }
        guard.completed.insert(correlation_id.clone(), row);
        while guard.order.len() > NMP_ACTION_RESULT_CACHE_LIMIT {
            if let Some(old) = guard.order.pop_front() {
                guard.completed.remove(&old);
            }
        }
    }

    async fn wait_for(
        &self,
        correlation_id: &str,
        timeout: Duration,
    ) -> Result<ActionResultRow, CoreError> {
        let correlation_id = correlation_id.trim();
        if correlation_id.is_empty() {
            return Err(CoreError::Other(
                "NMP action result wait requires a correlation_id".into(),
            ));
        }

        let rx = {
            let mut guard = self.inner.lock();
            if let Some(row) = guard.completed.remove(correlation_id) {
                guard.order.retain(|key| key != correlation_id);
                return Ok(row);
            }
            if guard.waiters.contains_key(correlation_id) {
                return Err(CoreError::Other(format!(
                    "duplicate NMP action result waiter for {correlation_id}"
                )));
            }
            let (tx, rx) = oneshot::channel();
            guard.waiters.insert(correlation_id.to_string(), tx);
            rx
        };

        // Unconditional cleanup, including when this future is DROPPED
        // mid-await (an off-actor op aborted by supersession/logout): without
        // it the dead sender would linger in `waiters` until a matching row
        // happened to arrive — never, on a dead network. Removing an entry
        // that `apply_row` already consumed is a no-op.
        struct WaiterCleanup<'a> {
            inner: &'a Mutex<NmpActionResultsSnapshot>,
            correlation_id: &'a str,
        }
        impl Drop for WaiterCleanup<'_> {
            fn drop(&mut self) {
                self.inner.lock().waiters.remove(self.correlation_id);
            }
        }
        let _cleanup = WaiterCleanup {
            inner: &self.inner,
            correlation_id,
        };

        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(row)) => Ok(row),
            Ok(Err(_)) => Err(CoreError::Other(format!(
                "NMP action result waiter dropped for {correlation_id}"
            ))),
            Err(_) => Err(CoreError::Other(format!(
                "NMP action result timed out for {correlation_id}"
            ))),
        }
    }
}

impl NostrSigner for NmpNostrSigner {
    fn backend(&self) -> SignerBackend<'_> {
        SignerBackend::Custom(Cow::Borrowed("nmp"))
    }

    fn get_public_key(&self) -> BoxedFuture<'_, Result<PublicKey, SignerError>> {
        Box::pin(async move {
            let pubkey = self
                .nmp
                .active_pubkey()
                .ok_or_else(|| SignerError::from("NMP signer has no active account"))?;
            PublicKey::from_hex(&pubkey)
                .map_err(|e| SignerError::from(format!("active NMP pubkey: {e}")))
        })
    }

    fn sign_event(&self, unsigned: UnsignedEvent) -> BoxedFuture<'_, Result<Event, SignerError>> {
        Box::pin(async move {
            let nmp_unsigned = unsigned_event_for_nmp(&unsigned);
            let signer_pubkey = Some(unsigned.pubkey.to_hex());
            let signed = self
                .nmp
                .sign_unsigned_event(signer_pubkey, nmp_unsigned)
                .await
                .map_err(|e| SignerError::from(e.to_string()))?;
            Event::from_json(signed.to_nip01_json())
                .map_err(|e| SignerError::from(format!("NMP signed event decode: {e}")))
        })
    }

    fn nip04_encrypt<'a>(
        &'a self,
        _public_key: &'a PublicKey,
        _content: &'a str,
    ) -> BoxedFuture<'a, Result<String, SignerError>> {
        Box::pin(async move {
            Err(SignerError::from(
                "NMP signer facade does not expose NIP-04 encryption",
            ))
        })
    }

    fn nip04_decrypt<'a>(
        &'a self,
        _public_key: &'a PublicKey,
        _encrypted_content: &'a str,
    ) -> BoxedFuture<'a, Result<String, SignerError>> {
        Box::pin(async move {
            Err(SignerError::from(
                "NMP signer facade does not expose NIP-04 decryption",
            ))
        })
    }

    fn nip44_encrypt<'a>(
        &'a self,
        _public_key: &'a PublicKey,
        _content: &'a str,
    ) -> BoxedFuture<'a, Result<String, SignerError>> {
        Box::pin(async move {
            Err(SignerError::from(
                "NMP signer facade does not expose NIP-44 encryption",
            ))
        })
    }

    fn nip44_decrypt<'a>(
        &'a self,
        _public_key: &'a PublicKey,
        _payload: &'a str,
    ) -> BoxedFuture<'a, Result<String, SignerError>> {
        Box::pin(async move {
            Err(SignerError::from(
                "NMP signer facade does not expose NIP-44 decryption",
            ))
        })
    }
}

impl HighlighterNmpRuntime {
    pub(crate) fn new(
        nostrdb_dir: &Path,
        ndb: Arc<Ndb>,
        initial_relays: &[RelayConfig],
    ) -> Result<Self, CoreError> {
        let storage_dir = nmp_storage_dir(nostrdb_dir);
        std::fs::create_dir_all(&storage_dir)
            .map_err(|e| CoreError::Cache(format!("create NMP store: {e}")))?;

        let relay_roles: Vec<(String, String)> =
            initial_relays.iter().filter_map(nmp_relay_role).collect();
        let applied_relays: HashMap<String, String> = relay_roles.iter().cloned().collect();

        let mut builder = NmpAppBuilder::new().with_relays(relay_roles);
        nmp_defaults::register_defaults(&mut builder);
        register_nmp_protocol_actions(&mut builder);
        nmp_blossom::register_actions(&mut builder);
        builder.set_nostrconnect_bootstrap_relay(nostr_connect_relay().to_string());
        let app = builder
            .storage_path(storage_dir.to_string_lossy().into_owned())
            .start(RunConfig::default());
        let app = NonNull::new(app)
            .ok_or_else(|| CoreError::Other("NMP app initialization returned null".into()))?;
        nmp_signer_broker_init(app.as_ptr());

        let app_ref = unsafe { app.as_ref() };
        let active = app_ref
            .active_account_handle()
            .lock()
            .ok()
            .and_then(|guard| guard.clone());
        let identity = Arc::new(NmpIdentityState::new(active));
        let identity_observer = identity.clone();
        app_ref.register_identity_change_observer(move |active| {
            identity_observer.apply(active);
        });

        let diagnostics = Arc::new(NmpRelayDiagnosticsState::default());
        let action_results = Arc::new(NmpActionResultsState::default());
        let action_result_stop = Arc::new(AtomicBool::new(false));
        let typed_projection_drain_lock = Arc::new(Mutex::new(()));
        let identity_tick_observer = identity.clone();
        let diagnostics_app = app.as_ptr() as usize;
        app_ref.register_snapshot_tick_observer(move || {
            let app_ref = unsafe { &*(diagnostics_app as *const NmpApp) };
            let active = app_ref
                .active_account_handle()
                .lock()
                .ok()
                .and_then(|guard| guard.clone());
            identity_tick_observer.apply(active);
        });

        let (action_result_wake, action_result_rx) = sync_channel::<()>(1);
        let update_frames = Arc::new(UpdateFrameSidecar {
            latest: Mutex::new(None),
            wake: action_result_wake.clone(),
        });
        let action_result_worker = spawn_action_result_drain(
            app.as_ptr() as usize,
            action_result_rx,
            action_results.clone(),
            diagnostics.clone(),
            update_frames.clone(),
            typed_projection_drain_lock.clone(),
            action_result_stop.clone(),
        )?;
        let action_result_tick_tx = action_result_wake.clone();
        app_ref.register_snapshot_tick_observer(move || match action_result_tick_tx.try_send(()) {
            Ok(()) | Err(TrySendError::Full(())) | Err(TrySendError::Disconnected(())) => {}
        });
        // Receive every emitted snapshot frame — the only transport carrying
        // the kernel's built-in typed sidecars (relay diagnostics included).
        // The context Arc lives in `Self::_update_frames`; Drop clears the
        // registration before it is released.
        nmp_app_set_update_callback(
            app.as_ptr(),
            Arc::as_ptr(&update_frames) as *mut std::ffi::c_void,
            Some(on_nmp_update_frame),
        );

        let (raw_mirror_observer, raw_mirror_worker) = spawn_raw_mirror(ndb)?;
        let raw_mirror_id =
            app_ref.register_raw_event_observer(KindFilter::default(), raw_mirror_observer);
        if raw_mirror_id.0 == 0 {
            action_result_stop.store(true, Ordering::Release);
            let _ = action_result_wake.try_send(());
            let _ = action_result_worker.join();
            let _ = raw_mirror_worker.join();
            nmp_app_free(app.as_ptr());
            return Err(CoreError::Other(
                "NMP raw event mirror registration failed".into(),
            ));
        }

        // NIP-55 external-signer (ADR-0048 Stage 2).
        // Register the capability trampoline before `nmp_external_signer_init`
        // so the first `get_public_key` dispatch has a handler. The context
        // value is a registry handle id, never a pointer (teardown-race
        // rationale on `external_signer_registry`).
        let (signer_request_tx, signer_request_rx) =
            sync_channel::<String>(NMP_SIGNER_REQUEST_CAPACITY);
        let external_signer_handle = NEXT_EXTERNAL_SIGNER_HANDLE.fetch_add(1, Ordering::Relaxed);
        external_signer_registry().lock().insert(
            external_signer_handle,
            Arc::new(ExternalSignerContext {
                tx: signer_request_tx,
            }),
        );
        nmp_app_set_capability_callback(
            app.as_ptr(),
            external_signer_handle as *mut std::ffi::c_void,
            Some(on_capability_request),
        );
        nmp_external_signer_init(app.as_ptr());

        Ok(Self {
            app,
            storage_dir,
            applied_relays: RwLock::new(applied_relays),
            identity,
            diagnostics,
            action_results,
            action_result_stop,
            action_result_wake,
            action_result_worker: Some(action_result_worker),
            _update_frames: update_frames,
            typed_projection_drain_lock,
            raw_mirror_id,
            raw_mirror_worker: Some(raw_mirror_worker),
            signer_request_rx: Mutex::new(signer_request_rx),
            external_signer_handle,
        })
    }

    pub(crate) fn storage_dir(&self) -> &Path {
        &self.storage_dir
    }

    pub(crate) fn nostr_signer(self: &Arc<Self>) -> NmpNostrSigner {
        NmpNostrSigner { nmp: self.clone() }
    }

    pub(crate) fn active_pubkey(&self) -> Option<String> {
        self.identity.active()
    }

    pub(crate) async fn wait_for_active_account_after(
        &self,
        previous: Option<String>,
        timeout: Duration,
    ) -> Result<String, CoreError> {
        self.identity.wait_for_change(previous, timeout).await
    }

    /// Wait for the identity actor to expose a newly-paired signer account
    /// (any backend — NIP-46 bunker or NIP-55 external; V-78).
    pub(crate) async fn wait_for_signer_pair_after(
        &self,
        previous: Option<String>,
    ) -> Result<String, CoreError> {
        self.wait_for_active_account_after(previous, NMP_SIGNER_PAIR_TIMEOUT)
            .await
    }

    pub(crate) fn nostrconnect_uri(
        &self,
        options: &NostrConnectOptions,
    ) -> Result<String, CoreError> {
        let relay = cstring_arg(nostr_connect_relay(), "NMP nostrconnect relay")?;
        let uri_ptr = nmp_app_nostrconnect_uri(self.app.as_ptr(), relay.as_ptr(), std::ptr::null());
        if uri_ptr.is_null() {
            return Err(CoreError::Signer(
                "NMP nostrconnect URI generation failed".into(),
            ));
        }
        let uri = unsafe { CStr::from_ptr(uri_ptr) }
            .to_string_lossy()
            .into_owned();
        nmp_free_string(uri_ptr);
        apply_nostrconnect_options(uri, options)
    }

    pub(crate) fn sign_in_nsec(&self, nsec: &str) -> Result<String, CoreError> {
        let secret = nsec.trim();
        if secret.is_empty() {
            return Err(CoreError::InvalidInput("nsec must not be empty".into()));
        }
        let keys = Keys::parse(secret)
            .map_err(|e| CoreError::InvalidInput(format!("invalid nsec: {e}")))?;
        let pubkey = keys.public_key().to_hex();
        self.app_ref().add_signer(
            SignerSource::LocalNsec(Zeroizing::new(secret.to_string())),
            true,
        );
        // `add_signer` is actor-queued and all later signing commands use the
        // same sender, so NMP will install the key before any publish/sign
        // request that follows this call. The app projection can move
        // immediately instead of waiting for a throttled snapshot tick.
        //
        // Before mirroring the identity locally, briefly wait for NMP's own
        // `active_account_handle` to reflect this pubkey. The queued `add_signer`
        // is purely local (no relay I/O) so this settles in milliseconds. Without
        // it, the snapshot-tick observer (which mirrors `active_account_handle`
        // into `self.identity` on every tick) can fire in the window before the
        // actor processes the queued add, reading `None` and clobbering the
        // optimistic apply below — which would make a rapid superseding sign-in
        // observe an unset prior identity.
        let confirm_deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let nmp_active = self
                .app_ref()
                .active_account_handle()
                .lock()
                .ok()
                .and_then(|guard| guard.clone());
            if nmp_active.as_deref() == Some(pubkey.as_str()) || Instant::now() >= confirm_deadline
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        self.identity.apply(Some(pubkey.clone()));
        Ok(pubkey)
    }

    pub(crate) fn sign_in_bunker_uri(&self, uri: &str) -> Result<(), CoreError> {
        let uri = uri.trim();
        if uri.is_empty() {
            return Err(CoreError::InvalidInput(
                "bunker URI must not be empty".into(),
            ));
        }
        nmp_signers::parse_bunker_uri(uri)
            .map_err(|e| CoreError::InvalidInput(format!("invalid bunker URI: {e}")))?;
        self.app_ref()
            .add_signer(SignerSource::BunkerUri(uri.to_string()), true);
        Ok(())
    }

    /// Begin a NIP-55 sign-in (ADR-0048 D2).
    ///
    /// Triggers the `Nip55Driver` to build and dispatch a `get_public_key`
    /// + permission-batch `ExternalSignerRequest` through the registered
    /// capability callback (the `on_capability_request` trampoline). The
    /// trampoline pushes the payload onto `signer_request_rx`; the Kotlin
    /// side drains it via `next_signer_request` and routes it to
    /// `ExternalSignerCapabilityBridge.handleJson`.
    ///
    /// `signer_package` is the Android package name of the signer app (e.g.
    /// `com.greenart7c3.nostrsigner` for Amber). `None` lets the OS resolver
    /// pick any installed NIP-55 signer.
    pub(crate) fn sign_in_nip55(&self, signer_package: Option<&str>) {
        let package = signer_package
            .filter(|s| !s.trim().is_empty())
            .and_then(|s| CString::new(s).ok());
        nmp_app_signin_nip55(
            self.app.as_ptr(),
            package.as_ref().map_or(std::ptr::null(), |c| c.as_ptr()),
        );
    }

    /// Deliver a raw `ExternalSignerResponse` JSON back to the NIP-55 driver
    /// (D7 — verbatim; the driver owns correlation routing and all policy).
    ///
    /// Called by the Kotlin `ExternalSignerCapabilityBridge` after the Intent
    /// round-trip or ContentResolver query completes.
    pub(crate) fn deliver_external_signer_response(&self, response_json: &str) {
        let response = match CString::new(response_json) {
            Ok(c) => c,
            Err(_) => return, // interior NUL — D6: silently drop
        };
        nmp_app_deliver_external_signer_response(self.app.as_ptr(), response.as_ptr());
    }

    /// Blocking timed drain of the outbound NIP-55 signer-request channel
    /// (D8 — the caller parks INSIDE `recv_timeout`, never in a sleep+check
    /// loop; same contract as `nmp-android-ffi`'s `recv_next_signer_request`).
    ///
    /// - [`SignerRequestDrain::Request`] — one `ExternalSignerRequest` JSON
    ///   payload (D7: raw bytes from Rust; Kotlin decides nothing).
    /// - [`SignerRequestDrain::Idle`] — normal timeout tick (≤250 ms); gives
    ///   the Kotlin reader a bounded-latency window to observe its shutdown
    ///   flag. Zero wake-ups beyond that tick when idle.
    /// - [`SignerRequestDrain::Closed`] — the sender is gone (runtime
    ///   teardown); the reader must exit its loop.
    ///
    /// Designed for a dedicated Kotlin daemon thread that loops on this call
    /// and routes each payload to `ExternalSignerCapabilityBridge.handleJson`.
    pub(crate) fn next_signer_request(&self) -> SignerRequestDrain {
        let rx = self.signer_request_rx.lock();
        match rx.recv_timeout(NMP_SIGNER_DRAIN_TICK) {
            Ok(payload) => SignerRequestDrain::Request(payload),
            Err(RecvTimeoutError::Timeout) => SignerRequestDrain::Idle,
            Err(RecvTimeoutError::Disconnected) => SignerRequestDrain::Closed,
        }
    }

    pub(crate) async fn sign_unsigned_event(
        &self,
        signer_pubkey: Option<String>,
        unsigned: NmpUnsignedEvent,
    ) -> Result<NmpSignedEvent, CoreError> {
        let (tx, rx) = oneshot::channel();
        let continuation = SignContinuation::new(move |outcome| {
            let _ = tx.send(outcome);
        });
        self.app_ref()
            .actor_sender()
            .send(ActorCommand::SignEventForAccount {
                unsigned,
                signer_pubkey,
                continuation,
            })
            .map_err(|e| CoreError::Signer(format!("NMP sign command failed: {e}")))?;

        match tokio::time::timeout(NMP_SIGN_TIMEOUT, rx).await {
            Ok(Ok(Ok(signed))) => Ok(signed),
            Ok(Ok(Err(reason))) => Err(CoreError::Signer(reason)),
            Ok(Err(_)) => Err(CoreError::Signer("NMP sign continuation dropped".into())),
            Err(_) => Err(CoreError::Signer("NMP sign timed out".into())),
        }
    }

    pub(crate) fn remove_account(&self, pubkey_hex: &str) {
        let pubkey_hex = pubkey_hex.trim();
        if !pubkey_hex.is_empty() {
            self.app_ref().remove_account(pubkey_hex.to_string());
        }
    }

    pub(crate) fn foreground(&self) {
        nmp_app_lifecycle_foreground(self.app.as_ptr());
    }

    pub(crate) fn background(&self) {
        nmp_app_lifecycle_background(self.app.as_ptr());
    }

    pub(crate) fn sync_relays(&self, rows: &[RelayConfig]) -> Result<(), CoreError> {
        let next: HashMap<String, String> = rows.iter().filter_map(nmp_relay_role).collect();
        let current = self.applied_relays.read().clone();

        for url in current.keys().filter(|url| !next.contains_key(*url)) {
            self.remove_relay(url)?;
        }

        for (url, role) in next.iter() {
            if current.get(url) != Some(role) {
                self.add_relay(url, role)?;
            }
        }

        *self.applied_relays.write() = next;
        Ok(())
    }

    pub(crate) fn open_filter_interest(
        &self,
        label: &str,
        owner: &str,
        filter: Filter,
        relay_pin: Option<String>,
        lifecycle: InterestLifecycle,
    ) -> Result<NmpInterestHandle, CoreError> {
        let filter_json = serde_json::to_string(&filter)
            .map_err(|e| CoreError::Other(format!("{label}: encode NMP filter: {e}")))?;
        let mut shape = InterestShape::from_filter_json(&filter_json)
            .ok_or_else(|| CoreError::InvalidInput(format!("{label}: invalid NMP filter")))?;
        shape.relay_pin = relay_pin.clone().filter(|url| !url.trim().is_empty());
        let id = stable_interest_id(
            label,
            owner,
            &filter_json,
            shape.relay_pin.as_deref(),
            &lifecycle,
        );
        let interest = LogicalInterest {
            id: InterestId(id),
            scope: InterestScope::Global,
            shape,
            lifecycle,
            ..LogicalInterest::default()
        };
        self.app_ref().push_interest(interest);
        Ok(NmpInterestHandle { id })
    }

    pub(crate) fn close_interest(&self, handle: NmpInterestHandle) {
        let _ = self
            .app_ref()
            .actor_sender()
            .send(ActorCommand::WithdrawInterest(InterestId(handle.id)));
    }

    pub(crate) fn set_relay_diagnostics_callback(&self, callback_slot: EventCallbackSlot) {
        self.diagnostics.set_callback(callback_slot);
    }

    pub(crate) fn relay_diagnostics_snapshot(&self) -> Vec<RelayDiagnostic> {
        let typed = self.drain_typed_snapshot_projections();
        apply_typed_projection_sidecars(&typed, &self.action_results, &self.diagnostics);
        self.diagnostics.snapshot()
    }

    /// All NIP-11 documents NMP has fetched for pool relays (ADR-0051).
    /// Sourced from the `relay_diagnostics` projection's `info` child —
    /// no HTTP or parsing happens on the Highlighter side.
    pub(crate) fn relay_info_documents(&self) -> Vec<Nip11Document> {
        let typed = self.drain_typed_snapshot_projections();
        apply_typed_projection_sidecars(&typed, &self.action_results, &self.diagnostics);
        self.diagnostics.info_snapshot()
    }

    /// The cached NIP-11 document for one relay URL, if NMP has fetched it.
    pub(crate) fn relay_info(&self, url: &str) -> Option<Nip11Document> {
        self.diagnostics.info_for(url)
    }

    pub(crate) fn publish_signed_auto(&self, source: &str, event: &Event) -> Result<(), CoreError> {
        self.dispatch_publish(source, event, PublishTarget::Auto)
    }

    pub(crate) fn publish_signed_to_relays(
        &self,
        source: &str,
        event: &Event,
        relays: Vec<String>,
    ) -> Result<(), CoreError> {
        let relays: Vec<String> = relays
            .into_iter()
            .map(|r| r.trim().to_string())
            .filter(|r| !r.is_empty())
            .collect();
        if relays.is_empty() {
            return Err(CoreError::Relay(format!(
                "{source}: explicit NMP publish target requires at least one relay"
            )));
        }
        self.dispatch_publish(source, event, PublishTarget::Explicit { relays })
    }

    pub(crate) async fn dispatch_action_for_result<T: serde::Serialize>(
        &self,
        source: &str,
        namespace: &str,
        action: &T,
    ) -> Result<ActionResultRow, CoreError> {
        let action_json = serde_json::to_string(action)
            .map_err(|e| CoreError::Other(format!("{source}: encode NMP action: {e}")))?;
        let correlation_id = self.dispatch_action_json(source, namespace, &action_json)?;
        let row = self
            .action_results
            .wait_for(&correlation_id, NMP_PROTOCOL_ACTION_TIMEOUT)
            .await?;
        if let Some(error) = row.error.as_ref().filter(|s| !s.trim().is_empty()) {
            return Err(CoreError::Other(format!("{source}: {error}")));
        }
        if row.status == "failed" {
            return Err(CoreError::Other(format!(
                "{source}: NMP action failed without an error body"
            )));
        }
        Ok(row)
    }

    fn dispatch_publish(
        &self,
        source: &str,
        event: &Event,
        target: PublishTarget,
    ) -> Result<(), CoreError> {
        let action = PublishAction::Publish {
            handle: event.id.to_hex(),
            event: signed_event_for_nmp(event),
            target,
        };
        let action_json = serde_json::to_string(&action)
            .map_err(|e| CoreError::Other(format!("{source}: encode NMP publish action: {e}")))?;
        let correlation_id =
            self.dispatch_action_json(source, NMP_PUBLISH_NAMESPACE, &action_json)?;
        if correlation_id.trim().is_empty() {
            return Err(CoreError::Relay(format!(
                "{source}: NMP publish response missing correlation_id"
            )));
        }
        Ok(())
    }

    fn dispatch_action_json(
        &self,
        source: &str,
        namespace: &str,
        action_json: &str,
    ) -> Result<String, CoreError> {
        let namespace = cstring_arg(namespace, "NMP action namespace")?;
        let action = cstring_arg(action_json, "NMP action")?;
        let response_ptr =
            nmp_app_dispatch_action(self.app.as_ptr(), namespace.as_ptr(), action.as_ptr());
        if response_ptr.is_null() {
            return Err(CoreError::Relay(format!(
                "{source}: NMP publish returned null"
            )));
        }

        let response = unsafe { CStr::from_ptr(response_ptr) }
            .to_string_lossy()
            .into_owned();
        nmp_free_string(response_ptr);

        let value: serde_json::Value = serde_json::from_str(&response)
            .map_err(|e| CoreError::Relay(format!("{source}: invalid NMP response: {e}")))?;
        if let Some(error) = value.get("error").and_then(serde_json::Value::as_str) {
            return Err(CoreError::Relay(format!("{source}: {error}")));
        }
        if let Some(correlation_id) = value
            .get("correlation_id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
        {
            return Ok(correlation_id);
        }
        Err(CoreError::Relay(format!(
            "{source}: NMP action response missing correlation_id"
        )))
    }

    fn drain_typed_snapshot_projections(&self) -> Vec<TypedProjectionData> {
        let _guard = self.typed_projection_drain_lock.lock();
        self.app_ref().run_typed_snapshot_projections()
    }

    fn add_relay(&self, url: &str, role: &str) -> Result<(), CoreError> {
        let url = cstring_arg(url, "NMP relay URL")?;
        let role = cstring_arg(role, "NMP relay role")?;
        nmp_app_add_relay(self.app.as_ptr(), url.as_ptr(), role.as_ptr());
        Ok(())
    }

    fn remove_relay(&self, url: &str) -> Result<(), CoreError> {
        let url = cstring_arg(url, "NMP relay URL")?;
        nmp_app_remove_relay(self.app.as_ptr(), url.as_ptr());
        Ok(())
    }

    fn app_ref(&self) -> &NmpApp {
        unsafe { self.app.as_ref() }
    }
}

fn register_nmp_protocol_actions(app: &mut impl ActionRegistrar) {
    app.register_action::<nmp_nip29::action::PostChatMessageAction>();
    app.register_action::<nmp_nip29::action::ReactInGroupAction>();
    app.register_action::<nmp_nip29::action::CreatePublicGroupAction>();
    app.register_action::<nmp_nip29::action::DiscoverGroupsAction>();
    app.register_action::<nmp_nip29::action::JoinGroupAction>();
}

/// Mailbox for snapshot frames delivered by `nmp_app_set_update_callback`.
///
/// The kernel's built-in typed projections (relay diagnostics, action
/// results) are merged into emitted frames ONLY — they never appear in
/// `run_typed_snapshot_projections()`, which runs just host-registered
/// closures. Capturing the frame here is therefore the only way to observe
/// relay connection status. Only the newest frame matters; older ones are
/// overwritten.
pub(crate) struct UpdateFrameSidecar {
    latest: Mutex<Option<Vec<u8>>>,
    wake: SyncSender<()>,
}

/// C update callback registered with the NMP kernel. Runs on the kernel
/// actor thread — copy the bytes and signal the drain; never block here.
extern "C" fn on_nmp_update_frame(ctx: *mut std::ffi::c_void, bytes: *const u8, len: usize) {
    if ctx.is_null() || bytes.is_null() || len == 0 {
        return;
    }
    // SAFETY: `ctx` is the `Arc<UpdateFrameSidecar>` registered in
    // `HighlighterNmpRuntime::new`; the runtime holds the Arc for the app's
    // lifetime and clears the registration (with quiescence) in Drop before
    // releasing it.
    let sidecar = unsafe { &*(ctx as *const UpdateFrameSidecar) };
    let frame = unsafe { std::slice::from_raw_parts(bytes, len) }.to_vec();
    *sidecar.latest.lock() = Some(frame);
    match sidecar.wake.try_send(()) {
        Ok(()) | Err(TrySendError::Full(())) | Err(TrySendError::Disconnected(())) => {}
    }
}

fn spawn_action_result_drain(
    app_ptr: usize,
    wake_rx: std::sync::mpsc::Receiver<()>,
    action_results: Arc<NmpActionResultsState>,
    diagnostics: Arc<NmpRelayDiagnosticsState>,
    update_frames: Arc<UpdateFrameSidecar>,
    drain_lock: Arc<Mutex<()>>,
    stop: Arc<AtomicBool>,
) -> Result<JoinHandle<()>, CoreError> {
    std::thread::Builder::new()
        .name("highlighter-nmp-action-results".into())
        .spawn(move || {
            while wake_rx.recv().is_ok() {
                if stop.load(Ordering::Acquire) {
                    break;
                }
                let mut typed = {
                    let _guard = drain_lock.lock();
                    let app_ref = unsafe { &*(app_ptr as *const NmpApp) };
                    app_ref.run_typed_snapshot_projections()
                };
                // The kernel's built-in sidecars (relay diagnostics, action
                // results) travel only in emitted frames; fold the newest
                // frame's entries in alongside the host-registered ones.
                let frame = update_frames.latest.lock().take();
                if let Some(frame) = frame {
                    match decode_snapshot_typed_projections(&frame) {
                        Ok(entries) => typed.extend(entries),
                        Err(err) => {
                            tracing::warn!(error = ?err, "decode NMP snapshot frame sidecars");
                        }
                    }
                }
                apply_typed_projection_sidecars(&typed, &action_results, &diagnostics);
            }
        })
        .map_err(|e| CoreError::Other(format!("spawn NMP action result drain: {e}")))
}

fn apply_typed_projection_sidecars(
    typed: &[TypedProjectionData],
    action_results: &NmpActionResultsState,
    diagnostics: &NmpRelayDiagnosticsState,
) {
    if let Some(rows) = action_results_from_typed_projections(typed) {
        action_results.apply(rows);
    }
    if let Some((rows, infos)) = relay_diagnostics_from_typed_projections(typed) {
        diagnostics.apply_infos(infos);
        diagnostics.apply(rows);
    }
}

fn action_results_from_typed_projections(
    typed: &[TypedProjectionData],
) -> Option<Vec<ActionResultRow>> {
    let entry = typed.iter().find(|entry| {
        entry.key == ACTION_RESULTS_SCHEMA_ID || entry.schema_id == ACTION_RESULTS_SCHEMA_ID
    })?;
    decode_action_results(&entry.payload)
        .ok()
        .map(|model| model.results)
}

fn stable_interest_id(
    label: &str,
    owner: &str,
    filter_json: &str,
    relay_pin: Option<&str>,
    lifecycle: &InterestLifecycle,
) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    label.hash(&mut hasher);
    owner.hash(&mut hasher);
    filter_json.hash(&mut hasher);
    relay_pin.hash(&mut hasher);
    lifecycle.hash(&mut hasher);
    let id = hasher.finish();
    if id == 0 {
        1
    } else {
        id
    }
}

fn spawn_raw_mirror(ndb: Arc<Ndb>) -> Result<(Arc<RawMirrorObserver>, JoinHandle<()>), CoreError> {
    let (tx, rx) = sync_channel::<RawMirrorMessage>(1024);
    let worker = std::thread::Builder::new()
        .name("highlighter-nmp-raw-mirror".into())
        .spawn(move || {
            while let Ok(message) = rx.recv() {
                let source = serde_json::to_string(&message.source)
                    .unwrap_or_else(|_| "\"nmp\"".to_string());
                let line = format!(r#"["EVENT",{source},{}]"#, message.json);
                if let Err(e) = ndb.process_event(&line) {
                    tracing::warn!(error = %e, "NMP raw event mirror to nostrdb");
                }
            }
        })
        .map_err(|e| CoreError::Other(format!("spawn NMP raw mirror: {e}")))?;
    Ok((Arc::new(RawMirrorObserver { tx }), worker))
}

fn nmp_storage_dir(nostrdb_dir: &Path) -> PathBuf {
    nostrdb_dir
        .parent()
        .map(|p| p.join("nmp"))
        .unwrap_or_else(|| nostrdb_dir.join("nmp"))
}

pub(crate) fn nmp_relay_role(row: &RelayConfig) -> Option<(String, String)> {
    let url = row.url.trim();
    if url.is_empty() {
        return None;
    }

    let mut roles: Vec<&str> = Vec::new();
    match (row.read || row.rooms, row.write) {
        (true, true) => roles.push("both"),
        (true, false) => roles.push("read"),
        (false, true) => roles.push("write"),
        (false, false) => {}
    }
    if row.indexer {
        roles.push("indexer");
    }
    if roles.is_empty() {
        return None;
    }
    Some((url.to_string(), roles.join(",")))
}

fn signed_event_for_nmp(event: &Event) -> NmpSignedEvent {
    NmpSignedEvent {
        id: event.id.to_hex(),
        sig: event.sig.to_string(),
        unsigned: NmpUnsignedEvent {
            pubkey: event.pubkey.to_hex(),
            created_at: event.created_at.as_secs(),
            kind: u32::from(event.kind.as_u16()),
            tags: event
                .tags
                .iter()
                .map(|tag| tag.as_slice().to_vec())
                .collect(),
            content: event.content.clone(),
        },
    }
}

fn unsigned_event_for_nmp(event: &UnsignedEvent) -> NmpUnsignedEvent {
    NmpUnsignedEvent {
        pubkey: event.pubkey.to_hex(),
        created_at: event.created_at.as_secs(),
        kind: u32::from(event.kind.as_u16()),
        tags: event
            .tags
            .iter()
            .map(|tag| tag.as_slice().to_vec())
            .collect(),
        content: event.content.clone(),
    }
}

fn relay_diagnostics_from_typed_projections(
    typed: &[TypedProjectionData],
) -> Option<(Vec<RelayDiagnostic>, Vec<Nip11Document>)> {
    let entry = typed.iter().find(|entry| {
        entry.key == RELAY_DIAGNOSTICS_SCHEMA_ID || entry.schema_id == RELAY_DIAGNOSTICS_SCHEMA_ID
    })?;
    let decoded = decode_relay_diagnostics(&entry.payload).ok()?;
    let mut diagnostics = Vec::with_capacity(decoded.relays.len());
    let mut infos = Vec::new();
    for row in decoded.relays {
        if let Some(info) = &row.info {
            infos.push(Nip11Document {
                url: row.relay_url.clone(),
                name: info.name.clone(),
                description: info.description.clone(),
                pubkey: info.pubkey.clone(),
                contact: info.contact.clone(),
                software: info.software.clone(),
                version: info.version.clone(),
                supported_nips: info.supported_nips.clone(),
                icon: info.icon.clone(),
            });
        }
        diagnostics.push(RelayDiagnostic {
            url: row.relay_url,
            state: relay_status_from_nmp(&row.connection_label, &row.connection_tone),
            rtt_ms: None,
            bytes_sent: parse_bytes_display(row.bytes_tx_display.as_deref()),
            bytes_received: parse_bytes_display(row.bytes_rx_display.as_deref()),
            connected_since_ts: None,
        });
    }
    Some((diagnostics, infos))
}

fn apply_nostrconnect_options(
    uri: String,
    options: &NostrConnectOptions,
) -> Result<String, CoreError> {
    let mut parsed = url::Url::parse(&uri)
        .map_err(|e| CoreError::Signer(format!("NMP nostrconnect URI parse failed: {e}")))?;
    let retained_pairs: Vec<(String, String)> = parsed
        .query_pairs()
        .filter_map(|(key, value)| {
            if matches!(key.as_ref(), "name" | "url" | "image" | "perms") {
                None
            } else {
                Some((key.into_owned(), value.into_owned()))
            }
        })
        .collect();

    parsed.set_query(None);
    {
        let mut pairs = parsed.query_pairs_mut();
        for (key, value) in retained_pairs {
            pairs.append_pair(&key, &value);
        }

        let name = options.name.trim();
        pairs.append_pair("name", if name.is_empty() { "Highlighter" } else { name });

        let app_url = options.url.trim();
        if !app_url.is_empty() {
            pairs.append_pair("url", app_url);
        }

        let image = options.image.trim();
        if !image.is_empty() {
            pairs.append_pair("image", image);
        }

        let perms = options.perms.trim();
        pairs.append_pair(
            "perms",
            if perms.is_empty() {
                crate::relays::DEFAULT_NOSTR_CONNECT_PERMS
            } else {
                perms
            },
        );
    }

    Ok(parsed.to_string())
}

fn relay_status_from_nmp(label: &str, tone: &str) -> RelayStatus {
    let joined = format!("{label} {tone}").to_ascii_lowercase();
    if joined.contains("banned") {
        RelayStatus::Banned
    } else if joined.contains("terminated") {
        RelayStatus::Terminated
    } else if joined.contains("disconnected") || joined.contains("closed") {
        RelayStatus::Disconnected
    } else if joined.contains("connected") {
        RelayStatus::Connected
    } else {
        RelayStatus::Connecting
    }
}

fn parse_bytes_display(display: Option<&str>) -> u64 {
    let Some(display) = display.map(str::trim).filter(|s| !s.is_empty()) else {
        return 0;
    };
    let normalized = display.replace(',', "");
    let mut parts = normalized.split_whitespace();
    let Some(value) = parts.next().and_then(|v| v.parse::<f64>().ok()) else {
        return normalized.parse::<u64>().unwrap_or(0);
    };
    let unit = parts.next().unwrap_or("B").to_ascii_lowercase();
    let factor = match unit.as_str() {
        "b" | "byte" | "bytes" => 1.0,
        "kb" | "kib" => 1024.0,
        "mb" | "mib" => 1024.0 * 1024.0,
        "gb" | "gib" => 1024.0 * 1024.0 * 1024.0,
        _ => 1.0,
    };
    (value * factor).round().max(0.0) as u64
}

fn cstring_arg(value: &str, label: &str) -> Result<CString, CoreError> {
    CString::new(value)
        .map_err(|_| CoreError::InvalidInput(format!("{label} contains an interior NUL byte")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(read: bool, write: bool, rooms: bool, indexer: bool) -> RelayConfig {
        RelayConfig {
            url: "wss://relay.example".into(),
            read,
            write,
            rooms,
            indexer,
        }
    }

    #[test]
    fn nmp_relay_role_maps_read_write_to_both() {
        assert_eq!(
            nmp_relay_role(&row(true, true, false, false)),
            Some(("wss://relay.example".into(), "both".into()))
        );
    }

    #[test]
    fn nmp_relay_role_maps_rooms_to_read() {
        assert_eq!(
            nmp_relay_role(&row(false, false, true, false)),
            Some(("wss://relay.example".into(), "read".into()))
        );
    }

    #[test]
    fn nmp_relay_role_preserves_indexer_composite() {
        assert_eq!(
            nmp_relay_role(&row(true, true, false, true)),
            Some(("wss://relay.example".into(), "both,indexer".into()))
        );
    }

    #[test]
    fn nmp_relay_role_skips_disabled_rows() {
        assert_eq!(nmp_relay_role(&row(false, false, false, false)), None);
    }

    #[test]
    fn nmp_signed_event_conversion_preserves_nip01_shape() {
        let keys = Keys::generate();
        let tag = Tag::parse(vec!["t".to_string(), "highlighter".to_string()]).expect("tag");
        let event = EventBuilder::new(Kind::Custom(9802), "quote")
            .tags([tag])
            .sign_with_keys(&keys)
            .expect("event");

        let converted = signed_event_for_nmp(&event);

        assert_eq!(converted.id, event.id.to_hex());
        assert_eq!(converted.sig, event.sig.to_string());
        assert_eq!(converted.unsigned.pubkey, event.pubkey.to_hex());
        assert_eq!(converted.unsigned.kind, 9802);
        assert_eq!(converted.unsigned.content, "quote");
        assert_eq!(
            converted.unsigned.tags,
            vec![vec!["t".to_string(), "highlighter".to_string()]]
        );
    }
}
