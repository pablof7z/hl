use crate::models::{CommunitySummary, RoomRecommendation};

#[derive(Debug, Clone, uniffi::Record)]
pub struct RoomExplorerSnapshot {
    pub featured: Vec<CommunitySummary>,
    pub new_noteworthy: Vec<CommunitySummary>,
    pub friends_shelf: Vec<RoomRecommendation>,
    pub authors_shelf: Vec<RoomRecommendation>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct RoomBrowseSnapshot {
    pub rooms: Vec<CommunitySummary>,
    pub error: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct RoomBrowseSnapshotApplyInput {
    pub rooms: Vec<CommunitySummary>,
    pub error: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct RoomBrowseSnapshotApplyProjection {
    pub rooms: Vec<CommunitySummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RoomExplorerJoinRequestResultInput {
    pub group_id: String,
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RoomExplorerJoinRequestResultProjection {
    pub should_log: bool,
    pub log_message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RoomExplorerFeaturedStartResultInput {
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RoomExplorerFeaturedStartResultProjection {
    pub should_mark_started: bool,
    pub should_log: bool,
    pub log_message: String,
}

pub fn room_browse_snapshot(rooms: &[CommunitySummary], query: &str) -> RoomBrowseSnapshot {
    RoomBrowseSnapshot {
        rooms: crate::discovery::search_rooms(rooms, query),
        error: String::new(),
    }
}

pub fn room_browse_error_snapshot(error: impl ToString) -> RoomBrowseSnapshot {
    RoomBrowseSnapshot {
        rooms: Vec::new(),
        error: error.to_string(),
    }
}

pub fn room_browse_snapshot_apply_projection(
    input: RoomBrowseSnapshotApplyInput,
) -> RoomBrowseSnapshotApplyProjection {
    RoomBrowseSnapshotApplyProjection {
        rooms: if input.error.trim().is_empty() {
            input.rooms
        } else {
            Vec::new()
        },
    }
}

pub fn room_explorer_join_request_result_projection(
    input: RoomExplorerJoinRequestResultInput,
) -> RoomExplorerJoinRequestResultProjection {
    let error = input.error.trim().to_string();
    let group_id = input.group_id.trim();
    RoomExplorerJoinRequestResultProjection {
        should_log: !error.is_empty(),
        log_message: if error.is_empty() {
            String::new()
        } else {
            format!("requestJoinRoom failed for {group_id}: {error}")
        },
    }
}

pub fn room_explorer_featured_start_result_projection(
    input: RoomExplorerFeaturedStartResultInput,
) -> RoomExplorerFeaturedStartResultProjection {
    let error = input.error.trim().to_string();
    RoomExplorerFeaturedStartResultProjection {
        should_mark_started: error.is_empty(),
        should_log: !error.is_empty(),
        log_message: if error.is_empty() {
            String::new()
        } else {
            format!("startRoomExplorerFeaturedRooms failed: {error}")
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn room(id: &str, name: &str, about: &str, created_at: u64) -> CommunitySummary {
        CommunitySummary {
            id: id.to_string(),
            name: name.to_string(),
            about: about.to_string(),
            picture: String::new(),
            access: "open".to_string(),
            visibility: "public".to_string(),
            admin_pubkeys: Vec::new(),
            member_count: None,
            relay_url: "wss://relay.example".to_string(),
            metadata_event_id: format!("{id}-event"),
            created_at: Some(created_at),
        }
    }

    #[test]
    fn room_browse_snapshot_filters_by_query() {
        let rooms = vec![
            room("one", "Bitcoin Readers", "Money and books", 3),
            room("two", "Design Shelf", "Typography", 2),
            room("three", "Rust Notes", "Systems programming", 1),
        ];

        let snapshot = room_browse_snapshot(&rooms, "  shelf ");

        assert!(snapshot.error.is_empty());
        assert_eq!(snapshot.rooms.len(), 1);
        assert_eq!(snapshot.rooms[0].id, "two");
    }

    #[test]
    fn room_browse_snapshot_keeps_empty_query_order() {
        let rooms = vec![
            room("one", "Bitcoin Readers", "Money and books", 3),
            room("two", "Design Shelf", "Typography", 2),
        ];

        let snapshot = room_browse_snapshot(&rooms, "");

        assert!(snapshot.error.is_empty());
        assert_eq!(
            snapshot
                .rooms
                .iter()
                .map(|room| room.id.as_str())
                .collect::<Vec<_>>(),
            vec!["one", "two"]
        );
    }

    #[test]
    fn room_browse_snapshot_apply_clears_rooms_on_error() {
        let rooms = vec![room("one", "Bitcoin Readers", "Money and books", 3)];

        let success = room_browse_snapshot_apply_projection(RoomBrowseSnapshotApplyInput {
            rooms: rooms.clone(),
            error: String::new(),
        });
        assert_eq!(success.rooms.len(), 1);

        let failed = room_browse_snapshot_apply_projection(RoomBrowseSnapshotApplyInput {
            rooms,
            error: " cache failed ".into(),
        });
        assert!(failed.rooms.is_empty());
    }

    #[test]
    fn room_explorer_join_request_result_projects_log_message() {
        let success =
            room_explorer_join_request_result_projection(RoomExplorerJoinRequestResultInput {
                group_id: "room-a".into(),
                error: String::new(),
            });
        assert!(!success.should_log);
        assert!(success.log_message.is_empty());

        let failed =
            room_explorer_join_request_result_projection(RoomExplorerJoinRequestResultInput {
                group_id: " room-a ".into(),
                error: " relay failed ".into(),
            });
        assert!(failed.should_log);
        assert_eq!(
            failed.log_message,
            "requestJoinRoom failed for room-a: relay failed"
        );
    }

    #[test]
    fn room_explorer_featured_start_result_projects_started_state() {
        let success =
            room_explorer_featured_start_result_projection(RoomExplorerFeaturedStartResultInput {
                error: String::new(),
            });
        assert!(success.should_mark_started);
        assert!(!success.should_log);
        assert!(success.log_message.is_empty());

        let failed =
            room_explorer_featured_start_result_projection(RoomExplorerFeaturedStartResultInput {
                error: " relay failed ".into(),
            });
        assert!(!failed.should_mark_started);
        assert!(failed.should_log);
        assert_eq!(
            failed.log_message,
            "startRoomExplorerFeaturedRooms failed: relay failed"
        );
    }
}
