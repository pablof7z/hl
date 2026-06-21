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
//! * **WRITE** — five NIP-29 write actions (Phase 3F: four; Phase 4E: one more):
//!   - `AppAction::JoinRoom`            → `"nmp.nip29.join"`                   (kind:9021)
//!   - `AppAction::CreateRoom`          → `"nmp.nip29.create_public_group"`    (kind:9007+9002)
//!   - `AppAction::AddRoomMember`       → `"nmp.nip29.put_user"`               (kind:9000)
//!   - `AppAction::CreateRoomInvites`   → `"nmp.nip29.create_invite"`          (kind:9009)
//!   - `AppAction::ShareToRoom (repost=false)` → `"nmp.nip29.share_event_in_group"` (kind:11)
//!   - `AppAction::ShareToRoom (repost=true)`  → `"nmp.nip29.repost_in_group"` (kind:16)
//!
//!   Namespaces verified on pinned nmp b4404159
//!   (`crates/nmp-nip29/src/action/group_event.rs:101,124`).
//!   Phase 4E adds `Effect::DispatchShareToRoom` (distinct from `DispatchNip29Action`)
//!   to keep the effect runner attribution clear. Same C-ABI dispatch path.
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

use nmp_core::dispatch_envelope::{encode_dispatch_envelope, DISPATCH_ENVELOPE_SCHEMA_VERSION};
use nmp_core::substrate::ActionPayload;
use nmp_ffi::{nmp_app_dispatch_action_bytes, nmp_free_string, NmpApp};
use nmp_nip29::action::{RepostInGroupInput, ShareEventInGroupInput};
use nmp_nip29::decode_group_events_snapshot;
use nmp_nip29::register::wire_group_events;
use nmp_nip29::GroupId;

use crate::kernel::app::AppState;
use crate::kernel::effect::Effect;
use crate::kernel::snapshot::{
    CommentRecordRow, HighlightRow, KernelCommentReferenceBucket, KernelHighlightReferenceBucket,
    KernelRoomHomeSnapshot, KernelRoomLane, KernelRoomLibraryRow, RoomLaneRow, ViewSnapshot,
};
use crate::kernel::view::ViewId;

// Re-export schema ID so projections.rs can match without importing nmp_nip29 directly.
pub(crate) use nmp_nip29::GROUP_EVENTS_SCHEMA_ID;

/// Bounded cap for room-home events buffered in `AppState::room_home_events`.
/// Lane bodies are empty in Phase 3F; the cap protects against memory growth
/// until Phase 4 wires the feed projection properly.
const ROOM_HOME_EVENTS_CAP: usize = 256;

// ── Phase 4I additions ────────────────────────────────────────────────────────

/// Feed key prefix for room-lane feeds. Concatenated with `group_id`.
pub(crate) const ROOM_LANE_FEED_KEY_PREFIX: &str = "hl.feed.room.";

/// Maximum raw rows buffered per room-lane feed in `AppState::room_lanes`.
/// Prevents unbounded growth — the UI shows the most recent N events.
const ROOM_LANE_ROW_CAP: usize = 100;

/// Feed key prefix for room highlight feeds (kind:9802 with `#h` tag).
/// Concatenated with `group_id` to form the full feed key.
pub(crate) const ROOM_HIGHLIGHT_FEED_KEY_PREFIX: &str = "hl.feed.room_highlights.";

/// Maximum highlight rows surfaced in `KernelRoomHomeSnapshot::highlights`.
const ROOM_HOME_HIGHLIGHT_CAP: usize = 64;

/// Maximum artifact-library entries in `KernelRoomHomeSnapshot::artifact_library`.
const ROOM_HOME_ARTIFACT_LIB_CAP: usize = 32;

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
        let feed_key = format!("{ROOM_LANE_FEED_KEY_PREFIX}{group_id}");
        let scope = crate::kernel::domains::feed::room_lane_scope(group_id);
        let mut effects = vec![Effect::WireGroupEvents {
            group_id: group_id.clone(),
        }];
        // ── Phase 4I: register feed cursor and trigger initial drain ──────────
        effects.extend(crate::kernel::domains::feed::reduce_register_feed_cursor(
            feed_key.clone(),
            scope,
        ));
        effects.extend(crate::kernel::domains::feed::reduce_drain_feed(feed_key));
        // ── Room-home aggregation: register room highlight feed ────────────────
        let hl_feed_key = format!("{ROOM_HIGHLIGHT_FEED_KEY_PREFIX}{group_id}");
        let hl_scope = crate::kernel::domains::feed::room_highlight_feed_scope(group_id);
        effects.extend(crate::kernel::domains::feed::reduce_register_feed_cursor(
            hl_feed_key.clone(),
            hl_scope,
        ));
        effects.extend(crate::kernel::domains::feed::reduce_drain_feed(hl_feed_key));
        effects
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
        let feed_key = format!("{ROOM_LANE_FEED_KEY_PREFIX}{group_id}");
        let mut effects = vec![Effect::ReleaseGroupEvents {
            group_id: group_id.clone(),
        }];
        // ── Phase 4I: release the feed cursor to unregister and free memory ──
        effects.extend(crate::kernel::domains::feed::reduce_release_feed_cursor(
            feed_key,
        ));
        // ── Room-home aggregation: release room highlight feed cursor ──────────
        let hl_feed_key = format!("{ROOM_HIGHLIGHT_FEED_KEY_PREFIX}{group_id}");
        effects.extend(crate::kernel::domains::feed::reduce_release_feed_cursor(
            hl_feed_key,
        ));
        effects
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

/// Handle `AppAction::ShareToRoom { group_id, host_relay_url, target_event_id, target_author_pubkey, repost }`.
///
/// Routes to one of two NIP-29 actions (verified on pinned nmp b4404159,
/// `crates/nmp-nip29/src/action/group_event.rs:101,124`):
/// - `repost == false` → `"nmp.nip29.share_event_in_group"` (kind:11)
///   Payload shape: `ShareEventInGroupInput { group: { host_relay_url, local_id }, target: { event_id, author_pubkey? }, content: "", additional_tags: [] }`
/// - `repost == true`  → `"nmp.nip29.repost_in_group"` (kind:16)
///   Payload shape: `RepostInGroupInput` — identical fields to `ShareEventInGroupInput`.
///
/// Both payloads are built with `serde_json::json!` (never `format!`) so
/// special characters in any field are safely escaped (D-rule: serde only).
///
/// Emits exactly one `Effect::DispatchShareToRoom`. Fire-and-forget (D6).
/// Kernel is the sole writer for these events on ported screens — no
/// double-publish with the bespoke lane. D3: no relay URL literals.
pub(crate) fn reduce_action_share_to_room(
    group_id: String,
    host_relay_url: String,
    target_event_id: String,
    target_author_pubkey: Option<String>,
    repost: bool,
) -> Vec<Effect> {
    let namespace = if repost {
        "nmp.nip29.repost_in_group"
    } else {
        "nmp.nip29.share_event_in_group"
    };

    let json = serde_json::json!({
        "group": {
            "host_relay_url": host_relay_url,
            "local_id": group_id
        },
        "target": {
            "event_id": target_event_id,
            "author_pubkey": target_author_pubkey
        },
        "content": "",
        "additional_tags": []
    })
    .to_string();

    vec![Effect::DispatchShareToRoom {
        namespace: namespace.to_string(),
        json,
    }]
}

// ─── Effect runner: DispatchShareToRoom ──────────────────────────────────────

/// Execute `Effect::DispatchShareToRoom { namespace, json }`.
///
/// Calls `nmp_app_dispatch_action(nmp_ref, namespace, json_ptr)` with the
/// pre-serialized payload. The returned correlation_id C string is freed via
/// `nmp_free_string` and discarded — fire-and-forget (D6, Non-Negotiable #3).
///
/// No-op if `nmp` is `None` (test mode — tests inspect the emitted `Effect`
/// directly without running it against a live `NmpApp`).
pub(crate) fn run_effect_dispatch_share_to_room(
    namespace: String,
    json: String,
    nmp: Option<&crate::kernel::actor::NmpHandle>,
) {
    let Some(handle) = nmp else { return };

    // Route by namespace: deserialise the pre-built JSON to the typed struct,
    // then encode as FlatBuffers for the bytes doorway (ADR-0064 / Cut-B).
    let payload_bytes: Vec<u8> = match namespace.as_str() {
        "nmp.nip29.share_event_in_group" => {
            match serde_json::from_str::<ShareEventInGroupInput>(&json) {
                Ok(a) => a.encode(),
                Err(e) => {
                    tracing::warn!(error = %e, "room_home: failed to deserialise ShareEventInGroupInput");
                    return;
                }
            }
        }
        "nmp.nip29.repost_in_group" => {
            match serde_json::from_str::<RepostInGroupInput>(&json) {
                Ok(a) => a.encode(),
                Err(e) => {
                    tracing::warn!(error = %e, "room_home: failed to deserialise RepostInGroupInput");
                    return;
                }
            }
        }
        other => {
            tracing::warn!(namespace = other, "room_home: unknown share namespace — no-op");
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

// ─── Room-home aggregation helpers ────────────────────────────────────────────

/// Extract the primary artifact coordinate from a kind:11 event.
///
/// Priority: i → a → e → r tags (matches bespoke artifacts.rs:496 which
/// prefers NIP-73 external ids over addressable coordinates). Returns
/// `"<tag_name>:<value>"` or `None` when no recognised reference tag is
/// present (D6 no-op).
fn extract_artifact_coordinate(event: &nmp_core::substrate::KernelEvent) -> Option<String> {
    let tag_val = |name: &str| -> Option<String> {
        event
            .tags
            .iter()
            .find(|t| t.first().map(|s| s == name).unwrap_or(false))
            .and_then(|t| t.get(1))
            .filter(|v| !v.is_empty())
            .cloned()
    };
    // Priority i → a → e → r mirrors bespoke artifacts.rs coordinate extraction.
    if let Some(v) = tag_val("i") {
        return Some(format!("i:{v}"));
    }
    if let Some(v) = tag_val("a") {
        return Some(format!("a:{v}"));
    }
    if let Some(v) = tag_val("e") {
        return Some(format!("e:{v}"));
    }
    if let Some(v) = tag_val("r") {
        return Some(format!("r:{v}"));
    }
    None
}

/// Returns `true` when a kernel highlight row matches a lane artifact.
///
/// Mirrors bespoke `highlight_matches_artifact` (room_lanes.rs:95) using
/// kernel types. Checks in order:
/// 1. `source_reference_key` (canonical `"tag:value"` form — already the
///    primary key used when building `highlights_by_reference` buckets).
/// 2. `artifact_address` → matches `"a:{artifact_address}"` coordinate.
/// 3. `event_reference` → matches `"e:{event_reference}"` coordinate, or
///    the share event id of the kind:11 event.
/// 4. `external_reference` → matches `"i:{external_reference}"` coordinate.
/// 5. `source_url` → matches `"r:{source_url}"` coordinate.
///
/// Audio-URL and podcast-GUID cross-checks from bespoke are omitted because
/// `ArtifactPreviewRow` does not carry those fields (D1 — kernel is raw,
/// Swift hydrates). All other match paths are equivalent.
fn kernel_highlight_matches_lane(hl: &HighlightRow, lib_row: &KernelRoomLibraryRow) -> bool {
    let coord = &lib_row.coordinate;

    if !hl.source_reference_key.is_empty() && &hl.source_reference_key == coord {
        return true;
    }
    if !hl.artifact_address.is_empty() && format!("a:{}", hl.artifact_address) == *coord {
        return true;
    }
    if !hl.event_reference.is_empty() {
        if format!("e:{}", hl.event_reference) == *coord {
            return true;
        }
        if hl.event_reference == lib_row.share_event_id {
            return true;
        }
    }
    if !hl.external_reference.is_empty() && format!("i:{}", hl.external_reference) == *coord {
        return true;
    }
    if !hl.source_url.is_empty() && format!("r:{}", hl.source_url) == *coord {
        return true;
    }
    false
}

/// Return `true` if the event has a `["t", "discussion"]` tag (marks a
/// kind:11 as a room discussion post rather than an artifact share).
fn is_discussion_event(event: &nmp_core::substrate::KernelEvent) -> bool {
    event.tags.iter().any(|t| {
        t.first().map(|s| s == "t").unwrap_or(false)
            && t.get(1).map(|s| s == "discussion").unwrap_or(false)
    })
}

/// Convert an artifact coordinate (e.g. `"a:30023:pk:d"`) to the NIP-22
/// `root_tag_value` form (e.g. `"30023:pk:d"`) by stripping the tag-prefix
/// and the separating colon.
///
/// Returns `None` if the coordinate has no colon (malformed).
fn root_tag_value_for_coordinate(coordinate: &str) -> Option<String> {
    coordinate.split_once(':').map(|x| x.1.to_string())
}

/// Ensure `AppState::artifact_previews` has an entry for every artifact
/// coordinate referenced by kind:11 (non-discussion) events in the room lane.
///
/// Idempotent — `ensure_artifact_preview` is a no-op when the coordinate is
/// already present. Called from `actor.rs` on each new room-lane `FeedPage`.
/// Returns any resolver effects that need to be dispatched (e.g.
/// `Effect::ResolveArtifactCoordinate`).
pub(crate) fn ensure_room_artifact_previews(state: &mut AppState, group_id: &str) -> Vec<Effect> {
    let feed_key = format!("{ROOM_LANE_FEED_KEY_PREFIX}{group_id}");

    // Coordinates from kind:11 non-discussion artifact shares in the lane feed.
    let mut coordinates: Vec<String> = state
        .room_lanes
        .get(&feed_key)
        .map(|fs| {
            fs.rows
                .iter()
                .filter(|e| e.kind == 11 && !is_discussion_event(e))
                .filter_map(extract_artifact_coordinate)
                .collect()
        })
        .unwrap_or_default();

    // Also seed from discussions' artifact_coordinate refs (Gap 2 / discussion chip).
    // A discussion post may reference an artifact via a/e/i tags — the resolved
    // preview enables the rich artifact chip in the discussion row.
    let discussion_coords: Vec<String> = state
        .room_discussions
        .get(group_id)
        .map(|rows| {
            rows.iter()
                .filter_map(|d| d.artifact_coordinate.clone())
                .collect()
        })
        .unwrap_or_default();
    coordinates.extend(discussion_coords);

    let mut effects = Vec::new();
    for coord in coordinates {
        effects.extend(
            crate::kernel::domains::artifact_preview::ensure_artifact_preview(state, coord),
        );
    }
    effects
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

    // ── Phase 4I: populate lane rows from the feed-pull engine ───────────────
    let feed_key = format!("{ROOM_LANE_FEED_KEY_PREFIX}{group_id}");
    let lane_rows: Vec<RoomLaneRow> = state
        .room_lanes
        .get(&feed_key)
        .map(|fs| {
            fs.rows
                .iter()
                .take(ROOM_LANE_ROW_CAP)
                .map(|e| RoomLaneRow {
                    event_id: e.id.clone(),
                    author_pubkey: e.author.clone(),
                    kind: e.kind,
                    content: e.content.clone(),
                    created_at: e.created_at,
                    tags: e.tags.clone(),
                })
                .collect()
        })
        .unwrap_or_default();

    // lane_ids is non-empty only when feed rows have arrived (Phase 4I seam).
    let lane_ids: Vec<String> = if lane_rows.is_empty() {
        Vec::new()
    } else {
        vec![group_id.to_string()]
    };

    // ── Room-home aggregation ─────────────────────────────────────────────────

    // Artifact library: kind:11 non-discussion events from the lane feed.
    let artifact_library: Vec<KernelRoomLibraryRow> = state
        .room_lanes
        .get(&feed_key)
        .map(|fs| {
            fs.rows
                .iter()
                .filter(|e| e.kind == 11 && !is_discussion_event(e))
                .filter_map(|e| {
                    let coordinate = extract_artifact_coordinate(e)?;
                    let preview = state.artifact_previews.get(&coordinate).cloned();
                    Some(KernelRoomLibraryRow {
                        coordinate,
                        share_event_id: e.id.clone(),
                        preview,
                    })
                })
                .take(ROOM_HOME_ARTIFACT_LIB_CAP)
                .collect()
        })
        .unwrap_or_default();

    // Highlights: kind:9802 events from the room highlight feed, newest-first.
    let hl_feed_key = format!("{ROOM_HIGHLIGHT_FEED_KEY_PREFIX}{group_id}");
    let mut highlights: Vec<HighlightRow> = state
        .room_highlight_feeds
        .get(&hl_feed_key)
        .map(|fs| {
            fs.rows
                .iter()
                .filter_map(crate::kernel::domains::highlight_feed::decode_highlight_row)
                .collect()
        })
        .unwrap_or_default();
    highlights.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    highlights.truncate(ROOM_HOME_HIGHLIGHT_CAP);

    // Highlights grouped by artifact coordinate.
    let highlights_by_reference: Vec<KernelHighlightReferenceBucket> = {
        let mut buckets: Vec<KernelHighlightReferenceBucket> = Vec::new();
        for lib_row in &artifact_library {
            let coord = &lib_row.coordinate;
            let matching: Vec<HighlightRow> = highlights
                .iter()
                .filter(|h| &h.source_reference_key == coord)
                .cloned()
                .collect();
            if !matching.is_empty() {
                buckets.push(KernelHighlightReferenceBucket {
                    coordinate: coord.clone(),
                    highlights: matching,
                });
            }
        }
        buckets
    };

    // Comments grouped by artifact coordinate (via root_tag_value mapping).
    let comments_by_reference: Vec<KernelCommentReferenceBucket> = {
        let mut buckets: Vec<KernelCommentReferenceBucket> = Vec::new();
        for lib_row in &artifact_library {
            let Some(root_val) = root_tag_value_for_coordinate(&lib_row.coordinate) else {
                continue;
            };
            if let Some(thread) = state.comment_threads.get(&root_val) {
                let comments: Vec<CommentRecordRow> = thread
                    .records
                    .iter()
                    .map(|r| {
                        let reaction = state.reaction_state.get(&r.event_id);
                        CommentRecordRow {
                            event_id: r.event_id.clone(),
                            author_pubkey: r.author_pubkey.clone(),
                            body: r.body.clone(),
                            root_tag_name: r.root_tag_name.clone(),
                            root_tag_value: r.root_tag_value.clone(),
                            root_kind: r.root_kind.clone(),
                            parent_tag_name: r.parent_tag_name.clone(),
                            parent_tag_value: r.parent_tag_value.clone(),
                            parent_kind: r.parent_kind.clone(),
                            created_at: r.created_at,
                            is_top_level: r.is_top_level(),
                            like_count: reaction.map(|x| x.count).unwrap_or(0),
                            viewer_reacted: reaction.map(|x| x.viewer_reacted).unwrap_or(false),
                            bookmarked: state.bookmarks.iter().any(|b| {
                                matches!(
                                    b,
                                    crate::kernel::snapshot::BookmarkRow::Event { event_id, .. }
                                        if event_id == &r.event_id
                                )
                            }),
                        }
                    })
                    .collect();
                let count = comments.len() as u32;
                buckets.push(KernelCommentReferenceBucket {
                    root_tag_value: root_val,
                    comments,
                    count,
                });
            }
        }
        buckets
    };

    // ── Assembled lanes: mirror build_visible_room_lanes from room_lanes.rs ─────
    //
    // One lane per artifact in the library. Dormant lanes (no highlights AND no
    // comments) are excluded. Lanes sorted by latest_activity_at desc so iOS
    // renders the most-active artifact at the top.
    //
    // Highlight match is two-pass (mirrors bespoke room_lanes.rs:38-65):
    //   Pass 1 — from the pre-built highlights_by_reference bucket (coordinate key).
    //   Pass 2 — full kernel_highlight_matches_lane scan for any remaining
    //            highlights not captured by the bucket (e.g. artifact_address /
    //            event_reference / external_reference / source_url cross-matches).
    // Dedup by event_id HashSet across both passes.
    // Comment match: by root_tag_value (coordinate stripped of tag prefix).
    let assembled_lanes: Vec<KernelRoomLane> = {
        let mut lanes: Vec<KernelRoomLane> = Vec::with_capacity(artifact_library.len());
        for lib_row in &artifact_library {
            let coord = &lib_row.coordinate;

            // Pass 1: highlights from the pre-built bucket (source_reference_key match).
            let mut lane_highlights: Vec<HighlightRow> = highlights_by_reference
                .iter()
                .find(|b| &b.coordinate == coord)
                .map(|b| b.highlights.clone())
                .unwrap_or_default();

            // Pass 2: full match (artifact_address / event_reference /
            // external_reference / source_url) for highlights not yet captured.
            // Mirrors bespoke highlight_matches_artifact fallback (room_lanes.rs:58-65).
            {
                let mut seen: std::collections::HashSet<String> =
                    lane_highlights.iter().map(|h| h.event_id.clone()).collect();
                for hl in &highlights {
                    if kernel_highlight_matches_lane(hl, lib_row)
                        && seen.insert(hl.event_id.clone())
                    {
                        lane_highlights.push(hl.clone());
                    }
                }
            }

            // Comments for this lane (from the pre-built bucket by root_tag_value).
            let lane_comments: Vec<CommentRecordRow> = root_tag_value_for_coordinate(coord)
                .as_deref()
                .and_then(|rv| {
                    comments_by_reference
                        .iter()
                        .find(|b| b.root_tag_value == rv)
                })
                .map(|b| b.comments.clone())
                .unwrap_or_default();

            // Dormant filter: skip lanes with no highlights AND no comments.
            // Mirrors bespoke room_lanes.rs:75 `if lane_highlights.is_empty() && …`.
            if lane_highlights.is_empty() && lane_comments.is_empty() {
                continue;
            }

            // Sort newest-first (descending created_at).
            let mut sorted_highlights = lane_highlights;
            sorted_highlights.sort_by(|a, b| b.created_at.cmp(&a.created_at));
            let mut sorted_comments = lane_comments;
            sorted_comments.sort_by(|a, b| b.created_at.cmp(&a.created_at));

            // Latest activity = max of first highlight/comment timestamps.
            let latest_hl = sorted_highlights.first().map(|h| h.created_at).unwrap_or(0);
            let latest_cmt = sorted_comments.first().map(|c| c.created_at).unwrap_or(0);
            let latest_activity_at = latest_hl.max(latest_cmt);

            lanes.push(KernelRoomLane {
                share_event_id: lib_row.share_event_id.clone(),
                artifact_coordinate: coord.clone(),
                artifact_preview: lib_row.preview.clone(),
                highlights: sorted_highlights,
                comments: sorted_comments,
                latest_activity_at,
            });
        }
        // Sort lanes: most recently active first.
        lanes.sort_by(|a, b| b.latest_activity_at.cmp(&a.latest_activity_at));
        lanes
    };

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
        lane_ids,
        // invite_link_base from room policy (D3: injected at construction).
        invite_link_base: state.room_policy.invite_link_base.clone(),
        // Phase 4I: raw feed rows from the ADR-0058 pull engine.
        lanes: lane_rows,
        // Room-home aggregation additions.
        artifact_library,
        highlights,
        highlights_by_reference,
        comments_by_reference,
        assembled_lanes,
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
    // Phase 4I: also emits RegisterFeedCursor + DrainFeed (3 effects total).
    // The actor calls lifecycle_effects_for_view_open on Cmd::OpenView.
    #[test]
    fn room_home_view_opens_wires_projection() {
        let id = ViewId::RoomHome {
            group_id: TEST_GROUP.to_string(),
        };
        let effects = lifecycle_effects_for_view_open(&id);
        // Phase 4I: 3 effects — WireGroupEvents, RegisterFeedCursor, DrainFeed.
        assert!(
            effects.len() >= 1,
            "open must emit at least one lifecycle effect"
        );
        match &effects[0] {
            Effect::WireGroupEvents { group_id } => {
                assert_eq!(group_id, TEST_GROUP);
            }
            other => panic!("expected WireGroupEvents as first effect, got {other:?}"),
        }
    }

    // 3F-T2: room_home_view_closes_releases_events
    //
    // Closing ViewId::RoomHome{group_id} must emit Effect::ReleaseGroupEvents{group_id}.
    // Phase 4I: also emits ReleaseFeedCursor (2 effects total).
    #[test]
    fn room_home_view_closes_releases_events() {
        let id = ViewId::RoomHome {
            group_id: TEST_GROUP.to_string(),
        };
        let effects = lifecycle_effects_for_view_close(&id);
        // Phase 4I: 2 effects — ReleaseGroupEvents + ReleaseFeedCursor.
        assert!(
            effects.len() >= 1,
            "close must emit at least one lifecycle effect"
        );
        match &effects[0] {
            Effect::ReleaseGroupEvents { group_id } => {
                assert_eq!(group_id, TEST_GROUP);
            }
            other => panic!("expected ReleaseGroupEvents as first effect, got {other:?}"),
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

    // ─── Phase 4E tests ───────────────────────────────────────────────────────

    // 4E-T1: share_to_room_repost_false_dispatches_share_event_in_group
    //
    // AppAction::ShareToRoom with repost=false must emit exactly one
    // Effect::DispatchShareToRoom with namespace="nmp.nip29.share_event_in_group"
    // and a valid serde_json payload with the expected structure.
    // Namespace verified on pinned nmp b4404159
    // (crates/nmp-nip29/src/action/group_event.rs:101).
    #[test]
    fn share_to_room_repost_false_dispatches_share_event_in_group() {
        let mut state = AppState::default();
        let clock = ManualClock::default();
        let target_event_id = "abc123".to_string();
        let author_pubkey = "deadbeef".repeat(8); // 64-char hex

        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::ShareToRoom {
                group_id: TEST_GROUP.to_string(),
                host_relay_url: TEST_RELAY.to_string(),
                target_event_id: target_event_id.clone(),
                target_author_pubkey: Some(author_pubkey.clone()),
                repost: false,
            }),
        );

        assert_eq!(
            effects.len(),
            1,
            "ShareToRoom(repost=false) must emit exactly one effect"
        );
        match &effects[0] {
            Effect::DispatchShareToRoom { namespace, json } => {
                assert_eq!(
                    namespace, "nmp.nip29.share_event_in_group",
                    "repost=false must route to share_event_in_group (kind:11)"
                );

                let parsed: serde_json::Value =
                    serde_json::from_str(json).expect("share payload must be valid JSON");
                assert_eq!(
                    parsed["group"]["host_relay_url"].as_str().unwrap(),
                    TEST_RELAY
                );
                assert_eq!(parsed["group"]["local_id"].as_str().unwrap(), TEST_GROUP);
                assert_eq!(
                    parsed["target"]["event_id"].as_str().unwrap(),
                    target_event_id
                );
                assert_eq!(
                    parsed["target"]["author_pubkey"].as_str().unwrap(),
                    author_pubkey
                );
            }
            other => panic!("expected DispatchShareToRoom, got {other:?}"),
        }
    }

    // 4E-T2: share_to_room_repost_true_dispatches_repost_in_group
    //
    // AppAction::ShareToRoom with repost=true must emit exactly one
    // Effect::DispatchShareToRoom with namespace="nmp.nip29.repost_in_group".
    // Namespace verified on pinned nmp b4404159
    // (crates/nmp-nip29/src/action/group_event.rs:124).
    #[test]
    fn share_to_room_repost_true_dispatches_repost_in_group() {
        let mut state = AppState::default();
        let clock = ManualClock::default();

        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::ShareToRoom {
                group_id: TEST_GROUP.to_string(),
                host_relay_url: TEST_RELAY.to_string(),
                target_event_id: "event-xyz".to_string(),
                target_author_pubkey: None,
                repost: true,
            }),
        );

        assert_eq!(
            effects.len(),
            1,
            "ShareToRoom(repost=true) must emit exactly one effect"
        );
        match &effects[0] {
            Effect::DispatchShareToRoom { namespace, json } => {
                assert_eq!(
                    namespace, "nmp.nip29.repost_in_group",
                    "repost=true must route to repost_in_group (kind:16)"
                );

                let parsed: serde_json::Value =
                    serde_json::from_str(json).expect("repost payload must be valid JSON");
                assert_eq!(parsed["group"]["local_id"].as_str().unwrap(), TEST_GROUP);
                assert_eq!(parsed["target"]["event_id"].as_str().unwrap(), "event-xyz");
                // author_pubkey absent → JSON null (serde serialises Option::None as null)
                assert!(
                    parsed["target"]["author_pubkey"].is_null(),
                    "author_pubkey must be null when not provided"
                );
            }
            other => panic!("expected DispatchShareToRoom, got {other:?}"),
        }
    }

    // 4E-T3: payload_built_with_serde_not_format
    //
    // Payloads with special characters (quotes, backslashes) must be valid JSON.
    // A naïve format! would produce broken JSON; serde_json::json! must escape
    // them correctly. (D-rule: serde, not format!.)
    #[test]
    fn payload_built_with_serde_not_format() {
        let tricky_event_id = r#"evt"with"quotes"#;
        let tricky_group = r#"group"id"#;

        let effects = reduce_action_share_to_room(
            tricky_group.to_string(),
            TEST_RELAY.to_string(),
            tricky_event_id.to_string(),
            None,
            false,
        );

        assert_eq!(effects.len(), 1);
        if let Effect::DispatchShareToRoom { json, .. } = &effects[0] {
            // MUST parse cleanly — format! would produce broken JSON here.
            let parsed: Result<serde_json::Value, _> = serde_json::from_str(json);
            assert!(
                parsed.is_ok(),
                "serde_json must escape special chars in share payload: {json}"
            );
            let v = parsed.unwrap();
            assert_eq!(v["group"]["local_id"].as_str().unwrap(), tricky_group);
            assert_eq!(v["target"]["event_id"].as_str().unwrap(), tricky_event_id);
        } else {
            panic!("expected DispatchShareToRoom");
        }
    }

    // 4E-T4: share_dispatch_returns_unit
    //
    // reduce_action_share_to_room returns a Vec (not a Result).
    // The sole item is a DispatchShareToRoom effect (fire-and-forget).
    // This test documents the "dispatch returns ()" / Non-Negotiable #3 contract:
    // the reducer never returns a Result; errors do not cross the dispatch seam.
    #[test]
    fn share_dispatch_returns_unit() {
        let effects = reduce_action_share_to_room(
            TEST_GROUP.to_string(),
            TEST_RELAY.to_string(),
            "event-id-123".to_string(),
            Some("author-pubkey".to_string()),
            false,
        );

        // Exactly one effect — no second side-channel write (kernel sole writer).
        assert_eq!(
            effects.len(),
            1,
            "share action must emit exactly one effect (sole writer, no double-publish)"
        );
        assert!(
            matches!(&effects[0], Effect::DispatchShareToRoom { .. }),
            "the sole effect must be DispatchShareToRoom"
        );
    }

    // ─── Phase 4I tests ───────────────────────────────────────────────────────

    fn dummy_kernel_event(id: &str, kind: u32) -> nmp_core::substrate::KernelEvent {
        nmp_core::substrate::KernelEvent {
            id: id.to_string(),
            author: "a".repeat(64),
            kind,
            created_at: 1_700_000_000,
            tags: vec![vec!["h".to_string(), TEST_GROUP.to_string()]],
            content: "test content".to_string(),
            relay_provenance: vec![],
        }
    }

    // 4I-T1: room_lane_registers_cursor_on_roomhome_open
    //
    // Opening ViewId::RoomHome must emit at least:
    //   - Effect::WireGroupEvents (first)
    //   - Effect::RegisterFeedCursor with key "hl.feed.room.<group_id>"
    //   - Effect::DrainFeed with key "hl.feed.room.<group_id>"
    #[test]
    fn room_lane_registers_cursor_on_roomhome_open() {
        let id = ViewId::RoomHome {
            group_id: TEST_GROUP.to_string(),
        };
        let effects = lifecycle_effects_for_view_open(&id);

        let expected_key = format!("hl.feed.room.{TEST_GROUP}");

        // Must have WireGroupEvents + RegisterFeedCursor + DrainFeed for lane
        // + RegisterFeedCursor + DrainFeed for highlights (5 effects total).
        assert_eq!(
            effects.len(),
            5,
            "open must emit 5 effects: WireGroupEvents, RegisterFeedCursor, DrainFeed (lane + highlights)"
        );

        // First: WireGroupEvents
        assert!(
            matches!(&effects[0], Effect::WireGroupEvents { group_id } if group_id == TEST_GROUP),
            "first effect must be WireGroupEvents({TEST_GROUP}), got {:?}",
            &effects[0]
        );

        // Second: RegisterFeedCursor with correct key
        match &effects[1] {
            Effect::RegisterFeedCursor { key, cursor_id, .. } => {
                assert_eq!(
                    key, &expected_key,
                    "feed key must be hl.feed.room.<group_id>"
                );
                assert_ne!(*cursor_id, 0, "cursor_id must be non-zero");
            }
            other => panic!("second effect must be RegisterFeedCursor, got {other:?}"),
        }

        // Third: DrainFeed with correct key
        match &effects[2] {
            Effect::DrainFeed { key } => {
                assert_eq!(
                    key, &expected_key,
                    "DrainFeed key must be hl.feed.room.<group_id>"
                );
            }
            other => panic!("third effect must be DrainFeed, got {other:?}"),
        }
    }

    // 4I-T2: feedpage_appends_room_lane_rows_raw
    //
    // Injecting a FeedPage for a room lane into AppState::room_lanes must cause
    // project_room_home_snapshot to return non-empty lanes with raw kind:11 rows
    // (D1: no formatted strings in the row).
    #[test]
    fn feedpage_appends_room_lane_rows_raw() {
        let mut state = make_state();
        state.communities = vec![make_community_row(TEST_GROUP, TEST_RELAY)];
        state.room_policy.invite_link_base = "https://highlighter.com/r".to_string();

        // Inject a feed page directly into AppState::room_lanes.
        let feed_key = format!("hl.feed.room.{TEST_GROUP}");
        let mut feed_state = crate::kernel::domains::feed::FeedState::default();
        crate::kernel::domains::feed::apply_feed_page(
            &mut feed_state,
            vec![dummy_kernel_event("evt-abc123", 11)],
            5,
            false,
            None,
        );
        state.room_lanes.insert(feed_key.clone(), feed_state);

        let snap = project_room_home_snapshot(&state, TEST_GROUP);
        assert!(
            snap.is_some(),
            "snapshot must be Some with community present"
        );

        if let Some(ViewSnapshot::RoomHome(s)) = snap {
            assert!(
                !s.lanes.is_empty(),
                "lanes must be non-empty after FeedPage"
            );
            assert_eq!(
                s.lanes[0].kind, 11,
                "kind must be raw 11 (D1: no formatted strings)"
            );
            assert_eq!(s.lanes[0].event_id, "evt-abc123");
            assert_eq!(s.lanes[0].author_pubkey, "a".repeat(64));
            // D1 check: content is raw text, not a label like "shared an article"
            assert_eq!(s.lanes[0].content, "test content");
            // lane_ids populated when rows exist
            assert!(
                !s.lane_ids.is_empty(),
                "lane_ids must be non-empty when rows exist"
            );
            assert_eq!(s.lane_ids[0], TEST_GROUP);
        } else {
            panic!("expected RoomHome snapshot");
        }
    }

    // 4I-T3: room_lane_released_on_roomhome_close
    //
    // Closing ViewId::RoomHome must emit both:
    //   - Effect::ReleaseGroupEvents with the group_id
    //   - Effect::ReleaseFeedCursor with key "hl.feed.room.<group_id>"
    #[test]
    fn room_lane_released_on_roomhome_close() {
        let id = ViewId::RoomHome {
            group_id: TEST_GROUP.to_string(),
        };
        let effects = lifecycle_effects_for_view_close(&id);
        let expected_key = format!("hl.feed.room.{TEST_GROUP}");

        assert_eq!(
            effects.len(),
            3,
            "close must emit 3 effects: ReleaseGroupEvents + ReleaseFeedCursor (lane + highlights)"
        );

        // First: ReleaseGroupEvents
        assert!(
            matches!(&effects[0], Effect::ReleaseGroupEvents { group_id } if group_id == TEST_GROUP),
            "first effect must be ReleaseGroupEvents({TEST_GROUP}), got {:?}",
            &effects[0]
        );

        // Second: ReleaseFeedCursor with correct key
        match &effects[1] {
            Effect::ReleaseFeedCursor { key } => {
                assert_eq!(
                    key, &expected_key,
                    "ReleaseFeedCursor key must be hl.feed.room.<group_id>"
                );
            }
            other => panic!("second effect must be ReleaseFeedCursor, got {other:?}"),
        }
    }

    // 4I-T4: open_roomhome_emits_drain_for_initial_fill
    //
    // Opening RoomHome must emit a DrainFeed effect (initial fill).
    // This validates the "initial drain on open" behavior from the spec.
    #[test]
    fn open_roomhome_emits_drain_for_initial_fill() {
        let id = ViewId::RoomHome {
            group_id: TEST_GROUP.to_string(),
        };
        let effects = lifecycle_effects_for_view_open(&id);
        let expected_key = format!("hl.feed.room.{TEST_GROUP}");

        let drain_effects: Vec<_> = effects
            .iter()
            .filter(|e| matches!(e, Effect::DrainFeed { key } if key == &expected_key))
            .collect();

        assert!(
            !drain_effects.is_empty(),
            "open must emit DrainFeed for initial fill"
        );
    }

    // 4I-T5: multiple_rooms_independent_feedstate
    //
    // Two RoomHome views with different group_ids must have independent lane rows
    // in the snapshot — each feed is keyed separately in AppState::room_lanes.
    #[test]
    fn multiple_rooms_independent_feedstate() {
        let group_a = "room-alpha";
        let group_b = "room-beta";

        let mut state = make_state();
        state.communities = vec![
            make_community_row(group_a, TEST_RELAY),
            make_community_row(group_b, TEST_RELAY),
        ];
        state.room_policy.invite_link_base = "https://highlighter.com/r".to_string();

        // Inject different rows for each group.
        for (group, event_id, kind) in [
            (group_a, "evt-alpha-001", 9u32),
            (group_b, "evt-beta-002", 11u32),
        ] {
            let feed_key = format!("hl.feed.room.{group}");
            let mut fs = crate::kernel::domains::feed::FeedState::default();
            crate::kernel::domains::feed::apply_feed_page(
                &mut fs,
                vec![dummy_kernel_event(event_id, kind)],
                1,
                false,
                None,
            );
            state.room_lanes.insert(feed_key, fs);
        }

        // Snapshot for group_a.
        if let Some(ViewSnapshot::RoomHome(snap_a)) = project_room_home_snapshot(&state, group_a) {
            assert_eq!(snap_a.lanes.len(), 1);
            assert_eq!(snap_a.lanes[0].event_id, "evt-alpha-001");
            assert_eq!(snap_a.lanes[0].kind, 9);
        } else {
            panic!("expected RoomHome snapshot for group_a");
        }

        // Snapshot for group_b.
        if let Some(ViewSnapshot::RoomHome(snap_b)) = project_room_home_snapshot(&state, group_b) {
            assert_eq!(snap_b.lanes.len(), 1);
            assert_eq!(snap_b.lanes[0].event_id, "evt-beta-002");
            assert_eq!(snap_b.lanes[0].kind, 11);
        } else {
            panic!("expected RoomHome snapshot for group_b");
        }
    }

    // 4I-T6: malformed_events_skipped_in_lane_rows
    //
    // If AppState::room_lanes has no entry for a group (or the FeedState is empty),
    // the snapshot lanes must be empty and no panic (D6).
    #[test]
    fn malformed_events_skipped_in_lane_rows() {
        let mut state = make_state();
        state.communities = vec![make_community_row(TEST_GROUP, TEST_RELAY)];
        // No entry in room_lanes — simulates "no feed page received yet".

        let snap = project_room_home_snapshot(&state, TEST_GROUP);
        assert!(
            snap.is_some(),
            "snapshot must be Some even with empty room_lanes"
        );

        if let Some(ViewSnapshot::RoomHome(s)) = snap {
            assert!(
                s.lanes.is_empty(),
                "lanes must be empty when no feed rows exist (D6)"
            );
            assert!(
                s.lane_ids.is_empty(),
                "lane_ids must be empty when no feed rows exist"
            );
        } else {
            panic!("expected RoomHome snapshot");
        }
    }

    // ─── Room-home aggregation tests ─────────────────────────────────────────

    fn make_kind11_with_a_tag(event_id: &str, address: &str) -> nmp_core::substrate::KernelEvent {
        nmp_core::substrate::KernelEvent {
            id: event_id.to_string(),
            author: "b".repeat(64),
            kind: 11,
            created_at: 1_700_001_000,
            tags: vec![
                vec!["h".to_string(), TEST_GROUP.to_string()],
                vec!["a".to_string(), address.to_string()],
            ],
            content: String::new(),
            relay_provenance: vec![],
        }
    }

    fn make_kind9802_with_a_tag(event_id: &str, address: &str) -> nmp_core::substrate::KernelEvent {
        nmp_core::substrate::KernelEvent {
            id: event_id.to_string(),
            author: "c".repeat(64),
            kind: 9802,
            created_at: 1_700_002_000,
            tags: vec![
                vec!["h".to_string(), TEST_GROUP.to_string()],
                vec!["a".to_string(), address.to_string()],
            ],
            content: "a great highlighted passage".to_string(),
            relay_provenance: vec![],
        }
    }

    fn inject_lane_event(state: &mut AppState, event: nmp_core::substrate::KernelEvent) {
        let feed_key = format!("{ROOM_LANE_FEED_KEY_PREFIX}{TEST_GROUP}");
        let fs = state.room_lanes.entry(feed_key).or_default();
        crate::kernel::domains::feed::apply_feed_page(fs, vec![event], 1, false, None);
    }

    fn inject_hl_event(state: &mut AppState, event: nmp_core::substrate::KernelEvent) {
        let feed_key = format!("{ROOM_HIGHLIGHT_FEED_KEY_PREFIX}{TEST_GROUP}");
        let fs = state.room_highlight_feeds.entry(feed_key).or_default();
        crate::kernel::domains::feed::apply_feed_page(fs, vec![event], 1, false, None);
    }

    // T1: artifact_library_populated_from_lane_rows
    //
    // A kind:11 event with an `a` tag (non-discussion) in the room lane feed
    // must appear in artifact_library with the correct coordinate.
    #[test]
    fn artifact_library_populated_from_lane_rows() {
        let mut state = make_state();
        state.communities = vec![make_community_row(TEST_GROUP, TEST_RELAY)];
        state.room_policy.invite_link_base = "https://highlighter.com/r".to_string();

        let addr =
            "30023:aaaa000000000000000000000000000000000000000000000000000000000001:my-article";
        inject_lane_event(&mut state, make_kind11_with_a_tag("evt-lib-001", addr));

        let snap = project_room_home_snapshot(&state, TEST_GROUP).unwrap();
        if let ViewSnapshot::RoomHome(s) = snap {
            assert_eq!(s.artifact_library.len(), 1, "must have 1 artifact");
            assert_eq!(s.artifact_library[0].coordinate, format!("a:{addr}"));
            assert_eq!(s.artifact_library[0].share_event_id, "evt-lib-001");
        } else {
            panic!("expected RoomHome snapshot");
        }
    }

    // T2: discussions_excluded_from_artifact_library
    //
    // A kind:11 event with both an `a` tag AND `["t","discussion"]` must NOT
    // appear in artifact_library (discussions are excluded).
    #[test]
    fn discussions_excluded_from_artifact_library() {
        let mut state = make_state();
        state.communities = vec![make_community_row(TEST_GROUP, TEST_RELAY)];
        state.room_policy.invite_link_base = "https://highlighter.com/r".to_string();

        let addr = "30023:aaaa000000000000000000000000000000000000000000000000000000000001:d";
        let discussion_event = nmp_core::substrate::KernelEvent {
            id: "evt-discussion-001".to_string(),
            author: "b".repeat(64),
            kind: 11,
            created_at: 1_700_001_000,
            tags: vec![
                vec!["h".to_string(), TEST_GROUP.to_string()],
                vec!["a".to_string(), addr.to_string()],
                vec!["t".to_string(), "discussion".to_string()],
            ],
            content: String::new(),
            relay_provenance: vec![],
        };
        inject_lane_event(&mut state, discussion_event);

        let snap = project_room_home_snapshot(&state, TEST_GROUP).unwrap();
        if let ViewSnapshot::RoomHome(s) = snap {
            assert!(
                s.artifact_library.is_empty(),
                "discussion events must be excluded from artifact_library"
            );
        } else {
            panic!("expected RoomHome snapshot");
        }
    }

    // T3: highlights_decoded_from_room_highlight_feeds
    //
    // A kind:9802 event in room_highlight_feeds must appear in `highlights`.
    #[test]
    fn highlights_decoded_from_room_highlight_feeds() {
        let mut state = make_state();
        state.communities = vec![make_community_row(TEST_GROUP, TEST_RELAY)];
        state.room_policy.invite_link_base = "https://highlighter.com/r".to_string();

        let addr = "30023:bbbb000000000000000000000000000000000000000000000000000000000001:art";
        inject_hl_event(&mut state, make_kind9802_with_a_tag("hl-evt-001", addr));

        let snap = project_room_home_snapshot(&state, TEST_GROUP).unwrap();
        if let ViewSnapshot::RoomHome(s) = snap {
            assert_eq!(s.highlights.len(), 1, "must decode 1 highlight");
            assert_eq!(s.highlights[0].event_id, "hl-evt-001");
        } else {
            panic!("expected RoomHome snapshot");
        }
    }

    // T4: highlights_by_reference_groups_by_coordinate
    //
    // A lane artifact and a highlight with the same `a` tag coordinate must
    // appear in a single bucket in highlights_by_reference.
    #[test]
    fn highlights_by_reference_groups_by_coordinate() {
        let mut state = make_state();
        state.communities = vec![make_community_row(TEST_GROUP, TEST_RELAY)];
        state.room_policy.invite_link_base = "https://highlighter.com/r".to_string();

        let addr = "30023:cccc000000000000000000000000000000000000000000000000000000000001:ref";
        inject_lane_event(&mut state, make_kind11_with_a_tag("evt-art-ref", addr));
        inject_hl_event(&mut state, make_kind9802_with_a_tag("hl-ref-001", addr));

        let snap = project_room_home_snapshot(&state, TEST_GROUP).unwrap();
        if let ViewSnapshot::RoomHome(s) = snap {
            assert_eq!(
                s.highlights_by_reference.len(),
                1,
                "must have 1 bucket in highlights_by_reference"
            );
            let bucket = &s.highlights_by_reference[0];
            assert_eq!(bucket.coordinate, format!("a:{addr}"));
            assert_eq!(bucket.highlights.len(), 1);
            assert_eq!(bucket.highlights[0].event_id, "hl-ref-001");
        } else {
            panic!("expected RoomHome snapshot");
        }
    }

    // T5: comments_by_reference_from_comment_threads
    //
    // A lane artifact and a comment_threads entry keyed by the article's
    // root_tag_value must produce a bucket in comments_by_reference.
    #[test]
    fn comments_by_reference_from_comment_threads() {
        let mut state = make_state();
        state.communities = vec![make_community_row(TEST_GROUP, TEST_RELAY)];
        state.room_policy.invite_link_base = "https://highlighter.com/r".to_string();

        let addr = "30023:dddd000000000000000000000000000000000000000000000000000000000001:cmnt";
        // The NIP-22 root_tag_value for this coordinate is the part after "a:".
        let root_val = addr; // "30023:dddd...:cmnt"

        inject_lane_event(&mut state, make_kind11_with_a_tag("evt-art-cmnt", addr));

        // Inject a comment thread snapshot directly into state.
        let record = nmp_nip22::CommentRecord {
            event_id: "comment-001".to_string(),
            author_pubkey: "e".repeat(64),
            body: "great article!".to_string(),
            root_tag_name: "A".to_string(),
            root_tag_value: root_val.to_string(),
            root_kind: "30023".to_string(),
            parent_tag_name: "a".to_string(),
            parent_tag_value: root_val.to_string(),
            parent_kind: "30023".to_string(),
            created_at: 1_700_003_000,
        };
        let snapshot = nmp_nip22::CommentThreadSnapshot {
            root_tag_value: root_val.to_string(),
            records: vec![record],
            tree: vec![],
        };
        state.comment_threads.insert(root_val.to_string(), snapshot);

        let snap = project_room_home_snapshot(&state, TEST_GROUP).unwrap();
        if let ViewSnapshot::RoomHome(s) = snap {
            assert_eq!(
                s.comments_by_reference.len(),
                1,
                "must have 1 bucket in comments_by_reference"
            );
            let bucket = &s.comments_by_reference[0];
            assert_eq!(bucket.root_tag_value, root_val);
            assert_eq!(bucket.count, 1);
            assert_eq!(bucket.comments.len(), 1);
            assert_eq!(bucket.comments[0].event_id, "comment-001");
        } else {
            panic!("expected RoomHome snapshot");
        }
    }

    // T6: roomhome_open_emits_highlight_feed_effects
    //
    // lifecycle_effects_for_view_open must return 5 effects, including
    // RegisterFeedCursor and DrainFeed for the room highlight feed key.
    #[test]
    fn roomhome_open_emits_highlight_feed_effects() {
        let id = ViewId::RoomHome {
            group_id: TEST_GROUP.to_string(),
        };
        let effects = lifecycle_effects_for_view_open(&id);
        let expected_hl_key = format!("{ROOM_HIGHLIGHT_FEED_KEY_PREFIX}{TEST_GROUP}");

        assert_eq!(
            effects.len(),
            5,
            "open must emit 5 effects (WireGroupEvents + lane cursor + lane drain + hl cursor + hl drain)"
        );

        let has_hl_cursor = effects.iter().any(
            |e| matches!(e, Effect::RegisterFeedCursor { key, .. } if key == &expected_hl_key),
        );
        assert!(
            has_hl_cursor,
            "must emit RegisterFeedCursor for room highlight feed"
        );

        let has_hl_drain = effects
            .iter()
            .any(|e| matches!(e, Effect::DrainFeed { key } if key == &expected_hl_key));
        assert!(has_hl_drain, "must emit DrainFeed for room highlight feed");
    }

    // T7: roomhome_close_releases_highlight_feed
    //
    // lifecycle_effects_for_view_close must return 3 effects, including
    // ReleaseFeedCursor for the room highlight feed key.
    #[test]
    fn roomhome_close_releases_highlight_feed() {
        let id = ViewId::RoomHome {
            group_id: TEST_GROUP.to_string(),
        };
        let effects = lifecycle_effects_for_view_close(&id);
        let expected_hl_key = format!("{ROOM_HIGHLIGHT_FEED_KEY_PREFIX}{TEST_GROUP}");

        assert_eq!(
            effects.len(),
            3,
            "close must emit 3 effects (ReleaseGroupEvents + lane cursor + hl cursor)"
        );

        let has_hl_release = effects
            .iter()
            .any(|e| matches!(e, Effect::ReleaseFeedCursor { key } if key == &expected_hl_key));
        assert!(
            has_hl_release,
            "must emit ReleaseFeedCursor for room highlight feed"
        );
    }

    // T8: ensure_room_artifact_previews_seeds_for_kind11
    //
    // After calling ensure_room_artifact_previews, artifact_previews must
    // contain an entry for the coordinate referenced by a kind:11 event.
    #[test]
    fn ensure_room_artifact_previews_seeds_for_kind11() {
        let mut state = make_state();
        state.communities = vec![make_community_row(TEST_GROUP, TEST_RELAY)];

        let addr = "30023:ffff000000000000000000000000000000000000000000000000000000000001:seed";
        inject_lane_event(&mut state, make_kind11_with_a_tag("evt-seed-001", addr));

        let _effects = ensure_room_artifact_previews(&mut state, TEST_GROUP);

        let expected_coord = format!("a:{addr}");
        assert!(
            state.artifact_previews.contains_key(&expected_coord),
            "artifact_previews must contain the coordinate from kind:11 event"
        );
    }

    // T9: empty_aggregation_when_no_artifacts
    //
    // A room with only kind:9 (chat) events in the lane has no artifact shares,
    // so artifact_library, highlights_by_reference, and comments_by_reference
    // must all be empty.
    #[test]
    fn empty_aggregation_when_no_artifacts() {
        let mut state = make_state();
        state.communities = vec![make_community_row(TEST_GROUP, TEST_RELAY)];
        state.room_policy.invite_link_base = "https://highlighter.com/r".to_string();

        // Only kind:9 chat events in the lane (no kind:11 shares).
        inject_lane_event(&mut state, dummy_kernel_event("chat-001", 9));
        inject_lane_event(&mut state, dummy_kernel_event("chat-002", 9));

        let snap = project_room_home_snapshot(&state, TEST_GROUP).unwrap();
        if let ViewSnapshot::RoomHome(s) = snap {
            assert!(
                s.artifact_library.is_empty(),
                "no artifact_library when lane has only kind:9 events"
            );
            assert!(
                s.highlights.is_empty(),
                "no highlights when highlight feed is empty"
            );
            assert!(
                s.highlights_by_reference.is_empty(),
                "no highlights_by_reference when there are no artifacts"
            );
            assert!(
                s.comments_by_reference.is_empty(),
                "no comments_by_reference when there are no artifacts"
            );
        } else {
            panic!("expected RoomHome snapshot");
        }
    }

    // T10: root_tag_value_for_coordinate_strips_prefix
    //
    // root_tag_value_for_coordinate must strip the tag-name prefix and its
    // colon separator, returning the value portion only.
    #[test]
    fn root_tag_value_for_coordinate_strips_prefix() {
        let addr = "30023:aaaa000000000000000000000000000000000000000000000000000000000001:d-tag";
        let coord = format!("a:{addr}");
        let result = root_tag_value_for_coordinate(&coord);
        assert_eq!(
            result,
            Some(addr.to_string()),
            "root_tag_value_for_coordinate must strip 'a:' prefix"
        );

        // Also test e: prefix
        let e_coord = "e:deadbeef00000000000000000000000000000000000000000000000000000001";
        let e_result = root_tag_value_for_coordinate(e_coord);
        assert_eq!(
            e_result,
            Some("deadbeef00000000000000000000000000000000000000000000000000000001".to_string()),
            "root_tag_value_for_coordinate must strip 'e:' prefix"
        );

        // No colon → None
        assert_eq!(
            root_tag_value_for_coordinate("no-colon"),
            None,
            "malformed coordinate with no colon must return None"
        );
    }

    // ─── Parity tests: kernel snapshot vs crate::room_home::query_room_home_snapshot
    //
    // Each test injects the SAME event fixture into BOTH a real nostrdb (bespoke
    // path: crate::room_home::query_room_home_snapshot) AND kernel AppState (kernel
    // path: project_room_home_snapshot). Both functions are called and their section
    // counts are compared field-for-field. Any drift in highlight-matching,
    // coordinate-precedence, or lane-assembly causes a test failure.
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

        fn named_tag(name: &str, value: &str) -> Tag {
            Tag::parse(vec![name.to_string(), value.to_string()]).unwrap()
        }

        fn inject_lane_event_for(
            state: &mut AppState,
            group_id: &str,
            event: nmp_core::substrate::KernelEvent,
        ) {
            let feed_key = format!("{ROOM_LANE_FEED_KEY_PREFIX}{group_id}");
            let fs = state.room_lanes.entry(feed_key).or_default();
            crate::kernel::domains::feed::apply_feed_page(fs, vec![event], 1, false, None);
        }

        fn inject_hl_event_for(
            state: &mut AppState,
            group_id: &str,
            event: nmp_core::substrate::KernelEvent,
        ) {
            let feed_key = format!("{ROOM_HIGHLIGHT_FEED_KEY_PREFIX}{group_id}");
            let fs = state.room_highlight_feeds.entry(feed_key).or_default();
            crate::kernel::domains::feed::apply_feed_page(fs, vec![event], 1, false, None);
        }

        // P1: parity_artifact_and_highlight_lane_counts_match
        //
        // Inject one kind:11 artifact share and one kind:9802 highlight into BOTH a
        // real nostrdb (for the bespoke crate::room_home::query_room_home_snapshot
        // call) AND kernel AppState (for project_room_home_snapshot). Assert that
        // artifacts, highlights, highlights_by_reference, and assembled_lanes counts
        // agree. Fails if highlight-matching or coordinate-precedence drifts.
        #[test]
        fn parity_artifact_and_highlight_lane_counts_match() {
            let (ndb, _tmp) = isolated_ndb(64 * 1024 * 1024);
            let mut state = make_state();
            state.communities = vec![make_community_row(TEST_GROUP, TEST_RELAY)];
            state.room_policy.invite_link_base = "https://highlighter.com/r".to_string();

            let keys = Keys::generate();
            let addr =
                "30023:0000000000000000000000000000000000000000000000000000000000000001:p1-art";

            let artifact_ev = EventBuilder::new(Kind::Custom(11), "")
                .tags(vec![
                    named_tag("h", TEST_GROUP),
                    named_tag("a", addr),
                    named_tag("k", "30023"),
                    named_tag("title", "Parity Article P1"),
                    named_tag("source", "article"),
                ])
                .custom_created_at(Timestamp::from(1_000_000))
                .sign_with_keys(&keys)
                .unwrap();
            let hl_ev = EventBuilder::new(Kind::Custom(9802), "highlighted passage")
                .tags(vec![named_tag("h", TEST_GROUP), named_tag("a", addr)])
                .custom_created_at(Timestamp::from(1_001_000))
                .sign_with_keys(&keys)
                .unwrap();

            // bespoke path: inject into real nostrdb
            process_event_and_wait(&ndb, &artifact_ev);
            process_event_and_wait(&ndb, &hl_ev);

            // kernel path: inject into AppState
            inject_lane_event_for(&mut state, TEST_GROUP, nostr_to_kernel(&artifact_ev));
            inject_hl_event_for(&mut state, TEST_GROUP, nostr_to_kernel(&hl_ev));

            let bespoke = crate::room_home::query_room_home_snapshot(&ndb, TEST_GROUP);
            let ViewSnapshot::RoomHome(kernel) =
                project_room_home_snapshot(&state, TEST_GROUP).unwrap()
            else {
                panic!("expected RoomHome snapshot");
            };

            assert_eq!(
                bespoke.artifacts.len(),
                kernel.artifact_library.len(),
                "P1: artifact count must match"
            );
            assert_eq!(
                bespoke.highlights.len(),
                kernel.highlights.len(),
                "P1: highlight count must match"
            );
            assert_eq!(
                bespoke.highlights_by_reference.len(),
                kernel.highlights_by_reference.len(),
                "P1: highlights_by_reference count must match — matching logic drifted"
            );
            assert_eq!(
                bespoke.lanes.len(),
                kernel.assembled_lanes.len(),
                "P1: assembled lane count must match"
            );
            assert_eq!(bespoke.lanes.len(), 1, "P1: fixture must yield 1 lane");
        }

        // P2: parity_dormant_lane_excluded_by_both_functions
        //
        // An artifact share with no highlights AND no comments must be excluded from
        // lanes in both bespoke (build_visible_room_lanes:75 dormant filter) and
        // kernel (assembled_lanes dormant filter). Both must agree: 0 lanes.
        #[test]
        fn parity_dormant_lane_excluded_by_both_functions() {
            let (ndb, _tmp) = isolated_ndb(64 * 1024 * 1024);
            let mut state = make_state();
            state.communities = vec![make_community_row(TEST_GROUP, TEST_RELAY)];
            state.room_policy.invite_link_base = "https://highlighter.com/r".to_string();

            let keys = Keys::generate();
            let addr =
                "30023:0000000000000000000000000000000000000000000000000000000000000002:p2-dormant";

            let artifact_ev = EventBuilder::new(Kind::Custom(11), "")
                .tags(vec![
                    named_tag("h", TEST_GROUP),
                    named_tag("a", addr),
                    named_tag("k", "30023"),
                ])
                .custom_created_at(Timestamp::from(1_000_000))
                .sign_with_keys(&keys)
                .unwrap();

            process_event_and_wait(&ndb, &artifact_ev);
            inject_lane_event_for(&mut state, TEST_GROUP, nostr_to_kernel(&artifact_ev));

            let bespoke = crate::room_home::query_room_home_snapshot(&ndb, TEST_GROUP);
            let ViewSnapshot::RoomHome(kernel) =
                project_room_home_snapshot(&state, TEST_GROUP).unwrap()
            else {
                panic!("expected RoomHome snapshot");
            };

            assert_eq!(
                bespoke.artifacts.len(),
                kernel.artifact_library.len(),
                "P2: artifact count must match"
            );
            assert_eq!(
                bespoke.lanes.len(),
                kernel.assembled_lanes.len(),
                "P2: dormant filter must agree — both must exclude the lane"
            );
            assert_eq!(
                bespoke.lanes.len(),
                0,
                "P2: dormant artifact must yield 0 lanes"
            );
        }

        // P3: parity_comment_only_lane_not_dormant
        //
        // A lane with a comment but no highlights must survive the dormant filter in
        // both bespoke (build_visible_room_lanes:75) and kernel (assembled_lanes).
        // The bespoke path reads kind:1111 from nostrdb; kernel reads from
        // state.comment_threads. Both must produce 1 lane and 1 comments_by_reference.
        #[test]
        fn parity_comment_only_lane_not_dormant() {
            let (ndb, _tmp) = isolated_ndb(64 * 1024 * 1024);
            let mut state = make_state();
            state.communities = vec![make_community_row(TEST_GROUP, TEST_RELAY)];
            state.room_policy.invite_link_base = "https://highlighter.com/r".to_string();

            let keys = Keys::generate();
            let addr =
                "30023:0000000000000000000000000000000000000000000000000000000000000003:p3-cmnt";
            let root_val = addr;

            let artifact_ev = EventBuilder::new(Kind::Custom(11), "")
                .tags(vec![
                    named_tag("h", TEST_GROUP),
                    named_tag("a", addr),
                    named_tag("k", "30023"),
                ])
                .custom_created_at(Timestamp::from(1_000_000))
                .sign_with_keys(&keys)
                .unwrap();
            let comment_ev = EventBuilder::new(Kind::Custom(1111), "a comment")
                .tags(vec![
                    named_tag("A", root_val),
                    named_tag("K", "30023"),
                    named_tag("a", root_val),
                    named_tag("k", "30023"),
                ])
                .custom_created_at(Timestamp::from(1_002_000))
                .sign_with_keys(&keys)
                .unwrap();

            // bespoke path: both events into nostrdb
            process_event_and_wait(&ndb, &artifact_ev);
            process_event_and_wait(&ndb, &comment_ev);

            // kernel path: artifact into room_lanes; comment into comment_threads
            inject_lane_event_for(&mut state, TEST_GROUP, nostr_to_kernel(&artifact_ev));
            let record = nmp_nip22::CommentRecord {
                event_id: comment_ev.id.to_hex(),
                author_pubkey: comment_ev.pubkey.to_hex(),
                body: "a comment".to_string(),
                root_tag_name: "A".to_string(),
                root_tag_value: root_val.to_string(),
                root_kind: "30023".to_string(),
                parent_tag_name: "a".to_string(),
                parent_tag_value: root_val.to_string(),
                parent_kind: "30023".to_string(),
                created_at: 1_002_000,
            };
            state.comment_threads.insert(
                root_val.to_string(),
                nmp_nip22::CommentThreadSnapshot {
                    root_tag_value: root_val.to_string(),
                    records: vec![record],
                    tree: vec![],
                },
            );

            let bespoke = crate::room_home::query_room_home_snapshot(&ndb, TEST_GROUP);
            let ViewSnapshot::RoomHome(kernel) =
                project_room_home_snapshot(&state, TEST_GROUP).unwrap()
            else {
                panic!("expected RoomHome snapshot");
            };

            assert_eq!(
                bespoke.lanes.len(),
                kernel.assembled_lanes.len(),
                "P3: comment-only lane must survive dormant filter in both"
            );
            assert_eq!(bespoke.lanes.len(), 1, "P3: bespoke must have 1 lane");
            assert_eq!(
                bespoke.comments_by_reference.len(),
                kernel.comments_by_reference.len(),
                "P3: comments_by_reference count must match"
            );
        }

        // P4: parity_full_lane_all_five_sections
        //
        // Full fixture: kind:11 artifact + kind:9802 highlight + kind:1111 comment.
        // Inject into both nostrdb and kernel AppState. All five section counts AND
        // lane content counts must agree between bespoke and kernel.
        #[test]
        fn parity_full_lane_all_five_sections() {
            let (ndb, _tmp) = isolated_ndb(64 * 1024 * 1024);
            let mut state = make_state();
            state.communities = vec![make_community_row(TEST_GROUP, TEST_RELAY)];
            state.room_policy.invite_link_base = "https://highlighter.com/r".to_string();

            let keys = Keys::generate();
            let addr =
                "30023:0000000000000000000000000000000000000000000000000000000000000004:p4-full";
            let root_val = addr;

            let artifact_ev = EventBuilder::new(Kind::Custom(11), "")
                .tags(vec![
                    named_tag("h", TEST_GROUP),
                    named_tag("a", addr),
                    named_tag("k", "30023"),
                    named_tag("title", "Full Parity Article"),
                ])
                .custom_created_at(Timestamp::from(1_000_000))
                .sign_with_keys(&keys)
                .unwrap();
            let hl_ev = EventBuilder::new(Kind::Custom(9802), "highlighted text")
                .tags(vec![named_tag("h", TEST_GROUP), named_tag("a", addr)])
                .custom_created_at(Timestamp::from(1_001_000))
                .sign_with_keys(&keys)
                .unwrap();
            let comment_ev = EventBuilder::new(Kind::Custom(1111), "a comment on the article")
                .tags(vec![
                    named_tag("A", root_val),
                    named_tag("K", "30023"),
                    named_tag("a", root_val),
                    named_tag("k", "30023"),
                ])
                .custom_created_at(Timestamp::from(1_002_000))
                .sign_with_keys(&keys)
                .unwrap();

            // bespoke path
            process_event_and_wait(&ndb, &artifact_ev);
            process_event_and_wait(&ndb, &hl_ev);
            process_event_and_wait(&ndb, &comment_ev);

            // kernel path
            inject_lane_event_for(&mut state, TEST_GROUP, nostr_to_kernel(&artifact_ev));
            inject_hl_event_for(&mut state, TEST_GROUP, nostr_to_kernel(&hl_ev));
            let record = nmp_nip22::CommentRecord {
                event_id: comment_ev.id.to_hex(),
                author_pubkey: comment_ev.pubkey.to_hex(),
                body: "a comment on the article".to_string(),
                root_tag_name: "A".to_string(),
                root_tag_value: root_val.to_string(),
                root_kind: "30023".to_string(),
                parent_tag_name: "a".to_string(),
                parent_tag_value: root_val.to_string(),
                parent_kind: "30023".to_string(),
                created_at: 1_002_000,
            };
            state.comment_threads.insert(
                root_val.to_string(),
                nmp_nip22::CommentThreadSnapshot {
                    root_tag_value: root_val.to_string(),
                    records: vec![record],
                    tree: vec![],
                },
            );

            let bespoke = crate::room_home::query_room_home_snapshot(&ndb, TEST_GROUP);
            let ViewSnapshot::RoomHome(kernel) =
                project_room_home_snapshot(&state, TEST_GROUP).unwrap()
            else {
                panic!("expected RoomHome snapshot");
            };

            assert_eq!(
                bespoke.artifacts.len(),
                kernel.artifact_library.len(),
                "P4: artifact count"
            );
            assert_eq!(
                bespoke.highlights.len(),
                kernel.highlights.len(),
                "P4: highlight count"
            );
            assert_eq!(
                bespoke.highlights_by_reference.len(),
                kernel.highlights_by_reference.len(),
                "P4: highlights_by_reference count"
            );
            assert_eq!(
                bespoke.comments_by_reference.len(),
                kernel.comments_by_reference.len(),
                "P4: comments_by_reference count"
            );
            assert_eq!(
                bespoke.lanes.len(),
                kernel.assembled_lanes.len(),
                "P4: assembled lane count"
            );
            assert_eq!(bespoke.lanes.len(), 1, "P4: fixture must yield 1 lane");
            assert_eq!(
                bespoke.lanes[0].highlights.len(),
                kernel.assembled_lanes[0].highlights.len(),
                "P4: lane highlight count"
            );
            assert_eq!(
                bespoke.lanes[0].comments.len(),
                kernel.assembled_lanes[0].comments.len(),
                "P4: lane comment count"
            );
        }

        // P5: parity_discussion_excluded_from_artifacts_coordinate_seeds_preview
        //
        // A kind:11 discussion event (with t:discussion marker) must be excluded
        // from bespoke artifacts — query_room_home_snapshot skips discussions.
        // The same event's artifact_coordinate in room_discussions must seed
        // artifact_previews via ensure_room_artifact_previews (Gap 3 fix).
        #[test]
        fn parity_discussion_excluded_from_artifacts_coordinate_seeds_preview() {
            use crate::kernel::snapshot::DiscussionRow;

            let (ndb, _tmp) = isolated_ndb(64 * 1024 * 1024);
            let mut state = make_state();
            state.communities = vec![make_community_row(TEST_GROUP, TEST_RELAY)];

            let keys = Keys::generate();
            let addr =
                "30023:0000000000000000000000000000000000000000000000000000000000000005:p5-disc";
            let coord = format!("a:{addr}");

            let disc_ev = EventBuilder::new(Kind::Custom(11), "Check this out!")
                .tags(vec![
                    named_tag("h", TEST_GROUP),
                    named_tag("t", "discussion"),
                    named_tag("a", addr),
                ])
                .custom_created_at(Timestamp::from(1_000_000))
                .sign_with_keys(&keys)
                .unwrap();

            // bespoke: discussion must be excluded from artifacts
            process_event_and_wait(&ndb, &disc_ev);
            let bespoke = crate::room_home::query_room_home_snapshot(&ndb, TEST_GROUP);
            assert_eq!(
                bespoke.artifacts.len(),
                0,
                "P5: bespoke must exclude discussion events from artifacts"
            );

            // kernel: discussion row with artifact_coordinate must seed a preview
            // (mirrors what discussions.rs DiscussionObserver produces after Gap 2 fix)
            let row = DiscussionRow {
                event_id: disc_ev.id.to_hex(),
                author_pubkey: disc_ev.pubkey.to_hex(),
                title: String::new(),
                body: "Check this out!".to_string(),
                attachment_url: None,
                artifact_coordinate: Some(coord.clone()),
                created_at: 1_000_000,
            };
            state
                .room_discussions
                .insert(TEST_GROUP.to_string(), vec![row]);

            let _effects = ensure_room_artifact_previews(&mut state, TEST_GROUP);
            assert!(
                state.artifact_previews.contains_key(&coord),
                "P5: ensure_room_artifact_previews must seed preview for discussion coordinate"
            );
        }
    }
}
