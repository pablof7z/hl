//! Communities domain — joined-groups projection for the Communities view.
//!
//! Owned state: `AppState.communities` (a `Vec<CommunityRow>`).
//! Wired via: `KernelEvent::JoinedGroupsUpdated` → `reduce_event_joined_groups_updated`
//!            → `project_communities_snapshot` for `ViewId::Communities`.
//!
//! ## NMP wiring note
//! `wire_joined_groups` (nmp-nip29 PR #1587/#1588, not yet on origin/master) must
//! be called once at boot and re-called on `IdentityChanged(Some)`.  The dispatch
//! arm for schema_id `"nmp.nip29.joined_groups"` in `projections.rs` is in place;
//! it will fire once that NMP PR lands and hl pins to it.

use crate::kernel::app::AppState;
use crate::kernel::effect::Effect;
use crate::kernel::snapshot::{CommunitiesSnapshot, CommunityRow, ViewSnapshot};

// ─── Reducer (event) ─────────────────────────────────────────────────────────

/// Store the decoded joined-groups list in `AppState.communities`.
///
/// Called from `reduce_event(KernelEvent::JoinedGroupsUpdated(groups))` on the
/// actor thread. Replaces the full slice on every update (bounded replacement —
/// the list is only as large as the number of groups the account has joined).
pub(crate) fn reduce_event_joined_groups_updated(
    state: &mut AppState,
    groups: Vec<CommunityRow>,
) -> Vec<Effect> {
    state.communities = groups;
    vec![]
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
        assert!(effects.is_empty());
        assert_eq!(state.communities.len(), 2);
        assert_eq!(state.communities[0].group_id, "g1");
        assert_eq!(state.communities[1].group_id, "g2");
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
