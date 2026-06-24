//! Bookmarks domain — NIP-51 kind:10003 bookmark-list projection (slice 4C).
//!
//! ## Responsibilities
//!
//! * **READ** — wrap `BookmarkListProjection::snapshot()` (Family B in-memory
//!   observer) under the hl-owned typed snapshot key `"hl.bookmarks"`. The
//!   registered closure serialises the snapshot to JSON, which is decoded in
//!   `projections::dispatch_typed_frame` via the `"hl.bookmarks"` schema_id arm.
//!   Result: `KernelEvent::BookmarksUpdated(Vec<BookmarkRow>)` → stored in
//!   `AppState::bookmarks`.
//!
//! * **WRITE** — `AppAction::AddBookmark{item}` / `RemoveBookmark{item}` →
//!   reducer emits `Effect::DispatchBookmarkAction{namespace, json}` → effect
//!   runner calls `nmp_app_dispatch_action("nmp.nip51.add_bookmark"|
//!   "nmp.nip51.remove_bookmark", BookmarkUpdateInput JSON)`. Fire-and-forget
//!   (D6, Non-Negotiable #3): the updated bookmark list arrives back through
//!   the `BookmarksUpdated` projection event via the NMP update callback.
//!
//! ## Scope
//!
//! Kind:10003 article-bookmark toggle **only**. Bookmark sets (30003/30004),
//! web bookmarks (39701), and curated communities (10009) have no nmp
//! projection at b4404159 — those stay on the bespoke lane (or are §6
//! follow-on gaps). The kernel is the sole writer for kind:10003 on ported
//! screens (no live-lane double-publish).
//!
//! ## NMP bookmark seam
//!
//! Action namespaces are `"nmp.nip51.add_bookmark"` and
//! `"nmp.nip51.remove_bookmark"` (`AddBookmarkAction::NAMESPACE` and
//! `RemoveBookmarkAction::NAMESPACE` in `nmp-nip51/src/bookmarks.rs:203,246`).
//! Wire shape: `BookmarkUpdateInput { account_pubkey, item: BookmarkItem }`.
//! `BookmarkItem` is a tagged-union with variants `Event{event_id, relay?}`,
//! `Address{coordinate, relay?}`, `Url{url}`, `Hashtag{hashtag}` — serialised
//! with `#[serde(tag = "type", rename_all = "snake_case")]`.
//!
//! NOTE: `nmp-defaults::register_bookmark_runtime` (called by
//! `register_defaults` at boot) already registers a `BookmarkListProjection`
//! as a kind:10003 observer AND wires the add/remove action modules. This
//! module creates a SECOND `BookmarkListProjection` (also pointing at the
//! live active-account slot) for the hl-typed-snapshot path. Double-
//! observation is harmless (both observe the same events, read-only). The hl
//! projection is NOT registered with `register_bookmark_actions` — the
//! actions are already wired by nmp-defaults and calling them again would
//! result in duplicate action registrations.
//!
//! ## Projection registration
//!
//! `register_bookmark_list_projection(nmp_ref)` (defined here) wires the hl
//! `BookmarkListProjection` event observer + typed snapshot projection against
//! the live `NmpApp`. Call it at boot (after `nmp_app_start`) and re-call on
//! `IdentityChanged(Some)` — the projection's active_pubkey slot
//! auto-tracks the active account because we pass `nmp_ref.active_account_handle()`.
//!
//! ## Threading
//!
//! The typed snapshot closure runs on the NMP projection-emit thread (non-
//! blocking: `snapshot()` acquires a Mutex briefly, clones, and serialises).
//! `apply_bookmarks` runs on the **actor thread** (JSON decode + Vec clone,
//! no I/O). D6: decode errors leave `AppState::bookmarks` unchanged.

use std::sync::{Arc, Mutex};

use nmp_core::KernelEventObserver;
use nmp_ffi::NmpApp;
use nmp_nip51::{BookmarkItem, BookmarkListProjection};

use crate::kernel::app::AppState;
use crate::kernel::effect::Effect;
use crate::kernel::snapshot::{ArtifactPreviewRow, BookmarkRow};

// ── hl schema_id for the typed snapshot projection ──────────────────────────

/// Schema id used when the hl-owned typed snapshot projection serialises the
/// `BookmarkListSnapshot` to JSON. Matched in `projections::dispatch_typed_frame`.
pub(crate) const BOOKMARK_SCHEMA_ID: &str = "hl.bookmarks";

// ─── Articles pane: artifact-preview hydration (Phase 7) ────────────────────

/// Collect the bookmarked kind:30023 article coordinates in bookmark order.
fn bookmarked_article_coordinates(state: &AppState) -> Vec<String> {
    state
        .bookmarks
        .iter()
        .filter_map(|row| match row {
            BookmarkRow::Address { coordinate, .. } if coordinate.starts_with("30023:") => {
                Some(coordinate.clone())
            }
            _ => None,
        })
        .collect()
}

/// Ensure an artifact preview exists for every bookmarked kind:30023 article.
/// Reuses the shared keystone `ensure_artifact_preview` (resolves from
/// `AppState::articles` immediately, else marks pending + emits a
/// `ResolveArtifactCoordinate` fetch). Idempotent — safe to call on bookmarks
/// open and on every `BookmarksUpdated`. Coordinates are collected first to
/// avoid borrowing `state.bookmarks` across the `&mut state` ensure calls.
pub(crate) fn ensure_bookmark_article_previews(state: &mut AppState) -> Vec<Effect> {
    let coordinates = bookmarked_article_coordinates(state);
    let mut effects = Vec::new();
    for coordinate in coordinates {
        effects.extend(super::artifact_preview::ensure_artifact_preview(
            state, coordinate,
        ));
    }
    effects
}

/// Project the bookmarked-article previews (in bookmark order) for the
/// `BookmarksSnapshot.article_previews` slice. Immutable — filters the already-
/// hydrated `AppState::artifact_previews` to the bookmarked 30023 coordinates.
pub(crate) fn bookmark_article_previews(state: &AppState) -> Vec<ArtifactPreviewRow> {
    bookmarked_article_coordinates(state)
        .into_iter()
        .filter_map(|coordinate| state.artifact_previews.get(&coordinate).cloned())
        .collect()
}

// ─── READ side: apply decoded snapshot ──────────────────────────────────────

/// Apply a decoded `"hl.bookmarks"` JSON payload to `state`.
///
/// Called from `projections::dispatch_typed_frame` when `schema_id ==
/// "hl.bookmarks"`. Decodes the serde-JSON representation of
/// `Vec<BookmarkItem>` (the `items` field from `BookmarkListSnapshot`) and
/// stores raw `BookmarkRow` items in `AppState::bookmarks`.
///
/// Wire format: a JSON array of `BookmarkItem`s serialised with
/// `#[serde(tag = "type", rename_all = "snake_case")]`. This is the subset
/// of `BookmarkListSnapshot` that `BookmarkItem` derives `Deserialize` for
/// (the `BookmarkListSnapshot` and `BookmarkListMetadata` types only derive
/// `Serialize` at b4404159 — using the items array directly avoids the gap).
///
/// D6: any decode error leaves `AppState::bookmarks` unchanged (silent no-op).
/// No presentation strings — raw fields only (D1).
///
/// Must be non-blocking — runs on the actor thread.
pub(crate) fn apply_bookmarks(state: &mut AppState, payload: &[u8]) {
    match serde_json::from_slice::<Vec<BookmarkItem>>(payload) {
        Ok(items) => {
            state.bookmarks = items.into_iter().map(bookmark_item_to_row).collect();
        }
        Err(e) => {
            tracing::trace!(
                error = %e,
                "bookmarks::apply_bookmarks: JSON decode error — AppState::bookmarks unchanged (D6)"
            );
        }
    }
}

/// Convert a `BookmarkItem` to the hl `BookmarkRow` representation.
/// Raw protocol data only — no labels, no presentation formatting (D1).
fn bookmark_item_to_row(item: BookmarkItem) -> BookmarkRow {
    match item {
        BookmarkItem::Event { event_id, relay } => BookmarkRow::Event { event_id, relay },
        BookmarkItem::Address { coordinate, relay } => BookmarkRow::Address { coordinate, relay },
        BookmarkItem::Url { url } => BookmarkRow::Url { url },
        BookmarkItem::Hashtag { hashtag } => BookmarkRow::Hashtag { hashtag },
    }
}

// ─── WRITE side: reduce_action helpers ──────────────────────────────────────

/// Handle `AppAction::AddBookmark{item}` — emit `Effect::DispatchBookmarkAction`.
///
/// Reads the active account pubkey from `state` and includes it in the
/// `BookmarkUpdateInput` JSON payload (nmp validates `account_pubkey` against
/// the live active account — supplying the wrong or empty pubkey is rejected).
///
/// The reducer does NOT speculatively update `AppState::bookmarks` — the
/// authoritative update arrives via the projection frame (`BookmarksUpdated`)
/// after NMP re-publishes kind:10003. Keeps state consistent with the actual
/// on-chain list (D6).
pub(crate) fn reduce_action_add_bookmark_for_state(
    state: &AppState,
    item: BookmarkRow,
) -> Vec<Effect> {
    let account_pubkey = match active_pubkey(state) {
        Some(pk) => pk,
        None => {
            tracing::trace!(
                "bookmarks::reduce_action_add_bookmark: no active account — no-op (D6)"
            );
            return vec![];
        }
    };
    match build_bookmark_update_input_json(account_pubkey, item) {
        Some(json) => vec![Effect::DispatchBookmarkAction {
            namespace: "nmp.nip51.add_bookmark".to_string(),
            json,
        }],
        None => {
            tracing::trace!(
                "bookmarks::reduce_action_add_bookmark: JSON serialisation failed — no-op (D6)"
            );
            vec![]
        }
    }
}

/// State-aware reducer for `AppAction::RemoveBookmark{item}`.
/// Symmetric with `reduce_action_add_bookmark_for_state`.
pub(crate) fn reduce_action_remove_bookmark_for_state(
    state: &AppState,
    item: BookmarkRow,
) -> Vec<Effect> {
    let account_pubkey = match active_pubkey(state) {
        Some(pk) => pk,
        None => {
            tracing::trace!(
                "bookmarks::reduce_action_remove_bookmark: no active account — no-op (D6)"
            );
            return vec![];
        }
    };
    match build_bookmark_update_input_json(account_pubkey, item) {
        Some(json) => vec![Effect::DispatchBookmarkAction {
            namespace: "nmp.nip51.remove_bookmark".to_string(),
            json,
        }],
        None => {
            tracing::trace!(
                "bookmarks::reduce_action_remove_bookmark: JSON serialisation failed — no-op (D6)"
            );
            vec![]
        }
    }
}

/// Extract the active account pubkey from `AppState::session`.
fn active_pubkey(state: &AppState) -> Option<String> {
    if let crate::kernel::app::SessionState::Present { pubkey, .. } = &state.session {
        Some(pubkey.clone())
    } else {
        None
    }
}

/// Serialise `BookmarkUpdateInput { account_pubkey, item }` to JSON.
/// Returns `None` if serialisation fails (D6).
fn build_bookmark_update_input_json(account_pubkey: String, item: BookmarkRow) -> Option<String> {
    let nmp_item = row_to_bookmark_item(item)?;
    let input = serde_json::json!({
        "account_pubkey": account_pubkey,
        "item": nmp_item,
    });
    serde_json::to_string(&input).ok()
}

/// Convert a `BookmarkRow` to a `serde_json::Value` matching `BookmarkItem`'s
/// `#[serde(tag = "type", rename_all = "snake_case")]` wire shape.
/// Returns `None` for malformed inputs (D6).
fn row_to_bookmark_item(row: BookmarkRow) -> Option<serde_json::Value> {
    match row {
        BookmarkRow::Event { event_id, relay } => {
            let mut m = serde_json::json!({ "type": "event", "event_id": event_id });
            if let Some(r) = relay {
                m["relay"] = serde_json::Value::String(r);
            }
            Some(m)
        }
        BookmarkRow::Address { coordinate, relay } => {
            let mut m = serde_json::json!({ "type": "address", "coordinate": coordinate });
            if let Some(r) = relay {
                m["relay"] = serde_json::Value::String(r);
            }
            Some(m)
        }
        BookmarkRow::Url { url } => Some(serde_json::json!({ "type": "url", "url": url })),
        BookmarkRow::Hashtag { hashtag } => {
            Some(serde_json::json!({ "type": "hashtag", "hashtag": hashtag }))
        }
    }
}

// ─── Effect runner ───────────────────────────────────────────────────────────

/// Execute `Effect::DispatchBookmarkAction` — calls `nmp_app_dispatch_action`
/// with the given `namespace` and `json` payload.
///
/// Namespaces: `"nmp.nip51.add_bookmark"` or `"nmp.nip51.remove_bookmark"`.
/// Payload: `BookmarkUpdateInput { account_pubkey, item: BookmarkItem }` JSON.
///
/// Fire-and-forget (D6): the returned correlation-id JSON string is freed and
/// discarded. The updated bookmark list arrives back as a `BookmarksUpdated`
/// projection event via the NMP update callback.
///
/// No-op if `nmp` is `None` (test mode — tests inject `BookmarksUpdated`
/// directly to drive the reducer).
pub(crate) fn run_effect_dispatch_bookmark_action(
    namespace: String,
    json: String,
    nmp: Option<&crate::kernel::actor::NmpHandle>,
) {
    let Some(handle) = nmp else { return };

    let _ = crate::kernel::domains::dispatch_bytes::dispatch_action_bytes_for(
        handle.ptr.as_ptr(),
        &namespace,
        &json,
    );
}

// ─── Projection registration ─────────────────────────────────────────────────

/// Wire the hl `BookmarkListProjection` event observer + typed snapshot
/// projection against `nmp_ref`.
///
/// This creates a SECOND `BookmarkListProjection` (beyond the one wired by
/// `nmp-defaults::register_bookmark_runtime` at boot). Both observe the same
/// kind:10003 events — the second one is exclusively for the hl typed-snapshot
/// path (`"hl.bookmarks"` key → serde-JSON payload → `dispatch_typed_frame`
/// arm → `BookmarksUpdated`). The write actions are NOT re-registered here
/// (nmp-defaults already wired them; double-registration would create
/// duplicate kind:10003 publishes).
///
/// `active_account_slot` is the live `Arc<Mutex<Option<String>>>` that NMP
/// updates on sign-in/switch/logout. Pass `nmp_ref.active_account_handle()`
/// so the projection auto-tracks the active account without manual updates.
///
/// Must be called once at boot (after `nmp_app_start`). The slot automatically
/// reflects future identity changes because NMP writes through the same Arc.
///
/// D6: a null or poisoned observer slot degrades to a silent return without
/// registering the typed projection (so the snapshot never updates but the
/// app does not crash).
pub(crate) fn register_bookmark_list_projection(
    nmp_ref: &NmpApp,
    active_account_slot: Arc<Mutex<Option<String>>>,
) {
    let projection = Arc::new(BookmarkListProjection::new(active_account_slot));

    let observer_id =
        nmp_ref.register_event_observer(Arc::clone(&projection) as Arc<dyn KernelEventObserver>);
    if observer_id.0 == 0 {
        // Observer slot is full or poisoned — skip projection registration.
        tracing::warn!(
            "bookmarks::register_bookmark_list_projection: event-observer registration failed (D6)"
        );
        return;
    }

    // Register the hl-owned typed snapshot projection under key "hl.bookmarks".
    // When NMP ticks, this closure is called, serialises the items array to JSON,
    // and the frame arrives at `dispatch_typed_frame` with schema_id "hl.bookmarks".
    //
    // Wire format: a JSON array of BookmarkItem (the items field of
    // BookmarkListSnapshot). BookmarkItem derives Deserialize; BookmarkListSnapshot
    // and BookmarkListMetadata do not (only Serialize) at b4404159 — so we
    // serialise the items Vec directly to avoid the asymmetry.
    let typed_proj = Arc::clone(&projection);
    nmp_ref.register_typed_snapshot_projection(BOOKMARK_SCHEMA_ID, move || {
        let snapshot = typed_proj.snapshot();
        // Serialise items Vec<BookmarkItem> to JSON bytes (serde envelope).
        let payload = match serde_json::to_vec(&snapshot.items) {
            Ok(b) => b,
            Err(_) => return None,
        };
        Some(nmp_core::TypedProjectionData {
            key: BOOKMARK_SCHEMA_ID.to_string(),
            schema_id: BOOKMARK_SCHEMA_ID.to_string(),
            schema_version: 1,
            file_identifier: String::new(),
            payload,
            ..Default::default()
        })
    });
}

// ─── Unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::action::{AppAction, KernelEvent};
    use crate::kernel::actor::{reduce, Cmd};
    use crate::kernel::app::SessionState;
    use crate::kernel::clock::{Clock, ManualClock};
    use crate::kernel::effect::Effect;
    use crate::kernel::snapshot::BookmarkRow;

    fn make_state() -> AppState {
        AppState::default()
    }

    fn make_state_with_session() -> AppState {
        let mut state = AppState::default();
        state.session = SessionState::Present {
            pubkey: "deadbeef00000000000000000000000000000000000000000000000000000001".to_string(),
            signer_kind: crate::kernel::action::SignerKind::LocalNsec,
        };
        state
    }

    fn step(state: &mut AppState, clock: &ManualClock, cmd: Cmd) -> Vec<Effect> {
        let now = clock.now_unix_seconds();
        reduce(state, cmd, now)
    }

    // 4C-T1: bookmark_frame_updates_state_raw
    //
    // Injecting KernelEvent::BookmarksUpdated with bookmark rows must store them
    // in AppState::bookmarks. Raw fields only — no labels or formatted strings (D1).
    #[test]
    fn bookmark_frame_updates_state_raw() {
        let mut state = make_state();
        let clock = ManualClock::default();

        assert!(state.bookmarks.is_empty(), "bookmarks must start empty");

        let address_row = BookmarkRow::Address {
            coordinate:
                "30023:deadbeef00000000000000000000000000000000000000000000000000000001:my-article"
                    .to_string(),
            relay: None,
        };
        let event_row = BookmarkRow::Event {
            event_id: "aabbcc0000000000000000000000000000000000000000000000000000000001"
                .to_string(),
            relay: None,
        };

        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::BookmarksUpdated(vec![
                address_row.clone(),
                event_row.clone(),
            ])),
        );

        assert_eq!(state.bookmarks.len(), 2, "both bookmark rows stored");
        assert_eq!(&state.bookmarks[0], &address_row);
        assert_eq!(&state.bookmarks[1], &event_row);
    }

    // 7-BM-T: the Articles pane hydration covers ONLY bookmarked kind:30023
    // address rows. BookmarksUpdated ensures previews (pending + a fetch for
    // missing coords); bookmark_article_previews returns them in bookmark order;
    // non-30023 addresses + url/event/hashtag rows are excluded.
    #[test]
    fn bookmarks_article_pane_hydrates_kind_30023_only() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let article_coord =
            "30023:deadbeef00000000000000000000000000000000000000000000000000000001:my-article";

        let effects = step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::BookmarksUpdated(vec![
                BookmarkRow::Address {
                    coordinate: article_coord.to_string(),
                    relay: None,
                },
                // Non-article addressable (a NIP-29 group) — must be excluded.
                BookmarkRow::Address {
                    coordinate: "34550:host:room".to_string(),
                    relay: None,
                },
                BookmarkRow::Url {
                    url: "https://example.com".to_string(),
                },
            ])),
        );

        // The missing article coordinate triggers a resolve fetch (so it fills
        // in over time) — and ONLY for the kind:30023 row.
        let resolves: Vec<_> = effects
            .iter()
            .filter_map(|e| match e {
                Effect::ResolveArtifactCoordinate { coordinate } => Some(coordinate.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            resolves,
            vec![article_coord],
            "only the kind:30023 bookmark gets an article preview fetch; got: {effects:?}"
        );

        // The projected slice carries exactly the one article preview, pending.
        let previews = bookmark_article_previews(&state);
        assert_eq!(
            previews.len(),
            1,
            "only the kind:30023 bookmark is in the articles pane"
        );
        assert_eq!(previews[0].coordinate, article_coord);
        assert!(
            previews[0].pending,
            "missing article starts pending until the fetch resolves"
        );
    }

    // 4C-T2: add_bookmark_dispatches_nip51_add_serde
    //
    // AppAction::AddBookmark{item:Address} must produce exactly one
    // Effect::DispatchBookmarkAction with namespace "nmp.nip51.add_bookmark"
    // and a valid JSON payload containing account_pubkey + item with correct
    // serde structure (tag="type", snake_case).
    #[test]
    fn add_bookmark_dispatches_nip51_add_serde() {
        let mut state = make_state_with_session();
        let clock = ManualClock::default();

        let item = BookmarkRow::Address {
            coordinate:
                "30023:deadbeef00000000000000000000000000000000000000000000000000000001:my-article"
                    .to_string(),
            relay: Some("wss://relay.example.com".to_string()),
        };

        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::AddBookmark { item }),
        );

        assert_eq!(effects.len(), 1, "AddBookmark must emit exactly one effect");
        match &effects[0] {
            Effect::DispatchBookmarkAction { namespace, json } => {
                assert_eq!(
                    namespace, "nmp.nip51.add_bookmark",
                    "namespace must be nmp.nip51.add_bookmark"
                );
                // Validate JSON structure: must parse and have account_pubkey + item.type
                let parsed: serde_json::Value =
                    serde_json::from_str(json).expect("action JSON must be valid");
                assert_eq!(
                    parsed["account_pubkey"].as_str(),
                    Some("deadbeef00000000000000000000000000000000000000000000000000000001"),
                    "account_pubkey must match active session pubkey"
                );
                assert_eq!(
                    parsed["item"]["type"].as_str(),
                    Some("address"),
                    "item type must be 'address' (snake_case serde tag)"
                );
                assert_eq!(
                    parsed["item"]["coordinate"].as_str(),
                    Some("30023:deadbeef00000000000000000000000000000000000000000000000000000001:my-article"),
                    "coordinate must thread through verbatim"
                );
                assert_eq!(
                    parsed["item"]["relay"].as_str(),
                    Some("wss://relay.example.com"),
                    "relay must be included when present"
                );
            }
            other => panic!("expected DispatchBookmarkAction, got {:?}", other),
        }
    }

    // 4C-T3: remove_bookmark_dispatches_nip51_remove
    //
    // AppAction::RemoveBookmark{item:Event} must produce exactly one
    // Effect::DispatchBookmarkAction with namespace "nmp.nip51.remove_bookmark".
    #[test]
    fn remove_bookmark_dispatches_nip51_remove() {
        let mut state = make_state_with_session();
        let clock = ManualClock::default();

        let item = BookmarkRow::Event {
            event_id: "aabbcc0000000000000000000000000000000000000000000000000000000001"
                .to_string(),
            relay: None,
        };

        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::RemoveBookmark { item }),
        );

        assert_eq!(
            effects.len(),
            1,
            "RemoveBookmark must emit exactly one effect"
        );
        match &effects[0] {
            Effect::DispatchBookmarkAction { namespace, json } => {
                assert_eq!(
                    namespace, "nmp.nip51.remove_bookmark",
                    "namespace must be nmp.nip51.remove_bookmark"
                );
                let parsed: serde_json::Value =
                    serde_json::from_str(json).expect("action JSON must be valid");
                assert_eq!(
                    parsed["item"]["type"].as_str(),
                    Some("event"),
                    "item type must be 'event'"
                );
                assert_eq!(
                    parsed["item"]["event_id"].as_str(),
                    Some("aabbcc0000000000000000000000000000000000000000000000000000000001"),
                    "event_id must thread through verbatim"
                );
            }
            other => panic!("expected DispatchBookmarkAction, got {:?}", other),
        }
    }

    // 4C-T4: bookmarks_cleared_on_logout
    //
    // AppAction::Logout must wipe AppState::bookmarks so stale bookmarks from
    // the previous account don't survive into the next session.
    #[test]
    fn bookmarks_cleared_on_logout() {
        let mut state = make_state();
        let clock = ManualClock::default();

        // Seed bookmarks.
        state.bookmarks = vec![BookmarkRow::Url {
            url: "https://example.com".to_string(),
        }];

        step(&mut state, &clock, Cmd::Action(AppAction::Logout));

        assert!(
            state.bookmarks.is_empty(),
            "bookmarks must be empty after Logout"
        );
    }

    // 4C-T5: bookmarks_snapshot_no_chrome_labels
    //
    // BookmarksSnapshot rows must not contain any presentation-layer strings.
    // Verify that apply_bookmarks stores raw protocol data only (D1).
    // Wire format: a JSON array of BookmarkItem (items field only).
    #[test]
    fn bookmarks_snapshot_no_chrome_labels() {
        let mut state = make_state();

        // Build a JSON payload matching the wire format: Vec<BookmarkItem> as array.
        // (BookmarkItem derives Deserialize with tag="type", rename_all="snake_case")
        let payload = serde_json::json!([
            { "type": "address", "coordinate": "30023:aabb000000000000000000000000000000000000000000000000000000000001:slug" },
            { "type": "event", "event_id": "ccdd000000000000000000000000000000000000000000000000000000000001" },
            { "type": "url", "url": "https://example.com/article" },
            { "type": "hashtag", "hashtag": "nostr" }
        ]);
        let payload_bytes = serde_json::to_vec(&payload).unwrap();

        apply_bookmarks(&mut state, &payload_bytes);

        assert_eq!(state.bookmarks.len(), 4);
        // Verify raw data — no formatted labels like "Bookmarked", "min read", etc.
        for row in &state.bookmarks {
            let debug_str = format!("{:?}", row);
            assert!(
                !debug_str.contains("Bookmarked"),
                "snapshots must not contain presentation labels (D1)"
            );
            assert!(
                !debug_str.contains("min read"),
                "snapshots must not contain presentation strings (D1)"
            );
        }
    }

    // 4C-T6: bookmarks_cleared_on_identity_changed_none
    //
    // KernelEvent::IdentityChanged(None) must clear AppState::bookmarks.
    #[test]
    fn bookmarks_cleared_on_identity_changed_none() {
        let mut state = make_state_with_session();
        let clock = ManualClock::default();

        // Seed bookmarks.
        state.bookmarks = vec![BookmarkRow::Hashtag {
            hashtag: "bitcoin".to_string(),
        }];

        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(None)),
        );

        assert!(
            state.bookmarks.is_empty(),
            "bookmarks must be empty after IdentityChanged(None)"
        );
    }

    // 4C-T7: malformed_bookmark_payload_is_noop
    //
    // apply_bookmarks with garbage bytes must not panic or corrupt state (D6).
    #[test]
    fn malformed_bookmark_payload_is_noop() {
        let mut state = make_state();
        // Seed with an existing entry to confirm it is left unchanged.
        state.bookmarks = vec![BookmarkRow::Url {
            url: "https://existing.example.com".to_string(),
        }];

        apply_bookmarks(&mut state, b"NOT VALID JSON AT ALL \x00\xFF");

        assert_eq!(
            state.bookmarks.len(),
            1,
            "malformed payload must leave AppState::bookmarks unchanged (D6)"
        );
    }

    // 4C-T8: add_bookmark_no_op_without_active_session
    //
    // AppAction::AddBookmark without an active session must produce no effects
    // (cannot supply account_pubkey — kernel stays consistent, D6).
    #[test]
    fn add_bookmark_no_op_without_active_session() {
        let mut state = make_state(); // no session
        let clock = ManualClock::default();

        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::AddBookmark {
                item: BookmarkRow::Hashtag {
                    hashtag: "nostr".to_string(),
                },
            }),
        );

        assert!(
            effects.is_empty(),
            "AddBookmark without active session must emit no effects (D6)"
        );
    }
}
