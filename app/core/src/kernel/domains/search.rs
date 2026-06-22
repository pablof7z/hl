//! Search domain — NIP-50 relay-search via NMP's higher-order `open_search`.
//!
//! ## Responsibilities
//!
//! * **READ** — NMP's `open_search` registers a typed `N50S` sidecar under the
//!   session key `nmp.nip50.search.<session_id>` with schema id
//!   [`nmp_nip50::SEARCH_RESULTS_SCHEMA_ID`] (`"nmp.nip50.search"`). On each
//!   snapshot tick the frame is decoded in `projections::dispatch_typed_frame`
//!   via the schema-id arm and stored as raw `SearchHitRow` items in
//!   `AppState::search_results`.
//!
//! * **WRITE** — `AppAction::RunSearch{query, scope}` → reducer emits
//!   `Effect::RunSearch{query, scope_json}` → effect runner:
//!     1. Builds a [`SearchRequest`] from `query` + `scope`.
//!     2. Calls [`NmpApp::open_search`] with a stable [`SEARCH_SESSION_ID`].
//!        NMP resolves `UserPreferred` relays from the installed
//!        `SearchRelaySource` (kind:10007 selection), runs the #1827 cache-FTS
//!        scope to seed results, registers the per-relay pinned interests, and
//!        owns the result projection + typed sidecar. Re-opening the same
//!        session id is idempotent (NMP tears the prior session down first).
//!
//! ## NMP search seam (verified at 6d5671f2)
//!
//! `nmp-nip50` crate:
//! - `SearchRequest::new(query, scope, targets, max_hits) -> Option<Self>` —
//!   returns `None` for empty/whitespace queries (`bounded_search_query`, D6).
//! - `SearchScope::{Users, LongForm, Kinds(BTreeSet<u32>), Custom(_)}`.
//! - `SearchTargets::{UserPreferred, AppDefault, Explicit(Vec<String>)}`.
//! - `SearchHit { id, author, kind, created_at, content, tags, relay_provenance,
//!   source: SearchHitSource }` — `source` is dropped at the hl boundary for
//!   the first cut (not threaded through UniFFI; see follow-up).
//! - `decode_search_results_snapshot(&[u8]) -> Result<SearchResultsSnapshot, _>`
//!   decodes the typed `N50S` FlatBuffers sidecar payload.
//!
//! `nmp-ffi`:
//! - `NmpApp::open_search(request, session_id) -> String` (snapshot key).
//! - `NmpApp::close_search(session_id)` — tears down the session (idempotent).
//!
//! ## Bounded results
//!
//! `SearchRequest::new` caps `max_hits` at `HARD_MAX_SEARCH_HITS = 500`; the
//! default is `DEFAULT_MAX_SEARCH_HITS = 200`. NMP's projection bounds the hit
//! set — Non-Negotiable #7 / D6.
//!
//! ## Clear on close / logout / identity change
//!
//! `AppState::search_results` is cleared by:
//!   - The `ViewId::Search` close arm in `actor_task` (inline; also calls
//!     `NmpApp::close_search` to tear the NMP session down).
//!   - `auth::reduce_event_identity_changed` on `IdentityChanged(None)`.
//!   - `AppAction::Logout` reducer arm.
//!
//! ## Live lane untouched
//!
//! The bespoke `search.rs` local-scan path (`HighlighterCore`) remains active
//! until Phase 7. This module adds the relay-search path ONLY. No double-
//! publish risk — search is read-only (no write action for search hits).

use nmp_ffi::NmpApp;
use nmp_nip50::{
    decode_search_results_snapshot, SearchRequest, SearchScope as NmpSearchScope, SearchTargets,
    DEFAULT_MAX_SEARCH_HITS, SEARCH_RESULTS_SCHEMA_ID,
};

use crate::kernel::action::SearchScope;
use crate::kernel::app::AppState;
use crate::kernel::effect::Effect;
use crate::kernel::snapshot::{KernelSearchHitRow, SearchHitRow, SearchSnapshot};

// ─── Schema id ───────────────────────────────────────────────────────────────

/// Schema id of NMP's typed `N50S` search-results sidecar
/// (`nmp_nip50::SEARCH_RESULTS_SCHEMA_ID` = `"nmp.nip50.search"`).
/// Matched in `projections::dispatch_typed_frame` against `proj.schema_id`.
pub(crate) const SEARCH_SCHEMA_ID: &str = SEARCH_RESULTS_SCHEMA_ID;

// ─── Stable session id for the search subscription ───────────────────────────

/// Stable `open_search` session id for the hl NIP-50 search session.
///
/// Re-opening the same session is idempotent — NMP tears the prior session
/// down before re-opening, so repeated `RunSearch` dispatches never leak a
/// subscription. Value is arbitrary but stable across runs.
pub(crate) const SEARCH_SESSION_ID: &str = "hl_search";

// ─── READ side: apply decoded snapshot ───────────────────────────────────────

/// Apply a decoded `N50S` search-results sidecar payload to `state`.
///
/// Called from `projections::dispatch_typed_frame` when `schema_id ==
/// SEARCH_SCHEMA_ID`. Decodes the typed FlatBuffers `SearchResultsSnapshot`
/// and maps each hit to a `SearchHitRow` stored in `AppState::search_results`.
///
/// Bounded by NMP's projection `max_hits` cap (default 200 — Non-Negotiable #7).
/// D6: any decode error leaves `AppState::search_results` unchanged.
/// D1: raw fields only — no "X results" count label, no formatted strings.
///
/// Non-blocking — runs on the actor thread.
pub(crate) fn apply_search_results(state: &mut AppState, payload: &[u8]) {
    match decode_search_results_snapshot(payload) {
        Ok(snapshot) => {
            state.search_results = snapshot.hits.into_iter().map(search_hit_to_row).collect();
        }
        Err(e) => {
            tracing::trace!(
                error = %e,
                "search::apply_search_results: N50S decode error — AppState::search_results unchanged (D6)"
            );
        }
    }
}

/// Convert an `nmp_nip50::SearchHit` to the hl `SearchHitRow` representation.
/// Raw protocol data only — no labels, no presentation formatting (D1).
fn search_hit_to_row(hit: nmp_nip50::SearchHit) -> SearchHitRow {
    // First cut: drop `source` (Cache vs Relay provenance). Threading it
    // through to the UniFFI `KernelSearchHitRow` is a follow-up.
    let _ = hit.source;
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

/// Execute `Effect::RunSearch` — open a NIP-50 search session via NMP's
/// higher-order `open_search`.
///
/// Steps:
///   1. Deserialise `scope_json` back to `NmpSearchScope`.
///   2. Build a `SearchRequest` from `query` + scope. If `SearchRequest::new`
///      returns `None` (blank query after nmp's `bounded_search_query` trim),
///      this is a no-op (D6).
///   3. Call `nmp_ref.open_search(request, SEARCH_SESSION_ID)`. NMP resolves
///      `UserPreferred` relays (kind:10007), runs the #1827 cache-FTS scope to
///      seed results, opens the per-relay pinned interests, and owns the result
///      projection + typed `N50S` sidecar. Re-opening the same session id is
///      idempotent (NMP tears the prior session down first).
///
/// No-op if `nmp` is `None` (test mode — tests drive `apply_search_results`
/// and the reducer directly).
pub(crate) fn run_effect_run_search(
    query: String,
    scope_json: String,
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

    // SAFETY: handle.ptr is a valid non-null NmpApp pointer kept alive by
    // NmpHandle for the full actor lifetime.
    let nmp_ref: &NmpApp = unsafe { handle.ptr.as_ref() };

    // Open the NIP-50 search session. NMP owns the relay resolution, cache-FTS
    // seed, per-relay pinned interests, result projection, and typed `N50S`
    // sidecar (registered under `nmp.nip50.search.<session_id>`). Idempotent
    // re-open on the same session id (prior session is torn down first).
    let _key = nmp_ref.open_search(request, SEARCH_SESSION_ID);
}

// ─── Snapshot projection ─────────────────────────────────────────────────────

/// Project the `ViewId::Search` snapshot from `AppState::search_results`.
///
/// Converts internal `SearchHitRow` (with `tags: Vec<Vec<String>>`) to the
/// FFI-compatible `KernelSearchHitRow` (tags omitted — uniffi does not support
/// `Vec<Vec<String>>`). D1: no count labels, no formatted strings.
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
/// No registration needed on view open — the NMP search session is opened
/// (`NmpApp::open_search`) when `AppAction::RunSearch` is dispatched. No-op
/// provided for symmetry with other domain lifecycle hooks.
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
    // exactly one Effect::RunSearch with the trimmed query and a valid
    // scope_json (the open_search session id is a stable const, not a field).
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
            Effect::RunSearch { query, scope_json } => {
                assert_eq!(query, "nostr rust", "query must be trimmed");
                let scope: NmpSearchScope =
                    serde_json::from_str(scope_json).expect("scope_json must be valid JSON");
                assert_eq!(
                    scope,
                    NmpSearchScope::LongForm,
                    "scope must map to LongForm"
                );
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
        use nmp_nip50::{
            encode_search_results_snapshot, SearchHit, SearchHitSource, SearchResultsSnapshot,
        };

        let mut state = make_state();

        let snapshot = SearchResultsSnapshot {
            hits: vec![SearchHit {
                id: "aabb000000000000000000000000000000000000000000000000000000000001"
                    .to_string(),
                author: "dead000000000000000000000000000000000000000000000000000000000001"
                    .to_string(),
                kind: 30023,
                created_at: 1_700_000_000,
                content: "article content".to_string(),
                tags: vec![vec!["t".to_string(), "nostr".to_string()]],
                relay_provenance: vec![],
                source: SearchHitSource::Cache,
            }],
        };
        let payload = encode_search_results_snapshot(&snapshot);

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
        use nmp_nip50::{
            encode_search_results_snapshot, SearchHit, SearchHitSource, SearchResultsSnapshot,
        };

        let mut state = make_state();

        let hits: Vec<SearchHit> = (0u64..3)
            .map(|i| SearchHit {
                id: format!("{:064x}", i),
                author: format!("{:064x}", i + 100),
                kind: 30023,
                created_at: 1_700_000_000 + i,
                content: format!("content {}", i),
                tags: vec![],
                relay_provenance: vec![],
                source: SearchHitSource::Cache,
            })
            .collect();

        let payload = encode_search_results_snapshot(&SearchResultsSnapshot { hits });
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
}
