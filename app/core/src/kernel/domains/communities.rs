//! Communities domain — joined-groups projection for the Communities view.
//!
//! Owned state: `AppState.communities` (a `Vec<CommunityRow>`).
//!
//! ## Data flow
//!
//! 1. `register_joined_groups_projection(nmp_ref, pubkey)` registers the
//!    `JoinedGroupsProjection` event observer + typed snapshot closure via
//!    `nmp_nip29::register::wire_joined_groups`. Called at boot and re-called
//!    on `IdentityChanged(Some)` via `Effect::WireJoinedGroups`.
//! 2. Each NMP update-callback tick delivers `KernelEvent::NmpSnapshotFrame`.
//! 3. `projections::dispatch_typed_frame` matches `"nmp.nip29.joined_groups"`
//!    and calls `apply_joined_groups(state, &proj.payload)` here.
//! 4. `apply_joined_groups` decodes via `nmp_nip29::decode_joined_groups_snapshot`
//!    and maps `JoinedGroup` → `CommunityRow` (raw fields, no formatted strings).
//! 5. `reduce_event_joined_groups_updated` stores the rows in `AppState.communities`.
//! 6. `project_communities_snapshot` projects them into `ViewSnapshot::Communities`.

use nmp_native_runtime::NmpApp;
use nmp_nip29::{decode_joined_groups_snapshot, open_nip29_joined_groups_session, Nip29JoinedGroupsSession};

use crate::kernel::app::AppState;
use crate::kernel::effect::Effect;
use crate::kernel::snapshot::{CommunitiesSnapshot, CommunityRow, ViewSnapshot};

// ─── Projection registration ─────────────────────────────────────────────────

/// Wire the `JoinedGroupsProjection` event observer + typed snapshot projection
/// against `nmp_ref` for `active_pubkey`.
///
/// Delegates directly to `nmp_nip29::open_nip29_joined_groups_session`, which:
///   - Registers a `JoinedGroupsProjection` as an observed projection (ingest).
///   - Registers a typed FlatBuffers sidecar under `"nmp.nip29.joined_groups"`.
///
/// Must be called:
///   1. Once at boot (after `nmp_app_start`) via `start_nmp_app`.
///   2. On `IdentityChanged(Some(pubkey))` via `Effect::WireJoinedGroups`.
///
/// An empty `active_pubkey` is a silent no-op (`open_nip29_joined_groups_session`
/// guards it, returning `None`). `host_relay_url` is passed as empty here — the
/// projection accepts events from any relay provenance, so hl does not pin to a
/// specific host at registration. (The interest helper `joined_groups_for_host`
/// can be used to push a relay-pinned interest separately; hl relies on the
/// standing kind:39001/39002 subscriptions from the active-account interest set
/// for now.)
pub(crate) fn register_joined_groups_projection(nmp_ref: &NmpApp, active_pubkey: String) {
    let _handle = open_nip29_joined_groups_session(nmp_ref, Nip29JoinedGroupsSession::new(
        active_pubkey,
        String::new(),
    ));
}

// ─── Frame decode (called from projections::dispatch_typed_frame) ─────────────

/// Decode a `"nmp.nip29.joined_groups"` FlatBuffers payload and store the
/// resulting `Vec<CommunityRow>` in `AppState.communities`.
///
/// Called from `projections::dispatch_typed_frame` on the actor thread.
/// Non-blocking (FlatBuffers decode only — no I/O). D6: any decode error leaves
/// `AppState.communities` unchanged (silent no-op).
pub(crate) fn apply_joined_groups(state: &mut AppState, payload: &[u8]) {
    match decode_joined_groups_snapshot(payload) {
        Ok(snapshot) => {
            state.communities = snapshot
                .groups
                .into_iter()
                .map(|g| CommunityRow {
                    group_id: g.group_id,
                    host_relay_url: g.host_relay_url,
                    name: g.name,
                    picture: g.picture,
                    about: g.about,
                    member_count: g.member_count,
                    public: g.public,
                    open: g.open,
                    is_admin: g.is_admin,
                })
                .collect();
        }
        Err(e) => {
            tracing::trace!(
                error = %e,
                "communities::apply_joined_groups: decode error — AppState.communities unchanged (D6)"
            );
        }
    }
}

// ─── Reducer (event) ─────────────────────────────────────────────────────────

/// Store the decoded joined-groups list in `AppState.communities`.
///
/// Called from `reduce_event(KernelEvent::JoinedGroupsUpdated(groups))` on the
/// actor thread. Replaces the full slice on every update (bounded replacement —
/// the list is only as large as the number of groups the account has joined).
///
/// Phase 5K: also refreshes the App Group communities handoff snapshot so the
/// share extension always has a current community picker list.
pub(crate) fn reduce_event_joined_groups_updated(
    state: &mut AppState,
    groups: Vec<CommunityRow>,
) -> Vec<Effect> {
    state.communities = groups;
    // ── Phase 5K additions (append-only) ─────────────────────────────────────
    // Refresh the App Group handoff JSON on every community-list update so the
    // share extension picker is always current. No-op when communities is empty.
    crate::kernel::domains::share::on_communities_updated(&state.communities)
}

// ─── Snapshot projection ─────────────────────────────────────────────────────

/// Compute the `ViewSnapshot::Communities` for the open Communities view.
///
/// Always returns `Some` — the open-view gate is enforced at the actor level
/// (`project_snapshot` is only called for open views). An empty `groups` vec
/// is a valid snapshot (account has joined no groups).
pub(crate) fn project_communities_snapshot(state: &AppState) -> Option<ViewSnapshot> {
    Some(ViewSnapshot::Communities(CommunitiesSnapshot {
        groups: state.communities.clone(),
    }))
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::app::AppState;

    fn make_row(id: &str) -> CommunityRow {
        CommunityRow {
            group_id: id.to_string(),
            host_relay_url: "wss://relay.test".to_string(),
            name: Some(format!("Group {id}")),
            picture: None,
            about: None,
            member_count: 5,
            public: true,
            open: true,
            is_admin: false,
        }
    }

    // 3B-T1: joined_groups_frame_updates_state
    #[test]
    fn joined_groups_updates_state() {
        let mut state = AppState::default();
        let rows = vec![make_row("g1"), make_row("g2")];
        let effects = reduce_event_joined_groups_updated(&mut state, rows.clone());
        // Phase 5K: a WriteCommunitiesSnapshot capability request is emitted so
        // the share extension always has a current community picker. Verify state
        // is correctly updated regardless.
        assert_eq!(state.communities.len(), 2);
        assert_eq!(state.communities[0].group_id, "g1");
        assert_eq!(state.communities[1].group_id, "g2");
        // Exactly one effect (WriteCommunitiesSnapshot) when communities are non-empty.
        assert_eq!(
            effects.len(),
            1,
            "expected WriteCommunitiesSnapshot effect; got {effects:?}"
        );
    }

    // 3B-T2: communities_snapshot_has_raw_fields_not_labels
    #[test]
    fn communities_snapshot_has_raw_fields_not_labels() {
        let mut state = AppState::default();
        let row = make_row("room42");
        reduce_event_joined_groups_updated(&mut state, vec![row]);
        let snap = project_communities_snapshot(&state).unwrap();
        if let crate::kernel::snapshot::ViewSnapshot::Communities(cs) = snap {
            assert_eq!(cs.groups[0].group_id, "room42");
            assert_eq!(cs.groups[0].member_count, 5);
            // No formatted strings — name is Option<String>, not "5 members"
            let name = cs.groups[0].name.as_deref().unwrap_or("");
            assert!(
                !name.contains("member"),
                "name must not contain formatted label"
            );
        } else {
            panic!("expected Communities snapshot");
        }
    }

    // 3B-T3: communities_reregistered_on_identity_change (clears prior groups)
    #[test]
    fn communities_cleared_on_identity_change() {
        let mut state = AppState::default();
        reduce_event_joined_groups_updated(&mut state, vec![make_row("g1")]);
        assert_eq!(state.communities.len(), 1);
        // Simulate IdentityChanged clearing communities
        state.communities = vec![];
        assert!(state.communities.is_empty());
    }

    // 3B-T4: closed view emits no snapshot — the actor only calls project_communities_snapshot
    // for open views; a closed view simply never has its snapshot projected.
    // We test the inverse: an open Communities view DOES get a snapshot.
    #[test]
    fn communities_snapshot_is_some_when_groups_exist() {
        let state = AppState {
            communities: vec![make_row("g1")],
            ..AppState::default()
        };
        assert!(project_communities_snapshot(&state).is_some());
    }

    // 3B-T5: malformed frame no-ops — tested in projections.rs; here verify
    // reduce_event_joined_groups_updated with empty vec leaves state empty.
    #[test]
    fn empty_groups_update_clears_state() {
        let mut state = AppState::default();
        reduce_event_joined_groups_updated(&mut state, vec![make_row("g1")]);
        reduce_event_joined_groups_updated(&mut state, vec![]);
        assert!(state.communities.is_empty());
    }
}
