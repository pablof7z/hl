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
//!        nmp d16aea608 `crates/nmp-ffi/src/lib.rs:1852`).
//!     3. Replaces the hl-owned `SearchResultsProjection` (registered under
//!        typed snapshot key `"hl.search"`) with a fresh instance seeded from
//!        the new `SearchRequest`, clearing stale results from the previous query.
//!
//! ## NMP search seam (verified at d16aea608)
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
//! `NmpApp::push_interest` (`crates/nmp-ffi/src/lib.rs:1852`):
//!   `pub fn push_interest(&self, interest: nmp_planner::LogicalInterest)`
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
//! ## Profiles bucket — kernel-owned local kind:0 scan (Phase 7, #1697)
//!
//! The people/profiles bucket is driven by a LOCAL scan of the kernel-owned
//! `EventStore`, NOT by relay search: the production NIP-50 interest runs the
//! articles/highlights scope (kinds 9802/30023) and never returns kind:0, so
//! relay hits cannot populate it. `run_effect_run_search` calls
//! `scan_local_profiles` (`EventStore::query(KindTime{kinds:[0]})` via the
//! `event_store_handle()` slot), decodes each kind:0 into a `ProfileSearchRow`,
//! and ships the matches back as `KernelEvent::ProfileSearchScanned` →
//! `merge_profile_search_rows`. This REPLACES the bespoke
//! `crate::search::search_profiles` nostrdb scan: the profiles bucket now has
//! exactly ONE production source (D4). `search_profiles` survives only as the
//! `#[cfg(test)]` parity oracle for `parity_profile_scan_matches_bespoke_algorithm`.
//!
//! Search is read-only (no write action for search hits) — no double-publish risk.

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
use crate::kernel::snapshot::{
    CommunitySearchRow, KernelSearchHitRow, SearchHitRow, SearchSnapshot,
};

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
/// `SearchResultsProjection` at d16aea608 does not implement
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

/// Bounded cap for the local community-search result list.
///
/// Mirrors `SEARCH_COMMUNITY_RESULTS_LIMIT` from `crate::search` (both = 20).
pub(crate) const COMMUNITY_SEARCH_CAP: usize = 20;

/// Handle `AppAction::RunSearch{query, scope}` — store trimmed query, emit
/// `Effect::RunSearch`, and optionally warm discovery (D8).
///
/// Empty or whitespace-only queries are a no-op (D6): `SearchRequest::new`
/// uses `bounded_search_query` which trims and returns `None` for blank
/// input. We pre-check here to avoid emitting an effect that silently
/// no-ops in the runner.
///
/// State side-effect: stores the trimmed query in `AppState::search_query` so
/// that `project_search_snapshot` can compute the local community bucket on
/// the next snapshot pass without a separate action.
///
/// D8 discovery warm-up: if `discovered_groups` is empty on a non-empty query
/// AND `room_policy.discovery_relay` is non-empty, emits the same two effects
/// as `AppAction::StartRoomDiscovery`. This reuses the existing
/// `DiscoveredGroupsProjection` via `"nmp.nip29.discover"` — no new interest.
/// The discovered-groups sidecar later updates `AppState::discovered_groups`;
/// the Search snapshot recomputes the community bucket from state.
///
/// The reducer does NOT speculatively update `AppState::search_results` —
/// the authoritative update arrives via the projection frame on the next
/// snapshot tick after the relay response.
pub(crate) fn reduce_action_run_search(
    state: &mut AppState,
    query: String,
    scope: SearchScope,
) -> Vec<Effect> {
    let trimmed = query.trim().to_string();
    if trimmed.is_empty() {
        tracing::trace!("search::reduce_action_run_search: empty query — no-op (D6)");
        return vec![];
    }

    // ── Bounded-by-active-view (D5): drop the previous query's profile cache ──
    // The profile_search_cache holds the kind:0 rows scanned for the PRIOR
    // query. On a query replacement it is stale and must not bleed into the new
    // query's people bucket (it is rebuilt by the local kind:0 scan this
    // RunSearch triggers). Clear only when the query actually changes so a
    // re-run of the same query keeps its freshly-scanned rows.
    if state.search_query != trimmed {
        state.profile_search_cache.clear();
    }

    // ── Generation token (D5 active-view bounding, race guard) ───────────────
    // Every new RunSearch supersedes any in-flight async kind:0 scan from a
    // prior query. Bumping the generation here means any ProfileSearchScanned
    // event already in-flight (or arriving after a CloseView) will be dropped
    // by the reducer's generation-check. The effect runner captures this value
    // and includes it in the emitted KernelEvent::ProfileSearchScanned.
    state.profile_search_generation = state.profile_search_generation.wrapping_add(1);

    // Store the trimmed query so project_search_snapshot can compute the
    // local community bucket from state (no second action required).
    state.search_query = trimmed.clone();

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

    let mut effects = vec![Effect::RunSearch {
        query: trimmed,
        scope_json,
        interest_id: SEARCH_INTEREST_ID,
        generation: state.profile_search_generation,
    }];

    // D8 discovery warm-up: if we have no discovered groups yet and the
    // discovery relay is configured, kick off discovery so the community
    // bucket can be populated on the next snapshot pass. Reuses
    // `StartRoomDiscovery` effects — no new interest or NMP work required.
    let discovery_relay = state.room_policy.discovery_relay.clone();
    if state.discovered_groups.is_empty() && !discovery_relay.is_empty() {
        tracing::trace!(
            relay = %discovery_relay,
            "search::reduce_action_run_search: discovered_groups empty — warming discovery (D8)"
        );
        effects.extend(
            crate::kernel::domains::discovery::reduce_action_start_room_discovery(discovery_relay),
        );
    }

    effects
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
        SearchScope::ArticlesAndHighlights => {
            use std::collections::BTreeSet;
            // kind:30023 articles + kind:9802 highlights in one query — the
            // unified search screen buckets the mixed hits by kind Swift-side.
            NmpSearchScope::Kinds(BTreeSet::from([9802u32, 30023u32]))
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
    generation: u64,
    nmp: Option<&crate::kernel::actor::NmpHandle>,
    tx: &tokio::sync::mpsc::UnboundedSender<crate::kernel::actor::Cmd>,
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

    // ── Profiles bucket: local kind:0 store scan (D4 single source) ───────────
    // The NIP-50 interest above runs the articles/highlights scope, so kind:0
    // never arrives from relays — the people bucket must be driven by a LOCAL
    // scan of the kernel-owned EventStore. This replaces the bespoke
    // `crate::search::search_profiles` nostrdb scan: there is now ONE production
    // source for the profiles bucket. Fire the scan and send the decoded rows
    // back as `KernelEvent::ProfileSearchScanned` for the reducer to upsert
    // (the runner cannot mutate AppState directly).
    let rows = scan_local_profiles(nmp_ref, &query, PROFILE_SEARCH_CACHE_SCAN_LIMIT);
    if !rows.is_empty() {
        let _ = tx.send(crate::kernel::actor::Cmd::Event(
            crate::kernel::action::KernelEvent::ProfileSearchScanned { generation, rows },
        ));
    }
}

/// Upper bound on the local kind:0 store scan candidate set.
///
/// Higher than `PROFILE_SEARCH_CAP` (the final result cap) so substring matches
/// still surface when the candidate set is dominated by non-matching profiles —
/// mirrors the bespoke `crate::search::search_profiles` scan-cap intent. Bounds
/// the per-search work + the rows shipped to the reducer (Non-Negotiable #7).
pub(crate) const PROFILE_SEARCH_CACHE_SCAN_LIMIT: usize = 2048;

/// Scan the kernel-owned `EventStore` for kind:0 profiles matching `query`.
///
/// Reads the published `EventStore` via `query(StoreQuery::KindTime { kinds: [0], … })` (newest-
/// first), decodes each into a `ProfileSearchRow`, and keeps only those whose
/// name/display_name/nip05/about contains the query (case-insensitive). The
/// store is newest-first, so the first row seen per pubkey is the freshest
/// replaceable kind:0 — later (older) duplicates are skipped (same "newest
/// wins" dedup as the bespoke scan).
///
/// Lock discipline mirrors `event_by_id_from_store`: clone the store `Arc`
/// under the slot lock, release the lock, then run the read against the clone.
/// Returns an empty vec on a blank query, an unpublished store, a poisoned lock,
/// or a store error (D6 — degrades gracefully, never panics).
fn scan_local_profiles(
    nmp_ref: &NmpApp,
    query: &str,
    limit: usize,
) -> Vec<crate::kernel::snapshot::ProfileSearchRow> {
    use nmp_store::{EventStore, StoreQuery};

    let q = query.trim();
    if q.is_empty() {
        return Vec::new();
    }

    // Clone the Arc<dyn EventStore> under the slot lock, then release it (D6:
    // any None / poisoned lock yields an empty scan).
    let slot = nmp_ref.event_store_handle();
    let store: std::sync::Arc<dyn EventStore> = {
        let Ok(guard) = slot.lock() else {
            return Vec::new();
        };
        match guard.clone() {
            Some(store) => store,
            None => return Vec::new(),
        }
    };

    let stored = match store.query(
        &StoreQuery::KindTime {
            kinds: vec![0],
            since: None,
            until: None,
        },
        limit,
    ) {
        Ok(events) => events,
        Err(_) => return Vec::new(),
    };

    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut rows: Vec<crate::kernel::snapshot::ProfileSearchRow> = Vec::new();
    for ev in &stored {
        let raw = &ev.raw;
        // Newest-first: keep only the first (freshest) kind:0 per pubkey.
        if !seen.insert(raw.pubkey.clone()) {
            continue;
        }
        let Some(row) = profile_search_row_from_parts(&raw.pubkey, raw.created_at, &raw.content)
        else {
            continue;
        };
        let matches = profile_contains_ci(&row.name, q)
            || profile_contains_ci(&row.display_name, q)
            || profile_contains_ci(&row.nip05, q)
            || profile_contains_ci(&row.about, q);
        if matches {
            rows.push(row);
        }
    }
    rows
}

// ─── Snapshot projection ─────────────────────────────────────────────────────

/// Project the `ViewId::Search` snapshot from `AppState`.
///
/// Converts internal `SearchHitRow` to the FFI `KernelSearchHitRow`, carrying the
/// raw NIP-01 `tags` (uniffi supports `Vec<Vec<String>>`) so Swift can bucket hits
/// by `kind` and hydrate per-kind result cards. Also computes the local community
/// bucket from `AppState::discovered_groups` + `AppState::communities` using the
/// stored `AppState::search_query`. D1: no count labels, no formatted strings.
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

    let communities = project_community_search_rows(
        &state.discovered_groups,
        &state.communities,
        &state.search_query,
        COMMUNITY_SEARCH_CAP,
    );

    // Highlights bucket: decode the kind:9802 hits via the SHARED
    // `decode_highlight_row` (reuse — same NIP-84/NIP-73 enrichment as the
    // highlight feed / article-reader overlay), so Swift renders the Highlights
    // search bucket without re-parsing kind:9802 tags. `SearchHitRow` carries the
    // same raw fields as `NmpKernelEvent`, so map 1:1; preserve hit order.
    let highlights: Vec<crate::kernel::snapshot::HighlightRow> = state
        .search_results
        .iter()
        .filter(|r| r.kind == 9802)
        .filter_map(|r| {
            crate::kernel::domains::highlight_feed::decode_highlight_row(&NmpKernelEvent {
                id: r.id.clone(),
                author: r.author.clone(),
                kind: r.kind,
                created_at: r.created_at,
                tags: r.tags.clone(),
                content: r.content.clone(),
                relay_provenance: r.relay_provenance.clone(),
            })
        })
        .collect();

    // Profiles bucket: local scan over `AppState::profile_search_cache` using
    // the same matching/ranking algorithm as `crate::search::search_profiles`.
    // Cache is populated from kind:0 hits in `SearchResultsUpdated` (actor.rs).
    let profiles = project_profile_search_rows(
        &state.profile_search_cache,
        &state.search_query,
        PROFILE_SEARCH_CAP,
    );

    Some(crate::kernel::snapshot::ViewSnapshot::Search(
        SearchSnapshot {
            hits,
            communities,
            highlights,
            profiles,
        },
    ))
}

// ─── Community local scan ────────────────────────────────────────────────────

/// Intermediate candidate used only during the merge / filter step.
///
/// Not exported — lives only inside `project_community_search_rows`.
#[derive(Debug)]
struct CandidateCommunity {
    group_id: String,
    host_relay_url: String,
    name: Option<String>,
    about: Option<String>,
    picture: Option<String>,
    member_count: u64,
    public: bool,
    open: bool,
}

/// Compute the local community-search rows from discovered and joined groups.
///
/// Parity target: `crate::search::search_communities` (bespoke live lane).
/// Algorithm mirrors the bespoke scan, adapted for the kernel's in-memory
/// `DiscoveredRow` + `CommunityRow` sources instead of nostrdb:
///
/// 1. Trim query. Return empty for blank (D6).
/// 2. Lowercase query once.
/// 3. Merge `communities` (joined) and `discovered_groups` into one map keyed
///    by `(host_relay_url, group_id)`. Discovered rows OVERWRITE joined rows for
///    the same key — the catalog source carries reliable `public`/`open` flags
///    and member counts; joined rows may carry membership state not needed here.
/// 4. Filter: `public && open` only (mirrors `is_public_open_room`).
/// 5. Match: `name` or `about` contains query case-insensitively.
/// 6. Sort: lowercase `name.unwrap_or(group_id)`, then `host_relay_url`, then
///    `group_id` (stable tie-breaker — NIP-29 group identity is relay-scoped).
/// 7. Truncate to `limit`.
/// 8. Emit raw `CommunitySearchRow` (D1: no formatted strings, no fallbacks).
///
/// Pure function — no I/O, no ndb scan. Called from `project_search_snapshot`.
pub(crate) fn project_community_search_rows(
    discovered: &[crate::kernel::snapshot::DiscoveredRow],
    communities: &[crate::kernel::snapshot::CommunityRow],
    query: &str,
    limit: usize,
) -> Vec<CommunitySearchRow> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let q_lower = trimmed.to_lowercase();

    // Build a composite-key BTreeMap. Joined rows fill in first; discovered
    // rows overwrite so the catalog source wins when both exist.
    let mut by_key: std::collections::BTreeMap<(String, String), CandidateCommunity> =
        std::collections::BTreeMap::new();

    for c in communities {
        let key = (c.host_relay_url.clone(), c.group_id.clone());
        by_key.entry(key).or_insert_with(|| CandidateCommunity {
            group_id: c.group_id.clone(),
            host_relay_url: c.host_relay_url.clone(),
            name: c.name.clone(),
            about: c.about.clone(),
            picture: c.picture.clone(),
            member_count: u64::from(c.member_count),
            public: c.public,
            open: c.open,
        });
    }

    for d in discovered {
        // Overwrite the joined row (if any) with the discovered-catalog row.
        let key = (d.host_relay_url.clone(), d.group_id.clone());
        by_key.insert(
            key,
            CandidateCommunity {
                group_id: d.group_id.clone(),
                host_relay_url: d.host_relay_url.clone(),
                name: d.name.clone(),
                about: d.about.clone(),
                picture: d.picture.clone(),
                member_count: u64::from(d.member_count),
                public: d.public,
                open: d.open,
            },
        );
    }

    // Filter: public && open, and name/about substring-match (case-insensitive).
    let mut matched: Vec<CandidateCommunity> = by_key
        .into_values()
        .filter(|c| c.public && c.open)
        .filter(|c| {
            let name_match = c
                .name
                .as_deref()
                .map(|n| n.to_lowercase().contains(&q_lower))
                .unwrap_or(false);
            let about_match = c
                .about
                .as_deref()
                .map(|a| a.to_lowercase().contains(&q_lower))
                .unwrap_or(false);
            name_match || about_match
        })
        .collect();

    // Sort by lowercase name (falling back to group_id), then relay, then
    // group_id for a fully deterministic order.
    matched.sort_by(|a, b| {
        let a_name = a
            .name
            .as_deref()
            .unwrap_or(a.group_id.as_str())
            .to_lowercase();
        let b_name = b
            .name
            .as_deref()
            .unwrap_or(b.group_id.as_str())
            .to_lowercase();
        a_name
            .cmp(&b_name)
            .then_with(|| a.host_relay_url.cmp(&b.host_relay_url))
            .then_with(|| a.group_id.cmp(&b.group_id))
    });

    // Truncate and emit raw rows — no formatted strings (D1).
    matched
        .into_iter()
        .take(limit)
        .map(|c| CommunitySearchRow {
            group_id: c.group_id,
            host_relay_url: c.host_relay_url,
            name: c.name,
            about: c.about,
            picture: c.picture,
            member_count: c.member_count,
        })
        .collect()
}

// ─── Profile local scan ──────────────────────────────────────────────────────

/// Case-insensitive substring check (mirrors `contains_ci` in `crate::search`).
///
/// Returns `false` for empty needle (blank query is handled upstream, but this
/// guard keeps individual field comparisons safe).
#[inline]
fn profile_contains_ci(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

/// Bounded cap for the local profile-search result list.
///
/// Mirrors `SEARCH_PROFILE_RESULTS_LIMIT` from `crate::search` (both = 20).
pub(crate) const PROFILE_SEARCH_CAP: usize = 20;

/// Parse a kind:0 `SearchHitRow` into a `ProfileSearchRow`.
///
/// Returns `None` when `hit.kind != 0` or the content is unparseable JSON.
/// Mirrors the field extraction of `crate::profile::parse_metadata` (trimmed
/// strings, `display_name`/`displayName`/`displayname` aliases, `image` fallback
/// for `picture`) so both paths return equivalent data from the same JSON blob.
///
/// Called from `upsert_profile_search_cache` when `SearchResultsUpdated` fires.
pub(crate) fn profile_search_row_from_hit(
    hit: &crate::kernel::snapshot::SearchHitRow,
) -> Option<crate::kernel::snapshot::ProfileSearchRow> {
    if hit.kind != 0 {
        return None;
    }
    profile_search_row_from_parts(&hit.author, hit.created_at, &hit.content)
}

/// Decode a kind:0 content blob into a `ProfileSearchRow`.
///
/// Shared by `profile_search_row_from_hit` (relay-hit path) and
/// `scan_local_profiles` (local kind:0 store-scan path) so both production
/// sources extract the SAME consumer fields from the SAME JSON blob (D4: one
/// decoder). Mirrors `crate::profile::parse_metadata`: trimmed strings,
/// `display_name`/`displayName`/`displayname` aliases, `image` fallback for
/// `picture`.
///
/// Returns `None` when the content is unparseable JSON.
fn profile_search_row_from_parts(
    author: &str,
    created_at: u64,
    content: &str,
) -> Option<crate::kernel::snapshot::ProfileSearchRow> {
    let content: serde_json::Value = serde_json::from_str(content).ok()?;
    let str_field = |key: &str| -> String {
        content
            .get(key)
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("")
            .to_string()
    };
    let name = str_field("name");
    let display_name = {
        let dn = str_field("display_name");
        if !dn.is_empty() {
            dn
        } else {
            let alias = str_field("displayName");
            if !alias.is_empty() {
                alias
            } else {
                str_field("displayname")
            }
        }
    };
    let picture = {
        let p = str_field("picture");
        if !p.is_empty() {
            p
        } else {
            str_field("image")
        }
    };
    Some(crate::kernel::snapshot::ProfileSearchRow {
        pubkey: author.to_string(),
        name,
        display_name,
        nip05: str_field("nip05"),
        picture,
        about: str_field("about"),
        created_at,
    })
}

/// Upsert kind:0 hits from `search_results` into `profile_search_cache`.
///
/// Called from the `SearchResultsUpdated` reducer arm in `actor.rs` after
/// `state.search_results` has been replaced. For each kind:0 row the cache
/// is updated by pubkey — the newest `created_at` wins (same dedup logic as
/// `search_profiles` in the bespoke live lane).
///
/// Cache growth is bounded naturally by the session's relay search scope; it
/// is cleared on `Logout` / `IdentityChanged(None)` (auth domain).
pub(crate) fn upsert_profile_search_cache(state: &mut crate::kernel::app::AppState) {
    let rows: Vec<crate::kernel::snapshot::ProfileSearchRow> = state
        .search_results
        .iter()
        .filter(|r| r.kind == 0)
        .filter_map(profile_search_row_from_hit)
        .collect();
    merge_profile_search_rows(state, rows);
}

/// Merge decoded `ProfileSearchRow`s into `AppState::profile_search_cache`.
///
/// Deduplicates by pubkey — newest `created_at` wins (kind:0 is replaceable,
/// same dedup as the bespoke `search_profiles` nostrdb scan). Shared by the
/// relay-hit path (`upsert_profile_search_cache`) and the local kind:0
/// store-scan path (`KernelEvent::ProfileSearchScanned`), so both production
/// sources write through ONE dedup (D4).
pub(crate) fn merge_profile_search_rows(
    state: &mut crate::kernel::app::AppState,
    rows: Vec<crate::kernel::snapshot::ProfileSearchRow>,
) {
    for row in rows {
        if let Some(existing) = state
            .profile_search_cache
            .iter_mut()
            .find(|p| p.pubkey == row.pubkey)
        {
            if row.created_at > existing.created_at {
                *existing = row;
            }
        } else {
            state.profile_search_cache.push(row);
        }
    }
}

/// Compute the local profile-search rows from `AppState::profile_search_cache`.
///
/// Parity target: `crate::search::search_profiles` (bespoke live lane).
/// Algorithm mirrors the bespoke scan, adapted for the kernel's in-memory
/// `ProfileSearchRow` cache instead of nostrdb:
///
/// 1. Trim query. Return empty for blank (D6).
/// 2. Filter: case-insensitive substring match on name/display_name/nip05/about.
/// 3. Rank: prefix-match on display_name or name first (mirrors bespoke's prefix
///    tier); within a tier, alphabetical by primary label (display_name → name →
///    nip05) case-insensitively.
/// 4. Truncate to `limit`.
/// 5. Emit raw `ProfileSearchRow` (D1: no formatted strings, no fallbacks).
///
/// Pure function — no I/O, no ndb scan. Called from `project_search_snapshot`.
pub(crate) fn project_profile_search_rows(
    cache: &[crate::kernel::snapshot::ProfileSearchRow],
    query: &str,
    limit: usize,
) -> Vec<crate::kernel::snapshot::ProfileSearchRow> {
    let q = query.trim();
    if q.is_empty() {
        return Vec::new();
    }

    let mut matched: Vec<crate::kernel::snapshot::ProfileSearchRow> = cache
        .iter()
        .filter(|p| {
            profile_contains_ci(&p.name, q)
                || profile_contains_ci(&p.display_name, q)
                || profile_contains_ci(&p.nip05, q)
                || profile_contains_ci(&p.about, q)
        })
        .cloned()
        .collect();

    // Mirror bespoke `search_profiles` ranking: prefix-match first, then
    // alphabetical by primary_label (display_name → name → nip05).
    let q_lower = q.to_lowercase();
    matched.sort_by(|a, b| {
        let a_prefix = a.display_name.to_lowercase().starts_with(&q_lower)
            || a.name.to_lowercase().starts_with(&q_lower);
        let b_prefix = b.display_name.to_lowercase().starts_with(&q_lower)
            || b.name.to_lowercase().starts_with(&q_lower);
        match (a_prefix, b_prefix) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => {
                let a_label = profile_search_row_primary_label(a).to_lowercase();
                let b_label = profile_search_row_primary_label(b).to_lowercase();
                a_label.cmp(&b_label)
            }
        }
    });

    matched.into_iter().take(limit).collect()
}

/// Primary label for sort tie-breaking — mirrors `primary_label` in `crate::search`.
fn profile_search_row_primary_label(p: &crate::kernel::snapshot::ProfileSearchRow) -> &str {
    if !p.display_name.is_empty() {
        &p.display_name
    } else if !p.name.is_empty() {
        &p.name
    } else {
        &p.nip05
    }
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
                ..
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

    // 7-SH-T1: the highlights bucket decodes ONLY the kind:9802 hits, via the
    // shared decode_highlight_row (enrichment parity itself is covered by
    // kernel_highlight_row_matches_bespoke_record_parse @ 1c3c5cd9). Non-9802
    // hits stay out of `highlights` but remain in `hits`.
    #[test]
    fn search_highlights_bucket_decodes_only_kind_9802() {
        let mut state = make_state();
        let row = |id: &str, kind: u32, content: &str, tags: Vec<Vec<String>>| SearchHitRow {
            id: id.to_string(),
            author: "dead000000000000000000000000000000000000000000000000000000000001".to_string(),
            kind,
            created_at: 1_700_000_000,
            content: content.to_string(),
            tags,
            relay_provenance: vec![],
        };
        state.search_results = vec![
            row(
                &format!("{:064x}", 1),
                9802,
                "the highlighted passage",
                vec![
                    vec!["a".to_string(), "30023:auth:slug".to_string()],
                    vec!["comment".to_string(), "my note".to_string()],
                ],
            ),
            row(&format!("{:064x}", 2), 30023, "article body", vec![]),
            row(&format!("{:064x}", 3), 0, "{\"name\":\"x\"}", vec![]),
        ];

        let snap = project_search_snapshot(&state).expect("snapshot");
        let crate::kernel::snapshot::ViewSnapshot::Search(s) = snap else {
            panic!("expected Search snapshot");
        };

        // hits carry all three (raw, unfiltered).
        assert_eq!(s.hits.len(), 3, "hits must carry every result kind");
        // highlights bucket = only the kind:9802 hit, decoded.
        assert_eq!(
            s.highlights.len(),
            1,
            "only the kind:9802 hit belongs in the highlights bucket"
        );
        assert_eq!(s.highlights[0].event_id, format!("{:064x}", 1));
        assert_eq!(s.highlights[0].content, "the highlighted passage");
    }

    // ── Phase 7 (gate #4) community-search tests ──────────────────────────────

    // Helper factories matching the discovery.rs test pattern.
    fn make_discovered(
        group_id: &str,
        relay: &str,
        name: &str,
        public: bool,
        open: bool,
    ) -> crate::kernel::snapshot::DiscoveredRow {
        crate::kernel::snapshot::DiscoveredRow {
            group_id: group_id.to_string(),
            host_relay_url: relay.to_string(),
            name: Some(name.to_string()),
            about: Some(format!("About {name}")),
            picture: None,
            member_count: 5,
            public,
            open,
        }
    }

    fn make_community(
        group_id: &str,
        relay: &str,
        name: &str,
        public: bool,
        open: bool,
    ) -> crate::kernel::snapshot::CommunityRow {
        crate::kernel::snapshot::CommunityRow {
            group_id: group_id.to_string(),
            host_relay_url: relay.to_string(),
            name: Some(name.to_string()),
            about: Some(format!("About {name}")),
            picture: None,
            member_count: 3,
            public,
            open,
            is_admin: false,
        }
    }

    // 7-CS-T1: blank query returns empty community list (D6).
    #[test]
    fn community_search_blank_query_returns_empty() {
        let discovered = vec![make_discovered(
            "g1",
            "wss://r.test",
            "Nostr Club",
            true,
            true,
        )];
        let result = project_community_search_rows(&discovered, &[], "", COMMUNITY_SEARCH_CAP);
        assert!(result.is_empty(), "blank query must return empty (D6)");

        let result_ws =
            project_community_search_rows(&discovered, &[], "   ", COMMUNITY_SEARCH_CAP);
        assert!(
            result_ws.is_empty(),
            "whitespace query must return empty (D6)"
        );
    }

    // 7-CS-T2: only public+open rows are returned.
    #[test]
    fn community_search_filters_public_open() {
        let relay = "wss://r.test";
        let discovered = vec![
            make_discovered("pub_open", relay, "Nostr Public", true, true),
            make_discovered("pub_closed", relay, "Nostr Closed", true, false),
            make_discovered("priv_open", relay, "Nostr Private", false, true),
        ];
        let result = project_community_search_rows(&discovered, &[], "nostr", COMMUNITY_SEARCH_CAP);
        assert_eq!(result.len(), 1, "only public+open should match");
        assert_eq!(result[0].group_id, "pub_open");
    }

    // 7-CS-T3: name and about are both matched case-insensitively.
    #[test]
    fn community_search_matches_name_and_about() {
        let relay = "wss://r.test";
        // "rust" appears in name of g1 and about of g2; g3 doesn't match.
        let discovered = vec![
            crate::kernel::snapshot::DiscoveredRow {
                group_id: "g1".to_string(),
                host_relay_url: relay.to_string(),
                name: Some("Rust Devs".to_string()),
                about: Some("A community".to_string()),
                picture: None,
                member_count: 10,
                public: true,
                open: true,
            },
            crate::kernel::snapshot::DiscoveredRow {
                group_id: "g2".to_string(),
                host_relay_url: relay.to_string(),
                name: Some("Open Source".to_string()),
                about: Some("We discuss RUST and systems programming".to_string()),
                picture: None,
                member_count: 5,
                public: true,
                open: true,
            },
            crate::kernel::snapshot::DiscoveredRow {
                group_id: "g3".to_string(),
                host_relay_url: relay.to_string(),
                name: Some("Python Users".to_string()),
                about: Some("Snake lovers".to_string()),
                picture: None,
                member_count: 3,
                public: true,
                open: true,
            },
        ];
        let result = project_community_search_rows(&discovered, &[], "RUST", COMMUNITY_SEARCH_CAP);
        let ids: Vec<&str> = result.iter().map(|r| r.group_id.as_str()).collect();
        assert!(ids.contains(&"g1"), "name match must be found");
        assert!(ids.contains(&"g2"), "about match must be found");
        assert!(!ids.contains(&"g3"), "non-matching must be excluded");
    }

    // 7-CS-T4: deduplication prefers discovered rows over joined rows for
    // the same (host_relay_url, group_id) composite key.
    #[test]
    fn community_search_dedupe_prefers_discovered() {
        let relay = "wss://r.test";
        // Joined row has `open: false` (closed). Discovered row has `open: true`.
        // The discovered row should win, making the group visible in results.
        let communities = vec![make_community("g1", relay, "Nostr Club", true, false)];
        let discovered = vec![make_discovered("g1", relay, "Nostr Club", true, true)];
        let result =
            project_community_search_rows(&discovered, &communities, "nostr", COMMUNITY_SEARCH_CAP);
        assert_eq!(result.len(), 1, "discovered row should win → group visible");
        assert_eq!(result[0].group_id, "g1");
        // member_count from discovered row (5) should be used, not joined (3).
        assert_eq!(
            result[0].member_count, 5,
            "member_count must come from discovered row"
        );
    }

    // 7-CS-T5: joined-only rows (no corresponding discovered row) still appear.
    #[test]
    fn community_search_joined_only_rows_included() {
        let relay = "wss://r.test";
        let communities = vec![make_community(
            "joined-only",
            relay,
            "Joined Nostr",
            true,
            true,
        )];
        let result =
            project_community_search_rows(&[], &communities, "nostr", COMMUNITY_SEARCH_CAP);
        assert_eq!(
            result.len(),
            1,
            "joined-only row must appear when no discovered override"
        );
        assert_eq!(result[0].group_id, "joined-only");
    }

    // 7-CS-T6: different relay same group_id → both rows included (composite key).
    #[test]
    fn community_search_different_relay_same_group_id_both_included() {
        let relay1 = "wss://relay1.test";
        let relay2 = "wss://relay2.test";
        let discovered = vec![
            make_discovered("g1", relay1, "Nostr Club", true, true),
            make_discovered("g1", relay2, "Nostr Club", true, true),
        ];
        let result = project_community_search_rows(&discovered, &[], "nostr", COMMUNITY_SEARCH_CAP);
        assert_eq!(
            result.len(),
            2,
            "same group_id on different relays = 2 distinct rows"
        );
    }

    // 7-CS-T7: results are sorted by lowercase name, then relay, then group_id.
    #[test]
    fn community_search_sorted_by_name() {
        let relay = "wss://r.test";
        let discovered = vec![
            make_discovered("g1", relay, "Zeta Nostr", true, true),
            make_discovered("g2", relay, "Alpha Nostr", true, true),
            make_discovered("g3", relay, "Meso Nostr", true, true),
        ];
        let result = project_community_search_rows(&discovered, &[], "nostr", COMMUNITY_SEARCH_CAP);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].group_id, "g2", "Alpha must be first");
        assert_eq!(result[1].group_id, "g3", "Meso must be second");
        assert_eq!(result[2].group_id, "g1", "Zeta must be last");
    }

    // 7-CS-T8: result list is bounded at `limit`.
    #[test]
    fn community_search_bounded_at_limit() {
        let relay = "wss://r.test";
        let discovered: Vec<_> = (0..30)
            .map(|i| {
                make_discovered(
                    &format!("g{i:02}"),
                    relay,
                    &format!("Nostr Group {i:02}"),
                    true,
                    true,
                )
            })
            .collect();
        let result = project_community_search_rows(&discovered, &[], "nostr", COMMUNITY_SEARCH_CAP);
        assert!(
            result.len() <= COMMUNITY_SEARCH_CAP,
            "result must be bounded at COMMUNITY_SEARCH_CAP ({COMMUNITY_SEARCH_CAP}), got {}",
            result.len()
        );
    }

    // 7-CS-T9: search_query stored in AppState after RunSearch.
    #[test]
    fn run_search_stores_query_in_state() {
        let mut state = make_state();
        let clock = ManualClock::default();

        step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::RunSearch {
                query: "  hello world  ".to_string(),
                scope: SearchScope::LongForm,
            }),
        );

        assert_eq!(
            state.search_query, "hello world",
            "search_query must hold the trimmed query after RunSearch"
        );
    }

    // 7-CS-T10: search_query cleared on Logout.
    #[test]
    fn search_query_cleared_on_logout() {
        let mut state = make_state();
        let clock = ManualClock::default();
        state.search_query = "something".to_string();

        step(&mut state, &clock, Cmd::Action(AppAction::Logout));

        assert!(
            state.search_query.is_empty(),
            "search_query must be empty after Logout"
        );
    }

    // 7-CS-T11: search_query cleared on IdentityChanged(None).
    #[test]
    fn search_query_cleared_on_identity_changed_none() {
        let mut state = make_state();
        let clock = ManualClock::default();
        state.search_query = "something".to_string();

        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(None)),
        );

        assert!(
            state.search_query.is_empty(),
            "search_query must be empty after IdentityChanged(None)"
        );
    }

    // 7-CS-T12: snapshot includes community bucket from stored query.
    #[test]
    fn snapshot_includes_community_bucket_from_query() {
        let mut state = make_state();
        let relay = "wss://r.test";

        state.search_query = "nostr".to_string();
        state.discovered_groups = vec![
            make_discovered("g1", relay, "Nostr Club", true, true),
            make_discovered("g2", relay, "Python World", true, true),
        ];

        let snap = project_search_snapshot(&state).expect("snapshot");
        let crate::kernel::snapshot::ViewSnapshot::Search(s) = snap else {
            panic!("expected Search snapshot");
        };
        assert_eq!(
            s.communities.len(),
            1,
            "only matching communities in bucket"
        );
        assert_eq!(s.communities[0].group_id, "g1");
    }

    // 7-CS-T13: D8 discovery warm-up emitted when discovered_groups is empty
    // and discovery_relay is configured.
    #[test]
    fn run_search_warms_discovery_when_empty() {
        let mut state = make_state();
        state.room_policy.discovery_relay = "wss://discovery.test".to_string();
        let clock = ManualClock::default();

        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::RunSearch {
                query: "nostr".to_string(),
                scope: SearchScope::LongForm,
            }),
        );

        // Must have RunSearch + DispatchNip29Action + WireGroupDiscovery.
        assert_eq!(
            effects.len(),
            3,
            "D8 warm-up must add 2 extra effects: got {effects:?}"
        );
        assert!(
            matches!(effects[0], Effect::RunSearch { .. }),
            "first effect must be RunSearch"
        );
        assert!(
            matches!(effects[1], Effect::DispatchNip29Action { .. }),
            "second effect must be DispatchNip29Action (warm-up)"
        );
        assert!(
            matches!(effects[2], Effect::WireGroupDiscovery { .. }),
            "third effect must be WireGroupDiscovery (warm-up)"
        );
    }

    // 7-CS-T14: D8 warm-up NOT emitted when discovered_groups already has rows.
    #[test]
    fn run_search_no_warmup_when_discovered_not_empty() {
        let mut state = make_state();
        state.room_policy.discovery_relay = "wss://discovery.test".to_string();
        // Pre-populate discovered groups — no warm-up needed.
        state.discovered_groups = vec![make_discovered("g1", "wss://r.test", "Nostr", true, true)];
        let clock = ManualClock::default();

        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::RunSearch {
                query: "nostr".to_string(),
                scope: SearchScope::LongForm,
            }),
        );

        assert_eq!(
            effects.len(),
            1,
            "no warm-up when discovered_groups non-empty"
        );
        assert!(matches!(effects[0], Effect::RunSearch { .. }));
    }

    // 7-CS-T15: D8 warm-up NOT emitted when discovery_relay is empty.
    #[test]
    fn run_search_no_warmup_when_no_discovery_relay() {
        let mut state = make_state();
        // discovery_relay is empty by default.
        let clock = ManualClock::default();

        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::RunSearch {
                query: "nostr".to_string(),
                scope: SearchScope::LongForm,
            }),
        );

        assert_eq!(effects.len(), 1, "no warm-up when discovery_relay empty");
        assert!(matches!(effects[0], Effect::RunSearch { .. }));
    }

    // 7-CS-T16: Parity test — kernel community scan vs bespoke search_communities.
    //
    // Both algorithms receive the same logical fixture:
    //   g_pub_open   : public+open, name "Rust Club", about "We love Rust"  → matches "rust"
    //   g_pub_closed : public+closed                                         → excluded
    //   g_priv_open  : private+open                                          → excluded
    //   g_dup_old    : public+open, same group_id as g_pub_open (older)      → deduped out
    //   g_other      : public+open, name "Python World"                       → no match
    //
    // The kernel scan receives equivalent DiscoveredRow inputs.
    // We assert the same group_ids in the same order as the expected output.
    //
    // NOTE: bespoke search_communities scans nostrdb (not available in kernel unit
    // tests). We validate parity by comparing against the documented algorithm
    // output on the shared fixture, confirmed by checking bespoke test 1403.
    #[test]
    fn parity_community_scan_matches_bespoke_algorithm() {
        let relay = "wss://relay.test";

        // Shared fixture as DiscoveredRow (kernel inputs).
        // g_dup_old has the same (relay, group_id) as g_pub_open — overwritten.
        // We insert g_dup_old first (lower priority), then g_pub_open overwrites.
        let communities_joined = vec![
            // g_dup_old (joined, older row — will be overwritten by discovered below)
            crate::kernel::snapshot::CommunityRow {
                group_id: "rust-club".to_string(),
                host_relay_url: relay.to_string(),
                name: Some("Rust Club OLD".to_string()),
                about: Some("Old about".to_string()),
                picture: None,
                member_count: 1,
                public: true,
                open: true,
                is_admin: false,
            },
        ];

        let discovered_groups = vec![
            // g_pub_open — matches "rust"
            crate::kernel::snapshot::DiscoveredRow {
                group_id: "rust-club".to_string(),
                host_relay_url: relay.to_string(),
                name: Some("Rust Club".to_string()),
                about: Some("We love Rust programming".to_string()),
                picture: None,
                member_count: 42,
                public: true,
                open: true,
            },
            // g_pub_closed — excluded (open: false)
            crate::kernel::snapshot::DiscoveredRow {
                group_id: "closed-rust".to_string(),
                host_relay_url: relay.to_string(),
                name: Some("Closed Rust").map(str::to_string),
                about: Some("Rust enthusiasts (closed)".to_string()),
                picture: None,
                member_count: 10,
                public: true,
                open: false,
            },
            // g_priv_open — excluded (public: false)
            crate::kernel::snapshot::DiscoveredRow {
                group_id: "private-rust".to_string(),
                host_relay_url: relay.to_string(),
                name: Some("Private Rust".to_string()),
                about: Some("Private Rust channel".to_string()),
                picture: None,
                member_count: 5,
                public: false,
                open: true,
            },
            // g_other — no match ("python" not in "rust" query)
            crate::kernel::snapshot::DiscoveredRow {
                group_id: "python-world".to_string(),
                host_relay_url: relay.to_string(),
                name: Some("Python World".to_string()),
                about: Some("Pythonistas unite".to_string()),
                picture: None,
                member_count: 15,
                public: true,
                open: true,
            },
        ];

        let results = project_community_search_rows(
            &discovered_groups,
            &communities_joined,
            "rust",
            COMMUNITY_SEARCH_CAP,
        );

        // Expected: only "rust-club" (public+open, name+about match "rust").
        // "closed-rust" excluded (open: false).
        // "private-rust" excluded (public: false).
        // "python-world" excluded (no match).
        // Dedup: discovered "rust-club" overwrites joined "rust-club OLD".
        assert_eq!(
            results.len(),
            1,
            "only 1 group should pass all filters: got {:?}",
            results.iter().map(|r| &r.group_id).collect::<Vec<_>>()
        );
        assert_eq!(results[0].group_id, "rust-club");
        assert_eq!(
            results[0].name.as_deref(),
            Some("Rust Club"),
            "discovered name (not joined OLD name) must win"
        );
        assert_eq!(
            results[0].member_count, 42,
            "discovered member_count must win"
        );
    }

    // ── Phase 7 (#1697 gate) profile-search tests ─────────────────────────────

    use crate::kernel::snapshot::ProfileSearchRow;

    fn make_profile_hit(
        keys: &nostr_sdk::prelude::Keys,
        created_at: u64,
        content: &str,
    ) -> SearchHitRow {
        let event =
            nostr_sdk::prelude::EventBuilder::new(nostr_sdk::prelude::Kind::Custom(0), content)
                .custom_created_at(nostr_sdk::prelude::Timestamp::from(created_at))
                .sign_with_keys(keys)
                .unwrap();
        SearchHitRow {
            id: event.id.to_hex(),
            author: event.pubkey.to_hex(),
            kind: 0,
            created_at,
            content: event.content.clone(),
            tags: vec![],
            relay_provenance: vec![],
        }
    }

    // 7-SP-T1: blank query returns empty profile list (D6).
    #[test]
    fn profile_search_blank_query_returns_empty() {
        let keys = nostr_sdk::prelude::Keys::generate();
        let cache = vec![ProfileSearchRow {
            pubkey: keys.public_key().to_hex(),
            name: "alice".into(),
            display_name: "Alice".into(),
            nip05: "alice@example.com".into(),
            picture: String::new(),
            about: String::new(),
            created_at: 1_000,
        }];

        assert!(
            project_profile_search_rows(&cache, "", 20).is_empty(),
            "blank query must return empty (D6)"
        );
        assert!(
            project_profile_search_rows(&cache, "   ", 20).is_empty(),
            "whitespace query must return empty (D6)"
        );
    }

    // 7-SP-T2: case-insensitive substring match on name, display_name, nip05, about.
    #[test]
    fn profile_search_case_insensitive_all_fields() {
        let k1 = nostr_sdk::prelude::Keys::generate();
        let k2 = nostr_sdk::prelude::Keys::generate();
        let k3 = nostr_sdk::prelude::Keys::generate();
        let k4 = nostr_sdk::prelude::Keys::generate();

        let cache = vec![
            ProfileSearchRow {
                pubkey: k1.public_key().to_hex(),
                name: "HUXLEY-fan".into(),
                display_name: String::new(),
                nip05: String::new(),
                picture: String::new(),
                about: String::new(),
                created_at: 1_000,
            },
            ProfileSearchRow {
                pubkey: k2.public_key().to_hex(),
                name: "bob".into(),
                display_name: "Aldous Huxley".into(),
                nip05: String::new(),
                picture: String::new(),
                about: String::new(),
                created_at: 1_000,
            },
            ProfileSearchRow {
                pubkey: k3.public_key().to_hex(),
                name: "charlie".into(),
                display_name: "Charlie".into(),
                nip05: "huxley@example.com".into(),
                picture: String::new(),
                about: String::new(),
                created_at: 1_000,
            },
            ProfileSearchRow {
                pubkey: k4.public_key().to_hex(),
                name: "dave".into(),
                display_name: "Dave".into(),
                nip05: String::new(),
                picture: String::new(),
                about: "I read all of Huxley's work".into(),
                created_at: 1_000,
            },
        ];

        let hits = project_profile_search_rows(&cache, "huxley", 20);
        assert_eq!(hits.len(), 4, "all four fields must be matched");
    }

    // 7-SP-T3: prefix-match ranks before contains-only.
    #[test]
    fn profile_search_prefix_ranks_first() {
        let ka = nostr_sdk::prelude::Keys::generate(); // contains-only
        let kb = nostr_sdk::prelude::Keys::generate(); // prefix match

        let cache = vec![
            ProfileSearchRow {
                pubkey: ka.public_key().to_hex(),
                name: "Prof. Aldous Huxley".into(),
                display_name: "Aldous H.".into(),
                nip05: String::new(),
                picture: String::new(),
                about: String::new(),
                created_at: 1_000,
            },
            ProfileSearchRow {
                pubkey: kb.public_key().to_hex(),
                name: "huxley-fan".into(),
                display_name: "Huxley's Reader".into(),
                nip05: String::new(),
                picture: String::new(),
                about: String::new(),
                created_at: 2_000,
            },
        ];

        let hits = project_profile_search_rows(&cache, "huxley", 20);
        assert_eq!(hits.len(), 2, "both profiles must match");
        assert_eq!(
            hits[0].pubkey,
            kb.public_key().to_hex(),
            "prefix match (display_name starts with 'huxley') must rank first"
        );
        assert_eq!(
            hits[1].pubkey,
            ka.public_key().to_hex(),
            "contains-only match must rank second"
        );
    }

    // 7-SP-T4: non-matching profiles are excluded.
    #[test]
    fn profile_search_excludes_non_matching() {
        let k = nostr_sdk::prelude::Keys::generate();
        let cache = vec![ProfileSearchRow {
            pubkey: k.public_key().to_hex(),
            name: "proust".into(),
            display_name: "Marcel Proust".into(),
            nip05: "proust@example.com".into(),
            picture: String::new(),
            about: "French novelist".into(),
            created_at: 1_000,
        }];

        let hits = project_profile_search_rows(&cache, "huxley", 20);
        assert!(hits.is_empty(), "non-matching profile must be excluded");
    }

    // 7-SP-T5: results bounded at limit.
    #[test]
    fn profile_search_bounded_at_limit() {
        let cache: Vec<ProfileSearchRow> = (0..30)
            .map(|i| {
                let k = nostr_sdk::prelude::Keys::generate();
                ProfileSearchRow {
                    pubkey: k.public_key().to_hex(),
                    name: format!("huxley-{i:02}"),
                    display_name: String::new(),
                    nip05: String::new(),
                    picture: String::new(),
                    about: String::new(),
                    created_at: 1_000 + i,
                }
            })
            .collect();

        let hits = project_profile_search_rows(&cache, "huxley", PROFILE_SEARCH_CAP);
        assert!(
            hits.len() <= PROFILE_SEARCH_CAP,
            "result must be bounded at PROFILE_SEARCH_CAP ({PROFILE_SEARCH_CAP}), got {}",
            hits.len()
        );
    }

    // 7-SP-T6: upsert deduplicates by pubkey — newest created_at wins.
    #[test]
    fn upsert_profile_cache_deduplicates_by_pubkey() {
        let keys = nostr_sdk::prelude::Keys::generate();
        let pubkey = keys.public_key().to_hex();

        let old_hit = make_profile_hit(&keys, 1_000, r#"{"name":"old-name","display_name":"Old"}"#);
        let new_hit = make_profile_hit(&keys, 2_000, r#"{"name":"new-name","display_name":"New"}"#);

        let mut state = make_state();
        // First: insert the older hit
        state.search_results = vec![old_hit];
        upsert_profile_search_cache(&mut state);
        assert_eq!(state.profile_search_cache.len(), 1);
        assert_eq!(state.profile_search_cache[0].name, "old-name");

        // Second: insert the newer hit for the same pubkey — must win
        state.search_results = vec![new_hit];
        upsert_profile_search_cache(&mut state);
        assert_eq!(
            state.profile_search_cache.len(),
            1,
            "same pubkey must not create duplicate cache entries"
        );
        assert_eq!(
            state.profile_search_cache[0].name, "new-name",
            "newer created_at must overwrite the older entry"
        );

        // Third: a stale hit (older created_at) must NOT overwrite the cached entry
        let stale_hit = make_profile_hit(
            &keys,
            500,
            r#"{"name":"stale-name","display_name":"Stale"}"#,
        );
        // stale_hit carries the same pubkey but lower created_at.
        // We need to produce a hit with the same author pubkey but lower created_at.
        let mut stale_row = stale_hit.clone();
        stale_row.author = pubkey.clone();
        stale_row.created_at = 500;
        state.search_results = vec![stale_row];
        upsert_profile_search_cache(&mut state);
        assert_eq!(
            state.profile_search_cache[0].name, "new-name",
            "stale entry must not overwrite newer cached entry"
        );
    }

    // 7-SP-T7: profile_search_cache cleared on Logout.
    #[test]
    fn profile_search_cache_cleared_on_logout() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let k = nostr_sdk::prelude::Keys::generate();
        state.profile_search_cache = vec![ProfileSearchRow {
            pubkey: k.public_key().to_hex(),
            name: "alice".into(),
            display_name: String::new(),
            nip05: String::new(),
            picture: String::new(),
            about: String::new(),
            created_at: 1_000,
        }];

        step(&mut state, &clock, Cmd::Action(AppAction::Logout));

        assert!(
            state.profile_search_cache.is_empty(),
            "profile_search_cache must be empty after Logout"
        );
    }

    // 7-SP-T8: profile_search_cache cleared on IdentityChanged(None).
    #[test]
    fn profile_search_cache_cleared_on_identity_changed_none() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let k = nostr_sdk::prelude::Keys::generate();
        state.profile_search_cache = vec![ProfileSearchRow {
            pubkey: k.public_key().to_hex(),
            name: "alice".into(),
            display_name: String::new(),
            nip05: String::new(),
            picture: String::new(),
            about: String::new(),
            created_at: 1_000,
        }];

        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(None)),
        );

        assert!(
            state.profile_search_cache.is_empty(),
            "profile_search_cache must be empty after IdentityChanged(None)"
        );
    }

    // 7-SP-T9: SearchResultsUpdated upserts kind:0 hits into cache.
    #[test]
    fn search_results_updated_upserts_kind0_hits() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let k1 = nostr_sdk::prelude::Keys::generate();
        let k2 = nostr_sdk::prelude::Keys::generate();

        let hit1 = SearchHitRow {
            id: format!("{:064x}", 1u64),
            author: k1.public_key().to_hex(),
            kind: 0,
            created_at: 1_000,
            content: r#"{"name":"alice","display_name":"Alice"}"#.into(),
            tags: vec![],
            relay_provenance: vec![],
        };
        let hit_article = SearchHitRow {
            id: format!("{:064x}", 2u64),
            author: k2.public_key().to_hex(),
            kind: 30023,
            created_at: 2_000,
            content: "article body".into(),
            tags: vec![],
            relay_provenance: vec![],
        };

        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::SearchResultsUpdated(vec![
                hit1.clone(),
                hit_article.clone(),
            ])),
        );

        assert_eq!(
            state.profile_search_cache.len(),
            1,
            "only kind:0 hits must be upserted into profile_search_cache"
        );
        assert_eq!(
            state.profile_search_cache[0].pubkey,
            k1.public_key().to_hex()
        );
        assert_eq!(state.profile_search_cache[0].name, "alice");
    }

    // 7-SP-T10: profiles bucket in SearchSnapshot populated from cache.
    #[test]
    fn search_snapshot_includes_profiles_bucket() {
        let mut state = make_state();
        let k = nostr_sdk::prelude::Keys::generate();

        state.profile_search_cache = vec![ProfileSearchRow {
            pubkey: k.public_key().to_hex(),
            name: "huxley-fan".into(),
            display_name: "Huxley Fan".into(),
            nip05: String::new(),
            picture: String::new(),
            about: String::new(),
            created_at: 1_000,
        }];
        state.search_query = "huxley".into();

        let snap = project_search_snapshot(&state).expect("snapshot");
        let crate::kernel::snapshot::ViewSnapshot::Search(s) = snap else {
            panic!("expected Search snapshot");
        };
        assert_eq!(
            s.profiles.len(),
            1,
            "matching profile must appear in bucket"
        );
        assert_eq!(s.profiles[0].pubkey, k.public_key().to_hex());
    }

    // 7-SP-T11: ProfileSearchScanned (local kind:0 store scan) upserts into the
    // cache via the SAME merge/dedup as the relay-hit path. This is the
    // production driver of the people bucket (relay search never returns kind:0).
    #[test]
    fn profile_search_scanned_merges_into_cache() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let k = nostr_sdk::prelude::Keys::generate();
        let pubkey = k.public_key().to_hex();

        let older = ProfileSearchRow {
            pubkey: pubkey.clone(),
            name: "old-name".into(),
            display_name: "Old".into(),
            nip05: String::new(),
            picture: String::new(),
            about: String::new(),
            created_at: 1_000,
        };
        let newer = ProfileSearchRow {
            pubkey: pubkey.clone(),
            name: "new-name".into(),
            display_name: "New".into(),
            nip05: String::new(),
            picture: String::new(),
            about: String::new(),
            created_at: 2_000,
        };

        // Pass generation 0 to match the default profile_search_generation.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::ProfileSearchScanned {
                generation: 0,
                rows: vec![older],
            }),
        );
        assert_eq!(state.profile_search_cache.len(), 1);
        assert_eq!(state.profile_search_cache[0].name, "old-name");

        // Newer row for the same pubkey wins (dedup parity with search_profiles).
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::ProfileSearchScanned {
                generation: 0,
                rows: vec![newer],
            }),
        );
        assert_eq!(
            state.profile_search_cache.len(),
            1,
            "same pubkey must not duplicate"
        );
        assert_eq!(
            state.profile_search_cache[0].name, "new-name",
            "newer created_at must win"
        );
    }

    // 7-SP-T-GEN-1: stale ProfileSearchScanned (from a superseded query) is
    // dropped — not merged into the cache (D5 active-view bounding, race guard).
    //
    // Setup:
    //   1. Dispatch RunSearch(A) → state.profile_search_generation becomes 1.
    //   2. Deliver ProfileSearchScanned { generation: 1, rows: [alice] } (A's scan).
    //      → accepted; cache = [alice].
    //   3. Dispatch RunSearch(B) → generation becomes 2.
    //   4. Deliver A's stale scan again (generation: 1, rows: [stale-bob]).
    //      → DROPPED; cache must still reflect alice from step 2 (not stale-bob).
    //
    // Pre-fix this would unconditionally call merge_profile_search_rows and
    // stale-bob would appear in the cache. The test PROVES it fails without the
    // generation guard and passes with it.
    #[test]
    fn stale_profile_scan_after_query_change_is_dropped() {
        let mut state = make_state();
        let clock = ManualClock::default();

        // Step 1: RunSearch(A) — generation bumps to 1.
        let _ = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::RunSearch {
                query: "alice".into(),
                scope: SearchScope::Users,
            }),
        );
        assert_eq!(
            state.profile_search_generation, 1,
            "RunSearch must bump profile_search_generation to 1"
        );

        // Step 2: deliver A's scan with generation 1 — should be accepted.
        let k_alice = nostr_sdk::prelude::Keys::generate();
        let alice = ProfileSearchRow {
            pubkey: k_alice.public_key().to_hex(),
            name: "alice".into(),
            display_name: "Alice".into(),
            nip05: String::new(),
            picture: String::new(),
            about: String::new(),
            created_at: 1_000,
        };
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::ProfileSearchScanned {
                generation: 1,
                rows: vec![alice.clone()],
            }),
        );
        assert_eq!(
            state.profile_search_cache.len(),
            1,
            "A's scan (generation 1) must be accepted"
        );
        assert_eq!(state.profile_search_cache[0].name, "alice");

        // Step 3: RunSearch(B) — generation bumps to 2 (supersedes A's in-flight scan).
        let _ = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::RunSearch {
                query: "bob".into(),
                scope: SearchScope::Users,
            }),
        );
        assert_eq!(
            state.profile_search_generation, 2,
            "second RunSearch must bump generation to 2"
        );
        // Cache cleared because query changed (query A → query B).
        assert!(
            state.profile_search_cache.is_empty(),
            "cache must be cleared when query changes"
        );

        // Step 4: deliver A's stale scan (generation 1 — must be DROPPED).
        let k_stale = nostr_sdk::prelude::Keys::generate();
        let stale_bob = ProfileSearchRow {
            pubkey: k_stale.public_key().to_hex(),
            name: "stale-bob".into(),
            display_name: "Stale Bob".into(),
            nip05: String::new(),
            picture: String::new(),
            about: String::new(),
            created_at: 3_000,
        };
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::ProfileSearchScanned {
                generation: 1, // A's old generation — stale
                rows: vec![stale_bob],
            }),
        );

        // Stale scan must NOT populate the cache; B's query is active.
        assert!(
            state.profile_search_cache.is_empty(),
            "stale ProfileSearchScanned (generation 1 < current 2) must be DROPPED (D5)"
        );
        // Generation must not have been changed by the dropped event.
        assert_eq!(state.profile_search_generation, 2);
    }

    // 7-SP-T-GEN-2: ProfileSearchScanned arriving after CloseView(Search) is dropped.
    //
    // CloseView(Search) in the actor_task clears the cache AND bumps the
    // generation (this is the inline state mutation that `reduce()` does NOT
    // handle — it lives in actor_task). The test directly applies the same
    // mutations to verify the reducer's generation guard produces the right
    // result: any ProfileSearchScanned with the pre-close generation must be
    // silently dropped.
    #[test]
    fn stale_profile_scan_after_close_view_is_dropped() {
        let mut state = make_state();
        let clock = ManualClock::default();

        // RunSearch → generation becomes 1.
        let _ = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::RunSearch {
                query: "nostr".into(),
                scope: SearchScope::Users,
            }),
        );
        assert_eq!(state.profile_search_generation, 1);

        // Simulate the inline CloseView(Search) logic from actor_task:
        // clear search state and bump the generation.
        // (actor_task handles CloseView inline; the `reduce` path is a no-op
        //  for CloseView so we apply the mutation directly in this unit test.)
        let pre_close_generation = state.profile_search_generation;
        state.search_results.clear();
        state.search_query.clear();
        state.profile_search_cache.clear();
        state.profile_search_generation = state.profile_search_generation.wrapping_add(1);
        assert_eq!(
            state.profile_search_generation, 2,
            "simulated CloseView(Search) must bump profile_search_generation to 2"
        );

        // In-flight scan arrives with generation 1 (pre-close) — must be dropped.
        let k = nostr_sdk::prelude::Keys::generate();
        let row = ProfileSearchRow {
            pubkey: k.public_key().to_hex(),
            name: "ghost".into(),
            display_name: String::new(),
            nip05: String::new(),
            picture: String::new(),
            about: String::new(),
            created_at: 999,
        };
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::ProfileSearchScanned {
                generation: pre_close_generation, // pre-close generation — stale
                rows: vec![row],
            }),
        );

        assert!(
            state.profile_search_cache.is_empty(),
            "post-close stale ProfileSearchScanned must be DROPPED (D5)"
        );
        assert_eq!(state.profile_search_generation, 2, "generation unchanged");
    }

    // 7-SP-T12: cache is bounded-by-active-query (D5/D8) — a RunSearch with a
    // DIFFERENT query clears the prior query's profile cache; re-running the
    // SAME query keeps it.
    #[test]
    fn run_search_clears_profile_cache_on_query_change() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let k = nostr_sdk::prelude::Keys::generate();
        state.search_query = "huxley".into();
        state.profile_search_cache = vec![ProfileSearchRow {
            pubkey: k.public_key().to_hex(),
            name: "huxley-fan".into(),
            display_name: String::new(),
            nip05: String::new(),
            picture: String::new(),
            about: String::new(),
            created_at: 1_000,
        }];

        // Same query → cache retained.
        step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::RunSearch {
                query: "huxley".into(),
                scope: SearchScope::ArticlesAndHighlights,
            }),
        );
        assert_eq!(
            state.profile_search_cache.len(),
            1,
            "re-running the same query must keep the cache"
        );

        // Different query → cache cleared (stale prior-query profiles dropped).
        step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::RunSearch {
                query: "proust".into(),
                scope: SearchScope::ArticlesAndHighlights,
            }),
        );
        assert!(
            state.profile_search_cache.is_empty(),
            "a query replacement must clear the prior query's profile cache (D5)"
        );
    }

    // 7-SP-T_PARITY: REAL identity-level parity test — kernel profile scan must
    // match bespoke `crate::search::search_profiles` on the same fixture.
    //
    // Gotcha #7/#7b compliance: calls BOTH functions on shared test data and
    // asserts pubkey IDENTITY in ORDER — not just counts.
    //
    // Fixture:
    //   keys_a — display_name starts with "huxley" (prefix-match tier)
    //   keys_b — name contains "Huxley" (contains-only tier)
    //   keys_c — unrelated (no match)
    //
    // Expected order: keys_a first (prefix), keys_b second (contains), keys_c absent.
    #[test]
    fn parity_profile_scan_matches_bespoke_algorithm() {
        use crate::test_ndb::{isolated_ndb, process_event_and_wait};

        let (ndb, _tmp) = isolated_ndb(4 * 1024 * 1024);

        let keys_a = nostr_sdk::prelude::Keys::generate(); // prefix-match
        let keys_b = nostr_sdk::prelude::Keys::generate(); // contains-only match
        let keys_c = nostr_sdk::prelude::Keys::generate(); // no match

        // Every consumer field is populated (name/display_name/about/picture/
        // nip05) so the per-field parity asserts compare non-empty values, not
        // empty==empty.
        let ev_a = nostr_sdk::prelude::EventBuilder::new(
            nostr_sdk::prelude::Kind::Custom(0),
            r#"{"name":"huxley-fan","display_name":"Huxley Reader","about":"Books","picture":"https://ex.com/a.png","nip05":"a@ex.com"}"#,
        )
        .custom_created_at(nostr_sdk::prelude::Timestamp::from(2_000u64))
        .sign_with_keys(&keys_a)
        .unwrap();

        let ev_b = nostr_sdk::prelude::EventBuilder::new(
            nostr_sdk::prelude::Kind::Custom(0),
            r#"{"name":"Prof. Aldous Huxley","display_name":"Aldous H.","about":"Writer","picture":"https://ex.com/b.png","nip05":"b@ex.com"}"#,
        )
        .custom_created_at(nostr_sdk::prelude::Timestamp::from(1_000u64))
        .sign_with_keys(&keys_b)
        .unwrap();

        let ev_c = nostr_sdk::prelude::EventBuilder::new(
            nostr_sdk::prelude::Kind::Custom(0),
            r#"{"name":"proust","display_name":"Marcel Proust","about":"French novelist"}"#,
        )
        .custom_created_at(nostr_sdk::prelude::Timestamp::from(3_000u64))
        .sign_with_keys(&keys_c)
        .unwrap();

        for ev in [&ev_a, &ev_b, &ev_c] {
            process_event_and_wait(&ndb, ev);
        }

        // Bespoke result — real nostrdb scan.
        let bespoke = crate::search::search_profiles(&ndb, "huxley", 20).unwrap();

        // Kernel: populate profile_search_cache from the same events.
        let make_hit = |ev: &nostr_sdk::prelude::Event| SearchHitRow {
            id: ev.id.to_hex(),
            author: ev.pubkey.to_hex(),
            kind: 0,
            created_at: ev.created_at.as_secs(),
            content: ev.content.clone(),
            tags: vec![],
            relay_provenance: vec![],
        };

        let mut state = make_state();
        state.profile_search_cache = [&ev_a, &ev_b, &ev_c]
            .iter()
            .filter_map(|ev| profile_search_row_from_hit(&make_hit(ev)))
            .collect();
        state.search_query = "huxley".to_string();

        let snap = project_search_snapshot(&state).expect("snapshot");
        let crate::kernel::snapshot::ViewSnapshot::Search(s) = snap else {
            panic!("expected Search snapshot");
        };
        let kernel = &s.profiles;

        // ── CONSUMER-FIELD parity: assert EVERY field Swift reads at
        // SearchView.swift:745 / SearchSeeAllView.swift:262 matches between the
        // bespoke `search_profiles` and the kernel port, in order. Not counts,
        // not pubkey-only — the guard must BITE if any field is dropped.
        // (proven: temporarily zero any kernel field below and this fails.)
        assert_eq!(
            kernel.len(),
            bespoke.len(),
            "kernel and bespoke must return the same number of rows\n\
             bespoke: {:?}\nkernel: {:?}",
            bespoke.iter().map(|p| &p.pubkey).collect::<Vec<_>>(),
            kernel.iter().map(|p| &p.pubkey).collect::<Vec<_>>(),
        );
        assert_eq!(
            kernel.len(),
            2,
            "fixture has 2 huxley-matching profiles; proust is excluded"
        );
        for (i, (k, b)) in kernel.iter().zip(bespoke.iter()).enumerate() {
            assert_eq!(k.pubkey, b.pubkey, "row {i}: pubkey must match (and order)");
            assert_eq!(k.name, b.name, "row {i} ({}): name must match", b.pubkey);
            assert_eq!(
                k.display_name, b.display_name,
                "row {i} ({}): displayName must match",
                b.pubkey
            );
            assert_eq!(k.about, b.about, "row {i} ({}): about must match", b.pubkey);
            assert_eq!(
                k.picture, b.picture,
                "row {i} ({}): picture must match",
                b.pubkey
            );
            assert_eq!(k.nip05, b.nip05, "row {i} ({}): nip05 must match", b.pubkey);
            // Bespoke carries created_at as Option<u64>; kernel as u64. The
            // fixture sets it on every event, so it must round-trip identically.
            assert_eq!(
                Some(k.created_at),
                b.created_at,
                "row {i} ({}): createdAt must match",
                b.pubkey
            );
        }

        // Prefix-match must be first (keys_a has display_name starting with "Huxley").
        assert_eq!(
            kernel[0].pubkey,
            keys_a.public_key().to_hex(),
            "prefix-match (display_name 'Huxley Reader') must rank before contains-only"
        );
        assert_eq!(
            kernel[1].pubkey,
            keys_b.public_key().to_hex(),
            "contains-only match ('Prof. Aldous Huxley') must rank second"
        );

        // Sanity: the matched rows actually carry non-empty consumer fields, so
        // the per-field asserts above are not vacuously comparing empty==empty.
        assert!(
            !kernel[0].display_name.is_empty()
                && !kernel[0].name.is_empty()
                && !kernel[0].about.is_empty(),
            "fixture must populate name/displayName/about so the field guard bites"
        );
    }
}
