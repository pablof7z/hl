//! Omnibox domain — the single-box / paste / search field brain, powered by
//! NMP's input-intent resolver (#1865 / #1804).
//!
//! ## What this is
//!
//! hl's search field is a full **omnibox**: one untyped string that may be
//!
//! * **free text** → multi-scope relay search (profiles kind:0 + notes kind:1 +
//!   articles kind:30023), surfaced through the existing `search_results`
//!   sidecar; the shell additionally keeps its local nostrdb buckets,
//! * a pasted **NIP-19/21 reference** (`npub…`, `nostr:nevent…`, `naddr…`) →
//!   navigate directly (profile / thread / article / group),
//! * a **NIP-05** identifier (`name@domain`) → HTTP `.well-known` reverse lookup
//!   → open the resolved profile,
//! * a **NIP-29 group** reference (`host'local-id`) → open the group,
//! * a **relay URL** (`wss://…`) → relay metadata,
//! * an **`nsec` / `ncryptsec` secret** → **safe reject** (never echoed).
//!
//! ## How the classification reaches NMP
//!
//! The pure classifier `nmp_intent::classify` reads the app's registered input
//! recognizers, but the registry (`NmpApp::input_scope_registry`) is
//! `pub(crate)` in nmp-ffi — an external Rust consumer cannot snapshot it. The
//! supported consumer surface is therefore the stable C-ABI exported by
//! nmp-ffi:
//!
//! * `nmp_app_intent_classify(app, request_json) -> *mut c_char` — STATELESS,
//!   pure, side-effect-free. Returns `{"ok":true,"classification":…}`.
//! * `nmp_app_intent_dispatch(app, request_json, session_id) -> *mut c_char` —
//!   classifies, then ACTS on the top candidate (used here only for the NIP-05
//!   case, whose HTTP reverse-lookup must be enqueued onto the actor).
//!
//! Both symbols are `#[no_mangle]` in nmp-ffi's (private) `intent_ffi` module
//! and are reached by symbol through the `extern "C"` block below — nmp-ffi is
//! linked as an rlib, so the symbols resolve at link time. The returned C
//! strings are heap-owned by Rust and freed via `nmp_free_string`.
//!
//! ## Recognizer registration
//!
//! The required recognizers are registered for free by
//! `nmp_defaults::register_defaults` (called in `actor::start_nmp_app`):
//!   * `nmp_nip50::register_input_scopes` (always) → `nip50.profiles`,
//!     `nip50.notes`, `nip50.longform`,
//!   * `nmp_nip29::register_input_scopes` (gated on `NmpDefaults::social`, which
//!     defaults to `true`) → `nip29.groups`.
//!
//! No additional composition wiring is needed in hl.
//!
//! ## Free-text dispatch
//!
//! Rather than open one session per `nip50.*` `TextQuery` candidate the
//! classifier emits, the omnibox opens a single multi-kind `NmpApp::open_search`
//! session (`SearchScope::Kinds({0,1,30023})`) under the existing
//! `search::SEARCH_SESSION_ID`. Results stream back through the same typed
//! `N50S` sidecar and the existing `search::apply_search_results` projection, so
//! `AppState::search_results` carries mixed-kind hits the shell buckets by kind.
//! No projection rework, no per-session demultiplexing.

use std::collections::BTreeSet;
use std::ffi::{c_char, CStr, CString};

use nmp_core::substrate::{
    InputIntentClassification, InputIntentRejection, InputIntentRequest, InputIntentTarget,
    InputScopeId, TextSearchTargets,
};
use nmp_ffi::{nmp_free_string, NmpApp};
use nmp_nip50::{SearchRequest, SearchScope as NmpSearchScope, SearchTargets};

use crate::kernel::action::KernelEvent;
use crate::kernel::actor::NmpHandle;
use crate::kernel::effect::Effect;
use crate::kernel::snapshot::OmniboxOutcome;

// ─── NMP input-intent C-ABI (reached by symbol; see module docs) ─────────────

#[allow(improper_ctypes)] // NmpApp is opaque; nmp-ffi uses the same ABI.
extern "C" {
    /// Pure classifier: parses an `InputIntentRequest` JSON, snapshots the app's
    /// registered recognizers, runs `nmp_intent::classify`, returns
    /// `{"ok":true,"classification":…}` (never NULL; a `SecretLike` rejection
    /// carries no copy of the input).
    fn nmp_app_intent_classify(app: *mut NmpApp, request_json: *const c_char) -> *mut c_char;

    /// Classify, then act on the top candidate. Used here only for a NIP-05 top
    /// candidate, whose `ResolveNip05Command` (HTTP reverse lookup) the dispatch
    /// lane enqueues onto the actor. Returns `{"ok":true,"dispatched":…}`.
    fn nmp_app_intent_dispatch(
        app: *mut NmpApp,
        request_json: *const c_char,
        session_id: *const c_char,
    ) -> *mut c_char;
}

/// Wrapper for the `nmp_app_intent_classify` JSON envelope.
#[derive(serde::Deserialize)]
struct ClassifyResponse {
    #[serde(default)]
    ok: bool,
    classification: Option<InputIntentClassification>,
}

/// NIP-29 `Registered` target payload (mirrors `nmp_nip29` `GroupIdentPayload`;
/// kept local so the omnibox does not couple to the exact export path).
#[derive(serde::Deserialize)]
struct GroupIdentPayload {
    host_relay_url: String,
    local_id: String,
}

// ─── Scopes ──────────────────────────────────────────────────────────────────

/// hl's omnibox allow-list: every result class the field accepts.
///
/// `nostr.ref` is the synthetic always-allowed direct-reference scope; the four
/// protocol scopes are served by the recognizers `register_defaults` installs.
fn omnibox_scopes() -> Vec<InputScopeId> {
    vec![
        InputScopeId::nostr_ref(),
        InputScopeId::new("nip50", "profiles"),
        InputScopeId::new("nip50", "notes"),
        InputScopeId::new("nip50", "longform"),
        InputScopeId::new("nip29", "groups"),
    ]
}

/// Build the `InputIntentRequest` for `query` (free-text relays = the active
/// account's preferred search relays).
fn build_request(query: &str) -> InputIntentRequest {
    InputIntentRequest {
        input: query.to_string(),
        scopes: omnibox_scopes(),
        text_targets: TextSearchTargets::UserPreferred,
    }
}

// ─── WRITE side: reduce action ────────────────────────────────────────────────

/// Handle `AppAction::RunOmnibox{query}` — emit `Effect::RunOmnibox`.
///
/// Empty / whitespace-only input is a no-op (D6): the classifier would reject it
/// as `Unparseable`, so we short-circuit before crossing the FFI boundary.
pub(crate) fn reduce_action_run_omnibox(query: String) -> Vec<Effect> {
    let trimmed = query.trim().to_string();
    if trimmed.is_empty() {
        return vec![];
    }
    vec![Effect::RunOmnibox { query: trimmed }]
}

// ─── Pure classification → outcome (unit-tested without a live NmpApp) ─────────

/// Map a classification into the omnibox outcome the shell routes on.
///
/// PURE: no IO, no NmpApp. The effect runner performs the side effects
/// (multi-kind `open_search` for free text, NIP-05 reverse-lookup enqueue) and
/// then emits the outcome this returns.
///
/// The top candidate (frozen-precedence first) decides the branch:
/// * `DirectRef`  → `Navigate{uri}` (shell decodes + routes via its existing
///   nostr-entity navigation),
/// * `Nip05`      → `ResolveNip05{identifier}`,
/// * `Registered` → `OpenGroup{…}` (NIP-29 group ident payload),
/// * `RelayUrl`   → `RelayUrl{url}`,
/// * `TextQuery`  → `FreeText{query}`.
///
/// `Rejection(SecretLike)` → `RejectSecret`; every other rejection → `NoMatch`.
pub(crate) fn classification_to_outcome(
    query: &str,
    classification: InputIntentClassification,
) -> OmniboxOutcome {
    match classification {
        InputIntentClassification::Rejection(InputIntentRejection::SecretLike) => {
            OmniboxOutcome::RejectSecret
        }
        InputIntentClassification::Rejection(_) => OmniboxOutcome::NoMatch,
        InputIntentClassification::Candidates(candidates) => match candidates.into_iter().next() {
            None => OmniboxOutcome::NoMatch,
            Some(candidate) => target_to_outcome(query, candidate.target),
        },
    }
}

fn target_to_outcome(query: &str, target: InputIntentTarget) -> OmniboxOutcome {
    match target {
        InputIntentTarget::DirectRef { uri } => OmniboxOutcome::Navigate { uri },
        InputIntentTarget::Nip05 { identifier } => OmniboxOutcome::ResolveNip05 { identifier },
        InputIntentTarget::RelayUrl { url } => OmniboxOutcome::RelayUrl { url },
        InputIntentTarget::Registered { payload_json } => {
            match serde_json::from_str::<GroupIdentPayload>(&payload_json) {
                Ok(p) => OmniboxOutcome::OpenGroup {
                    host_relay_url: p.host_relay_url,
                    local_id: p.local_id,
                },
                Err(_) => OmniboxOutcome::NoMatch,
            }
        }
        InputIntentTarget::TextQuery { .. } => OmniboxOutcome::FreeText {
            query: query.to_string(),
        },
    }
}

// ─── Effect runner ────────────────────────────────────────────────────────────

/// Execute `Effect::RunOmnibox` — classify `query` through NMP's resolver,
/// perform the side effect for the chosen branch, then emit
/// `KernelEvent::OmniboxResolved(outcome)` so the reducer stores it for the
/// shell to route on the next snapshot.
///
/// No-op if `nmp` is `None` (test mode — the pure `classification_to_outcome`
/// is unit-tested directly).
pub(crate) fn run_effect_run_omnibox(
    query: String,
    nmp: Option<&NmpHandle>,
    tx: &tokio::sync::mpsc::UnboundedSender<crate::kernel::actor::Cmd>,
) {
    let Some(handle) = nmp else { return };
    let app_ptr = handle.ptr.as_ptr();

    let request = build_request(&query);
    let request_json = match serde_json::to_string(&request) {
        Ok(j) => j,
        Err(_) => return,
    };

    let classification = match classify_ffi(app_ptr, &request_json) {
        Some(c) => c,
        None => return,
    };

    let outcome = classification_to_outcome(&query, classification);

    // SAFETY: app_ptr is a valid non-null NmpApp pointer kept alive by the
    // NmpHandle for the full actor lifetime.
    let nmp_ref: &NmpApp = unsafe { handle.ptr.as_ref() };
    match &outcome {
        // Free text → open the single multi-kind NIP-50 search session.
        OmniboxOutcome::FreeText { query } => {
            open_multi_kind_search(nmp_ref, query);
        }
        // NIP-05 → enqueue the HTTP `.well-known/nostr.json` reverse lookup. The
        // dispatch lane re-classifies and routes the (NIP-05 top) candidate to
        // `ResolveNip05Command`; the resolved profile claim arrives reactively.
        OmniboxOutcome::ResolveNip05 { .. } => {
            dispatch_ffi(app_ptr, &request_json);
        }
        // Navigate / OpenGroup / RelayUrl / RejectSecret / NoMatch — no in-core
        // side effect; the shell routes from the emitted outcome.
        _ => {}
    }

    let _ = tx.send(crate::kernel::actor::Cmd::Event(
        KernelEvent::OmniboxResolved(outcome),
    ));
}

/// Open the multi-kind free-text search session (profiles + notes + articles)
/// under the shared `search::SEARCH_SESSION_ID`. Idempotent re-open.
fn open_multi_kind_search(nmp_ref: &NmpApp, query: &str) {
    let kinds: BTreeSet<u32> = [0u32, 1u32, 30023u32].into_iter().collect();
    let request = match SearchRequest::new(
        query,
        NmpSearchScope::Kinds(kinds),
        SearchTargets::UserPreferred,
        None,
    ) {
        Some(r) => r,
        None => return,
    };
    let _key = nmp_ref.open_search(request, super::search::SEARCH_SESSION_ID);
}

/// Call `nmp_app_intent_classify` and parse the classification, freeing the
/// returned C string. Returns `None` on any FFI / parse error (D6).
fn classify_ffi(app_ptr: *mut NmpApp, request_json: &str) -> Option<InputIntentClassification> {
    let c_request = CString::new(request_json).ok()?;
    // SAFETY: app_ptr is valid; c_request is a valid NUL-terminated C string;
    // the returned pointer is heap-owned by Rust and freed below.
    let raw = unsafe { nmp_app_intent_classify(app_ptr, c_request.as_ptr()) };
    if raw.is_null() {
        return None;
    }
    let json = unsafe { CStr::from_ptr(raw) }
        .to_str()
        .ok()
        .map(str::to_owned);
    nmp_free_string(raw);
    let response: ClassifyResponse = serde_json::from_str(&json?).ok()?;
    if !response.ok {
        return None;
    }
    response.classification
}

/// Call `nmp_app_intent_dispatch` to act on the top candidate (NIP-05 enqueue),
/// freeing the returned C string. The result JSON is intentionally discarded —
/// the resolved profile arrives reactively (D6, fire-and-forget).
fn dispatch_ffi(app_ptr: *mut NmpApp, request_json: &str) {
    let Ok(c_request) = CString::new(request_json) else {
        return;
    };
    // Empty session id: the NIP-05 branch ignores it (only `TextQuery` uses it).
    let Ok(c_session) = CString::new("") else {
        return;
    };
    // SAFETY: app_ptr is valid; both C strings are valid NUL-terminated; the
    // returned pointer is heap-owned by Rust and freed immediately below.
    let raw = unsafe { nmp_app_intent_dispatch(app_ptr, c_request.as_ptr(), c_session.as_ptr()) };
    if !raw.is_null() {
        nmp_free_string(raw);
    }
}

// ─── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::action::{AppAction, KernelEvent};
    use crate::kernel::actor::{reduce, Cmd};
    use crate::kernel::app::AppState;
    use crate::kernel::clock::{Clock, ManualClock};
    use nmp_core::substrate::{InputIntentCandidate, InputIntentClassification, InputIntentTarget};

    fn candidates(target: InputIntentTarget) -> InputIntentClassification {
        InputIntentClassification::Candidates(vec![InputIntentCandidate {
            scope: InputScopeId::nostr_ref(),
            target,
        }])
    }

    // OB-T1: free-text classifies to FreeText{query}.
    #[test]
    fn free_text_maps_to_free_text() {
        let c = candidates(InputIntentTarget::TextQuery {
            request_json: "{}".to_string(),
        });
        assert_eq!(
            classification_to_outcome("rust nostr", c),
            OmniboxOutcome::FreeText {
                query: "rust nostr".to_string()
            }
        );
    }

    // OB-T2: a pasted reference classifies to Navigate{uri}.
    #[test]
    fn direct_ref_maps_to_navigate() {
        let c = candidates(InputIntentTarget::DirectRef {
            uri: "nostr:npub1xyz".to_string(),
        });
        assert_eq!(
            classification_to_outcome("npub1xyz", c),
            OmniboxOutcome::Navigate {
                uri: "nostr:npub1xyz".to_string()
            }
        );
    }

    // OB-T3: a NIP-05 identifier classifies to ResolveNip05{identifier}.
    #[test]
    fn nip05_maps_to_resolve() {
        let c = candidates(InputIntentTarget::Nip05 {
            identifier: "jb55@jb55.com".to_string(),
        });
        assert_eq!(
            classification_to_outcome("jb55@jb55.com", c),
            OmniboxOutcome::ResolveNip05 {
                identifier: "jb55@jb55.com".to_string()
            }
        );
    }

    // OB-T4: a NIP-29 group Registered payload classifies to OpenGroup.
    #[test]
    fn group_registered_maps_to_open_group() {
        let payload = r#"{"host_relay_url":"wss://groups.nostr.com","local_id":"abc-123"}"#;
        let c = candidates(InputIntentTarget::Registered {
            payload_json: payload.to_string(),
        });
        assert_eq!(
            classification_to_outcome("groups.nostr.com'abc-123", c),
            OmniboxOutcome::OpenGroup {
                host_relay_url: "wss://groups.nostr.com".to_string(),
                local_id: "abc-123".to_string(),
            }
        );
    }

    // OB-T5: a relay URL classifies to RelayUrl{url}.
    #[test]
    fn relay_url_maps_to_relay_url() {
        let c = candidates(InputIntentTarget::RelayUrl {
            url: "wss://relay.damus.io".to_string(),
        });
        assert_eq!(
            classification_to_outcome("wss://relay.damus.io", c),
            OmniboxOutcome::RelayUrl {
                url: "wss://relay.damus.io".to_string()
            }
        );
    }

    // OB-T6: a secret (nsec) is safely rejected — never echoed.
    #[test]
    fn secret_is_rejected() {
        let c = InputIntentClassification::Rejection(InputIntentRejection::SecretLike);
        assert_eq!(
            classification_to_outcome("nsec1...", c),
            OmniboxOutcome::RejectSecret
        );
    }

    // OB-T7: any other rejection → NoMatch.
    #[test]
    fn unparseable_is_no_match() {
        let c = InputIntentClassification::Rejection(InputIntentRejection::Unparseable);
        assert_eq!(classification_to_outcome("", c), OmniboxOutcome::NoMatch);
    }

    // OB-T8: a malformed group payload degrades to NoMatch (D6).
    #[test]
    fn malformed_group_payload_is_no_match() {
        let c = candidates(InputIntentTarget::Registered {
            payload_json: "not json".to_string(),
        });
        assert_eq!(classification_to_outcome("x", c), OmniboxOutcome::NoMatch);
    }

    // OB-T9: the scope allow-list is exactly the five omnibox classes.
    #[test]
    fn omnibox_scopes_cover_all_classes() {
        let scopes = omnibox_scopes();
        let labels: Vec<String> = scopes.iter().map(InputScopeId::label).collect();
        assert!(labels.contains(&"nostr.ref".to_string()));
        assert!(labels.contains(&"nip50.profiles".to_string()));
        assert!(labels.contains(&"nip50.notes".to_string()));
        assert!(labels.contains(&"nip50.longform".to_string()));
        assert!(labels.contains(&"nip29.groups".to_string()));
    }

    // OB-T10: the request carries the input and UserPreferred targets.
    #[test]
    fn request_carries_input_and_targets() {
        let req = build_request("hello");
        assert_eq!(req.input, "hello");
        assert_eq!(req.text_targets, TextSearchTargets::UserPreferred);
        assert_eq!(req.scopes.len(), 5);
    }

    // OB-T11: empty / whitespace omnibox query emits no effect (D6).
    #[test]
    fn empty_query_is_noop() {
        assert!(reduce_action_run_omnibox("   ".to_string()).is_empty());
    }

    // OB-T12: a non-empty query emits exactly one RunOmnibox effect, trimmed.
    #[test]
    fn nonempty_query_emits_one_effect() {
        let effects = reduce_action_run_omnibox("  npub1xyz ".to_string());
        assert_eq!(effects.len(), 1);
        match &effects[0] {
            Effect::RunOmnibox { query } => assert_eq!(query, "npub1xyz"),
            other => panic!("expected RunOmnibox, got {:?}", other),
        }
    }

    // OB-T13: KernelEvent::OmniboxResolved stores the outcome in AppState.
    #[test]
    fn omnibox_resolved_event_stores_outcome() {
        let mut state = AppState::default();
        let clock = ManualClock::default();
        let now = clock.now_unix_seconds();
        let outcome = OmniboxOutcome::Navigate {
            uri: "nostr:npub1xyz".to_string(),
        };
        reduce(
            &mut state,
            Cmd::Event(KernelEvent::OmniboxResolved(outcome.clone())),
            now,
        );
        assert_eq!(state.omnibox_outcome, Some(outcome));
    }

    // OB-T14: RunOmnibox action routes through the reducer to a RunOmnibox effect.
    #[test]
    fn run_omnibox_action_routes_to_effect() {
        let mut state = AppState::default();
        let clock = ManualClock::default();
        let now = clock.now_unix_seconds();
        let effects = reduce(
            &mut state,
            Cmd::Action(AppAction::RunOmnibox {
                query: "rust".to_string(),
            }),
            now,
        );
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], Effect::RunOmnibox { .. }));
    }

    // OB-T15: logout clears the omnibox outcome.
    #[test]
    fn logout_clears_omnibox_outcome() {
        let mut state = AppState::default();
        let clock = ManualClock::default();
        let now = clock.now_unix_seconds();
        state.omnibox_outcome = Some(OmniboxOutcome::RejectSecret);
        reduce(&mut state, Cmd::Action(AppAction::Logout), now);
        assert_eq!(state.omnibox_outcome, None);
    }
}
