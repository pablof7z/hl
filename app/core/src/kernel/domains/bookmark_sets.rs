//! Bookmark sets + web bookmarks domain — NIP-51 kind:30003/30004 + NIP-B0
//! kind:39701 projection (gate #1653).
//!
//! ## Responsibilities
//!
//! * **READ (sets)** — a `SetListProjection` implements `KernelEventObserver`
//!   for kind:30003 and kind:30004. It accumulates all observed events, one
//!   per `(author, d_tag)` key with newest-wins supersession. A registered
//!   typed-snapshot closure serialises the full collection to JSON every NMP
//!   tick under schema_id `"hl.bookmark_sets"`. `apply_bookmark_sets` decodes
//!   the JSON, applies active-account / follows filtering, and stores results
//!   in `AppState::all_bookmark_sets` + `AppState::all_curation_sets`.
//!
//! * **READ (web)** — a `WebBookmarkProjection` implements `KernelEventObserver`
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
//! `KernelEventObserver` implementations via `nmp_ref.register_event_observer`.
//!
//! ## Threading
//!
//! `SetListProjection` and `WebBookmarkProjection` run on the NMP event-observer
//! thread (non-blocking: Mutex lock + small HashMap insert). `apply_bookmark_sets`
//! and `apply_web_bookmarks` run on the **actor thread** (JSON decode + Vec
//! filter, no I/O). D6: decode errors leave AppState unchanged (silent no-op).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use nmp_core::substrate::KernelEvent as NmpKernelEvent;
use nmp_core::KernelEventObserver;
use nmp_ffi::NmpApp;

use crate::kernel::app::AppState;
use crate::kernel::effect::Effect;
use crate::kernel::snapshot::{BookmarkSetRow, WebBookmarkRow};

// ── Kind constants ────────────────────────────────────────────────────────────

const KIND_BOOKMARK_SET: u32 = 30003;
const KIND_CURATION_SET: u32 = 30004;
const KIND_WEB_BOOKMARK: u32 = 39701;

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

impl KernelEventObserver for SetListProjection {
    fn on_kernel_event(&self, event: &NmpKernelEvent) {
        if event.kind != KIND_BOOKMARK_SET && event.kind != KIND_CURATION_SET {
            return;
        }
        let row = parse_set_row_from_kernel(event);
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

impl KernelEventObserver for WebBookmarkProjection {
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
        let row = parse_web_row_from_kernel(event);
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
    let Some(mut row) = find_curation_set(state, &set_coordinate) else {
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
    row.article_addresses.push(item_coordinate);
    build_set_publish_effect(row)
}

/// Handle `AppAction::RemoveFromSet { set_coordinate, item_coordinate }`.
/// Symmetric with `reduce_action_add_to_set`. D6: no-op when set or item not found.
pub(crate) fn reduce_action_remove_from_set(
    state: &AppState,
    set_coordinate: String,
    item_coordinate: String,
) -> Vec<Effect> {
    let Some(mut row) = find_curation_set(state, &set_coordinate) else {
        tracing::trace!(
            set = %set_coordinate,
            "bookmark_sets::remove_from_set: set not found — no-op (D6)"
        );
        return vec![];
    };
    let before = row.article_addresses.len();
    row.article_addresses.retain(|a| *a != item_coordinate);
    if row.article_addresses.len() == before {
        tracing::trace!(
            item = %item_coordinate,
            "bookmark_sets::remove_from_set: item not found — no-op"
        );
        return vec![];
    }
    build_set_publish_effect(row)
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

/// Serialise a modified `BookmarkSetRow` into an `Effect::PublishSetEvent`
/// with the kind:30004 event template JSON. Returns an empty vec on failure (D6).
fn build_set_publish_effect(row: BookmarkSetRow) -> Vec<Effect> {
    let mut tags: Vec<serde_json::Value> = Vec::new();
    // d tag — always first
    tags.push(serde_json::json!(["d", row.d_tag]));
    // optional metadata tags
    if let Some(title) = &row.title {
        tags.push(serde_json::json!(["title", title]));
    }
    if let Some(description) = &row.description {
        tags.push(serde_json::json!(["description", description]));
    }
    if let Some(image) = &row.image {
        tags.push(serde_json::json!(["image", image]));
    }
    // article address tags
    for addr in &row.article_addresses {
        tags.push(serde_json::json!(["a", addr]));
    }
    // note id tags
    for id in &row.note_ids {
        tags.push(serde_json::json!(["e", id]));
    }

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
        .send(nmp_core::ActorCommand::PublishRawEvent {
            kind: template.kind,
            content: template.content,
            tags: template.tags,
            target: nmp_core::publish::PublishTarget::Auto,
            signer_pubkey: None,
            correlation_id: None,
        });
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

    let observer_id =
        nmp_ref.register_event_observer(Arc::clone(&set_proj) as Arc<dyn KernelEventObserver>);
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

    let web_observer_id =
        nmp_ref.register_event_observer(Arc::clone(&web_proj) as Arc<dyn KernelEventObserver>);
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
}

// ── Kernel event parsing helpers ──────────────────────────────────────────────

/// Parse a `KernelEvent` (kind:30003 or kind:30004) into a `BookmarkSetRow`.
/// Raw fields only — no presentation strings (D1).
fn parse_set_row_from_kernel(event: &NmpKernelEvent) -> BookmarkSetRow {
    let mut d_tag = String::new();
    let mut title = None;
    let mut description = None;
    let mut image = None;
    let mut article_addresses = Vec::new();
    let mut note_ids = Vec::new();

    for tag in &event.tags {
        match tag.first().map(String::as_str) {
            Some("d") => {
                if d_tag.is_empty() {
                    d_tag = tag.get(1).cloned().unwrap_or_default();
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
                if let Some(v) = tag.get(1) {
                    article_addresses.push(v.clone());
                }
            }
            Some("e") => {
                if let Some(v) = tag.get(1) {
                    note_ids.push(v.clone());
                }
            }
            _ => {}
        }
    }

    BookmarkSetRow {
        d_tag,
        pubkey: event.author.clone(),
        kind: event.kind,
        title,
        description,
        image,
        article_addresses,
        note_ids,
        created_at: event.created_at,
    }
}

/// Parse a `KernelEvent` (kind:39701) into a `WebBookmarkRow`.
fn parse_web_row_from_kernel(event: &NmpKernelEvent) -> WebBookmarkRow {
    let mut d = String::new();
    let mut title = None;
    let mut topics = Vec::new();
    let mut published_at = None;

    for tag in &event.tags {
        match tag.first().map(String::as_str) {
            Some("d") => {
                if d.is_empty() {
                    d = tag.get(1).cloned().unwrap_or_default();
                }
            }
            Some("title") => {
                if title.is_none() {
                    title = tag.get(1).filter(|v| !v.is_empty()).cloned();
                }
            }
            Some("t") => {
                if let Some(v) = tag.get(1) {
                    topics.push(v.clone());
                }
            }
            Some("published_at") if published_at.is_none() => {
                published_at = tag.get(1).and_then(|v| v.parse::<u64>().ok());
            }
            _ => {}
        }
    }

    let url = if d.is_empty() {
        String::new()
    } else {
        format!("https://{d}")
    };

    let description = if event.content.is_empty() {
        None
    } else {
        Some(event.content.clone())
    };

    WebBookmarkRow {
        url,
        pubkey: event.author.clone(),
        title,
        description,
        topics,
        published_at,
        created_at: event.created_at,
    }
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

    // 1653-T1: BookmarkSetsUpdated stores raw rows in AppState.
    #[test]
    fn bookmark_sets_updated_stores_rows() {
        let pk = "aaaa000000000000000000000000000000000000000000000000000000000001";
        let mut state = make_state_with_session(pk);
        let clock = ManualClock::default();

        let row = BookmarkSetRow {
            d_tag: "my-set".to_string(),
            pubkey: pk.to_string(),
            kind: 30003,
            title: Some("My Set".to_string()),
            description: None,
            image: None,
            article_addresses: vec!["30023:aabb:slug".to_string()],
            note_ids: vec![],
            created_at: 1000,
        };

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

        let row = WebBookmarkRow {
            url: "https://example.com/article".to_string(),
            pubkey: pk.to_string(),
            title: Some("Article Title".to_string()),
            description: Some("A description".to_string()),
            topics: vec!["nostr".to_string()],
            published_at: None,
            created_at: 2000,
        };

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

        let make_set = |pk: &str, d: &str, kind: u32| BookmarkSetRow {
            d_tag: d.to_string(),
            pubkey: pk.to_string(),
            kind,
            title: None,
            description: None,
            image: None,
            article_addresses: vec![],
            note_ids: vec![],
            created_at: 1000,
        };

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

        state.all_curation_sets = vec![BookmarkSetRow {
            d_tag: "my-curations".to_string(),
            pubkey: pk.to_string(),
            kind: 30004,
            title: Some("My Curations".to_string()),
            description: None,
            image: None,
            article_addresses: vec![],
            note_ids: vec![],
            created_at: 1000,
        }];

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

        state.all_curation_sets = vec![BookmarkSetRow {
            d_tag: "my-curations".to_string(),
            pubkey: pk.to_string(),
            kind: 30004,
            title: None,
            description: None,
            image: None,
            article_addresses: vec![target.to_string(), "30023:ccdd:other".to_string()],
            note_ids: vec![],
            created_at: 1000,
        }];

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

        state.all_curation_sets = vec![BookmarkSetRow {
            d_tag: "my-curations".to_string(),
            pubkey: pk.to_string(),
            kind: 30004,
            title: None,
            description: None,
            image: None,
            article_addresses: vec![existing.to_string()],
            note_ids: vec![],
            created_at: 1000,
        }];

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

        state.all_bookmark_sets = vec![BookmarkSetRow {
            d_tag: "s".to_string(),
            pubkey: pk.to_string(),
            kind: 30003,
            title: None,
            description: None,
            image: None,
            article_addresses: vec![],
            note_ids: vec![],
            created_at: 1,
        }];
        state.all_curation_sets = state.all_bookmark_sets.clone();
        state.web_bookmarks = vec![WebBookmarkRow {
            url: "https://example.com".to_string(),
            pubkey: pk.to_string(),
            title: None,
            description: None,
            topics: vec![],
            published_at: None,
            created_at: 1,
        }];

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

    // ── Parity tests: kernel projection vs bespoke query_bookmark_library_snapshot
    mod parity {
        use super::*;
        use crate::test_ndb::{isolated_ndb, process_event_and_wait};
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

        fn make_set_event(keys: &Keys, kind: u16, d: &str, articles: &[&str]) -> Event {
            make_set_event_with_notes(keys, kind, d, articles, &[])
        }

        fn make_set_event_with_notes(
            keys: &Keys,
            kind: u16,
            d: &str,
            articles: &[&str],
            note_ids: &[&str],
        ) -> Event {
            let mut tags = vec![Tag::parse(vec!["d".to_string(), d.to_string()]).unwrap()];
            for addr in articles {
                tags.push(Tag::parse(vec!["a".to_string(), addr.to_string()]).unwrap());
            }
            for nid in note_ids {
                tags.push(Tag::parse(vec!["e".to_string(), nid.to_string()]).unwrap());
            }
            EventBuilder::new(Kind::from(kind), "")
                .tags(tags)
                .sign_with_keys(keys)
                .unwrap()
        }

        fn make_web_event(keys: &Keys, url_without_scheme: &str, title: &str) -> Event {
            let tags = vec![
                Tag::parse(vec!["d".to_string(), url_without_scheme.to_string()]).unwrap(),
                Tag::parse(vec!["title".to_string(), title.to_string()]).unwrap(),
            ];
            EventBuilder::new(Kind::from(39701u16), "A description")
                .tags(tags)
                .sign_with_keys(keys)
                .unwrap()
        }

        // P1: my_bookmark_sets identity — kernel and bespoke return the same set coordinates and members.
        #[test]
        fn parity_my_bookmark_sets_identity() {
            let keys = make_keys();
            let user_hex = keys.public_key().to_hex();
            let article_addr = format!("30023:{}:slug", Keys::generate().public_key().to_hex());

            let set_ev = make_set_event(&keys, 30003, "my-bookmarks", &[article_addr.as_str()]);

            // ── Bespoke path ─────────────────────────────────────────────────
            let (ndb, _tmp) = isolated_ndb(64 * 1024 * 1024);
            process_event_and_wait(&ndb, &set_ev);
            let bespoke = crate::lists::query_bookmark_library_snapshot(&ndb, &user_hex);
            assert_eq!(
                bespoke.my_bookmark_sets.len(),
                1,
                "bespoke: one bookmark set"
            );
            let bespoke_set = &bespoke.my_bookmark_sets[0];

            // ── Kernel path ──────────────────────────────────────────────────
            let mut state = make_state_with_session(&user_hex);
            let kernel_ev = nostr_to_kernel(&set_ev);
            let row = parse_set_row_from_kernel(&kernel_ev);
            state.all_bookmark_sets = vec![row];
            let kernel_sets = project_my_bookmark_sets(&state);
            assert_eq!(kernel_sets.len(), 1, "kernel: one bookmark set");
            let kernel_set = &kernel_sets[0];

            // Identity assertions
            assert_eq!(
                kernel_set.d_tag, bespoke_set.id,
                "d_tag must match bespoke id"
            );
            assert_eq!(
                kernel_set.article_addresses, bespoke_set.article_addresses,
                "article_addresses must match"
            );
            assert_eq!(kernel_set.pubkey, bespoke_set.pubkey, "pubkey must match");
            assert_eq!(kernel_set.kind, bespoke_set.kind, "kind must match");
        }

        // P2: my_curation_sets identity — kernel and bespoke return the same set.
        #[test]
        fn parity_my_curation_sets_identity() {
            let keys = make_keys();
            let user_hex = keys.public_key().to_hex();
            let article_addr = format!("30023:{}:slug", Keys::generate().public_key().to_hex());

            let cur_ev = make_set_event(&keys, 30004, "my-curations", &[article_addr.as_str()]);

            let (ndb, _tmp) = isolated_ndb(64 * 1024 * 1024);
            process_event_and_wait(&ndb, &cur_ev);
            let bespoke = crate::lists::query_bookmark_library_snapshot(&ndb, &user_hex);
            assert_eq!(
                bespoke.my_curation_sets.len(),
                1,
                "bespoke: one curation set"
            );
            let bespoke_set = &bespoke.my_curation_sets[0];

            let mut state = make_state_with_session(&user_hex);
            let kernel_ev = nostr_to_kernel(&cur_ev);
            let row = parse_set_row_from_kernel(&kernel_ev);
            state.all_curation_sets = vec![row];
            let kernel_sets = project_my_curation_sets(&state);
            assert_eq!(kernel_sets.len(), 1, "kernel: one curation set");
            let kernel_set = &kernel_sets[0];

            assert_eq!(kernel_set.d_tag, bespoke_set.id);
            let mut k_addrs = kernel_set.article_addresses.clone();
            k_addrs.sort();
            let mut b_addrs = bespoke_set.article_addresses.clone();
            b_addrs.sort();
            assert_eq!(k_addrs, b_addrs, "article_addresses must match (sorted)");
            assert_eq!(kernel_set.pubkey, bespoke_set.pubkey);
            assert_eq!(kernel_set.kind, bespoke_set.kind);
        }

        // P3: following_curation_sets identity — kernel follows filter matches bespoke.
        #[test]
        fn parity_following_curation_sets_identity() {
            let my_keys = make_keys();
            let followed_keys = make_keys();
            let user_hex = my_keys.public_key().to_hex();
            let followed_hex = followed_keys.public_key().to_hex();

            // Include a note_id so explorable_curation_sets keeps this set (it
            // filters out sets with neither articles nor notes).
            let fake_note_id = "a".repeat(64);
            let cur_ev = make_set_event_with_notes(
                &followed_keys,
                30004,
                "fol-curations",
                &[],
                &[fake_note_id.as_str()],
            );

            let (ndb, _tmp) = isolated_ndb(64 * 1024 * 1024);
            // Also inject a follow-list event so bespoke can find the follows.
            let follow_ev = {
                let tags = vec![Tag::parse(vec!["p".to_string(), followed_hex.clone()]).unwrap()];
                EventBuilder::new(Kind::ContactList, "")
                    .tags(tags)
                    .sign_with_keys(&my_keys)
                    .unwrap()
            };
            process_event_and_wait(&ndb, &follow_ev);
            process_event_and_wait(&ndb, &cur_ev);

            let bespoke = crate::lists::query_bookmark_library_snapshot(&ndb, &user_hex);
            // bespoke returns explorable curation sets (may filter further) — we
            // assert that the followed set coordinate is present in EITHER
            // bespoke.following_curation_sets OR my_curation_sets (if both accounts
            // are the same, which they're not here).
            let bespoke_has = bespoke
                .following_curation_sets
                .iter()
                .any(|s| s.id == "fol-curations" && s.pubkey == followed_hex);

            let mut state = make_state_with_session(&user_hex);
            state.follows = vec![followed_hex.clone()];
            let kernel_ev = nostr_to_kernel(&cur_ev);
            let row = parse_set_row_from_kernel(&kernel_ev);
            state.all_curation_sets = vec![row];
            let kernel_following = project_following_curation_sets(&state);
            let kernel_has = kernel_following
                .iter()
                .any(|s| s.d_tag == "fol-curations" && s.pubkey == followed_hex);

            assert!(
                bespoke_has,
                "bespoke must include the followed curation set"
            );
            assert!(kernel_has, "kernel must include the followed curation set");
        }

        // P4: my_web_bookmarks identity — url and title match.
        #[test]
        fn parity_my_web_bookmarks_identity() {
            let keys = make_keys();
            let user_hex = keys.public_key().to_hex();

            let web_ev = make_web_event(&keys, "example.com/article", "Great Article");

            let (ndb, _tmp) = isolated_ndb(64 * 1024 * 1024);
            process_event_and_wait(&ndb, &web_ev);
            let bespoke = crate::lists::query_bookmark_library_snapshot(&ndb, &user_hex);
            assert_eq!(
                bespoke.my_web_bookmarks.len(),
                1,
                "bespoke: one web bookmark"
            );
            let bespoke_wb = &bespoke.my_web_bookmarks[0];

            let mut state = make_state_with_session(&user_hex);
            let kernel_ev = nostr_to_kernel(&web_ev);
            let row = parse_web_row_from_kernel(&kernel_ev);
            state.web_bookmarks = vec![row];
            let kernel_wbs = project_my_web_bookmarks(&state);
            assert_eq!(kernel_wbs.len(), 1, "kernel: one web bookmark");
            let kernel_wb = &kernel_wbs[0];

            assert_eq!(kernel_wb.url, bespoke_wb.url, "url must match");
            assert_eq!(kernel_wb.pubkey, bespoke_wb.pubkey, "pubkey must match");
            // Title: bespoke uses "" when absent, kernel uses None; compare after normalising.
            let bespoke_title = if bespoke_wb.title.is_empty() {
                None
            } else {
                Some(bespoke_wb.title.as_str())
            };
            assert_eq!(
                kernel_wb.title.as_deref(),
                bespoke_title,
                "title must match"
            );
        }

        // P5 (write parity): AddToSet emitted event tags match bespoke toggle_address_in_curation_set shape.
        #[test]
        fn parity_add_to_set_event_tags() {
            let pk = "aaaa000000000000000000000000000000000000000000000000000000000001";
            let item =
                "30023:bbbb000000000000000000000000000000000000000000000000000000000002:slug";
            let mut state = make_state_with_session(pk);
            state.all_curation_sets = vec![BookmarkSetRow {
                d_tag: "test-set".to_string(),
                pubkey: pk.to_string(),
                kind: 30004,
                title: Some("Test Set".to_string()),
                description: None,
                image: None,
                article_addresses: vec![],
                note_ids: vec![],
                created_at: 1000,
            }];

            let effects =
                reduce_action_add_to_set(&state, format!("30004:{pk}:test-set"), item.to_string());

            assert_eq!(effects.len(), 1);
            let Effect::PublishSetEvent { json } = &effects[0] else {
                panic!("expected PublishSetEvent");
            };
            let v: serde_json::Value = serde_json::from_str(json).unwrap();

            // Must be kind:30004
            assert_eq!(v["kind"].as_u64(), Some(30004));

            // Tags must include "d" tag with the set identifier
            let tags = v["tags"].as_array().unwrap();
            let d_val: Vec<_> = tags
                .iter()
                .filter_map(|t| {
                    let arr = t.as_array()?;
                    if arr.get(0)?.as_str() == Some("d") {
                        arr.get(1)?.as_str()
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(d_val, vec!["test-set"], "must have d tag = test-set");

            // Tags must include the new "a" tag
            let a_val: Vec<_> = tags
                .iter()
                .filter_map(|t| {
                    let arr = t.as_array()?;
                    if arr.get(0)?.as_str() == Some("a") {
                        arr.get(1)?.as_str()
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(a_val, vec![item], "must have a tag with item_coordinate");
        }
    }
}
