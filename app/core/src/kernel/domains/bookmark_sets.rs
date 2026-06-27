//! Bookmark sets + web bookmarks domain — NIP-51 kind:30003/30004 + NIP-B0
//! kind:39701 projection (gate #1653).
//!
//! ## Responsibilities
//!
//! * **READ (sets)** — a `SetListProjection` implements `ObservedProjectionSink`
//!   for kind:30003 and kind:30004. It accumulates all observed events, one
//!   per `(author, d_tag)` key with newest-wins supersession. A registered
//!   typed-snapshot closure serialises the full collection to JSON every NMP
//!   tick under schema_id `"hl.bookmark_sets"`. `apply_bookmark_sets` decodes
//!   the JSON, applies active-account / follows filtering, and stores results
//!   in `AppState::all_bookmark_sets` + `AppState::all_curation_sets`.
//!
//! * **READ (web)** — a `WebBookmarkProjection` implements `ObservedProjectionSink`
//!   for kind:39701. Only the active account's events are kept (like
//!   `BookmarkListProjection`). Schema_id `"hl.web_bookmarks"`. `apply_web_bookmarks`
//!   stores `WebBookmarkRow`s in `AppState::web_bookmarks`.
//!
//! * **WRITE** — `AppAction::AddToSet{set_coordinate, item_coordinate}` /
//!   `RemoveFromSet{...}` → reducer emits `Effect::PublishSetEvent{json}` →
//!   effect runner calls `ActorCommand::PublishRawEvent` with a kind:30004
//!   event template. Kernel is the SOLE kind:30004 writer for ported screens
//!   (no live-lane double-publish). NMP has no built-in action namespace for
//!   kind:30004 at d16aea60 — raw publish path matches `PublishHighlightEvent`.
//!
//! ## NMP at d16aea60
//!
//! nmp-nip51 only ships `BookmarkListProjection` (kind:10003). Kinds 30003,
//! 30004, and 39701 have no native nmp observer — this module registers custom
//! `ObservedProjectionSink` implementations via `nmp_ref.open_observed_projection`.
//!
//! ## Threading
//!
//! `SetListProjection` and `WebBookmarkProjection` run on the NMP event-observer
//! thread (non-blocking: Mutex lock + small HashMap insert). `apply_bookmark_sets`
//! and `apply_web_bookmarks` run on the **actor thread** (JSON decode + Vec
//! filter, no I/O). D6: decode errors leave AppState unchanged (silent no-op).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use nmp_core::substrate::{
    KernelEvent as NmpKernelEvent, ObservedProjection, ObservedProjectionRegistrar,
};
use nmp_core::ObservedProjectionSink;
use nmp_ffi::NmpApp;
use nmp_planner::{InterestId, InterestLifecycle, InterestScope, InterestShape, LogicalInterest};
// Pubkey is `type Pubkey = String` in the planner; authors are admitted as raw hex.

use crate::kernel::app::AppState;
use crate::kernel::effect::Effect;
use crate::kernel::snapshot::{BookmarkSetRow, WebBookmarkRow};
use crate::kernel::view::ViewId;

// ── Kind constants ────────────────────────────────────────────────────────────

const KIND_BOOKMARK_SET: u32 = 30003;
const KIND_CURATION_SET: u32 = 30004;
const KIND_WEB_BOOKMARK: u32 = 39701;

/// Stable planner `InterestId` for the view-scoped bookmark-sets subscription
/// (kind:30003 + kind:30004 + kind:39701). Non-zero (0 is the planner's
/// "unassigned" sentinel). Idempotent push: re-opening the view replaces the
/// prior entry under this id.
pub(crate) const BOOKMARK_SETS_INTEREST_ID: u64 = 0x1653_5001;

fn bookmark_sets_sub_identity() -> nmp_core::subs::SubIdentity {
    nmp_core::subs::SubIdentity::new(
        nmp_core::subs::SubOwnerKey::new(BOOKMARK_SETS_INTEREST_ID),
        nmp_core::subs::SubKey::new(BOOKMARK_SETS_INTEREST_ID),
        nmp_core::subs::SubScope::Global,
    )
}

// ── Schema IDs ────────────────────────────────────────────────────────────────

/// Schema id for the `SetListProjection` typed snapshot (`"hl.bookmark_sets"`).
pub(crate) const BOOKMARK_SETS_SCHEMA_ID: &str = "hl.bookmark_sets";
/// Schema id for the `WebBookmarkProjection` typed snapshot (`"hl.web_bookmarks"`).
pub(crate) const WEB_BOOKMARKS_SCHEMA_ID: &str = "hl.web_bookmarks";

// ── Wire types for JSON serialisation ────────────────────────────────────────

/// The JSON payload emitted by `SetListProjection`'s typed snapshot closure.
/// Contains all observed kind:30003 and kind:30004 rows unfiltered by identity.
/// Decoded by `apply_bookmark_sets` on the actor thread.
#[derive(serde::Serialize, serde::Deserialize)]
struct SetListPayload {
    all_bookmark_sets: Vec<BookmarkSetRow>,
    all_curation_sets: Vec<BookmarkSetRow>,
}

// ── SetListProjection ─────────────────────────────────────────────────────────

/// In-memory state accumulated by `SetListProjection`.
#[derive(Default)]
struct SetListState {
    /// Newest kind:30003 event per `(author_hex, d_tag)`.
    bookmark_sets: HashMap<(String, String), BookmarkSetRow>,
    /// Newest kind:30004 event per `(author_hex, d_tag)`.
    curation_sets: HashMap<(String, String), BookmarkSetRow>,
}

/// Observes kind:30003 and kind:30004 events from any author, accumulates
/// with newest-wins supersession per `(author, d_tag)` key.
///
/// The projection does NOT filter by active account — filtering by pubkey and
/// follows happens in `apply_bookmark_sets` on the actor thread where
/// `AppState::session` and `AppState::follows` are available.
pub struct SetListProjection {
    state: Mutex<SetListState>,
}

impl SetListProjection {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(SetListState::default()),
        }
    }

    /// Drop all accumulated rows (D5/D8 — bound memory when the view closes so
    /// the observer does not grow unbounded across the session, #1653 HIGH #7).
    pub fn clear(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.bookmark_sets.clear();
            state.curation_sets.clear();
        }
    }

    /// Serialise all accumulated rows to a `SetListPayload` JSON payload.
    pub fn snapshot_payload(&self) -> Option<Vec<u8>> {
        let Ok(state) = self.state.lock() else {
            return None;
        };
        // Collect newest-first (sort by created_at desc).
        let mut bookmark_sets: Vec<BookmarkSetRow> =
            state.bookmark_sets.values().cloned().collect();
        bookmark_sets.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        let mut curation_sets: Vec<BookmarkSetRow> =
            state.curation_sets.values().cloned().collect();
        curation_sets.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        serde_json::to_vec(&SetListPayload {
            all_bookmark_sets: bookmark_sets,
            all_curation_sets: curation_sets,
        })
        .ok()
    }
}

impl ObservedProjectionSink for SetListProjection {
    fn on_kernel_event(&self, event: &NmpKernelEvent) {
        if event.kind != KIND_BOOKMARK_SET && event.kind != KIND_CURATION_SET {
            return;
        }
        // Fail closed: a malformed set (no usable `d`) is dropped entirely.
        let Some(row) = parse_set_row_from_kernel(event) else {
            return;
        };
        let key = (event.author.clone(), row.d_tag.clone());

        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let map = if event.kind == KIND_BOOKMARK_SET {
            &mut state.bookmark_sets
        } else {
            &mut state.curation_sets
        };
        let entry = map.entry(key).or_insert_with(|| row.clone());
        if event.created_at > entry.created_at {
            *entry = row;
        }
    }
}

// ── WebBookmarkProjection ─────────────────────────────────────────────────────

/// Observes kind:39701 events for the active account.
/// Filters by `active_pubkey` at observation time (mirrors `BookmarkListProjection`).
pub struct WebBookmarkProjection {
    active_pubkey: Arc<Mutex<Option<String>>>,
    state: Mutex<HashMap<String, WebBookmarkRow>>, // d_tag → row
}

impl WebBookmarkProjection {
    pub fn new(active_pubkey: Arc<Mutex<Option<String>>>) -> Self {
        Self {
            active_pubkey,
            state: Mutex::new(HashMap::new()),
        }
    }

    /// Drop all accumulated rows (D5/D8 — bound on view close, #1653 HIGH #7).
    pub fn clear(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.clear();
        }
    }

    /// Serialise the active account's web bookmarks as a JSON array.
    pub fn snapshot_payload(&self) -> Option<Vec<u8>> {
        let active = self.active_pubkey.lock().ok()?.as_ref().cloned()?;
        let Ok(state) = self.state.lock() else {
            return None;
        };
        let mut rows: Vec<WebBookmarkRow> = state
            .values()
            .filter(|r| r.pubkey == active)
            .cloned()
            .collect();
        rows.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        serde_json::to_vec(&rows).ok()
    }
}

impl ObservedProjectionSink for WebBookmarkProjection {
    fn on_kernel_event(&self, event: &NmpKernelEvent) {
        if event.kind != KIND_WEB_BOOKMARK {
            return;
        }
        let active = match self.active_pubkey.lock() {
            Ok(g) => g.as_ref().cloned(),
            Err(_) => return,
        };
        if active.as_deref() != Some(event.author.as_str()) {
            return;
        }
        // Fail closed: a malformed web bookmark (no usable `d`/URL) is dropped.
        let Some(row) = parse_web_row_from_kernel(event) else {
            return;
        };
        let key = row.url.clone();
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let entry = state.entry(key).or_insert_with(|| row.clone());
        if event.created_at > entry.created_at {
            *entry = row;
        }
    }
}

// ── READ side: apply decoded payloads ─────────────────────────────────────────

/// Apply a decoded `"hl.bookmark_sets"` JSON payload to `state`.
///
/// Decodes a `SetListPayload` (all observed kind:30003 and kind:30004 rows),
/// then stores them into `AppState::all_bookmark_sets` and
/// `AppState::all_curation_sets`. Both collections are stored unfiltered by
/// active pubkey — filtering happens at snapshot-projection time in
/// `project_bookmarks_snapshot`.
///
/// D6: any decode error leaves the fields unchanged (silent no-op).
/// D1: raw fields only — no presentation strings stored.
pub(crate) fn apply_bookmark_sets(state: &mut AppState, payload: &[u8]) {
    match serde_json::from_slice::<SetListPayload>(payload) {
        Ok(data) => {
            state.all_bookmark_sets = data.all_bookmark_sets;
            state.all_curation_sets = data.all_curation_sets;
        }
        Err(e) => {
            tracing::trace!(
                error = %e,
                "bookmark_sets::apply_bookmark_sets: JSON decode error — state unchanged (D6)"
            );
        }
    }
}

/// Apply a decoded `"hl.web_bookmarks"` JSON payload to `state`.
///
/// Decodes a `Vec<WebBookmarkRow>` and stores it in `AppState::web_bookmarks`.
/// D6: any decode error leaves the field unchanged (silent no-op). D1: raw.
pub(crate) fn apply_web_bookmarks(state: &mut AppState, payload: &[u8]) {
    match serde_json::from_slice::<Vec<WebBookmarkRow>>(payload) {
        Ok(rows) => {
            state.web_bookmarks = rows;
        }
        Err(e) => {
            tracing::trace!(
                error = %e,
                "bookmark_sets::apply_web_bookmarks: JSON decode error — state unchanged (D6)"
            );
        }
    }
}

// ── Snapshot projection helpers ───────────────────────────────────────────────

/// Extract the active account pubkey from `AppState::session` (raw hex).
fn active_pubkey(state: &AppState) -> Option<&str> {
    if let crate::kernel::app::SessionState::Present { pubkey, .. } = &state.session {
        Some(pubkey.as_str())
    } else {
        None
    }
}

/// Project `my_bookmark_sets` — kind:30003 rows authored by the active account.
/// Returned in `created_at` descending order (already sorted by `apply_bookmark_sets`).
pub(crate) fn project_my_bookmark_sets(state: &AppState) -> Vec<BookmarkSetRow> {
    let Some(pk) = active_pubkey(state) else {
        return Vec::new();
    };
    state
        .all_bookmark_sets
        .iter()
        .filter(|r| r.pubkey == pk)
        .cloned()
        .collect()
}

/// Project `my_curation_sets` — kind:30004 rows authored by the active account.
pub(crate) fn project_my_curation_sets(state: &AppState) -> Vec<BookmarkSetRow> {
    let Some(pk) = active_pubkey(state) else {
        return Vec::new();
    };
    state
        .all_curation_sets
        .iter()
        .filter(|r| r.pubkey == pk)
        .cloned()
        .collect()
}

/// Project `following_curation_sets` — kind:30004 rows authored by any pubkey in
/// `AppState::follows`. Returned in `created_at` descending order.
pub(crate) fn project_following_curation_sets(state: &AppState) -> Vec<BookmarkSetRow> {
    let follows: std::collections::HashSet<&str> =
        state.follows.iter().map(String::as_str).collect();
    state
        .all_curation_sets
        .iter()
        .filter(|r| follows.contains(r.pubkey.as_str()))
        .cloned()
        .collect()
}

/// Project `my_web_bookmarks` — already filtered at apply time.
pub(crate) fn project_my_web_bookmarks(state: &AppState) -> Vec<WebBookmarkRow> {
    state.web_bookmarks.clone()
}

// ── WRITE side: reducer helpers ────────────────────────────────────────────────

/// Handle `AppAction::AddToSet { set_coordinate, item_coordinate }`.
///
/// Finds the curation set in `AppState::all_curation_sets` by matching the
/// `set_coordinate` against `"<kind>:<pubkey>:<d_tag>"`. Builds an updated
/// kind:30004 event with `item_coordinate` added to the `a`-tag list (no-op
/// if already present). Emits `Effect::PublishSetEvent` with the event template
/// JSON. D6: no-op when the set cannot be found or serialisation fails.
///
/// Kernel is the SOLE kind:30004 writer for ported screens.
pub(crate) fn reduce_action_add_to_set(
    state: &AppState,
    set_coordinate: String,
    item_coordinate: String,
) -> Vec<Effect> {
    let Some(row) = find_curation_set(state, &set_coordinate) else {
        tracing::trace!(
            set = %set_coordinate,
            "bookmark_sets::add_to_set: set not found — no-op (D6)"
        );
        return vec![];
    };
    // Idempotent: skip if already present.
    if row.article_addresses.contains(&item_coordinate) {
        tracing::trace!(
            item = %item_coordinate,
            "bookmark_sets::add_to_set: item already present — no-op"
        );
        return vec![];
    }
    let mut new_a = row.article_addresses.clone();
    new_a.push(item_coordinate);
    build_set_publish_effect(&row, &new_a)
}

/// Handle `AppAction::RemoveFromSet { set_coordinate, item_coordinate }`.
/// Symmetric with `reduce_action_add_to_set`. D6: no-op when set or item not found.
pub(crate) fn reduce_action_remove_from_set(
    state: &AppState,
    set_coordinate: String,
    item_coordinate: String,
) -> Vec<Effect> {
    let Some(row) = find_curation_set(state, &set_coordinate) else {
        tracing::trace!(
            set = %set_coordinate,
            "bookmark_sets::remove_from_set: set not found — no-op (D6)"
        );
        return vec![];
    };
    let before = row.article_addresses.len();
    let mut new_a = row.article_addresses.clone();
    new_a.retain(|a| *a != item_coordinate);
    if new_a.len() == before {
        tracing::trace!(
            item = %item_coordinate,
            "bookmark_sets::remove_from_set: item not found — no-op"
        );
        return vec![];
    }
    build_set_publish_effect(&row, &new_a)
}

/// Create a brand-new kind:30004 curation set with `title` and immediately add
/// `item_coordinate` as its first member. The `d_tag` is derived from `title` +
/// the current unix timestamp (collision-resistant, human-readable).
///
/// Returns a single `Effect::PublishSetEvent` with the new event template, or an
/// empty vec on serialisation failure (D6).
pub(crate) fn reduce_action_create_and_add_to_set(
    _state: &AppState,
    title: String,
    item_coordinate: String,
    now: u64,
) -> Vec<Effect> {
    // Derive a URL-safe d_tag: lowercase, spaces→hyphens, strip non-alphanumeric
    // non-hyphen chars, truncate to 40 chars, append timestamp.
    let slug: String = title
        .to_lowercase()
        .chars()
        .map(|c| if c == ' ' { '-' } else { c })
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .take(40)
        .collect();
    let d_tag = format!("{slug}-{now}");

    let tags = vec![
        serde_json::json!(["d", d_tag]),
        serde_json::json!(["title", &title]),
        serde_json::json!(["a", &item_coordinate]),
    ];

    let template = serde_json::json!({
        "kind": KIND_CURATION_SET,
        "content": "",
        "tags": tags,
    });
    match serde_json::to_string(&template) {
        Ok(json) => vec![Effect::PublishSetEvent { json }],
        Err(e) => {
            tracing::trace!(
                error = %e,
                "bookmark_sets::create_and_add_to_set: serialisation failed — no-op (D6)"
            );
            vec![]
        }
    }
}

/// Handle `AppAction::RenameSet { set_coordinate, title }`.
///
/// Finds the curation set in `AppState::all_curation_sets` by matching the
/// `set_coordinate` against `"<kind>:<pubkey>:<d_tag>"`. Builds an updated
/// kind:30004 event replacing the `title` tag (and injecting one if absent)
/// while preserving ALL other tags verbatim — membership (`a`), description,
/// image, `e`/`r`/`t`, relay hints, and any custom tags.
///
/// The lossless round-trip is identical to `build_set_publish_effect` but
/// additionally replaces `title` in the non-`a` tag loop: for each `title`
/// tag found in `raw_tags` it is skipped (not emitted verbatim), then the
/// new title is appended once after the loop, before the `a` block.
///
/// D6: no-op when the set cannot be found or serialisation fails.
pub(crate) fn reduce_action_rename_set(
    state: &AppState,
    set_coordinate: String,
    title: String,
) -> Vec<Effect> {
    let Some(row) = find_owned_curation_set(state, &set_coordinate) else {
        tracing::trace!(
            set = %set_coordinate,
            "bookmark_sets::rename_set: set not found or not owned — no-op (D6)"
        );
        return vec![];
    };

    let mut tags: Vec<serde_json::Value> = Vec::new();
    let content = row.content.clone();

    if row.raw_tags.is_empty() {
        // Synthesised fallback (no source event to round-trip).
        tags.push(serde_json::json!(["d", row.d_tag]));
        // New title replaces old
        tags.push(serde_json::json!(["title", &title]));
        if let Some(description) = &row.description {
            tags.push(serde_json::json!(["description", description]));
        }
        if let Some(image) = &row.image {
            tags.push(serde_json::json!(["image", image]));
        }
        for id in &row.note_ids {
            tags.push(serde_json::json!(["e", id]));
        }
        for r in &row.r_refs {
            tags.push(serde_json::json!(["r", r]));
        }
        for t in &row.topics {
            tags.push(serde_json::json!(["t", t]));
        }
        for addr in &row.article_addresses {
            tags.push(serde_json::json!(["a", addr]));
        }
    } else {
        // Lossless round-trip: copy every non-`a`, non-`title` tag verbatim.
        // The `a` block and `title` are managed dimensions re-emitted below.
        for raw in &row.raw_tags {
            let key = raw.first().map(String::as_str);
            if key == Some("a") {
                continue; // managed — re-emitted below
            }
            if key == Some("title") {
                continue; // managed — replaced below
            }
            tags.push(serde_json::Value::Array(
                raw.iter()
                    .map(|s| serde_json::Value::String(s.clone()))
                    .collect(),
            ));
        }
        // Emit the new title once, after the non-managed tags (whether or not the
        // source event carried a `title` tag — create-if-absent, replace-if-present).
        tags.push(serde_json::json!(["title", &title]));
        // Re-emit the `a` membership block (unchanged)
        for addr in &row.article_addresses {
            tags.push(serde_json::json!(["a", addr]));
        }
    }

    let template = serde_json::json!({
        "kind": KIND_CURATION_SET,
        "content": content,
        "tags": tags,
    });
    match serde_json::to_string(&template) {
        Ok(json) => vec![Effect::PublishSetEvent { json }],
        Err(e) => {
            tracing::trace!(
                error = %e,
                "bookmark_sets::rename_set: serialisation failed — no-op (D6)"
            );
            vec![]
        }
    }
}

/// Handle `AppAction::DeleteSet { set_coordinate }`.
///
/// Finds the curation set in `AppState::all_curation_sets` to verify it
/// exists (no-op if not found — D6). Builds a NIP-09 kind:5 deletion event
/// template with an `["a", "<set_coordinate>"]` tag and a `["k", "30004"]`
/// tag, then emits `Effect::PublishSetEvent` with the template JSON.
///
/// The kind-agnostic `run_effect_publish_set_event` runner handles the actual
/// publish via `ActorCommand::PublishRawEvent` with `kind: 5`. No local state
/// mutation — the deleted set disappears from `myCurationSets` after the
/// relay-echo loop completes and the `SetListProjection` re-snapshots.
///
/// D6: no-op when the set cannot be found or serialisation fails.
pub(crate) fn reduce_action_delete_set(
    state: &AppState,
    set_coordinate: String,
) -> Vec<Effect> {
    // Verify the set exists AND is owned by the active account before issuing the
    // deletion (D6 no-op on miss / not-mine — never kind:5 someone else's set).
    if find_owned_curation_set(state, &set_coordinate).is_none() {
        tracing::trace!(
            set = %set_coordinate,
            "bookmark_sets::delete_set: set not found or not owned — no-op (D6)"
        );
        return vec![];
    }

    let template = serde_json::json!({
        "kind": 5u32,
        "content": "",
        "tags": [
            ["a", set_coordinate],
            ["k", "30004"],
        ],
    });
    match serde_json::to_string(&template) {
        Ok(json) => vec![Effect::PublishSetEvent { json }],
        Err(e) => {
            tracing::trace!(
                error = %e,
                "bookmark_sets::delete_set: serialisation failed — no-op (D6)"
            );
            vec![]
        }
    }
}

/// Handle `AppAction::CreateSet { title }`.
///
/// Creates a brand-new empty kind:30004 curation set. Like
/// `reduce_action_create_and_add_to_set` but without an initial member. The
/// `d_tag` is derived from `title` + `now` (same slug algorithm). Emits
/// `Effect::PublishSetEvent` with the event template. D6: no-op on
/// serialisation failure.
pub(crate) fn reduce_action_create_set(
    _state: &AppState,
    title: String,
    now: u64,
) -> Vec<Effect> {
    let slug: String = title
        .to_lowercase()
        .chars()
        .map(|c| if c == ' ' { '-' } else { c })
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .take(40)
        .collect();
    let d_tag = format!("{slug}-{now}");

    let tags = vec![
        serde_json::json!(["d", d_tag]),
        serde_json::json!(["title", &title]),
        // No `a` tags — empty set by design
    ];

    let template = serde_json::json!({
        "kind": KIND_CURATION_SET,
        "content": "",
        "tags": tags,
    });
    match serde_json::to_string(&template) {
        Ok(json) => vec![Effect::PublishSetEvent { json }],
        Err(e) => {
            tracing::trace!(
                error = %e,
                "bookmark_sets::create_set: serialisation failed — no-op (D6)"
            );
            vec![]
        }
    }
}

/// Find a curation set by its `"<kind>:<pubkey>:<d_tag>"` coordinate.
///
/// Parses the coordinate into `(kind_str, pubkey, d_tag)` and looks up
/// `AppState::all_curation_sets` by `(pubkey, d_tag)`. Returns `None` on
/// any parse failure or when the set is not present in state.
fn find_curation_set(state: &AppState, set_coordinate: &str) -> Option<BookmarkSetRow> {
    // Expected format: "30004:<pubkey>:<d_tag>"
    let mut parts = set_coordinate.splitn(3, ':');
    let _ = parts.next()?; // kind string
    let pubkey = parts.next()?;
    let d_tag = parts.next()?;
    state
        .all_curation_sets
        .iter()
        .find(|r| r.pubkey == pubkey && r.d_tag == d_tag)
        .cloned()
}

/// Like `find_curation_set`, but additionally enforces that the resolved set is
/// owned by the active account (`row.pubkey == active_pubkey(state)`).
///
/// `all_curation_sets` also holds the user's `following_curation_sets` (others'
/// sets, surfaced read-only in the Explore pane). Rename/delete MUST never touch
/// those — publishing a kind:30004 replacement or a kind:5 deletion under
/// someone else's coordinate is both invalid (we can't sign as them) and a
/// data-safety hazard. Returns `None` (→ D6 no-op) when the set is missing OR
/// not owned by the active account.
fn find_owned_curation_set(state: &AppState, set_coordinate: &str) -> Option<BookmarkSetRow> {
    let row = find_curation_set(state, set_coordinate)?;
    match active_pubkey(state) {
        Some(active) if active == row.pubkey => Some(row),
        _ => None,
    }
}

/// Serialise a modified curation set into an `Effect::PublishSetEvent` with the
/// kind:30004 event template JSON, **round-trip-preserving** all data the
/// reducer does not manage (#1653 codex BLOCKING #3).
///
/// The kernel modifies exactly one dimension: the `a`-tag membership list
/// (`new_a_addresses`). Every other tag from the source event — `d`, `title`,
/// `description`, `image`, `e`, `r`, `t`, relay hints, and any custom client
/// tag — is carried verbatim from `row.raw_tags`, and the original `content` is
/// preserved. The `a` block is dropped and re-emitted from `new_a_addresses`
/// while preserving every other tag's original position, mirroring the bespoke
/// `update_address_in_curation_set` (other_tags ++ rebuilt a-tags).
///
/// Fallback: when `row.raw_tags` is empty (e.g. a row synthesised in a test
/// that never observed a raw event) the builder synthesises a minimal tag set
/// from the scalar fields so the write path still functions.
///
/// Returns an empty vec on serialisation failure (D6).
fn build_set_publish_effect(row: &BookmarkSetRow, new_a_addresses: &[String]) -> Vec<Effect> {
    let mut tags: Vec<serde_json::Value> = Vec::new();
    let mut content = row.content.clone();

    if row.raw_tags.is_empty() {
        // Synthesised fallback (no source event to round-trip).
        content = String::new();
        tags.push(serde_json::json!(["d", row.d_tag]));
        if let Some(title) = &row.title {
            tags.push(serde_json::json!(["title", title]));
        }
        if let Some(description) = &row.description {
            tags.push(serde_json::json!(["description", description]));
        }
        if let Some(image) = &row.image {
            tags.push(serde_json::json!(["image", image]));
        }
        for id in &row.note_ids {
            tags.push(serde_json::json!(["e", id]));
        }
        for r in &row.r_refs {
            tags.push(serde_json::json!(["r", r]));
        }
        for t in &row.topics {
            tags.push(serde_json::json!(["t", t]));
        }
        for addr in new_a_addresses {
            tags.push(serde_json::json!(["a", addr]));
        }
    } else {
        // Lossless round-trip: copy every non-`a` tag verbatim, then append the
        // new `a` membership block.
        for raw in &row.raw_tags {
            if raw.first().map(String::as_str) == Some("a") {
                continue; // managed dimension — re-emitted below
            }
            tags.push(serde_json::Value::Array(
                raw.iter()
                    .map(|s| serde_json::Value::String(s.clone()))
                    .collect(),
            ));
        }
        for addr in new_a_addresses {
            tags.push(serde_json::json!(["a", addr]));
        }
    }

    let template = serde_json::json!({
        "kind": KIND_CURATION_SET,
        "content": content,
        "tags": tags,
    });
    match serde_json::to_string(&template) {
        Ok(json) => vec![Effect::PublishSetEvent { json }],
        Err(e) => {
            tracing::trace!(
                error = %e,
                "bookmark_sets::build_set_publish_effect: serialisation failed — no-op (D6)"
            );
            vec![]
        }
    }
}

// ── Effect runner ─────────────────────────────────────────────────────────────

/// Execute `Effect::PublishSetEvent` — calls `ActorCommand::PublishRawEvent`
/// with the kind:30004 event template. Same pattern as `run_effect_publish_highlight`.
///
/// No-op when `nmp` is `None` (test mode — tests inspect the `Effect` directly).
pub(crate) fn run_effect_publish_set_event(
    json: String,
    nmp: Option<&crate::kernel::actor::NmpHandle>,
) {
    let Some(handle) = nmp else {
        tracing::debug!("PublishSetEvent: no live NmpApp (test mode)");
        return;
    };
    let nmp_ref: &NmpApp = unsafe { handle.ptr.as_ref() };

    #[derive(serde::Deserialize)]
    struct EventTemplate {
        kind: u32,
        content: String,
        tags: Vec<Vec<String>>,
    }

    let Ok(template) = serde_json::from_str::<EventTemplate>(&json) else {
        tracing::warn!("PublishSetEvent: failed to deserialise event template — no-op (D6)");
        return;
    };

    let _ = nmp_ref
        .actor_sender()
        .send(nmp_core::actor::ActorCommand::Publish(
            nmp_core::actor::PublishCommand::RawEvent {
                kind: template.kind,
                content: template.content,
                tags: template.tags,
                target: nmp_core::publish::PublishTarget::Auto,
                signer_pubkey: None,
                correlation_id: None,
            },
        ));
}

// ── View-scoped interest lifecycle (#1653 BLOCKING #1 + HIGH #7) ─────────────

/// Holds the live projection Arcs so the view-close effect runner can clear the
/// observers' accumulated state (D5/D8 — bound memory across the session).
///
/// The typed-snapshot projection closures and event observers are registered
/// once at boot (their Arcs live forever inside the closures). What makes the
/// data flow VIEW-SCOPED is the interest lifecycle: the REQ-driving
/// `LogicalInterest` is pushed on `ViewId::Bookmarks` open and withdrawn on
/// close, and on close the accumulators are cleared here so nothing grows
/// unbounded while the view is shut.
struct SetProjectionsController {
    set_proj: Arc<SetListProjection>,
    web_proj: Arc<WebBookmarkProjection>,
}

static SET_PROJECTIONS_CONTROLLER: OnceLock<SetProjectionsController> = OnceLock::new();

/// Lifecycle hook on `Cmd::OpenView` — push the bookmark-sets subscription so
/// nmp emits REQ frames for kind:30003/30004/39701 authored by the active
/// account + follows (#1653 BLOCKING #1). No-op for any other view.
pub(crate) fn lifecycle_effects_for_view_open(id: &ViewId, state: &AppState) -> Vec<Effect> {
    if !matches!(id, ViewId::Bookmarks) {
        return vec![];
    }
    let mut authors: Vec<String> = Vec::new();
    if let Some(pk) = active_pubkey(state) {
        authors.push(pk.to_string());
    }
    authors.extend(state.follows.iter().cloned());
    authors.sort();
    authors.dedup();
    // Only emit when there is at least one author to subscribe for — an
    // unscoped (wildcard-author) sets interest would fan out to every relay.
    if authors.is_empty() {
        return vec![];
    }
    vec![Effect::PushBookmarkSetsInterest { authors }]
}

/// Re-push the bookmark-sets subscription for an already-open `ViewId::Bookmarks`
/// view with the refreshed author set (current user + current follows).
///
/// Called from the actor's post-reduce hook when a `FollowListUpdated` (follow
/// change) or `IdentityChanged` (account switch) event arrives WHILE the
/// Bookmarks view is open (#1653 codex BLOCKING #2). Without this, the interest
/// stays pinned to the authors captured at open time, so curation sets from
/// newly-followed authors are starved until the view is closed and reopened.
///
/// The push is idempotent: it reuses the stable `BOOKMARK_SETS_INTEREST_ID`, so
/// re-pushing replaces the prior interest in the planner (withdraw-then-push is
/// unnecessary — the planner keys on `InterestId`). Mirrors
/// `home_feed::lifecycle_effects_for_follow_update`.
///
/// #1653 BLOCKING #1 (withdraw-on-empty): when the refreshed author set is empty
/// — e.g. an `IdentityChanged(None)` (logout / account removal) arrives WHILE the
/// Bookmarks view is open — we must NOT just return `[]` and leave the stable
/// `BOOKMARK_SETS_INTEREST_ID` interest LIVE until the view eventually closes.
/// Instead emit `WithdrawBookmarkSetsInterest` so logout-while-open tears the
/// interest down immediately (and clears the boot-registered accumulators). This
/// mirrors how a follow-scoped lane stops subscribing once its author set drains.
///
/// D8: effect-driven, not polling — triggered by the projection event. Does not
/// ask Swift to close/reopen the view.
pub(crate) fn lifecycle_effects_for_follow_update(state: &AppState) -> Vec<Effect> {
    let mut authors: Vec<String> = Vec::new();
    if let Some(pk) = active_pubkey(state) {
        authors.push(pk.to_string());
    }
    authors.extend(state.follows.iter().cloned());
    authors.sort();
    authors.dedup();
    // Withdraw-on-empty: with no active account and no follows there is nothing
    // to subscribe for. Tear the live interest down rather than leaving it
    // pinned to the prior author set until the view closes (#1653 BLOCKING #1).
    // An empty author set only happens after logout / account removal while the
    // view is open — never push an unscoped (wildcard-author) interest (D6).
    if authors.is_empty() {
        return vec![Effect::WithdrawBookmarkSetsInterest];
    }
    vec![Effect::PushBookmarkSetsInterest { authors }]
}

/// Lifecycle hook on `Cmd::CloseView` — withdraw the bookmark-sets subscription
/// and clear the observers' accumulators (#1653 HIGH #7). No-op for other views.
pub(crate) fn lifecycle_effects_for_view_close(id: &ViewId) -> Vec<Effect> {
    if !matches!(id, ViewId::Bookmarks) {
        return vec![];
    }
    vec![Effect::WithdrawBookmarkSetsInterest]
}

/// Execute `Effect::PushBookmarkSetsInterest` — push a Tailing, ActiveAccount
/// `LogicalInterest` for kinds [30003,30004,39701] scoped to `authors`.
///
/// No-op when `nmp` is `None` (test mode — the reducer/lifecycle is tested by
/// inspecting the emitted `Effect`).
pub(crate) fn run_effect_push_interest(
    authors: Vec<String>,
    nmp: Option<&crate::kernel::actor::NmpHandle>,
) {
    let Some(handle) = nmp else {
        tracing::debug!("PushBookmarkSetsInterest: no live NmpApp (test mode)");
        return;
    };
    let nmp_ref: &NmpApp = unsafe { handle.ptr.as_ref() };

    let mut shape = InterestShape::default();
    shape.kinds.insert(KIND_BOOKMARK_SET);
    shape.kinds.insert(KIND_CURATION_SET);
    shape.kinds.insert(KIND_WEB_BOOKMARK);
    for a in &authors {
        // Pubkey is a raw hex String in the planner; only admit well-formed
        // 64-char lowercase-hex keys so a malformed author never reaches the wire.
        if a.len() == 64 && a.bytes().all(|b| b.is_ascii_hexdigit()) {
            shape.authors.insert(a.clone());
        }
    }
    if shape.authors.is_empty() {
        tracing::warn!(
            "PushBookmarkSetsInterest: no parseable authors — skipping unscoped interest (D6)"
        );
        return;
    }

    nmp_ref.ensure_interest(
        bookmark_sets_sub_identity(),
        LogicalInterest {
            id: InterestId(BOOKMARK_SETS_INTEREST_ID),
            scope: InterestScope::ActiveAccount,
            shape,
            hints: Vec::new(),
            lifecycle: InterestLifecycle::Tailing,
            is_indexer_discovery: false,
        },
    );
}

/// Execute `Effect::WithdrawBookmarkSetsInterest` — withdraw the interest and
/// clear the boot-registered projections' accumulators (D5/D8, #1653 HIGH #7).
///
/// No-op when `nmp` is `None`. The accumulator clear happens regardless so the
/// memory bound holds even in degraded modes.
pub(crate) fn run_effect_withdraw_interest(nmp: Option<&crate::kernel::actor::NmpHandle>) {
    if let Some(ctrl) = SET_PROJECTIONS_CONTROLLER.get() {
        ctrl.set_proj.clear();
        ctrl.web_proj.clear();
    }
    let Some(handle) = nmp else {
        return;
    };
    let nmp_ref: &NmpApp = unsafe { handle.ptr.as_ref() };
    let _ = nmp_ref
        .actor_sender()
        .send(nmp_core::actor::ActorCommand::Interests(
            nmp_core::actor::InterestsCommand::DropInterestOwner(bookmark_sets_sub_identity()),
        ));
}

// ── Projection registration ───────────────────────────────────────────────────

/// Wire the `SetListProjection` and `WebBookmarkProjection` event observers
/// plus their typed-snapshot projections against `nmp_ref`.
///
/// Must be called once at boot (after `nmp_app_start`). The `SetListProjection`
/// accumulates kind:30003/30004 events from all authors seen this session.
/// The `WebBookmarkProjection` filters to the active account via `active_account_slot`.
///
/// D6: observer-registration failures log a warning and degrade gracefully
/// (snapshots never update but the app does not crash).
pub(crate) fn register_set_projections(
    nmp_ref: &NmpApp,
    active_account_slot: Arc<Mutex<Option<String>>>,
) {
    // ── SetListProjection (kind:30003 + kind:30004, all authors) ─────────────
    let set_proj = Arc::new(SetListProjection::new());

    let observer_id = nmp_ref.open_observed_projection(ObservedProjection::from_kinds(
        Arc::clone(&set_proj) as Arc<dyn ObservedProjectionSink>,
        BOOKMARK_SETS_SCHEMA_ID,
        1,
        [KIND_BOOKMARK_SET, KIND_CURATION_SET],
        512,
    ));
    if observer_id.0 == 0 {
        tracing::warn!(
            "bookmark_sets::register_set_projections: SetListProjection observer registration failed (D6)"
        );
        // Still register the typed snapshot — it will just emit empty payloads.
    }

    let set_proj_ref = Arc::clone(&set_proj);
    nmp_ref.register_typed_snapshot_projection(BOOKMARK_SETS_SCHEMA_ID, move || {
        let payload = set_proj_ref.snapshot_payload()?;
        Some(nmp_core::TypedProjectionData {
            key: BOOKMARK_SETS_SCHEMA_ID.to_string(),
            schema_id: BOOKMARK_SETS_SCHEMA_ID.to_string(),
            schema_version: 1,
            file_identifier: String::new(),
            payload,
            ..Default::default()
        })
    });

    // ── WebBookmarkProjection (kind:39701, active account only) ──────────────
    let web_proj = Arc::new(WebBookmarkProjection::new(active_account_slot));

    let web_observer_id = nmp_ref.open_observed_projection(ObservedProjection::from_kinds(
        Arc::clone(&web_proj) as Arc<dyn ObservedProjectionSink>,
        WEB_BOOKMARKS_SCHEMA_ID,
        0,
        [KIND_WEB_BOOKMARK],
        512,
    ));
    if web_observer_id.0 == 0 {
        tracing::warn!(
            "bookmark_sets::register_set_projections: WebBookmarkProjection observer registration failed (D6)"
        );
    }

    let web_proj_ref = Arc::clone(&web_proj);
    nmp_ref.register_typed_snapshot_projection(WEB_BOOKMARKS_SCHEMA_ID, move || {
        let payload = web_proj_ref.snapshot_payload()?;
        Some(nmp_core::TypedProjectionData {
            key: WEB_BOOKMARKS_SCHEMA_ID.to_string(),
            schema_id: WEB_BOOKMARKS_SCHEMA_ID.to_string(),
            schema_version: 1,
            file_identifier: String::new(),
            payload,
            ..Default::default()
        })
    });

    // Store the projection Arcs so the view-close effect runner can clear their
    // accumulators (#1653 HIGH #7). `set` only succeeds on the first boot; a
    // Reset re-registers fresh observers but keeps the original controller Arcs
    // — the accumulators it clears are the live ones because the projections are
    // long-lived singletons (last-writer-wins on the typed-snapshot key).
    let _ = SET_PROJECTIONS_CONTROLLER.set(SetProjectionsController {
        set_proj: Arc::clone(&set_proj),
        web_proj: Arc::clone(&web_proj),
    });
}

// ── Kernel event parsing helpers ──────────────────────────────────────────────

/// Parse a `KernelEvent` (kind:30003 or kind:30004) into a `BookmarkSetRow`.
/// Raw fields only — no presentation strings (D1).
///
/// Fail-closed (D6, codex BLOCKING #2): returns `None` when the `d` tag is
/// missing or empty (NIP-33 requires a non-empty identifier; an empty `d`
/// would collide every author's sets under one key). Per-item values that are
/// missing or empty are skipped (the bad item is dropped, the row survives) —
/// matching the bespoke `parse_set_event` skip-on-empty behaviour. Total
/// parsing: `a`, `e`, AND `r` references plus `t` topics are all carried.
fn parse_set_row_from_kernel(event: &NmpKernelEvent) -> Option<BookmarkSetRow> {
    let mut d_tag: Option<String> = None;
    let mut title = None;
    let mut description = None;
    let mut image = None;
    let mut article_addresses = Vec::new();
    let mut note_ids = Vec::new();
    let mut r_refs = Vec::new();
    let mut topics = Vec::new();

    for tag in &event.tags {
        match tag.first().map(String::as_str) {
            Some("d") => {
                if d_tag.is_none() {
                    d_tag = tag.get(1).filter(|v| !v.is_empty()).cloned();
                }
            }
            Some("title") => {
                if title.is_none() {
                    title = tag.get(1).filter(|v| !v.is_empty()).cloned();
                }
            }
            Some("description") => {
                if description.is_none() {
                    description = tag.get(1).filter(|v| !v.is_empty()).cloned();
                }
            }
            Some("image") => {
                if image.is_none() {
                    image = tag.get(1).filter(|v| !v.is_empty()).cloned();
                }
            }
            Some("a") => {
                if let Some(v) = tag.get(1).filter(|v| !v.is_empty()) {
                    article_addresses.push(v.clone());
                }
            }
            Some("e") => {
                if let Some(v) = tag.get(1).filter(|v| !v.is_empty()) {
                    note_ids.push(v.clone());
                }
            }
            Some("r") => {
                if let Some(v) = tag.get(1).filter(|v| !v.is_empty()) {
                    r_refs.push(v.clone());
                }
            }
            Some("t") => {
                if let Some(v) = tag.get(1).filter(|v| !v.is_empty()) {
                    topics.push(v.clone());
                }
            }
            _ => {}
        }
    }

    // Fail closed: a set with no usable `d` identifier is rejected entirely.
    let d_tag = d_tag?;

    Some(BookmarkSetRow {
        d_tag,
        pubkey: event.author.clone(),
        kind: event.kind,
        title,
        description,
        image,
        article_addresses,
        note_ids,
        r_refs,
        topics,
        // Preserve the full raw event for lossless round-trip on write (#3).
        raw_tags: event.tags.clone(),
        content: event.content.clone(),
        created_at: event.created_at,
    })
}

/// Parse a `KernelEvent` (kind:39701) into a `WebBookmarkRow`.
///
/// Fail-closed (D6, codex BLOCKING #2): returns `None` when the `d` tag (the
/// URL-without-scheme that keys the bookmark) is missing or empty — a web
/// bookmark with no URL is meaningless and must not produce `url=""`.
fn parse_web_row_from_kernel(event: &NmpKernelEvent) -> Option<WebBookmarkRow> {
    let mut d: Option<String> = None;
    let mut title = None;
    let mut topics = Vec::new();
    let mut published_at = None;

    for tag in &event.tags {
        match tag.first().map(String::as_str) {
            Some("d") => {
                if d.is_none() {
                    d = tag.get(1).filter(|v| !v.is_empty()).cloned();
                }
            }
            Some("title") => {
                if title.is_none() {
                    title = tag.get(1).filter(|v| !v.is_empty()).cloned();
                }
            }
            Some("t") => {
                if let Some(v) = tag.get(1).filter(|v| !v.is_empty()) {
                    topics.push(v.clone());
                }
            }
            Some("published_at") if published_at.is_none() => {
                published_at = tag.get(1).and_then(|v| v.parse::<u64>().ok());
            }
            _ => {}
        }
    }

    // Fail closed: no `d` → no URL → reject the row entirely (never url="").
    let d = d?;
    let url = format!("https://{d}");

    let description = if event.content.is_empty() {
        None
    } else {
        Some(event.content.clone())
    };

    Some(WebBookmarkRow {
        url,
        pubkey: event.author.clone(),
        title,
        description,
        topics,
        published_at,
        created_at: event.created_at,
    })
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::action::{AppAction, KernelEvent};
    use crate::kernel::actor::{reduce, Cmd};
    use crate::kernel::app::SessionState;
    use crate::kernel::clock::{Clock, ManualClock};

    fn make_state() -> AppState {
        AppState::default()
    }

    fn make_state_with_session(pubkey: &str) -> AppState {
        let mut state = AppState::default();
        state.session = SessionState::Present {
            pubkey: pubkey.to_string(),
            signer_kind: crate::kernel::action::SignerKind::LocalNsec,
        };
        state
    }

    fn step(state: &mut AppState, clock: &ManualClock, cmd: Cmd) -> Vec<Effect> {
        let now = clock.now_unix_seconds();
        reduce(state, cmd, now)
    }

    /// Minimal `BookmarkSetRow` test builder. New lossless fields (`r_refs`,
    /// `topics`, `raw_tags`, `content`) default empty — tests that exercise the
    /// write path set `raw_tags` explicitly.
    fn row_set(d: &str, pk: &str, kind: u32) -> BookmarkSetRow {
        BookmarkSetRow {
            d_tag: d.to_string(),
            pubkey: pk.to_string(),
            kind,
            title: None,
            description: None,
            image: None,
            article_addresses: vec![],
            note_ids: vec![],
            r_refs: vec![],
            topics: vec![],
            raw_tags: vec![],
            content: String::new(),
            created_at: 1000,
        }
    }

    fn row_web(url: &str, pk: &str) -> WebBookmarkRow {
        WebBookmarkRow {
            url: url.to_string(),
            pubkey: pk.to_string(),
            title: None,
            description: None,
            topics: vec![],
            published_at: None,
            created_at: 1000,
        }
    }

    // 1653-T1: BookmarkSetsUpdated stores raw rows in AppState.
    #[test]
    fn bookmark_sets_updated_stores_rows() {
        let pk = "aaaa000000000000000000000000000000000000000000000000000000000001";
        let mut state = make_state_with_session(pk);
        let clock = ManualClock::default();

        let mut row = row_set("my-set", pk, 30003);
        row.title = Some("My Set".to_string());
        row.article_addresses = vec!["30023:aabb:slug".to_string()];

        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::BookmarkSetsUpdated {
                all_bookmark_sets: vec![row.clone()],
                all_curation_sets: vec![],
            }),
        );

        assert_eq!(state.all_bookmark_sets.len(), 1);
        assert_eq!(state.all_bookmark_sets[0].d_tag, "my-set");
        assert_eq!(
            state.all_bookmark_sets[0].article_addresses,
            vec!["30023:aabb:slug"]
        );
    }

    // 1653-T2: WebBookmarksUpdated stores rows in AppState.
    #[test]
    fn web_bookmarks_updated_stores_rows() {
        let pk = "aaaa000000000000000000000000000000000000000000000000000000000001";
        let mut state = make_state_with_session(pk);
        let clock = ManualClock::default();

        let mut row = row_web("https://example.com/article", pk);
        row.title = Some("Article Title".to_string());
        row.description = Some("A description".to_string());
        row.topics = vec!["nostr".to_string()];
        row.created_at = 2000;

        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::WebBookmarksUpdated(vec![row.clone()])),
        );

        assert_eq!(state.web_bookmarks.len(), 1);
        assert_eq!(state.web_bookmarks[0].url, "https://example.com/article");
    }

    // 1653-T3: projection helpers filter by active pubkey and follows.
    #[test]
    fn projection_filters_by_identity() {
        let my_pk = "aaaa000000000000000000000000000000000000000000000000000000000001";
        let followed_pk = "bbbb000000000000000000000000000000000000000000000000000000000002";
        let other_pk = "cccc000000000000000000000000000000000000000000000000000000000003";
        let mut state = make_state_with_session(my_pk);
        state.follows = vec![followed_pk.to_string()];

        let make_set = |pk: &str, d: &str, kind: u32| row_set(d, pk, kind);

        state.all_bookmark_sets = vec![
            make_set(my_pk, "bm-mine", 30003),
            make_set(other_pk, "bm-other", 30003),
        ];
        state.all_curation_sets = vec![
            make_set(my_pk, "cur-mine", 30004),
            make_set(followed_pk, "cur-followed", 30004),
            make_set(other_pk, "cur-other", 30004),
        ];

        let my_bm = project_my_bookmark_sets(&state);
        assert_eq!(my_bm.len(), 1, "only my bookmark set");
        assert_eq!(my_bm[0].d_tag, "bm-mine");

        let my_cur = project_my_curation_sets(&state);
        assert_eq!(my_cur.len(), 1, "only my curation set");
        assert_eq!(my_cur[0].d_tag, "cur-mine");

        let following = project_following_curation_sets(&state);
        assert_eq!(following.len(), 1, "only followed curation set");
        assert_eq!(following[0].d_tag, "cur-followed");
    }

    // 1653-T4: AddToSet emits PublishSetEvent with correct tags.
    #[test]
    fn add_to_set_emits_publish_effect() {
        let pk = "aaaa000000000000000000000000000000000000000000000000000000000001";
        let mut state = make_state_with_session(pk);
        let clock = ManualClock::default();

        let mut set_row = row_set("my-curations", pk, 30004);
        set_row.title = Some("My Curations".to_string());
        state.all_curation_sets = vec![set_row];

        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::AddToSet {
                set_coordinate: format!("30004:{pk}:my-curations"),
                item_coordinate: "30023:aabb:article".to_string(),
            }),
        );

        assert_eq!(effects.len(), 1, "AddToSet must emit exactly one effect");
        match &effects[0] {
            Effect::PublishSetEvent { json } => {
                let parsed: serde_json::Value = serde_json::from_str(json).unwrap();
                assert_eq!(parsed["kind"].as_u64(), Some(30004));
                let tags = parsed["tags"].as_array().unwrap();
                let has_a_tag = tags.iter().any(|t| {
                    t.as_array().and_then(|arr| {
                        if arr.get(0)?.as_str() == Some("a") {
                            arr.get(1)?.as_str()
                        } else {
                            None
                        }
                    }) == Some("30023:aabb:article")
                });
                assert!(
                    has_a_tag,
                    "event must include the new article address as 'a' tag"
                );
                let has_d_tag = tags.iter().any(|t| {
                    t.as_array().and_then(|arr| {
                        if arr.get(0)?.as_str() == Some("d") {
                            arr.get(1)?.as_str()
                        } else {
                            None
                        }
                    }) == Some("my-curations")
                });
                assert!(has_d_tag, "event must include the 'd' tag");
            }
            other => panic!("expected PublishSetEvent, got: {other:?}"),
        }
    }

    // 1653-T5: RemoveFromSet emits PublishSetEvent without the removed item.
    #[test]
    fn remove_from_set_emits_publish_effect() {
        let pk = "aaaa000000000000000000000000000000000000000000000000000000000001";
        let mut state = make_state_with_session(pk);
        let clock = ManualClock::default();
        let target = "30023:aabb:article";

        let mut set_row = row_set("my-curations", pk, 30004);
        set_row.article_addresses = vec![target.to_string(), "30023:ccdd:other".to_string()];
        state.all_curation_sets = vec![set_row];

        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::RemoveFromSet {
                set_coordinate: format!("30004:{pk}:my-curations"),
                item_coordinate: target.to_string(),
            }),
        );

        assert_eq!(
            effects.len(),
            1,
            "RemoveFromSet must emit exactly one effect"
        );
        match &effects[0] {
            Effect::PublishSetEvent { json } => {
                let parsed: serde_json::Value = serde_json::from_str(json).unwrap();
                let tags = parsed["tags"].as_array().unwrap();
                let removed = tags.iter().any(|t| {
                    t.as_array().and_then(|arr| {
                        if arr.get(0)?.as_str() == Some("a") {
                            arr.get(1)?.as_str()
                        } else {
                            None
                        }
                    }) == Some(target)
                });
                assert!(
                    !removed,
                    "removed article must NOT appear in the event tags"
                );
            }
            other => panic!("expected PublishSetEvent, got: {other:?}"),
        }
    }

    // 1653-T6: AddToSet is idempotent — no-op if item already present.
    #[test]
    fn add_to_set_idempotent_when_already_present() {
        let pk = "aaaa000000000000000000000000000000000000000000000000000000000001";
        let mut state = make_state_with_session(pk);
        let clock = ManualClock::default();
        let existing = "30023:aabb:article";

        let mut set_row = row_set("my-curations", pk, 30004);
        set_row.article_addresses = vec![existing.to_string()];
        state.all_curation_sets = vec![set_row];

        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::AddToSet {
                set_coordinate: format!("30004:{pk}:my-curations"),
                item_coordinate: existing.to_string(),
            }),
        );

        assert!(
            effects.is_empty(),
            "AddToSet must be no-op when item already present"
        );
    }

    // 1653-T7: AddToSet no-op when set not found.
    #[test]
    fn add_to_set_noop_when_set_not_found() {
        let pk = "aaaa000000000000000000000000000000000000000000000000000000000001";
        let mut state = make_state_with_session(pk);
        let clock = ManualClock::default();

        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::AddToSet {
                set_coordinate: format!("30004:{pk}:nonexistent"),
                item_coordinate: "30023:aabb:article".to_string(),
            }),
        );

        assert!(
            effects.is_empty(),
            "AddToSet must be no-op when set not found (D6)"
        );
    }

    // 1653-T8: sets and web bookmarks cleared on logout.
    #[test]
    fn sets_and_web_cleared_on_logout() {
        let pk = "aaaa000000000000000000000000000000000000000000000000000000000001";
        let mut state = make_state_with_session(pk);
        let clock = ManualClock::default();

        state.all_bookmark_sets = vec![row_set("s", pk, 30003)];
        state.all_curation_sets = state.all_bookmark_sets.clone();
        state.web_bookmarks = vec![row_web("https://example.com", pk)];

        step(&mut state, &clock, Cmd::Action(AppAction::Logout));

        assert!(
            state.all_bookmark_sets.is_empty(),
            "bookmark_sets must clear on logout"
        );
        assert!(
            state.all_curation_sets.is_empty(),
            "curation_sets must clear on logout"
        );
        assert!(
            state.web_bookmarks.is_empty(),
            "web_bookmarks must clear on logout"
        );
    }

    // 1653-BLOCKING-#2: a FollowListUpdated arriving WHILE Bookmarks is open must
    // re-push the bookmarks interest with the refreshed author set (current user
    // + current follows), so newly-followed curation-set authors are subscribed
    // without a view close/reopen. This is the domain hook the actor invokes from
    // its post-reduce gate (`registry.is_open(&ViewId::Bookmarks)`).
    #[test]
    fn follow_update_repushes_bookmarks_interest_with_new_authors() {
        let my_pk = "aaaa000000000000000000000000000000000000000000000000000000000001";
        let old_follow = "bbbb000000000000000000000000000000000000000000000000000000000002";
        let new_follow = "cccc000000000000000000000000000000000000000000000000000000000003";
        let mut state = make_state_with_session(my_pk);

        // Bookmarks opened with only the original follow in scope.
        state.follows = vec![old_follow.to_string()];
        let open = lifecycle_effects_for_view_open(&ViewId::Bookmarks, &state);
        match open.as_slice() {
            [Effect::PushBookmarkSetsInterest { authors }] => {
                assert!(authors.contains(&my_pk.to_string()));
                assert!(authors.contains(&old_follow.to_string()));
                assert!(
                    !authors.contains(&new_follow.to_string()),
                    "new follow not yet known at open time"
                );
            }
            other => panic!("expected one PushBookmarkSetsInterest on open, got {other:?}"),
        }

        // A FollowListUpdated arrives while the view is open: follows now include
        // the newly-followed author.
        state.follows = vec![old_follow.to_string(), new_follow.to_string()];
        let effects = lifecycle_effects_for_follow_update(&state);
        match effects.as_slice() {
            [Effect::PushBookmarkSetsInterest { authors }] => {
                assert!(
                    authors.contains(&new_follow.to_string()),
                    "re-pushed interest must include the newly-followed author"
                );
                assert!(authors.contains(&old_follow.to_string()));
                assert!(
                    authors.contains(&my_pk.to_string()),
                    "re-pushed interest must keep the current user"
                );
            }
            other => {
                panic!("expected one PushBookmarkSetsInterest on follow update, got {other:?}")
            }
        }
    }

    // 1653-BLOCKING-#1: the follow-update hook stays fail-closed AND withdraws —
    // with no active account and no follows there is no author to scope, so it
    // must NOT push an unscoped (wildcard-author) interest. Instead it must emit
    // `WithdrawBookmarkSetsInterest` so a stable interest left LIVE from a prior
    // (logged-in) author set is torn down immediately rather than lingering until
    // the view closes (withdraw-on-empty).
    #[test]
    fn follow_update_withdraws_without_authors() {
        let state = make_state();
        let effects = lifecycle_effects_for_follow_update(&state);
        assert!(
            matches!(effects.as_slice(), [Effect::WithdrawBookmarkSetsInterest]),
            "follow update with no user + no follows must withdraw the interest \
             (never push an unscoped interest), got {effects:?}"
        );
    }

    // 1653-BLOCKING-#1 (logout-while-open): a logout / account removal
    // (IdentityChanged(None)) arriving WHILE the Bookmarks view is open must
    // WITHDRAW the live BOOKMARK_SETS_INTEREST_ID interest, not leave it pinned
    // to the prior account's author set until the view eventually closes.
    //
    // The actor routes IdentityChanged(None) through the auth reducer (which
    // clears session + follows) and then invokes
    // `lifecycle_effects_for_follow_update` when the Bookmarks view is open. This
    // test drives that exact sequence and asserts the hook tears the interest
    // down. PRE-FIX the hook returned `vec![]` on an empty author set, so this
    // assertion (matching WithdrawBookmarkSetsInterest) would fail.
    #[test]
    fn logout_while_bookmarks_open_withdraws_interest() {
        let my_pk = "aaaa000000000000000000000000000000000000000000000000000000000001";
        let follow = "bbbb000000000000000000000000000000000000000000000000000000000002";
        let mut state = make_state_with_session(my_pk);
        let clock = ManualClock::default();

        // Bookmarks opened with an active account + one follow → an interest with
        // a non-empty author set is live.
        state.follows = vec![follow.to_string()];
        let open = lifecycle_effects_for_view_open(&ViewId::Bookmarks, &state);
        assert!(
            matches!(open.as_slice(), [Effect::PushBookmarkSetsInterest { .. }]),
            "open must push a scoped interest, got {open:?}"
        );

        // Logout (IdentityChanged(None)) arrives while the view is open. The auth
        // reducer clears session + follows.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(None)),
        );
        assert!(active_pubkey(&state).is_none(), "session must be absent");
        assert!(
            state.follows.is_empty(),
            "follows must be cleared on logout"
        );

        // The actor's post-reduce hook (gated on the Bookmarks view being open)
        // refreshes the interest. With no authors it must WITHDRAW, not no-op.
        let effects = lifecycle_effects_for_follow_update(&state);
        assert!(
            matches!(effects.as_slice(), [Effect::WithdrawBookmarkSetsInterest]),
            "logout while Bookmarks open must withdraw the live interest, got {effects:?}"
        );
    }

    // 1653-BLOCKING-#2 (direct account switch — no cross-account leak): a DIRECT
    // IdentityChanged(Some(new_pk)) with NO intervening None (nmp supports this —
    // `active_account_handle_reflects_account_switch`) arriving WHILE the
    // Bookmarks view is open must re-push an interest scoped to ONLY the new
    // account — NEVER the prior account's follows. The prior account's follows
    // are still in `state.follows` until the new account's follow sidecar
    // arrives, so the auth reducer rebaselines (clears) follows on the Some arm.
    //
    // This test drives the actor's exact sequence: open under account A with
    // follows, then IdentityChanged(Some(B)), then the post-reduce re-push hook.
    // PRE-FIX (no follows-clear on the Some arm) the re-pushed interest would
    // still contain A's prior follow — this test's "must NOT contain prior
    // follow" assertion would fail.
    #[test]
    fn direct_account_switch_while_open_drops_prior_follows() {
        let acct_a = "aaaa000000000000000000000000000000000000000000000000000000000001";
        let a_follow = "bbbb000000000000000000000000000000000000000000000000000000000002";
        let acct_b = "cccc000000000000000000000000000000000000000000000000000000000003";
        let mut state = make_state_with_session(acct_a);
        let clock = ManualClock::default();

        // Account A open with one follow in scope.
        state.follows = vec![a_follow.to_string()];
        let open = lifecycle_effects_for_view_open(&ViewId::Bookmarks, &state);
        match open.as_slice() {
            [Effect::PushBookmarkSetsInterest { authors }] => {
                assert!(authors.contains(&acct_a.to_string()));
                assert!(authors.contains(&a_follow.to_string()));
            }
            other => panic!("expected one PushBookmarkSetsInterest on open, got {other:?}"),
        }

        // DIRECT switch to account B (no intervening None). The auth reducer
        // rebaselines: session → B, follows → empty.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(Some(acct_b.to_string()))),
        );
        assert_eq!(active_pubkey(&state), Some(acct_b));
        assert!(
            state.follows.is_empty(),
            "direct switch must rebaseline follows so A's follows don't leak into B"
        );

        // The post-reduce re-push hook (gated on Bookmarks open) refreshes the
        // interest. It must contain ONLY account B — never account A or A's follow.
        let effects = lifecycle_effects_for_follow_update(&state);
        match effects.as_slice() {
            [Effect::PushBookmarkSetsInterest { authors }] => {
                assert!(
                    authors.contains(&acct_b.to_string()),
                    "re-pushed interest must include the new account B"
                );
                assert!(
                    !authors.contains(&a_follow.to_string()),
                    "re-pushed interest must NOT include the prior account's follow (privacy leak)"
                );
                assert!(
                    !authors.contains(&acct_a.to_string()),
                    "re-pushed interest must NOT include the prior account itself"
                );
                assert_eq!(
                    authors.as_slice(),
                    [acct_b.to_string()],
                    "after a direct switch the interest contains ONLY the new account \
                     (its own follows fold in later via FollowListUpdated)"
                );
            }
            other => panic!("expected one PushBookmarkSetsInterest on switch, got {other:?}"),
        }
    }

    // ── Issue #63: RenameSet, DeleteSet, CreateSet ────────────────────────────

    // 1653-T-RENAME-1: rename_set emits PublishSetEvent preserving membership and
    // all non-`title` tags (round-trip fidelity — top correctness risk).
    #[test]
    fn rename_set_emits_publish_preserving_membership() {
        let pk = "aaaa000000000000000000000000000000000000000000000000000000000001";
        let mut state = make_state_with_session(pk);
        let clock = ManualClock::default();

        let a1 = "30023:bbbb000000000000000000000000000000000000000000000000000000000002:essay";
        let a2 = "30023:cccc000000000000000000000000000000000000000000000000000000000003:talk";

        let mut set_row = row_set("d", pk, 30004);
        set_row.title = Some("Old Title".to_string());
        set_row.description = Some("A description".to_string());
        set_row.article_addresses = vec![a1.to_string(), a2.to_string()];
        // raw_tags simulating a real round-trip event
        set_row.raw_tags = vec![
            vec!["d".to_string(), "d".to_string()],
            vec!["title".to_string(), "Old Title".to_string()],
            vec!["description".to_string(), "A description".to_string()],
            vec!["a".to_string(), a1.to_string()],
            vec!["a".to_string(), a2.to_string()],
        ];
        state.all_curation_sets = vec![set_row];

        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::RenameSet {
                set_coordinate: format!("30004:{pk}:d"),
                title: "New Title".to_string(),
            }),
        );

        assert_eq!(effects.len(), 1, "RenameSet must emit exactly one effect");
        match &effects[0] {
            Effect::PublishSetEvent { json } => {
                let parsed: serde_json::Value = serde_json::from_str(json).unwrap();
                assert_eq!(parsed["kind"].as_u64(), Some(30004));
                let tags = parsed["tags"].as_array().unwrap();

                // d_tag preserved
                let has_d = tags.iter().any(|t| {
                    t.as_array()
                        .map(|a| {
                            a.first().and_then(|v| v.as_str()) == Some("d")
                                && a.get(1).and_then(|v| v.as_str()) == Some("d")
                        })
                        .unwrap_or(false)
                });
                assert!(has_d, "d tag must be preserved");

                // NEW title present
                let has_new_title = tags.iter().any(|t| {
                    t.as_array()
                        .map(|a| {
                            a.first().and_then(|v| v.as_str()) == Some("title")
                                && a.get(1).and_then(|v| v.as_str()) == Some("New Title")
                        })
                        .unwrap_or(false)
                });
                assert!(has_new_title, "new title must be in tags");

                // OLD title NOT present
                let has_old_title = tags.iter().any(|t| {
                    t.as_array()
                        .map(|a| {
                            a.first().and_then(|v| v.as_str()) == Some("title")
                                && a.get(1).and_then(|v| v.as_str()) == Some("Old Title")
                        })
                        .unwrap_or(false)
                });
                assert!(!has_old_title, "old title must NOT be in tags");

                // description preserved
                let has_desc = tags.iter().any(|t| {
                    t.as_array()
                        .map(|a| {
                            a.first().and_then(|v| v.as_str()) == Some("description")
                                && a.get(1).and_then(|v| v.as_str()) == Some("A description")
                        })
                        .unwrap_or(false)
                });
                assert!(has_desc, "description must be preserved");

                // BOTH `a` tags preserved
                let has_a1 = tags.iter().any(|t| {
                    t.as_array()
                        .map(|a| {
                            a.first().and_then(|v| v.as_str()) == Some("a")
                                && a.get(1).and_then(|v| v.as_str()) == Some(a1)
                        })
                        .unwrap_or(false)
                });
                let has_a2 = tags.iter().any(|t| {
                    t.as_array()
                        .map(|a| {
                            a.first().and_then(|v| v.as_str()) == Some("a")
                                && a.get(1).and_then(|v| v.as_str()) == Some(a2)
                        })
                        .unwrap_or(false)
                });
                assert!(has_a1, "first a tag must be preserved");
                assert!(has_a2, "second a tag must be preserved");
            }
            other => panic!("expected PublishSetEvent, got: {other:?}"),
        }
    }

    // 1653-T-RENAME-2: rename_set no-op when set not found (D6).
    #[test]
    fn rename_set_noop_when_set_not_found() {
        let pk = "aaaa000000000000000000000000000000000000000000000000000000000001";
        let mut state = make_state_with_session(pk);
        let clock = ManualClock::default();

        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::RenameSet {
                set_coordinate: format!("30004:{pk}:nonexistent"),
                title: "New Title".to_string(),
            }),
        );
        assert!(
            effects.is_empty(),
            "RenameSet must be no-op when set not found (D6)"
        );
    }

    // 1653-T-DELETE-1: delete_set emits kind:5 PublishSetEvent with `a` coordinate
    // and `k` == "30004" tags.
    #[test]
    fn delete_set_emits_kind5_with_a_coordinate() {
        let pk = "aaaa000000000000000000000000000000000000000000000000000000000001";
        let mut state = make_state_with_session(pk);
        let clock = ManualClock::default();

        let set_row = row_set("my-set", pk, 30004);
        state.all_curation_sets = vec![set_row];

        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::DeleteSet {
                set_coordinate: format!("30004:{pk}:my-set"),
            }),
        );

        assert_eq!(effects.len(), 1, "DeleteSet must emit exactly one effect");
        match &effects[0] {
            Effect::PublishSetEvent { json } => {
                let parsed: serde_json::Value = serde_json::from_str(json).unwrap();
                assert_eq!(
                    parsed["kind"].as_u64(),
                    Some(5),
                    "deletion event must be kind 5"
                );
                let tags = parsed["tags"].as_array().unwrap();

                // `a` tag with the coordinate
                let expected_coord = format!("30004:{pk}:my-set");
                let has_a_coord = tags.iter().any(|t| {
                    t.as_array()
                        .map(|a| {
                            a.first().and_then(|v| v.as_str()) == Some("a")
                                && a.get(1).and_then(|v| v.as_str())
                                    == Some(expected_coord.as_str())
                        })
                        .unwrap_or(false)
                });
                assert!(
                    has_a_coord,
                    "deletion event must have `a` tag with the set coordinate"
                );

                // `k` == "30004" tag
                let has_k_tag = tags.iter().any(|t| {
                    t.as_array()
                        .map(|a| {
                            a.first().and_then(|v| v.as_str()) == Some("k")
                                && a.get(1).and_then(|v| v.as_str()) == Some("30004")
                        })
                        .unwrap_or(false)
                });
                assert!(
                    has_k_tag,
                    "deletion event must have `k`==\"30004\" tag"
                );
            }
            other => panic!("expected PublishSetEvent, got: {other:?}"),
        }
    }

    // 1653-T-DELETE-2: delete_set no-op when set not found (D6).
    #[test]
    fn delete_set_noop_when_set_not_found() {
        let pk = "aaaa000000000000000000000000000000000000000000000000000000000001";
        let mut state = make_state_with_session(pk);
        let clock = ManualClock::default();

        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::DeleteSet {
                set_coordinate: format!("30004:{pk}:nonexistent"),
            }),
        );
        assert!(
            effects.is_empty(),
            "DeleteSet must be no-op when set not found (D6)"
        );
    }

    // #63 ownership safety: rename_set must NOT touch a set authored by someone
    // else (a `following_curation_sets` row living in `all_curation_sets`). We
    // can't sign as them and must never publish a kind:30004 under their
    // coordinate — D6 no-op.
    #[test]
    fn rename_set_noop_when_not_owned() {
        let me = "aaaa000000000000000000000000000000000000000000000000000000000001";
        let other = "dddd000000000000000000000000000000000000000000000000000000000004";
        let mut state = make_state_with_session(me);
        let clock = ManualClock::default();

        // A set authored by `other`, present in all_curation_sets (Explore pane).
        let mut set_row = row_set("their-set", other, 30004);
        set_row.title = Some("Their Title".to_string());
        set_row.raw_tags = vec![
            vec!["d".to_string(), "their-set".to_string()],
            vec!["title".to_string(), "Their Title".to_string()],
        ];
        state.all_curation_sets = vec![set_row];

        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::RenameSet {
                set_coordinate: format!("30004:{other}:their-set"),
                title: "Hijacked Title".to_string(),
            }),
        );
        assert!(
            effects.is_empty(),
            "RenameSet must be no-op for a set not owned by the active account (D6)"
        );
    }

    // #63 ownership safety: delete_set must NOT issue a kind:5 deletion for a set
    // authored by someone else — D6 no-op.
    #[test]
    fn delete_set_noop_when_not_owned() {
        let me = "aaaa000000000000000000000000000000000000000000000000000000000001";
        let other = "dddd000000000000000000000000000000000000000000000000000000000004";
        let mut state = make_state_with_session(me);
        let clock = ManualClock::default();

        let set_row = row_set("their-set", other, 30004);
        state.all_curation_sets = vec![set_row];

        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::DeleteSet {
                set_coordinate: format!("30004:{other}:their-set"),
            }),
        );
        assert!(
            effects.is_empty(),
            "DeleteSet must be no-op for a set not owned by the active account (D6)"
        );
    }

    // 1653-T-CREATE-1: create_set emits empty kind:30004 event with title and
    // derived d tag, NO `a` tag.
    #[test]
    fn create_set_emits_empty_curation_set() {
        let pk = "aaaa000000000000000000000000000000000000000000000000000000000001";
        let mut state = make_state_with_session(pk);
        let clock = ManualClock::default();

        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::CreateSet {
                title: "My New Collection".to_string(),
            }),
        );

        assert_eq!(effects.len(), 1, "CreateSet must emit exactly one effect");
        match &effects[0] {
            Effect::PublishSetEvent { json } => {
                let parsed: serde_json::Value = serde_json::from_str(json).unwrap();
                assert_eq!(parsed["kind"].as_u64(), Some(30004));
                let tags = parsed["tags"].as_array().unwrap();

                // title tag present
                let has_title = tags.iter().any(|t| {
                    t.as_array()
                        .map(|a| {
                            a.first().and_then(|v| v.as_str()) == Some("title")
                                && a.get(1).and_then(|v| v.as_str())
                                    == Some("My New Collection")
                        })
                        .unwrap_or(false)
                });
                assert!(has_title, "title tag must be present");

                // d tag present (derived from title)
                let has_d = tags.iter().any(|t| {
                    t.as_array()
                        .map(|a| a.first().and_then(|v| v.as_str()) == Some("d"))
                        .unwrap_or(false)
                });
                assert!(has_d, "d tag must be present");

                // NO `a` tag
                let has_a = tags.iter().any(|t| {
                    t.as_array()
                        .map(|a| a.first().and_then(|v| v.as_str()) == Some("a"))
                        .unwrap_or(false)
                });
                assert!(!has_a, "create_set must NOT have any `a` tags (empty set)");
            }
            other => panic!("expected PublishSetEvent, got: {other:?}"),
        }
    }

    // 1653-T-NS-RENAME: string-namespace routing for rename_set must produce
    // the same effect as the typed arm.
    #[test]
    fn namespace_routing_rename_set() {
        use crate::kernel::action::AppActionEnvelope;
        let pk = "aaaa000000000000000000000000000000000000000000000000000000000001";
        let mut state = make_state_with_session(pk);
        let clock = ManualClock::default();

        let mut set_row = row_set("my-set", pk, 30004);
        set_row.title = Some("Old Title".to_string());
        set_row.raw_tags = vec![
            vec!["d".to_string(), "my-set".to_string()],
            vec!["title".to_string(), "Old Title".to_string()],
        ];
        state.all_curation_sets = vec![set_row.clone()];

        // Via typed arm
        let typed_effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::RenameSet {
                set_coordinate: format!("30004:{pk}:my-set"),
                title: "New Title".to_string(),
            }),
        );
        assert_eq!(typed_effects.len(), 1, "typed RenameSet must emit one effect");
        let Effect::PublishSetEvent { json: typed_json } = &typed_effects[0] else {
            panic!("expected PublishSetEvent");
        };

        // Re-seed state
        state.all_curation_sets = vec![set_row];

        // Via string-namespace arm
        let ns_payload = serde_json::json!({
            "set_coordinate": format!("30004:{pk}:my-set"),
            "title": "New Title",
        });
        let ns_effects = step(
            &mut state,
            &clock,
            Cmd::ActionEnvelope(AppActionEnvelope {
                namespace: "hl.curation.rename_set".to_string(),
                json: ns_payload.to_string(),
            }),
        );
        assert_eq!(
            ns_effects.len(),
            1,
            "string-namespace RenameSet must emit one effect"
        );
        let Effect::PublishSetEvent { json: ns_json } = &ns_effects[0] else {
            panic!("expected PublishSetEvent from namespace route");
        };

        let typed_v: serde_json::Value = serde_json::from_str(typed_json).unwrap();
        let ns_v: serde_json::Value = serde_json::from_str(ns_json).unwrap();
        assert_eq!(
            typed_v, ns_v,
            "typed and namespace paths must produce identical effects"
        );
    }

    // 1653-T-NS-DELETE: string-namespace routing for delete_set.
    #[test]
    fn namespace_routing_delete_set() {
        use crate::kernel::action::AppActionEnvelope;
        let pk = "aaaa000000000000000000000000000000000000000000000000000000000001";
        let mut state = make_state_with_session(pk);
        let clock = ManualClock::default();

        let set_row = row_set("my-set", pk, 30004);
        state.all_curation_sets = vec![set_row.clone()];

        // Via typed arm
        let typed_effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::DeleteSet {
                set_coordinate: format!("30004:{pk}:my-set"),
            }),
        );
        assert_eq!(typed_effects.len(), 1);

        // Re-seed state
        state.all_curation_sets = vec![set_row];

        // Via string-namespace arm
        let ns_payload = serde_json::json!({
            "set_coordinate": format!("30004:{pk}:my-set"),
        });
        let ns_effects = step(
            &mut state,
            &clock,
            Cmd::ActionEnvelope(AppActionEnvelope {
                namespace: "hl.curation.delete_set".to_string(),
                json: ns_payload.to_string(),
            }),
        );
        assert_eq!(
            ns_effects.len(),
            1,
            "string-namespace DeleteSet must emit one effect"
        );
        let Effect::PublishSetEvent { json: typed_j } = &typed_effects[0] else {
            panic!()
        };
        let Effect::PublishSetEvent { json: ns_j } = &ns_effects[0] else {
            panic!()
        };
        let t: serde_json::Value = serde_json::from_str(typed_j).unwrap();
        let n: serde_json::Value = serde_json::from_str(ns_j).unwrap();
        assert_eq!(
            t, n,
            "typed and namespace delete paths must produce identical effects"
        );
    }

    // 1653-T-NS-CREATE: string-namespace routing for create_set.
    #[test]
    fn namespace_routing_create_set() {
        use crate::kernel::action::AppActionEnvelope;
        let pk = "aaaa000000000000000000000000000000000000000000000000000000000001";
        let mut state = make_state_with_session(pk);
        let clock = ManualClock::default();

        // Via typed arm
        let typed_effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::CreateSet {
                title: "Collection A".to_string(),
            }),
        );
        assert_eq!(typed_effects.len(), 1);

        // Via string-namespace arm
        let ns_payload = serde_json::json!({
            "title": "Collection A",
        });
        let ns_effects = step(
            &mut state,
            &clock,
            Cmd::ActionEnvelope(AppActionEnvelope {
                namespace: "hl.curation.create_set".to_string(),
                json: ns_payload.to_string(),
            }),
        );
        assert_eq!(
            ns_effects.len(),
            1,
            "string-namespace CreateSet must emit one effect"
        );
        let Effect::PublishSetEvent { json: typed_j } = &typed_effects[0] else {
            panic!()
        };
        let Effect::PublishSetEvent { json: ns_j } = &ns_effects[0] else {
            panic!()
        };
        let t: serde_json::Value = serde_json::from_str(typed_j).unwrap();
        let n: serde_json::Value = serde_json::from_str(ns_j).unwrap();
        // kind must match; d_tag differs (different timestamps) so only check kind+title
        assert_eq!(t["kind"], n["kind"], "kind must match");
        let t_title = t["tags"]
            .as_array()
            .unwrap()
            .iter()
            .find(|tag| tag[0].as_str() == Some("title"))
            .and_then(|t| t[1].as_str())
            .unwrap_or("");
        let n_title = n["tags"]
            .as_array()
            .unwrap()
            .iter()
            .find(|tag| tag[0].as_str() == Some("title"))
            .and_then(|t| t[1].as_str())
            .unwrap_or("");
        assert_eq!(
            t_title, n_title,
            "title must match between typed and namespace paths"
        );
    }

    // ── Parity tests: ONE consumer-shaped fixture, bespoke vs kernel ──────────
    //
    // Gotcha #7b/#7c: a single rich fixture event carries every tag dimension a
    // real consumer set has — `d` + `title` + `a` items + `e` items + `r` items
    // (+ `t` topics + description/image + custom client tag + non-empty content).
    // Both the bespoke parser/writer and the kernel port consume the SAME bytes,
    // and we `assert_eq` on the concrete VALUES (id, title, every coordinate
    // incl. `r`, web url/title) — never on counts. A `guard_bites_*` test breaks
    // one field and proves the equality assertion fails.
    mod parity {
        use super::*;
        use nostr_sdk::prelude::*;

        fn nostr_to_kernel(e: &Event) -> nmp_core::substrate::KernelEvent {
            nmp_core::substrate::KernelEvent {
                id: e.id.to_hex(),
                author: e.pubkey.to_hex(),
                kind: e.kind.as_u16() as u32,
                created_at: e.created_at.as_secs(),
                tags: e.tags.iter().map(|t| t.as_slice().to_vec()).collect(),
                content: e.content.clone(),
                relay_provenance: vec![],
            }
        }

        fn make_keys() -> Keys {
            Keys::generate()
        }

        const A1: &str =
            "30023:bbbb000000000000000000000000000000000000000000000000000000000002:essay";
        const A2: &str =
            "30023:cccc000000000000000000000000000000000000000000000000000000000003:talk";
        const E1: &str = "1111111111111111111111111111111111111111111111111111111111111111";
        const R1: &str = "https://example.com/reference";
        const R2: &str = "https://example.org/another";

        /// The single shared consumer-shaped curation-set fixture. Carries the
        /// full tag spectrum a real client emits, including a `["client","hl"]`
        /// custom tag and a non-empty content body that the writer must preserve.
        fn fixture_set_event(keys: &Keys, kind: u16, d: &str) -> Event {
            let tags = vec![
                Tag::parse(vec!["d".to_string(), d.to_string()]).unwrap(),
                Tag::parse(vec!["title".to_string(), "Reading List".to_string()]).unwrap(),
                Tag::parse(vec![
                    "description".to_string(),
                    "things to read".to_string(),
                ])
                .unwrap(),
                Tag::parse(vec![
                    "image".to_string(),
                    "https://img.example/x.png".to_string(),
                ])
                .unwrap(),
                Tag::parse(vec!["a".to_string(), A1.to_string()]).unwrap(),
                Tag::parse(vec!["a".to_string(), A2.to_string()]).unwrap(),
                Tag::parse(vec!["e".to_string(), E1.to_string()]).unwrap(),
                Tag::parse(vec!["r".to_string(), R1.to_string()]).unwrap(),
                Tag::parse(vec!["r".to_string(), R2.to_string()]).unwrap(),
                Tag::parse(vec!["t".to_string(), "nostr".to_string()]).unwrap(),
                Tag::parse(vec!["client".to_string(), "hl".to_string()]).unwrap(),
            ];
            EventBuilder::new(Kind::from(kind), "set body content")
                .tags(tags)
                .sign_with_keys(keys)
                .unwrap()
        }

        fn fixture_web_event(keys: &Keys, url_without_scheme: &str, title: &str) -> Event {
            let tags = vec![
                Tag::parse(vec!["d".to_string(), url_without_scheme.to_string()]).unwrap(),
                Tag::parse(vec!["title".to_string(), title.to_string()]).unwrap(),
                Tag::parse(vec!["t".to_string(), "reading".to_string()]).unwrap(),
            ];
            EventBuilder::new(Kind::from(39701u16), "A description")
                .tags(tags)
                .sign_with_keys(keys)
                .unwrap()
        }

        /// Reference oracle for the deleted bespoke `update_address_in_curation_set`
        /// writer (#1653 D4 — the live writer is gone, the kernel is sole writer).
        /// Encodes its EXACT tag algorithm: copy every non-`a` tag verbatim, then
        /// append the flipped `a` membership block, preserving content. The write
        /// parity test asserts the kernel writer produces the same tag multiset.
        fn bespoke_write_oracle(event: &Event, add_addr: &str) -> (Vec<Vec<String>>, String) {
            let mut a_addresses: Vec<String> = Vec::new();
            let mut other_tags: Vec<Vec<String>> = Vec::new();
            for tag in event.tags.iter() {
                let s = tag.as_slice();
                match s.first().map(String::as_str) {
                    Some("a") => {
                        if let Some(v) = s.get(1) {
                            a_addresses.push(v.clone());
                        }
                    }
                    _ => other_tags.push(s.to_vec()),
                }
            }
            if !a_addresses.iter().any(|a| a == add_addr) {
                a_addresses.push(add_addr.to_string());
            }
            let mut tags = other_tags;
            for addr in &a_addresses {
                tags.push(vec!["a".to_string(), addr.clone()]);
            }
            (tags, event.content.clone())
        }

        fn json_tags(json: &str) -> (Vec<Vec<String>>, String) {
            let v: serde_json::Value = serde_json::from_str(json).unwrap();
            let tags = v["tags"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| {
                    t.as_array()
                        .unwrap()
                        .iter()
                        .map(|x| x.as_str().unwrap().to_string())
                        .collect::<Vec<String>>()
                })
                .collect();
            let content = v["content"].as_str().unwrap().to_string();
            (tags, content)
        }

        // SET-READ: kernel parse of the rich fixture event carries id, title, and
        // EVERY coordinate dimension (incl. `r`) with the exact fixture values.
        #[test]
        fn set_read_all_coordinates_incl_r() {
            let keys = make_keys();
            let pubkey_hex = keys.public_key().to_hex();
            let set_ev = fixture_set_event(&keys, 30004, "reading-list");

            let kernel_ev = nostr_to_kernel(&set_ev);
            let k = parse_set_row_from_kernel(&kernel_ev).expect("kernel set parses");

            // Identity, title, and EVERY coordinate dimension match the fixture.
            assert_eq!(k.d_tag, "reading-list", "set id (d) must match");
            assert_eq!(k.title.as_deref(), Some("Reading List"), "title must match");
            assert_eq!(k.pubkey, pubkey_hex, "pubkey must match");
            assert_eq!(k.kind, 30004, "kind must match");
            assert_eq!(
                k.article_addresses,
                vec![A1.to_string(), A2.to_string()],
                "a coordinates must match exactly"
            );
            assert_eq!(k.note_ids, vec![E1.to_string()], "e coordinates must match");
            assert_eq!(
                k.r_refs,
                vec![R1.to_string(), R2.to_string()],
                "r coordinates must match exactly"
            );
            assert_eq!(k.topics, vec!["nostr".to_string()], "t topics must match");
        }

        // WEB-READ: kernel web parse of the fixture — url + title.
        #[test]
        fn web_read_url_and_title() {
            let keys = make_keys();
            let pubkey_hex = keys.public_key().to_hex();
            let web_ev = fixture_web_event(&keys, "example.com/article", "Great Article");

            let kernel_ev = nostr_to_kernel(&web_ev);
            let k = parse_web_row_from_kernel(&kernel_ev).expect("kernel web parses");

            assert_eq!(k.url, "https://example.com/article", "web url must match");
            assert_eq!(k.pubkey, pubkey_hex, "pubkey must match");
            assert_eq!(
                k.title.as_deref(),
                Some("Great Article"),
                "web title must match"
            );
        }

        // FOLLOWS: kernel follows filter includes a followed author's set.
        #[test]
        fn following_curation_sets_identity() {
            let my_keys = make_keys();
            let followed_keys = make_keys();
            let user_hex = my_keys.public_key().to_hex();
            let followed_hex = followed_keys.public_key().to_hex();

            let cur_ev = fixture_set_event(&followed_keys, 30004, "fol-curations");

            let mut state = make_state_with_session(&user_hex);
            state.follows = vec![followed_hex.clone()];
            let kernel_ev = nostr_to_kernel(&cur_ev);
            let row = parse_set_row_from_kernel(&kernel_ev).expect("parses");
            state.all_curation_sets = vec![row];
            let kernel_has = project_following_curation_sets(&state)
                .iter()
                .any(|s| s.d_tag == "fol-curations" && s.pubkey == followed_hex);

            assert!(kernel_has, "kernel must include the followed set");
        }

        // P-WRITE: the kernel writer (AddToSet) must produce the SAME event the
        // bespoke writer would — preserving every non-`a` tag (title,
        // description, image, e, r, t, the custom `client` tag) AND the content,
        // adding only the new `a` item (#1653 BLOCKING #3 + #5).
        #[test]
        fn parity_add_to_set_preserves_all_non_a_tags() {
            let keys = make_keys();
            let pk = keys.public_key().to_hex();
            let new_item =
                "30023:dddd000000000000000000000000000000000000000000000000000000000004:new";

            // The set already in kernel state was parsed from the rich fixture,
            // so its raw_tags + content drive the lossless round-trip.
            let set_ev = fixture_set_event(&keys, 30004, "reading-list");
            let kernel_ev = nostr_to_kernel(&set_ev);
            let mut state = make_state_with_session(&pk);
            state.all_curation_sets = vec![parse_set_row_from_kernel(&kernel_ev).expect("parses")];

            let effects = reduce_action_add_to_set(
                &state,
                format!("30004:{pk}:reading-list"),
                new_item.to_string(),
            );
            assert_eq!(effects.len(), 1);
            let Effect::PublishSetEvent { json } = &effects[0] else {
                panic!("expected PublishSetEvent");
            };
            let (kernel_tags, kernel_content) = json_tags(json);

            // Bespoke oracle: the deleted writer's exact tag algorithm.
            let (oracle_tags, oracle_content) = bespoke_write_oracle(&set_ev, new_item);

            // Content preserved verbatim (NOT clobbered to "").
            assert_eq!(kernel_content, oracle_content, "content must be preserved");
            assert_eq!(kernel_content, "set body content");

            // Compare as sorted multisets — both writers emit the same tag set
            // (the bespoke oracle keeps original non-`a` order then a-block;
            // the kernel writer does the same, so a sorted compare is exact).
            let mut k = kernel_tags.clone();
            let mut o = oracle_tags.clone();
            k.sort();
            o.sort();
            assert_eq!(k, o, "kernel writer tags must equal bespoke writer tags");

            // Spot-check the load-bearing preserved dimensions are actually there.
            let has = |tags: &[Vec<String>], key: &str, val: &str| {
                tags.iter().any(|t| {
                    t.first().map(String::as_str) == Some(key)
                        && t.get(1).map(String::as_str) == Some(val)
                })
            };
            assert!(
                has(&kernel_tags, "title", "Reading List"),
                "title preserved"
            );
            assert!(
                has(&kernel_tags, "description", "things to read"),
                "description preserved"
            );
            assert!(
                has(&kernel_tags, "image", "https://img.example/x.png"),
                "image preserved"
            );
            assert!(has(&kernel_tags, "e", E1), "e preserved");
            assert!(has(&kernel_tags, "r", R1), "r preserved (not dropped)");
            assert!(has(&kernel_tags, "r", R2), "r preserved (not dropped)");
            assert!(has(&kernel_tags, "t", "nostr"), "t preserved");
            assert!(
                has(&kernel_tags, "client", "hl"),
                "custom client tag preserved"
            );
            assert!(has(&kernel_tags, "a", A1), "existing a preserved");
            assert!(has(&kernel_tags, "a", A2), "existing a preserved");
            assert!(has(&kernel_tags, "a", new_item), "new a added");
        }

        // GUARD: prove the read assertion BITES — corrupt the kernel row's
        // `r_refs` and confirm the equality check against the fixture would fail.
        #[test]
        fn guard_bites_when_r_refs_diverge() {
            let keys = make_keys();
            let set_ev = fixture_set_event(&keys, 30004, "reading-list");
            let expected_r_refs = vec![R1.to_string(), R2.to_string()];

            let kernel_ev = nostr_to_kernel(&set_ev);
            let mut k = parse_set_row_from_kernel(&kernel_ev).expect("parses");

            // Sanity: they agree before corruption.
            assert_eq!(k.r_refs, expected_r_refs, "precondition: r_refs agree");

            // Corrupt one field — the guard must now see a difference.
            k.r_refs.push("https://evil.example/injected".to_string());
            assert_ne!(
                k.r_refs, expected_r_refs,
                "guard bites: a divergent r_refs must fail the read equality"
            );
        }

        // GUARD: prove the write-parity assertion BITES — a writer that dropped a
        // non-`a` tag (the old lossy behaviour) would NOT equal the oracle.
        #[test]
        fn guard_bites_when_writer_drops_non_a_tag() {
            let keys = make_keys();
            let set_ev = fixture_set_event(&keys, 30004, "reading-list");
            let new_item = "30023:eeee:new";

            let (oracle_tags, _) = bespoke_write_oracle(&set_ev, new_item);

            // Simulate the OLD lossy writer: rebuild from scalars, dropping `r`.
            let lossy: Vec<Vec<String>> = oracle_tags
                .iter()
                .filter(|t| t.first().map(String::as_str) != Some("r"))
                .cloned()
                .collect();

            let mut a = lossy.clone();
            let mut b = oracle_tags.clone();
            a.sort();
            b.sort();
            assert_ne!(
                a, b,
                "guard bites: a writer that drops `r` must not equal the lossless oracle"
            );
        }
    }
}
