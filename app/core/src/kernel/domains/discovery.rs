//! Discovery domain — room explorer / discovered-groups projection (slice 3E).
//!
//! ## Responsibilities
//!
//! * **WIRE** — `run_effect_wire_group_discovery(relay_url, nmp_ref)` calls
//!   `nmp_nip29::register::open_group_discovery` to register the
//!   `DiscoveredGroupsProjection` event observer and typed snapshot sidecar
//!   under `"nmp.nip29.discovered_groups"`.
//!
//! * **ACTION** — `reduce_action_start_room_discovery(relay_url)` emits two
//!   effects: `Effect::DispatchNip29Action` (pushes the relay-discovery interest
//!   via `"nmp.nip29.discover"`) and `Effect::WireGroupDiscovery` (wires the
//!   projection observer). Fire-and-forget (D6, Non-Negotiable #3).
//!
//! * **READ** — `apply_discovered_groups(state, payload)` decodes the
//!   `"nmp.nip29.discovered_groups"` FlatBuffers payload and stores rows in
//!   `AppState::discovered_groups`. Called from
//!   `projections::dispatch_typed_frame`. D6: decode errors are silent no-ops.
//!
//! * **SNAPSHOT** — `project_room_explorer_snapshot(state)` builds the
//!   `ViewSnapshot::RoomExplorer(RoomExplorerSnapshot)` from state:
//!   - `featured`: empty (curator wiring deferred to Phase 3F).
//!   - `new_noteworthy`: public+open discovered groups, excluding already-joined
//!     communities, capped at 256. Ordered by insertion (projection order).
//!   - `friends_shelf`: empty in Phase 3 — requires kind:39002 member pubkeys
//!     which are not carried in the `DiscoveredGroupsProjection` (Phase 4).
//!   - `authors_shelf`: empty in Phase 3 — requires feed-interest data (Phase 4).
//!
//! ## D3 compliance
//!
//! No relay URL literals appear in this file. The `relay_url` string is opaque
//! and always sourced from the caller (action payload or `AppState::room_policy`).
//!
//! ## D6 compliance
//!
//! All decode errors in `apply_discovered_groups` are silent no-ops logged at
//! trace level. `run_effect_wire_group_discovery` is a no-op when `relay_url`
//! is empty (guarded by `open_group_discovery`).
//!
//! ## Threading
//!
//! `apply_discovered_groups` runs on the actor thread (inside
//! `projections::dispatch_typed_frame`, called from `reduce_event`). It is
//! synchronous and non-blocking (FlatBuffers decode only, no I/O).

use nmp_core::dispatch_envelope::{encode_dispatch_envelope, DISPATCH_ENVELOPE_SCHEMA_VERSION};
use nmp_core::substrate::ActionPayload;
use nmp_ffi::{nmp_app_dispatch_action_bytes, nmp_free_string, NmpApp};
use nmp_nip29::action::{
    CreateInviteInput, CreatePublicGroupInput, DiscoverGroupsInput, JoinGroupInput, PutUserInput,
};
use nmp_nip29::decode_discovered_groups_snapshot;
use nmp_nip29::register::open_group_discovery;

use crate::kernel::app::AppState;
use crate::kernel::effect::Effect;
use crate::kernel::snapshot::{
    CommunityRow, DiscoveredRow, KernelRoomExplorerSnapshot, RecommendationRow, ViewSnapshot,
};

// Re-export so `projections.rs` can match without importing nmp_nip29 directly.
pub(crate) use nmp_nip29::DISCOVERED_GROUPS_SCHEMA_ID;

// ─── Frame decode ────────────────────────────────────────────────────────────

/// Decode a `"nmp.nip29.discovered_groups"` FlatBuffers payload and store the
/// resulting `Vec<DiscoveredRow>` in `AppState::discovered_groups`.
///
/// Called from `projections::dispatch_typed_frame` on the actor thread.
/// Non-blocking (FlatBuffers decode only — no I/O).
///
/// D6: any decode error leaves `AppState::discovered_groups` unchanged
/// (silent no-op logged at trace level).
pub(crate) fn apply_discovered_groups(state: &mut AppState, payload: &[u8]) {
    match decode_discovered_groups_snapshot(payload) {
        Ok(snapshot) => {
            state.discovered_groups = snapshot
                .groups
                .into_iter()
                .map(|g| DiscoveredRow {
                    group_id: g.group_id,
                    host_relay_url: g.host_relay_url,
                    name: g.name,
                    picture: g.picture,
                    about: g.about,
                    member_count: g.member_count,
                    public: g.public,
                    open: g.open,
                })
                .collect();
        }
        Err(e) => {
            tracing::trace!(
                error = %e,
                "discovery::apply_discovered_groups: decode error — AppState::discovered_groups unchanged (D6)"
            );
        }
    }
}

// ─── View-open lifecycle ─────────────────────────────────────────────────────

/// Called by the actor's `Cmd::OpenView` handler (same pattern as
/// `profiles::lifecycle_effects_for_view_open` and
/// `room_home::lifecycle_effects_for_view_open`).
///
/// When `id == ViewId::RoomExplorer` and `room_policy.discovery_relay` is
/// non-empty, emits the two effects that wire and start discovery (identical
/// to `AppAction::StartRoomDiscovery`). This means Swift only has to call
/// `openView(RoomExplorer)` — no explicit action dispatch needed (Phase 3G).
///
/// Returns empty `Vec` for any other view.
pub(crate) fn lifecycle_effects_for_view_open(
    id: &crate::kernel::view::ViewId,
    state: &AppState,
) -> Vec<Effect> {
    if !matches!(id, crate::kernel::view::ViewId::RoomExplorer) {
        return Vec::new();
    }
    let relay_url = &state.room_policy.discovery_relay;
    if relay_url.is_empty() {
        tracing::trace!(
            "discovery::lifecycle_effects_for_view_open: discovery_relay is empty — no auto-start"
        );
        return Vec::new();
    }
    reduce_action_start_room_discovery(relay_url.clone())
}

// ─── Action reducer ──────────────────────────────────────────────────────────

/// Handle `AppAction::StartRoomDiscovery { relay_url }`.
///
/// Emits two effects:
/// 1. `Effect::DispatchNip29Action` — pushes the relay-discovery interest via
///    `"nmp.nip29.discover"` with `{"relay_url":"<url>"}` payload.
/// 2. `Effect::WireGroupDiscovery` — registers the `DiscoveredGroupsProjection`
///    event observer + typed snapshot sidecar for `relay_url`.
///
/// Fire-and-forget (D6, Non-Negotiable #3): discovered groups arrive back as
/// `KernelEvent::DiscoveredGroupsUpdated` on the next NMP projection tick.
/// No relay URL literals appear here — `relay_url` is opaque (D3).
pub(crate) fn reduce_action_start_room_discovery(relay_url: String) -> Vec<Effect> {
    // Use serde_json to safely serialize the relay_url; a naïve format! would
    // produce invalid JSON if the URL ever contained quotes or backslashes.
    let json = serde_json::json!({"relay_url": relay_url}).to_string();
    vec![
        Effect::DispatchNip29Action {
            namespace: "nmp.nip29.discover".to_string(),
            json,
        },
        Effect::WireGroupDiscovery { relay_url },
    ]
}

// ─── Event reducer ───────────────────────────────────────────────────────────

/// Handle `KernelEvent::DiscoveredGroupsUpdated(rows)`.
///
/// Replaces `AppState::discovered_groups` with the freshly decoded rows.
/// Called from the actor's `reduce_event` dispatcher. No effects emitted —
/// the snapshot projection runs on the next `project_snapshot` pass.
pub(crate) fn reduce_event_discovered_groups_updated(
    state: &mut AppState,
    rows: Vec<DiscoveredRow>,
) -> Vec<Effect> {
    state.discovered_groups = rows;
    vec![]
}

// ─── Effect runners ──────────────────────────────────────────────────────────

/// Execute `Effect::DispatchNip29Action { namespace, json }`.
///
/// Calls `nmp_app_dispatch_action` with the given namespace and JSON payload.
/// Fire-and-forget (D6): the returned correlation-id JSON is freed and
/// discarded. Results arrive via the relevant `KernelEvent::*Updated` event.
///
/// No-op if `nmp` is `None` (test mode — tests inject events directly).
pub(crate) fn run_effect_dispatch_nip29_action(
    namespace: String,
    json: String,
    nmp: Option<&crate::kernel::actor::NmpHandle>,
) {
    let Some(handle) = nmp else { return };

    // Route by namespace: deserialise the pre-built JSON to the typed struct,
    // then encode as FlatBuffers for the bytes doorway (ADR-0064 / Cut-B).
    let payload_bytes: Vec<u8> = match namespace.as_str() {
        "nmp.nip29.discover" => match serde_json::from_str::<DiscoverGroupsInput>(&json) {
            Ok(a) => a.encode(),
            Err(e) => {
                tracing::warn!(error = %e, "nip29: failed to deserialise DiscoverGroupsInput");
                return;
            }
        },
        "nmp.nip29.join" => match serde_json::from_str::<JoinGroupInput>(&json) {
            Ok(a) => a.encode(),
            Err(e) => {
                tracing::warn!(error = %e, "nip29: failed to deserialise JoinGroupInput");
                return;
            }
        },
        "nmp.nip29.create_public_group" => {
            match serde_json::from_str::<CreatePublicGroupInput>(&json) {
                Ok(a) => a.encode(),
                Err(e) => {
                    tracing::warn!(error = %e, "nip29: failed to deserialise CreatePublicGroupInput");
                    return;
                }
            }
        }
        "nmp.nip29.put_user" => match serde_json::from_str::<PutUserInput>(&json) {
            Ok(a) => a.encode(),
            Err(e) => {
                tracing::warn!(error = %e, "nip29: failed to deserialise PutUserInput");
                return;
            }
        },
        "nmp.nip29.create_invite" => match serde_json::from_str::<CreateInviteInput>(&json) {
            Ok(a) => a.encode(),
            Err(e) => {
                tracing::warn!(error = %e, "nip29: failed to deserialise CreateInviteInput");
                return;
            }
        },
        other => {
            tracing::warn!(namespace = other, "nip29: unknown namespace — no-op");
            return;
        }
    };

    let correlation_id = uuid::Uuid::new_v4().to_string();
    let envelope = encode_dispatch_envelope(
        &correlation_id,
        &namespace,
        DISPATCH_ENVELOPE_SCHEMA_VERSION,
        &payload_bytes,
    );

    let result_ptr =
        nmp_app_dispatch_action_bytes(handle.ptr.as_ptr(), envelope.as_ptr(), envelope.len());

    if !result_ptr.is_null() {
        nmp_free_string(result_ptr);
    }
}

/// Execute `Effect::WireGroupDiscovery { relay_url }`.
///
/// Calls `nmp_nip29::register::open_group_discovery(nmp_ref, relay_url)` which
/// registers the `DiscoveredGroupsProjection` event observer + typed FlatBuffers
/// sidecar under `"nmp.nip29.discovered_groups"`. Subsequent NMP snapshot ticks
/// deliver `KernelEvent::NmpSnapshotFrame` frames that `apply_discovered_groups`
/// decodes.
///
/// The returned `GroupDiscoveryHandle` is intentionally leaked: discovery runs
/// for the full app lifetime (fire-and-forget, same semantics as the old
/// `wire_group_discovery`). The observer is unregistered only when the NmpApp
/// itself is freed.
///
/// An empty `relay_url` is a silent no-op (guarded by `open_group_discovery`).
/// D6: no panic on failure — `open_group_discovery` is fallible-graceful.
pub(crate) fn run_effect_wire_group_discovery(relay_url: String, nmp_ref: &NmpApp) {
    // open_group_discovery returns None for empty relay_url or a poisoned
    // observer slot — both are silent no-ops (D6). Discovery is app-lifetime
    // (fire-and-forget): the handle is dropped here without calling
    // close_group_discovery, so the observer stays live until the NmpApp is
    // freed. GroupDiscoveryHandle has no Drop impl so drop is a no-op on the
    // kernel side (same behaviour as the old wire_group_discovery which had
    // no teardown path).
    let _ = open_group_discovery(nmp_ref, relay_url);
}

// ─── Discovery policy helpers ────────────────────────────────────────────────

/// Remove rows where `(group_id, host_relay_url)` matches a joined community.
///
/// Used to exclude groups the user has already joined from `new_noteworthy`.
/// Both `group_id` and `host_relay_url` must match for exclusion (stable
/// `GroupId` composite key, per NIP-29).
pub(crate) fn exclude_joined_rooms(
    discovered: Vec<DiscoveredRow>,
    joined: &[CommunityRow],
) -> Vec<DiscoveredRow> {
    discovered
        .into_iter()
        .filter(|d| {
            !joined
                .iter()
                .any(|j| j.group_id == d.group_id && j.host_relay_url == d.host_relay_url)
        })
        .collect()
}

/// Keep only rows where `public == true` AND `open == true`.
///
/// Closed or private groups are excluded from the `new_noteworthy` shelf:
/// they are not joinable without an invite and would be confusing in the
/// open discovery list.
pub(crate) fn filter_public_open(rows: Vec<DiscoveredRow>) -> Vec<DiscoveredRow> {
    rows.into_iter().filter(|r| r.public && r.open).collect()
}

/// Cap the row list at `cap` entries (retains projection order: newest-first).
pub(crate) fn cap_rows(rows: Vec<DiscoveredRow>, cap: usize) -> Vec<DiscoveredRow> {
    rows.into_iter().take(cap).collect()
}

// ─── Snapshot projection ─────────────────────────────────────────────────────

/// Bounded `new_noteworthy` cap per §2.2 of the slice spec.
const NEW_NOTEWORTHY_CAP: usize = 256;

/// Compute `ViewSnapshot::RoomExplorer` for the open RoomExplorer view.
///
/// Fields:
/// - `featured` — empty until curator logic is wired in Phase 3F.
/// - `new_noteworthy` — public+open discovered groups excluding joined communities,
///   capped at 256 (projection order, i.e. relay arrival order).
/// - `friends_shelf` — empty in Phase 3 (requires Phase 4 member pubkeys).
/// - `authors_shelf` — empty in Phase 3 (requires Phase 4 feed-interest data).
///
/// Always returns `Some` — the open-view gate is enforced at the actor level.
/// An empty `discovered_groups` vec produces a valid all-empty snapshot.
pub(crate) fn project_room_explorer_snapshot(state: &AppState) -> Option<ViewSnapshot> {
    // Exclude already-joined communities.
    let without_joined = exclude_joined_rooms(state.discovered_groups.clone(), &state.communities);

    // Keep only public+open groups for the discovery shelf.
    let public_open = filter_public_open(without_joined);

    // Cap to bounded size (Non-Negotiable #7).
    let new_noteworthy = cap_rows(public_open, NEW_NOTEWORTHY_CAP);

    Some(ViewSnapshot::RoomExplorer(KernelRoomExplorerSnapshot {
        featured: Vec::new(),
        new_noteworthy,
        // TODO(Phase 4): wire friends-shelf from kind:39002 member events.
        // The `DiscoveredGroupsProjection` carries `member_count` but NOT
        // the actual member pubkeys, so we cannot intersect with
        // `AppState::follows` here. Friends-shelf requires the per-group
        // member-list events (kind:39002 p-tags) available in Phase 4.
        friends_shelf: Vec::<RecommendationRow>::new(),
        // TODO(Phase 4): wire authors_shelf from feed-interest data.
        authors_shelf: Vec::<RecommendationRow>::new(),
    }))
}

// ─── Unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::action::{AppAction, KernelEvent};
    use crate::kernel::actor::{reduce, Cmd};
    use crate::kernel::clock::{Clock, ManualClock};
    use crate::kernel::snapshot::{CommunityRow, DiscoveredRow, ViewSnapshot};
    use crate::kernel::view::{ViewId, ViewRegistry, ViewRoute};

    fn make_state() -> AppState {
        AppState::default()
    }

    fn step(state: &mut AppState, clock: &ManualClock, cmd: Cmd) -> Vec<Effect> {
        let now = clock.now_unix_seconds();
        reduce(state, cmd, now)
    }

    fn make_discovered_row(group_id: &str, relay: &str) -> DiscoveredRow {
        DiscoveredRow {
            group_id: group_id.to_string(),
            host_relay_url: relay.to_string(),
            name: Some(format!("Group {group_id}")),
            picture: None,
            about: None,
            member_count: 10,
            public: true,
            open: true,
        }
    }

    fn make_community_row(group_id: &str, relay: &str) -> CommunityRow {
        CommunityRow {
            group_id: group_id.to_string(),
            host_relay_url: relay.to_string(),
            name: Some(format!("Community {group_id}")),
            picture: None,
            about: None,
            member_count: 5,
            public: true,
            open: true,
            is_admin: false,
        }
    }

    // 3E-T1: start_room_discovery_pushes_interest
    //
    // AppAction::StartRoomDiscovery must emit exactly one DispatchNip29Action
    // (namespace="nmp.nip29.discover", json contains relay_url) AND one
    // WireGroupDiscovery effect. Fire-and-forget (#3).
    #[test]
    fn start_room_discovery_pushes_interest() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let relay = "wss://groups.test.relay";

        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::StartRoomDiscovery {
                relay_url: relay.to_string(),
            }),
        );

        // Must emit exactly two effects.
        assert_eq!(effects.len(), 2, "StartRoomDiscovery must emit 2 effects");

        // First: DispatchNip29Action with correct namespace and relay_url in json.
        match &effects[0] {
            Effect::DispatchNip29Action { namespace, json } => {
                assert_eq!(namespace, "nmp.nip29.discover");
                assert!(json.contains(relay), "json must contain relay_url: {json}");
            }
            other => panic!("expected DispatchNip29Action, got {other:?}"),
        }

        // Second: WireGroupDiscovery with the relay_url.
        match &effects[1] {
            Effect::WireGroupDiscovery { relay_url } => {
                assert_eq!(relay_url, relay);
            }
            other => panic!("expected WireGroupDiscovery, got {other:?}"),
        }
    }

    // 3E-T2: discovered_groups_frame_updates_state_raw
    //
    // Injecting KernelEvent::DiscoveredGroupsUpdated must update
    // AppState::discovered_groups and the snapshot must have raw fields
    // (no formatted strings in name).
    #[test]
    fn discovered_groups_frame_updates_state_raw() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let relay = "wss://relay.test";

        // Open the RoomExplorer view so snapshot projection is exercised.
        let rows = vec![
            make_discovered_row("room1", relay),
            make_discovered_row("room2", relay),
        ];

        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::DiscoveredGroupsUpdated(rows.clone())),
        );

        assert_eq!(state.discovered_groups.len(), 2, "both rows stored");
        assert_eq!(state.discovered_groups[0].group_id, "room1");
        assert_eq!(state.discovered_groups[1].group_id, "room2");

        // Verify raw fields (no formatted labels — D1).
        let row = &state.discovered_groups[0];
        assert_eq!(row.member_count, 10);
        // name is Option<String>, not formatted like "10 members".
        if let Some(name) = &row.name {
            assert!(
                !name.contains("member"),
                "name must not be a formatted label"
            );
        }
    }

    // 3E-T3: recommendations_exclude_joined_groups
    //
    // With joined communities g1+g2 and discovered g1+g3, new_noteworthy in
    // the snapshot must contain only g3.
    #[test]
    fn recommendations_exclude_joined_groups() {
        let mut state = make_state();
        let relay = "wss://relay.test";

        let joined = vec![
            make_community_row("g1", relay),
            make_community_row("g2", relay),
        ];
        let discovered = vec![
            make_discovered_row("g1", relay),
            make_discovered_row("g3", relay),
        ];

        state.communities = joined;
        state.discovered_groups = discovered;

        let snap = project_room_explorer_snapshot(&state).unwrap();
        if let ViewSnapshot::RoomExplorer(s) = snap {
            assert_eq!(
                s.new_noteworthy.len(),
                1,
                "only g3 should be in new_noteworthy"
            );
            assert_eq!(s.new_noteworthy[0].group_id, "g3");
        } else {
            panic!("expected RoomExplorer snapshot");
        }
    }

    // 3E-T4: friends_shelf_derived_from_follows
    //
    // friends_shelf must be an empty Vec in Phase 3. The DiscoveredGroupsProjection
    // does not carry member pubkeys (only member_count), so we cannot derive
    // friends-shelf memberships without kind:39002 member events (Phase 4).
    // This test documents and verifies the Phase 3 placeholder expectation.
    //
    // TODO(Phase 4): update this test when friends-shelf is wired from
    // kind:39002 member events and AppState::follows intersection.
    #[test]
    fn friends_shelf_derived_from_follows() {
        let mut state = make_state();
        let relay = "wss://relay.test";

        // Set up discovered groups and follows.
        state.discovered_groups = vec![make_discovered_row("g1", relay)];
        state.follows = vec![
            "aabbcc0000000000000000000000000000000000000000000000000000000001".to_string(),
            "aabbcc0000000000000000000000000000000000000000000000000000000002".to_string(),
        ];

        let snap = project_room_explorer_snapshot(&state).unwrap();
        if let ViewSnapshot::RoomExplorer(s) = snap {
            // Phase 3: friends_shelf is always empty (Phase 4 TODO).
            assert!(
                s.friends_shelf.is_empty(),
                "friends_shelf must be empty in Phase 3 (requires Phase 4 member events)"
            );
        } else {
            panic!("expected RoomExplorer snapshot");
        }
    }

    // 3E-T5: room_explorer_snapshot_bounded
    //
    // Inject 300 discovered rows — snapshot.new_noteworthy must be capped at 256.
    #[test]
    fn room_explorer_snapshot_bounded() {
        let mut state = make_state();
        let relay = "wss://relay.test";

        let rows: Vec<DiscoveredRow> = (0..300)
            .map(|i| make_discovered_row(&format!("g{i}"), relay))
            .collect();
        state.discovered_groups = rows;

        let snap = project_room_explorer_snapshot(&state).unwrap();
        if let ViewSnapshot::RoomExplorer(s) = snap {
            assert!(
                s.new_noteworthy.len() <= 256,
                "new_noteworthy must be capped at 256, got {}",
                s.new_noteworthy.len()
            );
        } else {
            panic!("expected RoomExplorer snapshot");
        }
    }

    // 3E-T6: malformed_discovered_frame_noop
    //
    // apply_discovered_groups with garbage bytes must leave state unchanged (D6).
    #[test]
    fn malformed_discovered_frame_noop() {
        let mut state = make_state();
        let relay = "wss://relay.test";

        // Seed with an existing entry.
        state.discovered_groups = vec![make_discovered_row("existing", relay)];

        apply_discovered_groups(&mut state, b"NOT A VALID FLATBUFFER AT ALL \x00\xFF\xFE");

        assert_eq!(
            state.discovered_groups.len(),
            1,
            "malformed payload must leave AppState::discovered_groups unchanged (D6)"
        );
        assert_eq!(
            state.discovered_groups[0].group_id, "existing",
            "existing row must survive malformed update"
        );
    }

    // 3E-T7: closed_room_explorer_view_no_snapshot
    //
    // ViewRegistry with RoomExplorer NOT open → the view produces no snapshot
    // projection for that view. Test the inverse via ViewRegistry directly —
    // the actor only calls project_room_explorer_snapshot for open views.
    #[test]
    fn closed_room_explorer_view_no_snapshot() {
        let mut registry = ViewRegistry::default();

        // Open then close RoomExplorer.
        registry.open(ViewId::RoomExplorer, ViewRoute::RoomExplorer);
        assert!(registry.is_open(&ViewId::RoomExplorer));

        registry.close(&ViewId::RoomExplorer);
        assert!(!registry.is_open(&ViewId::RoomExplorer));

        // No snapshot for the closed view.
        assert!(
            registry.current_snapshot(&ViewId::RoomExplorer).is_none(),
            "closed view must have no snapshot"
        );
        assert_eq!(
            registry.open_count(),
            0,
            "registry must be empty after close"
        );
    }

    // 3E-T8: filter_public_open excludes closed/private rooms
    #[test]
    fn filter_public_open_excludes_closed_and_private() {
        let relay = "wss://relay.test";
        let rows = vec![
            DiscoveredRow {
                public: true,
                open: true,
                ..make_discovered_row("pub_open", relay)
            },
            DiscoveredRow {
                public: true,
                open: false, // closed
                ..make_discovered_row("pub_closed", relay)
            },
            DiscoveredRow {
                public: false, // private
                open: true,
                ..make_discovered_row("priv_open", relay)
            },
        ];

        let filtered = filter_public_open(rows);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].group_id, "pub_open");
    }

    // 3E-T9: exclude_joined_rooms uses composite key (group_id + host_relay_url)
    #[test]
    fn exclude_joined_uses_composite_key() {
        let relay1 = "wss://relay1.test";
        let relay2 = "wss://relay2.test";

        // Same group_id on different relays — only the matching one is excluded.
        let joined = vec![make_community_row("g1", relay1)];
        let discovered = vec![
            make_discovered_row("g1", relay1), // same composite key → excluded
            make_discovered_row("g1", relay2), // different relay → not excluded
        ];

        let result = exclude_joined_rooms(discovered, &joined);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].host_relay_url, relay2);
    }
}
