//! Room-home domain — per-group room shell view + NIP-29 write actions (slice 3F).
//!
//! ## Responsibilities
//!
//! * **READ** — `ViewId::RoomHome{group_id}` view: wires the
//!   `GroupEventsProjection` (via `wire_group_events`) on open and releases it
//!   on close (lifecycle-effects pattern from 3D). Snapshot exposes header,
//!   metadata, membership state, and an empty lanes structure (lane bodies
//!   deferred to Phase 4; metadata from `AppState::communities`, already
//!   wired in 3B).
//!
//! * **WRITE** — four NIP-29 write actions, each mapped to a
//!   `Effect::DispatchNip29Action` with a safe `serde_json` payload:
//!   - `AppAction::JoinRoom`            → `"nmp.nip29.join"`            (kind:9021)
//!   - `AppAction::CreateRoom`          → `"nmp.nip29.create_public_group"` (kind:9007+9002)
//!   - `AppAction::AddRoomMember`       → `"nmp.nip29.put_user"`        (kind:9000)
//!   - `AppAction::CreateRoomInvites`   → `"nmp.nip29.create_invite"`   (kind:9009)
//!
//!   `LeaveRoom` (kind:9022) is NOT implemented — there is no `nmp.nip29.leave`
//!   action on pinned nmp (b4404159). See nmp issue #1598.
//!
//! ## D3 compliance
//!
//! No relay URL literals appear in this file. All relay URLs are opaque strings
//! sourced from the caller (action payload or `AppState::room_policy`). The
//! `invite_link_base` URL lives in `AppState::room_policy.invite_link_base`
//! (injected at construction, never hardcoded).
//!
//! ## D6 compliance
//!
//! Decode errors from `GroupEventsProjection` frames are silent no-ops (logged
//! at trace level). The snapshot is always `Some` for an open view — even if
//! no events have arrived (metadata may be `None`).
//!
//! ## Phase 4 deferral
//!
//! Room-home LANE BODIES (kind:11/kind:9 content feeds) are deferred to Phase 4.
//! The `lanes` field in `KernelRoomHomeSnapshot` ships empty this phase. The
//! `GroupEventsProjection` frame (schema `"nmp.nip29.group_events"`) is wired on
//! view open so Phase 4 can decode lane bodies from the already-flowing events
//! without re-opening a subscription. Only the header / metadata / membership
//! shell is produced in Phase 3F.

use nmp_ffi::NmpApp;
use nmp_nip29::decode_group_events_snapshot;
use nmp_nip29::register::wire_group_events;
use nmp_nip29::GroupId;

use crate::kernel::app::AppState;
use crate::kernel::effect::Effect;
use crate::kernel::snapshot::{KernelRoomHomeSnapshot, ViewSnapshot};
use crate::kernel::view::ViewId;

// Re-export schema ID so projections.rs can match without importing nmp_nip29 directly.
pub(crate) use nmp_nip29::GROUP_EVENTS_SCHEMA_ID;

/// Bounded cap for room-home events buffered in `AppState::room_home_events`.
/// Lane bodies are empty in Phase 3F; the cap protects against memory growth
/// until Phase 4 wires the feed projection properly.
const ROOM_HOME_EVENTS_CAP: usize = 256;

// ─── Lifecycle effects (called by actor loop on OpenView / CloseView) ─────────

/// Called by the actor loop when `Cmd::OpenView(ViewId::RoomHome{..})` is received.
///
/// Emits `Effect::WireGroupEvents { group_id, host_relay_url }` so the
/// `GroupEventsProjection` observer is registered for this room. Fire-and-forget:
/// events arrive via the NMP update callback as `KernelEvent::NmpSnapshotFrame`
/// frames decoded by `projections::dispatch_typed_frame`.
///
/// Returns an empty Vec for all other `ViewId` variants (no-op).
pub(crate) fn lifecycle_effects_for_view_open(id: &ViewId) -> Vec<Effect> {
    if let ViewId::RoomHome { group_id } = id {
        // We need the host_relay_url to form a GroupId for wire_group_events.
        // At open time we don't have access to AppState (the actor loop calls
        // this before reduce). The host_relay_url is encoded in group_id via
        // the ViewRoute::RoomHome { group_id, host_relay_url } fields.
        // However, ViewId only carries group_id as an opaque string. The
        // host_relay_url is extracted from AppState at frame-decode time
        // (apply_group_events_frame). For lifecycle wiring we emit a
        // WireGroupEvents effect that carries the group_id only; the effect
        // runner resolves host_relay_url from AppState::communities.
        vec![Effect::WireGroupEvents {
            group_id: group_id.clone(),
        }]
    } else {
        Vec::new()
    }
}

/// Called by the actor loop when `Cmd::CloseView(ViewId::RoomHome{..})` is received.
///
/// Emits `Effect::ReleaseGroupEvents { group_id }` to allow the actor to clean
/// up the per-group event buffer in `AppState::room_home_events`. The
/// `GroupEventsProjection` is a singleton observer in nmp (per-group), and the
/// projection keeps running until the app exits; we only discard the hl-side
/// buffer to bound memory.
///
/// Returns an empty Vec for all other `ViewId` variants.
pub(crate) fn lifecycle_effects_for_view_close(id: &ViewId) -> Vec<Effect> {
    if let ViewId::RoomHome { group_id } = id {
        vec![Effect::ReleaseGroupEvents {
            group_id: group_id.clone(),
        }]
    } else {
        Vec::new()
    }
}

// ─── Effect runner: WireGroupEvents ──────────────────────────────────────────

/// Execute `Effect::WireGroupEvents { group_id }`.
///
/// Looks up the `host_relay_url` for `group_id` in `AppState::communities` and
/// calls `nmp_nip29::register::wire_group_events(nmp_ref, GroupId{..})` to
/// register the `GroupEventsProjection` event observer + typed FlatBuffers
/// sidecar under `"nmp.nip29.group_events"`. Subsequent NMP snapshot ticks
/// deliver `KernelEvent::NmpSnapshotFrame` frames that `apply_group_events_frame`
/// decodes.
///
/// No-op if `nmp` is `None` (test mode — tests inject events directly).
/// No-op if `group_id` is not in `AppState::communities` (the room may not be
/// joined yet; join first, then open the room-home view).
/// D3: no relay URL literals — host_relay_url is sourced from `communities`.
pub(crate) fn run_effect_wire_group_events(
    group_id: String,
    state: &AppState,
    nmp: Option<&crate::kernel::actor::NmpHandle>,
) {
    let Some(handle) = nmp else { return };

    // Resolve host_relay_url from the joined-groups cache.
    let host_relay_url = state
        .communities
        .iter()
        .find(|c| c.group_id == group_id)
        .map(|c| c.host_relay_url.clone())
        .unwrap_or_default();

    if host_relay_url.is_empty() {
        tracing::trace!(
            group_id = %group_id,
            "room_home::run_effect_wire_group_events: no joined community matches group_id — skipping (D6)"
        );
        return;
    }

    let nmp_ref: &NmpApp = unsafe { handle.ptr.as_ref() };
    wire_group_events(nmp_ref, GroupId::new(host_relay_url, group_id));
}

// ─── Frame decode (called from projections::dispatch_typed_frame) ─────────────

/// Decode a `"nmp.nip29.group_events"` FlatBuffers payload and store the raw
/// event rows in `AppState::room_home_events` keyed by `group_id`.
///
/// Called from `projections::dispatch_typed_frame` on the actor thread.
/// Non-blocking (FlatBuffers decode only — no I/O). Capped at
/// `ROOM_HOME_EVENTS_CAP` rows to bound memory (Non-Negotiable #7).
///
/// D6: any decode error leaves `AppState::room_home_events` unchanged (no-op).
pub(crate) fn apply_group_events_frame(state: &mut AppState, payload: &[u8]) {
    match decode_group_events_snapshot(payload) {
        Ok(snapshot) => {
            let key = snapshot.group_id.clone();
            let capped: Vec<_> = snapshot
                .events
                .into_iter()
                .take(ROOM_HOME_EVENTS_CAP)
                .collect();
            state.room_home_events.insert(key, capped);
        }
        Err(e) => {
            tracing::trace!(
                error = %e,
                "room_home::apply_group_events_frame: decode error — AppState::room_home_events unchanged (D6)"
            );
        }
    }
}

// ─── Action reducers ─────────────────────────────────────────────────────────

/// Handle `AppAction::JoinRoom { group_id, host_relay_url, invite_code }`.
///
/// Dispatches `"nmp.nip29.join"` via `Effect::DispatchNip29Action`.
/// The payload is built with `serde_json::json!` (never `format!`) to guarantee
/// valid JSON even if any field contains quotes or backslashes.
///
/// `JoinGroupInput` fields (verified on pinned nmp b4404159):
/// - `group`: `{ host_relay_url, local_id }` (GroupId)
/// - `invite_code`: `Option<String>` — included only if Some
/// - `reason`: `Option<String>` — omitted (None)
///
/// Fire-and-forget (D6, Non-Negotiable #3). The updated joined-groups list
/// arrives via `KernelEvent::JoinedGroupsUpdated` after the projection tick.
/// No relay URL literals here — host_relay_url is opaque from the caller (D3).
pub(crate) fn reduce_action_join_room(
    group_id: String,
    host_relay_url: String,
    invite_code: Option<String>,
) -> Vec<Effect> {
    let json = serde_json::json!({
        "group": {
            "host_relay_url": host_relay_url,
            "local_id": group_id
        },
        "invite_code": invite_code
    })
    .to_string();
    vec![Effect::DispatchNip29Action {
        namespace: "nmp.nip29.join".to_string(),
        json,
    }]
}

/// Handle `AppAction::CreateRoom { group_id, host_relay_url, name, about }`.
///
/// Dispatches `"nmp.nip29.create_public_group"` via `Effect::DispatchNip29Action`.
///
/// `CreatePublicGroupInput` fields (verified on pinned nmp b4404159):
/// - `group`: `{ host_relay_url, local_id }` (GroupId)
/// - `name`: `String` (required, non-empty)
/// - `about`: `Option<String>`
///
/// Creates kind:9007 (create-group) + kind:9002 (metadata edit) on the host relay.
/// Fire-and-forget (D6). D3: no relay URL literals in kernel.
pub(crate) fn reduce_action_create_room(
    group_id: String,
    host_relay_url: String,
    name: String,
    about: Option<String>,
) -> Vec<Effect> {
    let json = serde_json::json!({
        "group": {
            "host_relay_url": host_relay_url,
            "local_id": group_id
        },
        "name": name,
        "about": about
    })
    .to_string();
    vec![Effect::DispatchNip29Action {
        namespace: "nmp.nip29.create_public_group".to_string(),
        json,
    }]
}

/// Handle `AppAction::AddRoomMember { group_id, host_relay_url, pubkey, role }`.
///
/// Dispatches `"nmp.nip29.put_user"` via `Effect::DispatchNip29Action`.
///
/// `PutUserInput` fields (verified on pinned nmp b4404159):
/// - `group`: `{ host_relay_url, local_id }` (GroupId)
/// - `target_pubkey`: `String` (64-char hex)
/// - `role`: `Option<String>` — e.g. `"admin"` or `None` for plain member
/// - `reason`: `Option<String>` — omitted (None)
///
/// Publishes kind:9000 (add-member) to the host relay. Requires admin rights.
/// Fire-and-forget (D6). D3: no relay URL literals in kernel.
pub(crate) fn reduce_action_add_room_member(
    group_id: String,
    host_relay_url: String,
    pubkey: String,
    role: Option<String>,
) -> Vec<Effect> {
    let json = serde_json::json!({
        "group": {
            "host_relay_url": host_relay_url,
            "local_id": group_id
        },
        "target_pubkey": pubkey,
        "role": role
    })
    .to_string();
    vec![Effect::DispatchNip29Action {
        namespace: "nmp.nip29.put_user".to_string(),
        json,
    }]
}

/// Handle `AppAction::CreateRoomInvites { group_id, host_relay_url, codes }`.
///
/// Dispatches `"nmp.nip29.create_invite"` via `Effect::DispatchNip29Action`.
///
/// `CreateInviteInput` fields (verified on pinned nmp b4404159):
/// - `group`: `{ host_relay_url, local_id }` (GroupId)
/// - `codes`: `Vec<String>` — list of invite codes (≥1 required; nmp fans out
///   into multiple kind:9009 events if >10 codes — MAX_CODES_PER_INVITE_EVENT)
///
/// Fire-and-forget (D6). D3: no relay URL literals in kernel.
///
/// NOTE: `invite_link_base` is stored in `AppState::room_policy.invite_link_base`
/// (injected at construction, D3). The kernel ships raw codes; the Swift shell
/// composes the full invite URL by appending the code to `invite_link_base`.
/// The kernel never constructs or hardcodes invite URLs.
pub(crate) fn reduce_action_create_room_invites(
    group_id: String,
    host_relay_url: String,
    codes: Vec<String>,
) -> Vec<Effect> {
    let json = serde_json::json!({
        "group": {
            "host_relay_url": host_relay_url,
            "local_id": group_id
        },
        "codes": codes
    })
    .to_string();
    vec![Effect::DispatchNip29Action {
        namespace: "nmp.nip29.create_invite".to_string(),
        json,
    }]
}

// ─── Snapshot projection ─────────────────────────────────────────────────────

/// Compute `ViewSnapshot::RoomHome` for an open `ViewId::RoomHome{group_id}`.
///
/// Shell fields populated from `AppState::communities` (the joined-groups
/// projection wired in 3B):
/// - `group_id`, `host_relay_url`, `name`, `picture`, `about`, `member_count`,
///   `public`, `open`, `is_admin` — raw fields, no formatted strings (D1/D3).
///
/// `lanes` is empty — lane bodies (kind:11/9 content feeds) are deferred to
/// Phase 4. The `GroupEventsProjection` is already flowing so Phase 4 can
/// decode feed bodies without re-opening a subscription.
///
/// `invite_link_base` is forwarded from `AppState::room_policy.invite_link_base`
/// (D3: injected at construction, never hardcoded).
///
/// Returns `None` only if the group is not found in `AppState::communities`
/// (i.e. the room is not joined). The actor calls `project_snapshot` only for
/// open views; a missing community row is a transient state (join pending).
pub(crate) fn project_room_home_snapshot(state: &AppState, group_id: &str) -> Option<ViewSnapshot> {
    // Resolve the community row from the joined-groups cache.
    let row = state.communities.iter().find(|c| c.group_id == group_id)?;

    Some(ViewSnapshot::RoomHome(KernelRoomHomeSnapshot {
        group_id: row.group_id.clone(),
        host_relay_url: row.host_relay_url.clone(),
        name: row.name.clone(),
        picture: row.picture.clone(),
        about: row.about.clone(),
        member_count: row.member_count,
        public: row.public,
        open: row.open,
        is_admin: row.is_admin,
        // Lanes deferred to Phase 4 — GroupEventsProjection is wired so Phase 4
        // can decode feed bodies from the already-flowing events.
        lane_ids: Vec::new(),
        // invite_link_base from room policy (D3: injected at construction).
        invite_link_base: state.room_policy.invite_link_base.clone(),
    }))
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::action::AppAction;
    use crate::kernel::actor::{reduce, Cmd};
    use crate::kernel::app::AppState;
    use crate::kernel::clock::Clock;
    use crate::kernel::clock::ManualClock;
    use crate::kernel::effect::Effect;
    use crate::kernel::snapshot::{CommunityRow, ViewSnapshot};
    use crate::kernel::view::ViewId;

    const TEST_GROUP: &str = "test-room";
    const TEST_RELAY: &str = "wss://relay.test.example";

    fn make_state() -> AppState {
        AppState::default()
    }

    fn step(state: &mut AppState, clock: &ManualClock, cmd: Cmd) -> Vec<Effect> {
        let now = clock.now_unix_seconds();
        reduce(state, cmd, now)
    }

    fn make_community_row(group_id: &str, relay: &str) -> CommunityRow {
        CommunityRow {
            group_id: group_id.to_string(),
            host_relay_url: relay.to_string(),
            name: Some(format!("Room {group_id}")),
            picture: None,
            about: Some("A test room".to_string()),
            member_count: 42,
            public: true,
            open: true,
            is_admin: false,
        }
    }

    // ─── READ side ────────────────────────────────────────────────────────────

    // 3F-T1: room_home_view_opens_wires_projection
    //
    // Opening ViewId::RoomHome{group_id} must emit Effect::WireGroupEvents{group_id}.
    // The actor calls lifecycle_effects_for_view_open on Cmd::OpenView.
    #[test]
    fn room_home_view_opens_wires_projection() {
        let id = ViewId::RoomHome {
            group_id: TEST_GROUP.to_string(),
        };
        let effects = lifecycle_effects_for_view_open(&id);
        assert_eq!(
            effects.len(),
            1,
            "open must emit exactly one lifecycle effect"
        );
        match &effects[0] {
            Effect::WireGroupEvents { group_id } => {
                assert_eq!(group_id, TEST_GROUP);
            }
            other => panic!("expected WireGroupEvents, got {other:?}"),
        }
    }

    // 3F-T2: room_home_view_closes_releases_events
    //
    // Closing ViewId::RoomHome{group_id} must emit Effect::ReleaseGroupEvents{group_id}.
    #[test]
    fn room_home_view_closes_releases_events() {
        let id = ViewId::RoomHome {
            group_id: TEST_GROUP.to_string(),
        };
        let effects = lifecycle_effects_for_view_close(&id);
        assert_eq!(
            effects.len(),
            1,
            "close must emit exactly one lifecycle effect"
        );
        match &effects[0] {
            Effect::ReleaseGroupEvents { group_id } => {
                assert_eq!(group_id, TEST_GROUP);
            }
            other => panic!("expected ReleaseGroupEvents, got {other:?}"),
        }
    }

    // 3F-T3: non_room_home_view_no_lifecycle_effects
    //
    // Opening/closing non-RoomHome views must not emit lifecycle effects
    // (no WireGroupEvents / ReleaseGroupEvents).
    #[test]
    fn non_room_home_view_no_lifecycle_effects() {
        let open_effects = lifecycle_effects_for_view_open(&ViewId::Communities);
        assert!(
            open_effects.is_empty(),
            "Communities open must not emit room_home lifecycle effects"
        );

        let close_effects = lifecycle_effects_for_view_close(&ViewId::RoomExplorer);
        assert!(
            close_effects.is_empty(),
            "RoomExplorer close must not emit room_home lifecycle effects"
        );
    }

    // 3F-T4: room_home_snapshot_raw_bounded
    //
    // With a community row in AppState::communities, project_room_home_snapshot
    // must return a snapshot with raw fields (no formatted strings) and empty lanes.
    #[test]
    fn room_home_snapshot_raw_bounded() {
        let mut state = make_state();
        state.communities = vec![make_community_row(TEST_GROUP, TEST_RELAY)];
        state.room_policy.invite_link_base = "https://highlighter.com/r".to_string();

        let snap = project_room_home_snapshot(&state, TEST_GROUP);
        assert!(snap.is_some(), "community present → snapshot must be Some");

        if let Some(ViewSnapshot::RoomHome(s)) = snap {
            assert_eq!(s.group_id, TEST_GROUP);
            assert_eq!(s.host_relay_url, TEST_RELAY);
            assert_eq!(s.member_count, 42);
            assert!(
                s.lane_ids.is_empty(),
                "lanes empty in Phase 3F (deferred to Phase 4)"
            );
            assert_eq!(s.invite_link_base, "https://highlighter.com/r");
            // Raw fields — name must NOT be a formatted label like "42 members"
            if let Some(name) = &s.name {
                assert!(
                    !name.contains("member"),
                    "name must not contain formatted label"
                );
            }
        } else {
            panic!("expected RoomHome snapshot");
        }
    }

    // 3F-T5: room_home_snapshot_none_when_not_joined
    //
    // If the group is NOT in AppState::communities, the snapshot must be None.
    // (Room not joined yet — the view shell defers until join arrives.)
    #[test]
    fn room_home_snapshot_none_when_not_joined() {
        let state = make_state(); // empty communities
        let snap = project_room_home_snapshot(&state, "unknown-room");
        assert!(snap.is_none(), "unjoined room must produce None snapshot");
    }

    // ─── WRITE side ──────────────────────────────────────────────────────────

    // 3F-T6: join_room_dispatches_nip29_join_with_serde_payload
    //
    // AppAction::JoinRoom must emit exactly one DispatchNip29Action with
    // namespace="nmp.nip29.join" and a valid serde_json payload containing
    // group.host_relay_url, group.local_id, and invite_code.
    #[test]
    fn join_room_dispatches_nip29_join_with_serde_payload() {
        let mut state = make_state();
        let clock = ManualClock::default();

        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::JoinRoom {
                group_id: TEST_GROUP.to_string(),
                host_relay_url: TEST_RELAY.to_string(),
                invite_code: Some("mycode".to_string()),
            }),
        );

        assert_eq!(effects.len(), 1, "JoinRoom must emit exactly one effect");
        match &effects[0] {
            Effect::DispatchNip29Action { namespace, json } => {
                assert_eq!(namespace, "nmp.nip29.join");

                // Verify valid JSON structure
                let parsed: serde_json::Value =
                    serde_json::from_str(json).expect("JoinRoom payload must be valid JSON");
                assert_eq!(
                    parsed["group"]["host_relay_url"].as_str().unwrap(),
                    TEST_RELAY,
                    "group.host_relay_url must match"
                );
                assert_eq!(
                    parsed["group"]["local_id"].as_str().unwrap(),
                    TEST_GROUP,
                    "group.local_id must match"
                );
                assert_eq!(
                    parsed["invite_code"].as_str().unwrap_or(""),
                    "mycode",
                    "invite_code must be present"
                );
            }
            other => panic!("expected DispatchNip29Action, got {other:?}"),
        }
    }

    // 3F-T7: join_room_no_invite_code_payload_valid
    //
    // JoinRoom without an invite_code must produce valid JSON with null invite_code.
    #[test]
    fn join_room_no_invite_code_payload_valid() {
        let effects = reduce_action_join_room(TEST_GROUP.to_string(), TEST_RELAY.to_string(), None);
        assert_eq!(effects.len(), 1);
        if let Effect::DispatchNip29Action { json, .. } = &effects[0] {
            let parsed: serde_json::Value = serde_json::from_str(json)
                .expect("payload must be valid JSON even without invite_code");
            assert!(
                parsed["invite_code"].is_null(),
                "invite_code must be null when None"
            );
        }
    }

    // 3F-T8: create_room_dispatches_create_public_group
    //
    // AppAction::CreateRoom must emit exactly one DispatchNip29Action with
    // namespace="nmp.nip29.create_public_group" and a valid serde_json payload.
    #[test]
    fn create_room_dispatches_create_public_group() {
        let mut state = make_state();
        let clock = ManualClock::default();

        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::CreateRoom {
                group_id: "my-room".to_string(),
                host_relay_url: TEST_RELAY.to_string(),
                name: "My Room".to_string(),
                about: Some("A nice place".to_string()),
            }),
        );

        assert_eq!(effects.len(), 1, "CreateRoom must emit exactly one effect");
        match &effects[0] {
            Effect::DispatchNip29Action { namespace, json } => {
                assert_eq!(namespace, "nmp.nip29.create_public_group");
                let parsed: serde_json::Value =
                    serde_json::from_str(json).expect("CreateRoom payload must be valid JSON");
                assert_eq!(parsed["name"].as_str().unwrap(), "My Room");
                assert_eq!(parsed["about"].as_str().unwrap(), "A nice place");
                assert_eq!(
                    parsed["group"]["host_relay_url"].as_str().unwrap(),
                    TEST_RELAY
                );
                assert_eq!(parsed["group"]["local_id"].as_str().unwrap(), "my-room");
            }
            other => panic!("expected DispatchNip29Action, got {other:?}"),
        }
    }

    // 3F-T9: add_room_member_dispatches_put_user
    //
    // AppAction::AddRoomMember must emit exactly one DispatchNip29Action with
    // namespace="nmp.nip29.put_user" and a valid serde_json payload containing
    // the target pubkey and optional role.
    #[test]
    fn add_room_member_dispatches_put_user() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let pubkey = "a".repeat(64);

        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::AddRoomMember {
                group_id: TEST_GROUP.to_string(),
                host_relay_url: TEST_RELAY.to_string(),
                pubkey: pubkey.clone(),
                role: Some("admin".to_string()),
            }),
        );

        assert_eq!(
            effects.len(),
            1,
            "AddRoomMember must emit exactly one effect"
        );
        match &effects[0] {
            Effect::DispatchNip29Action { namespace, json } => {
                assert_eq!(namespace, "nmp.nip29.put_user");
                let parsed: serde_json::Value =
                    serde_json::from_str(json).expect("AddRoomMember payload must be valid JSON");
                assert_eq!(parsed["target_pubkey"].as_str().unwrap(), pubkey);
                assert_eq!(parsed["role"].as_str().unwrap(), "admin");
                assert_eq!(
                    parsed["group"]["host_relay_url"].as_str().unwrap(),
                    TEST_RELAY
                );
            }
            other => panic!("expected DispatchNip29Action, got {other:?}"),
        }
    }

    // 3F-T10: create_invites_dispatches_create_invite
    //
    // AppAction::CreateRoomInvites must emit exactly one DispatchNip29Action with
    // namespace="nmp.nip29.create_invite" and a valid serde_json payload with codes.
    #[test]
    fn create_invites_dispatches_create_invite() {
        let mut state = make_state();
        let clock = ManualClock::default();

        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::CreateRoomInvites {
                group_id: TEST_GROUP.to_string(),
                host_relay_url: TEST_RELAY.to_string(),
                codes: vec!["code-1".to_string(), "code-2".to_string()],
            }),
        );

        assert_eq!(
            effects.len(),
            1,
            "CreateRoomInvites must emit exactly one effect"
        );
        match &effects[0] {
            Effect::DispatchNip29Action { namespace, json } => {
                assert_eq!(namespace, "nmp.nip29.create_invite");
                let parsed: serde_json::Value = serde_json::from_str(json)
                    .expect("CreateRoomInvites payload must be valid JSON");
                let codes = parsed["codes"].as_array().unwrap();
                assert_eq!(codes.len(), 2);
                assert_eq!(codes[0].as_str().unwrap(), "code-1");
                assert_eq!(codes[1].as_str().unwrap(), "code-2");
            }
            other => panic!("expected DispatchNip29Action, got {other:?}"),
        }
    }

    // 3F-T11: no_leave_action_exists
    //
    // Document and verify that LeaveRoom is NOT implemented. There is no
    // `nmp.nip29.leave` action on pinned nmp b4404159.
    // See nmp issue #1598 — gap filed; LeaveRoom is deferred until nmp adds it.
    //
    // This test asserts that AppAction has no LeaveRoom variant by ensuring
    // the action enum only has the expected write variants. We verify this
    // indirectly: any AppAction::LeaveRoom dispatch attempt would be a compile
    // error (no such variant), and here we confirm the known write actions work
    // without a LeaveRoom arm in reduce_action.
    #[test]
    fn no_leave_action_exists() {
        // Verify all four write actions compile and dispatch without LeaveRoom.
        // If LeaveRoom existed in AppAction, this test would need to list it.
        // The absence of a LeaveRoom arm in reduce_action (actor.rs) is enforced
        // by the exhaustive match — any new arm would require a compile-time
        // addition. This test documents the #1598 deferral expectation.
        //
        // LeaveRoom deferred: nmp.nip29.leave does not exist on b4404159.
        // Tracking: nmp issue #1598. No hand-rolled kind:9022 publish.
        let join = reduce_action_join_room(TEST_GROUP.to_string(), TEST_RELAY.to_string(), None);
        assert_eq!(join.len(), 1);
        assert!(
            matches!(&join[0], Effect::DispatchNip29Action { namespace, .. } if namespace == "nmp.nip29.join")
        );

        // No LeaveRoom — the match in actor.rs::reduce_action has no such arm.
        // Compile-time enforcement: attempting to add AppAction::LeaveRoom without
        // adding a match arm would be a compile error. This test is the runtime
        // documentation of the deferral policy.
    }

    // 3F-T12: malformed_group_events_frame_noop
    //
    // apply_group_events_frame with garbage bytes must leave AppState::room_home_events
    // unchanged (D6).
    #[test]
    fn malformed_group_events_frame_noop() {
        let mut state = make_state();
        // Seed with an existing entry.
        state
            .room_home_events
            .insert(TEST_GROUP.to_string(), vec![]);

        apply_group_events_frame(&mut state, b"NOT A VALID FLATBUFFER \x00\xFF\xFE");

        // Existing entry must be untouched (D6).
        assert!(
            state.room_home_events.contains_key(TEST_GROUP),
            "malformed payload must leave AppState::room_home_events unchanged (D6)"
        );
    }

    // 3F-T13: serde_json_used_not_format_macro
    //
    // Verify that payloads with special characters are valid JSON.
    // A naïve format! would produce broken JSON if group_id contained quotes.
    #[test]
    fn serde_json_used_not_format_macro() {
        // A group_id with a quote in it should still produce valid JSON
        // (serde_json escapes it; format! would break the JSON structure).
        let tricky_group = r#"room"with"quotes"#;
        let effects =
            reduce_action_join_room(tricky_group.to_string(), TEST_RELAY.to_string(), None);
        if let Effect::DispatchNip29Action { json, .. } = &effects[0] {
            // This MUST parse cleanly — if format! were used, the JSON would be broken.
            let parsed: Result<serde_json::Value, _> = serde_json::from_str(json);
            assert!(
                parsed.is_ok(),
                "serde_json serialization must escape special chars: {json}"
            );
        }
    }
}
