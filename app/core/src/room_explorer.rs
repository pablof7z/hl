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
}
