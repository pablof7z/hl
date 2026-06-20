//! Search domain — NIP-50 relay-search projection (slice 4D).
//!
//! ## Responsibilities
//!
//! * **READ** — wrap `SearchResultsProjection::snapshot()` (Family B in-memory
//!   accumulator) under the hl-owned typed snapshot key `"hl.search"`. A thin
//!   `SearchObserver` wrapper implements `KernelEventObserver` by forwarding
//!   accepted events into the projection's `ingest_cache_event`. The registered
//!   closure serialises the snapshot to serde JSON on each tick, which is decoded
//!   in `projections::dispatch_typed_frame` via the `"hl.search"` schema_id arm.
//!   Result: `KernelEvent::SearchResultsUpdated(Vec<SearchHitRow>)` →
//!   stored in `AppState::search_results`.
//!
//! * **WRITE** — `AppAction::RunSearch{query, scope}` → reducer emits
//!   `Effect::RunSearch{query, scope_json, interest_id}` → effect runner:
//!     1. Builds a `SearchRequest` from `query` + `scope` and derives the
//!        `LogicalInterest` via `request.interest_shape()`.
//!     2. Calls `NmpApp::push_interest(interest)` so the planner issues NIP-50
//!        REQs on connected search-capable relays. nmp-nip50 has NO action
//!        namespace — submission is via `push_interest` (confirmed on pinned
//!        nmp b4404159 `crates/nmp-ffi/src/lib.rs:1828`).
//!     3. Replaces the hl-owned `SearchResultsProjection` (registered under
//!        typed snapshot key `"hl.search"`) with a fresh instance seeded from
//!        the new `SearchRequest`, clearing stale results from the previous query.
//!
//! ## NMP search seam (verified at b4404159)
//!
//! `nmp-nip50` crate (`crates/nmp-nip50/src/lib.rs`):
//! - `SearchRequest::new(query, scope, targets, max_hits) -> Option<Self>` —
//!   returns `None` for empty/whitespace queries (built-in D6 no-op gate,
//!   via `bounded_search_query` in nmp-planner).
//! - `SearchRequest::interest_shape() -> InterestShape` — builds the
//!   `InterestShape{kinds, search: Some(query), limit}` for the planner.
//! - `SearchResultsProjection::new(request)` — in-memory accumulator.
//! - `SearchResultsProjection::ingest_cache_event(event)` — ingests a
//!   `KernelEvent` (filters by kind + text match).
//! - `SearchResultsProjection::snapshot() -> SearchResultsSnapshot{hits}`.
//! - `SearchResultsProjection` does NOT implement `KernelEventObserver` —
//!   hl wraps it in `SearchObserver` (below) which does.
//!
//! `NmpApp::push_interest` (`crates/nmp-ffi/src/lib.rs:1828`):
//!   `pub fn push_interest(&self, interest: nmp_core::planner::LogicalInterest)`
//!   — sends `ActorCommand::PushInterest` on the actor channel. Idempotent
//!   for the same `InterestId` (registry replaces the prior entry).
//!
//! ## Bounded results
//!
//! `SearchRequest::new` caps `max_hits` at `HARD_MAX_SEARCH_HITS = 500`; the
//! default is `DEFAULT_MAX_SEARCH_HITS = 200`. The projection's `insert_hit`
//! silently ignores arrivals past the cap — Non-Negotiable #7 / D6.
//!
//! ## Clear on close / logout / identity change
//!
//! `AppState::search_results` is cleared by:
//!   - The `ViewId::Search` close arm in `actor_task` (inline, like
//!     `ReleaseGroupEvents` — avoids a separate effect variant).
//!   - `auth::reduce_event_identity_changed` on `IdentityChanged(None)`.
//!   - `AppAction::Logout` reducer arm.
//!
//! ## Live lane untouched
//!
//! The bespoke `search.rs` local-scan path (`HighlighterCore`) remains active
//! until Phase 7. This module adds the relay-search path ONLY. No double-
//! publish risk — search is read-only (no write action for search hits).

use std::sync::{Arc, Mutex};

use nmp_core::substrate::KernelEvent as NmpKernelEvent;
use nmp_core::KernelEventObserver;
use nmp_ffi::NmpApp;
use nmp_nip50::{
    SearchRequest, SearchResultsProjection, SearchResultsSnapshot, SearchScope as NmpSearchScope,
    SearchTargets, DEFAULT_MAX_SEARCH_HITS,
};
use nmp_planner::{InterestId, InterestLifecycle, InterestScope, LogicalInterest};

use crate::kernel::action::SearchScope;
use crate::kernel::app::AppState;
use crate::kernel::effect::Effect;
use crate::kernel::snapshot::{KernelSearchHitRow, SearchHitRow, SearchSnapshot};

// ─── Schema id ───────────────────────────────────────────────────────────────

/// Typed-snapshot key for the hl-owned search projection.
/// Matched in `projections::dispatch_typed_frame`.
pub(crate) const SEARCH_SCHEMA_ID: &str = "hl.search";

// ─── Stable interest id for the search subscription ─────────────────────────

/// Stable planner `InterestId` for the hl NIP-50 search interest.
///
/// Non-zero (planner sentinel for "unassigned" is 0). Replacing the prior
/// search interest is idempotent because the planner registry replaces on
/// same-id re-push — no subscription leak on repeated `RunSearch` dispatches.
/// Value is arbitrary but stable across runs.
pub(crate) const SEARCH_INTEREST_ID: u64 = 0x0000_484c_5f53; // "HL_S" bytes

// ─── KernelEventObserver wrapper ─────────────────────────────────────────────

/// Thin `KernelEventObserver` wrapper over `SearchResultsProjection`.
///
/// `SearchResultsProjection` at b4404159 does not implement
/// `KernelEventObserver` directly — it exposes `ingest_cache_event` and
/// `ingest_relay_event` separately. `SearchObserver` bridges the two:
/// on every accepted kernel event it forwards via `ingest_cache_event`
/// (relay provenance is already embedded in `KernelEvent.relay_provenance`
/// so the cache path carries full provenance — no information loss).
///
/// Interior mutability: `Mutex<SearchResultsProjection>` — the projection
/// accumulates hits behind a lock. The observer callback is called on the
/// actor thread (cheap: one Mutex lock + one BTreeMap insert or cap check).
pub(crate) struct SearchObserver {
    inner: Mutex<SearchResultsProjection>,
}

impl SearchObserver {
    fn new(projection: SearchResultsProjection) -> Self {
        Self {
            inner: Mutex::new(projection),
        }
    }

    /// Serialise the current snapshot to JSON bytes.
    /// Returns `None` if the lock is poisoned or serialisation fails (D6).
    pub(crate) fn snapshot_json_bytes(&self) -> Option<Vec<u8>> {
        let guard = self.inner.lock().ok()?;
        let snapshot: SearchResultsSnapshot = guard.snapshot();
        serde_json::to_vec(&snapshot).ok()
    }
}

impl KernelEventObserver for SearchObserver {
    fn on_kernel_event(&self, event: &NmpKernelEvent) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.ingest_cache_event(event);
        }
        // Poisoned lock: D6 silent no-op.
    }
}

// ─── READ side: apply decoded snapshot ───────────────────────────────────────

/// Apply a decoded `"hl.search"` JSON payload to `state`.
///
/// Called from `projections::dispatch_typed_frame` when `schema_id ==
/// "hl.search"`. Decodes the serde-JSON representation of
/// `SearchResultsSnapshot { hits: Vec<SearchHit> }` and maps each hit to a
/// `SearchHitRow` stored in `AppState::search_results`.
///
/// Bounded by the projection's `max_hits` cap (default 200 — Non-Negotiable #7).
/// D6: any decode error leaves `AppState::search_results` unchanged.
/// D1: raw fields only — no "X results" count label, no formatted strings.
///
/// Non-blocking — runs on the actor thread.
pub(crate) fn apply_search_results(state: &mut AppState, payload: &[u8]) {
    match serde_json::from_slice::<SearchResultsSnapshot>(payload) {
        Ok(snapshot) => {
            state.search_results = snapshot.hits.into_iter().map(search_hit_to_row).collect();
        }
        Err(e) => {
            tracing::trace!(
                error = %e,
                "search::apply_search_results: JSON decode error — AppState::search_results unchanged (D6)"
            );
        }
    }
}

/// Convert an `nmp_nip50::SearchHit` to the hl `SearchHitRow` representation.
/// Raw protocol data only — no labels, no presentation formatting (D1).
fn search_hit_to_row(hit: nmp_nip50::SearchHit) -> SearchHitRow {
    SearchHitRow {
        id: hit.id,
        author: hit.author,
        kind: hit.kind,
        created_at: hit.created_at,
        content: hit.content,
        tags: hit.tags,
        relay_provenance: hit.relay_provenance,
    }
}

// ─── WRITE side: reduce_action helpers ───────────────────────────────────────

/// Handle `AppAction::RunSearch{query, scope}` — emit `Effect::RunSearch`.
///
/// Empty or whitespace-only queries are a no-op (D6): `SearchRequest::new`
/// uses `bounded_search_query` which trims and returns `None` for blank
/// input. We pre-check here to avoid emitting an effect that silently
/// no-ops in the runner.
///
/// The reducer does NOT speculatively update `AppState::search_results` —
/// the authoritative update arrives via the projection frame on the next
/// snapshot tick after the relay response.
pub(crate) fn reduce_action_run_search(query: String, scope: SearchScope) -> Vec<Effect> {
    let trimmed = query.trim().to_string();
    if trimmed.is_empty() {
        tracing::trace!("search::reduce_action_run_search: empty query — no-op (D6)");
        return vec![];
    }

    let nmp_scope = hl_scope_to_nmp(&scope);
    let scope_json = match serde_json::to_string(&nmp_scope) {
        Ok(j) => j,
        Err(e) => {
            tracing::trace!(
                error = %e,
                "search::reduce_action_run_search: scope serialisation failed — no-op (D6)"
            );
            return vec![];
        }
    };

    vec![Effect::RunSearch {
        query: trimmed,
        scope_json,
        interest_id: SEARCH_INTEREST_ID,
    }]
}

/// Map hl `SearchScope` → `nmp_nip50::SearchScope`.
fn hl_scope_to_nmp(scope: &SearchScope) -> NmpSearchScope {
    match scope {
        SearchScope::Users => NmpSearchScope::Users,
        SearchScope::LongForm => NmpSearchScope::LongForm,
        SearchScope::Notes => {
            use std::collections::BTreeSet;
            // kind:1 short text notes.
            NmpSearchScope::Kinds(BTreeSet::from([1u32]))
        }
    }
}

// ─── Effect runner ────────────────────────────────────────────────────────────

/// Execute `Effect::RunSearch` — push the NIP-50 interest and replace the
/// hl-owned search projection.
///
/// Steps:
///   1. Deserialise `scope_json` back to `NmpSearchScope`.
///   2. Build a `SearchRequest` from `query` + scope. If `SearchRequest::new`
///      returns `None` (blank query after nmp's `bounded_search_query` trim),
///      this is a no-op (D6).
///   3. Derive `InterestShape` via `request.interest_shape()`.
///   4. Call `nmp_ref.push_interest(LogicalInterest { id, scope, shape, .. })`.
///      The planner replaces any prior interest with the same `InterestId`.
///   5. Build a fresh `SearchResultsProjection`, wrap it in `SearchObserver`,
///      register the observer with `nmp_ref.register_event_observer`, and
///      register the typed snapshot projection under key `"hl.search"`.
///
/// No-op if `nmp` is `None` (test mode — tests inject
/// `KernelEvent::SearchResultsUpdated` directly to drive the reducer).
pub(crate) fn run_effect_run_search(
    query: String,
    scope_json: String,
    interest_id: u64,
    nmp: Option<&crate::kernel::actor::NmpHandle>,
) {
    let Some(handle) = nmp else { return };

    let nmp_scope: NmpSearchScope = match serde_json::from_str(&scope_json) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(
                error = %e,
                "search::run_effect_run_search: scope JSON decode failed — no-op (D6)"
            );
            return;
        }
    };

    let request = match SearchRequest::new(
        &query,
        nmp_scope,
        SearchTargets::UserPreferred,
        Some(DEFAULT_MAX_SEARCH_HITS),
    ) {
        Some(r) => r,
        None => {
            tracing::trace!(
                query = %query,
                "search::run_effect_run_search: SearchRequest::new returned None — no-op (D6)"
            );
            return;
        }
    };

    let interest_shape = request.interest_shape();

    let interest = LogicalInterest {
        id: InterestId(interest_id),
        scope: InterestScope::ActiveAccount,
        shape: interest_shape,
        hints: Vec::new(),
        lifecycle: InterestLifecycle::OneShot,
        is_indexer_discovery: false,
    };

    // SAFETY: handle.ptr is a valid non-null NmpApp pointer kept alive by
    // NmpHandle for the full actor lifetime.
    let nmp_ref: &NmpApp = unsafe { handle.ptr.as_ref() };

    // Push the NIP-50 search interest — causes the planner to issue REQs.
    // Idempotent on same InterestId (registry replaces prior entry).
    nmp_ref.push_interest(interest);

    // Build and register a fresh SearchObserver wrapping a new projection.
    let projection = SearchResultsProjection::new(request);
    let observer = Arc::new(SearchObserver::new(projection));

    // Register as event observer — receives every accepted kernel event.
    // The observer filters by kind/text via the projection's interest_shape.
    let _obs_id =
        nmp_ref.register_event_observer(Arc::clone(&observer) as Arc<dyn KernelEventObserver>);

    // Register the typed snapshot projection under "hl.search".
    // On each NMP snapshot tick the closure calls snapshot_json_bytes(),
    // serialises the current hits to JSON, and the frame is decoded by
    // `dispatch_typed_frame` → `KernelEvent::SearchResultsUpdated`.
    // Replacing the key re-registers with a fresh projection (old closure Arc
    // is dropped after registration; the projection behind it is abandoned).
    let snapshot_observer = Arc::clone(&observer);
    nmp_ref.register_typed_snapshot_projection(SEARCH_SCHEMA_ID, move || {
        let payload = snapshot_observer.snapshot_json_bytes()?;
        Some(nmp_core::TypedProjectionData {
            key: SEARCH_SCHEMA_ID.to_string(),
            schema_id: SEARCH_SCHEMA_ID.to_string(),
            schema_version: 1,
            file_identifier: String::new(),
            payload,
            ..Default::default()
        })
    });
}

// ─── Snapshot projection ─────────────────────────────────────────────────────

/// Project the `ViewId::Search` snapshot from `AppState::search_results`.
///
/// Converts internal `SearchHitRow` to the FFI `KernelSearchHitRow`, carrying the
/// raw NIP-01 `tags` (uniffi supports `Vec<Vec<String>>`) so Swift can bucket hits
/// by `kind` and hydrate per-kind result cards. D1: no count labels, no formatted
/// strings — raw protocol data only.
pub(crate) fn project_search_snapshot(
    state: &AppState,
) -> Option<crate::kernel::snapshot::ViewSnapshot> {
    let hits: Vec<KernelSearchHitRow> = state
        .search_results
        .iter()
        .map(|r| KernelSearchHitRow {
            id: r.id.clone(),
            author: r.author.clone(),
            kind: r.kind,
            created_at: r.created_at,
            content: r.content.clone(),
            tags: r.tags.clone(),
            relay_provenance: r.relay_provenance.clone(),
        })
        .collect();

    Some(crate::kernel::snapshot::ViewSnapshot::Search(
        SearchSnapshot { hits },
    ))
}

// ─── Lifecycle (view open / close) ───────────────────────────────────────────

/// Lifecycle effects for `ViewId::Search` open.
///
/// No registration needed on view open — the `SearchResultsProjection` is
/// wired when `AppAction::RunSearch` is dispatched. No-op provided for
/// symmetry with other domain lifecycle hooks.
pub(crate) fn lifecycle_effects_for_view_open(_id: &crate::kernel::view::ViewId) -> Vec<Effect> {
    vec![]
}

/// Lifecycle effects for `ViewId::Search` close.
///
/// Signals that `AppState::search_results` should be cleared to bound memory
/// between search sessions. Handled INLINE in `actor_task` (same pattern as
/// `ReleaseGroupEvents` for room_home — the clear requires mutating `AppState`
/// directly, which `run_effect` cannot do).
///
/// Returns an empty vec; the actor_task `Cmd::CloseView(ViewId::Search)` arm
/// clears `state.search_results` directly.
pub(crate) fn lifecycle_effects_for_view_close(_id: &crate::kernel::view::ViewId) -> Vec<Effect> {
    vec![]
}

// ─── Unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::action::{AppAction, KernelEvent, SearchScope};
    use crate::kernel::actor::{reduce, Cmd};
    use crate::kernel::clock::{Clock, ManualClock};
    use crate::kernel::effect::Effect;
    use crate::kernel::snapshot::SearchHitRow;

    fn make_state() -> AppState {
        AppState::default()
    }

    fn step(state: &mut AppState, clock: &ManualClock, cmd: Cmd) -> Vec<Effect> {
        let now = clock.now_unix_seconds();
        reduce(state, cmd, now)
    }

    // 4D-T1: run_search_pushes_search_interest
    //
    // AppAction::RunSearch{query, scope} with a non-empty query must produce
    // exactly one Effect::RunSearch with the trimmed query, a valid scope_json,
    // and a non-zero interest_id.
    #[test]
    fn run_search_pushes_search_interest() {
        let mut state = make_state();
        let clock = ManualClock::default();

        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::RunSearch {
                query: "  nostr rust  ".to_string(),
                scope: SearchScope::LongForm,
            }),
        );

        assert_eq!(effects.len(), 1, "RunSearch must emit exactly one effect");
        match &effects[0] {
            Effect::RunSearch {
                query,
                scope_json,
                interest_id,
            } => {
                assert_eq!(query, "nostr rust", "query must be trimmed");
                let scope: NmpSearchScope =
                    serde_json::from_str(scope_json).expect("scope_json must be valid JSON");
                assert_eq!(
                    scope,
                    NmpSearchScope::LongForm,
                    "scope must map to LongForm"
                );
                assert_ne!(*interest_id, 0, "interest_id must be non-zero");
            }
            other => panic!("expected Effect::RunSearch, got {:?}", other),
        }
    }

    // 4D-T2: search_results_frame_updates_state_raw
    //
    // Injecting KernelEvent::SearchResultsUpdated with raw hit rows must store
    // them in AppState::search_results. Raw fields only — no labels (D1).
    #[test]
    fn search_results_frame_updates_state_raw() {
        let mut state = make_state();
        let clock = ManualClock::default();

        assert!(
            state.search_results.is_empty(),
            "search_results must start empty"
        );

        let hit = SearchHitRow {
            id: "aabb000000000000000000000000000000000000000000000000000000000001".to_string(),
            author: "dead000000000000000000000000000000000000000000000000000000000001".to_string(),
            kind: 30023,
            created_at: 1_700_000_000,
            content: "test content".to_string(),
            tags: vec![vec!["t".to_string(), "nostr".to_string()]],
            relay_provenance: vec![],
        };

        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::SearchResultsUpdated(vec![hit.clone()])),
        );

        assert_eq!(state.search_results.len(), 1, "one hit must be stored");
        assert_eq!(&state.search_results[0], &hit);
    }

    // 4D-T3: search_snapshot_no_result_count_labels
    //
    // apply_search_results must store raw protocol data only — no "X results"
    // count labels, no formatted strings (D1).
    #[test]
    fn search_snapshot_no_result_count_labels() {
        let mut state = make_state();

        let snapshot = serde_json::json!({
            "hits": [
                {
                    "id": "aabb000000000000000000000000000000000000000000000000000000000001",
                    "author": "dead000000000000000000000000000000000000000000000000000000000001",
                    "kind": 30023,
                    "created_at": 1700000000u64,
                    "content": "article content",
                    "tags": [["t", "nostr"]],
                    "relay_provenance": [],
                    "source": "Cache"
                }
            ]
        });
        let payload = serde_json::to_vec(&snapshot).unwrap();

        apply_search_results(&mut state, &payload);

        assert_eq!(state.search_results.len(), 1);
        let row = &state.search_results[0];

        // D1: the content field is raw — no "X results" label injected.
        assert_eq!(row.content, "article content", "content must be raw (D1)");
        // The raw author field is a hex pubkey, not a formatted npub.
        assert!(
            !row.author.contains("npub"),
            "author must be raw hex pubkey, not formatted npub (D1)"
        );
    }

    // 4D-T4: search_results_bounded
    //
    // apply_search_results must accept a payload up to the cap and store all
    // entries — the bounding is enforced by the projection, not by apply.
    #[test]
    fn search_results_bounded() {
        let mut state = make_state();

        let hits: Vec<serde_json::Value> = (0u64..3)
            .map(|i| {
                serde_json::json!({
                    "id": format!("{:064x}", i),
                    "author": format!("{:064x}", i + 100),
                    "kind": 30023u32,
                    "created_at": 1_700_000_000u64 + i,
                    "content": format!("content {}", i),
                    "tags": [],
                    "relay_provenance": [],
                    "source": "Cache"
                })
            })
            .collect();

        let payload = serde_json::to_vec(&serde_json::json!({ "hits": hits })).unwrap();
        apply_search_results(&mut state, &payload);

        assert_eq!(state.search_results.len(), 3, "all 3 hits must be stored");
    }

    // 4D-T5: malformed_search_payload_is_noop
    //
    // apply_search_results with garbage bytes must not panic or corrupt state (D6).
    #[test]
    fn malformed_search_payload_is_noop() {
        let mut state = make_state();
        state.search_results = vec![SearchHitRow {
            id: "existing".to_string(),
            author: "dead000000000000000000000000000000000000000000000000000000000001".to_string(),
            kind: 1,
            created_at: 0,
            content: "existing".to_string(),
            tags: vec![],
            relay_provenance: vec![],
        }];

        apply_search_results(&mut state, b"NOT VALID JSON AT ALL \x00\xFF");

        assert_eq!(
            state.search_results.len(),
            1,
            "malformed payload must leave AppState::search_results unchanged (D6)"
        );
    }

    // 4D-T6: empty_query_run_search_is_noop
    //
    // AppAction::RunSearch with an empty / whitespace-only query must produce
    // no effects (D6).
    #[test]
    fn empty_query_run_search_is_noop() {
        let mut state = make_state();
        let clock = ManualClock::default();

        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::RunSearch {
                query: "   ".to_string(),
                scope: SearchScope::LongForm,
            }),
        );

        assert!(
            effects.is_empty(),
            "whitespace-only query must produce no effects (D6)"
        );
    }

    // 4D-T7: dispatch_returns_unit
    //
    // The dispatch path returns a Vec<Effect> (never a Result) — confirmed by
    // the type signature. This test exercises the full reducer round-trip.
    #[test]
    fn dispatch_returns_unit() {
        let mut state = make_state();
        let clock = ManualClock::default();

        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::RunSearch {
                query: "hello".to_string(),
                scope: SearchScope::Users,
            }),
        );

        assert_eq!(effects.len(), 1, "valid query must emit one effect");
        assert!(
            matches!(effects[0], Effect::RunSearch { .. }),
            "effect must be RunSearch"
        );
    }

    // 4D-T8: search_results_cleared_on_logout
    //
    // AppAction::Logout must wipe AppState::search_results.
    #[test]
    fn search_results_cleared_on_logout() {
        let mut state = make_state();
        let clock = ManualClock::default();

        state.search_results = vec![SearchHitRow {
            id: "aabb".to_string(),
            author: "dead000000000000000000000000000000000000000000000000000000000001".to_string(),
            kind: 30023,
            created_at: 0,
            content: "something".to_string(),
            tags: vec![],
            relay_provenance: vec![],
        }];

        step(&mut state, &clock, Cmd::Action(AppAction::Logout));

        assert!(
            state.search_results.is_empty(),
            "search_results must be empty after Logout"
        );
    }

    // 4D-T9: search_results_cleared_on_identity_changed_none
    //
    // KernelEvent::IdentityChanged(None) must clear AppState::search_results.
    #[test]
    fn search_results_cleared_on_identity_changed_none() {
        let mut state = make_state();
        let clock = ManualClock::default();

        state.search_results = vec![SearchHitRow {
            id: "ccdd".to_string(),
            author: "dead000000000000000000000000000000000000000000000000000000000001".to_string(),
            kind: 1,
            created_at: 0,
            content: "something".to_string(),
            tags: vec![],
            relay_provenance: vec![],
        }];

        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(None)),
        );

        assert!(
            state.search_results.is_empty(),
            "search_results must be empty after IdentityChanged(None)"
        );
    }

    // Phase 7: project_search_snapshot must carry the raw NIP-01 tags verbatim
    // onto KernelSearchHitRow so Swift can bucket hits by kind + extract per-kind
    // card fields (article title/summary/image/d, highlight a/e/context, etc.).
    #[test]
    fn search_snapshot_carries_raw_tags() {
        let mut state = make_state();
        let tags = vec![
            vec!["d".to_string(), "my-article".to_string()],
            vec!["title".to_string(), "My Article".to_string()],
            vec![
                "image".to_string(),
                "https://example.com/hero.jpg".to_string(),
            ],
        ];
        state.search_results = vec![SearchHitRow {
            id: "aa".to_string(),
            author: "dead000000000000000000000000000000000000000000000000000000000001".to_string(),
            kind: 30023,
            created_at: 1,
            content: "body".to_string(),
            tags: tags.clone(),
            relay_provenance: vec![],
        }];

        let snap = project_search_snapshot(&state).expect("snapshot");
        let crate::kernel::snapshot::ViewSnapshot::Search(s) = snap else {
            panic!("expected Search snapshot");
        };
        assert_eq!(s.hits.len(), 1);
        assert_eq!(
            s.hits[0].tags, tags,
            "raw NIP-01 tags must flow through to KernelSearchHitRow verbatim"
        );
    }
}
