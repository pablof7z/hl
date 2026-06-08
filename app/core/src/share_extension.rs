//! Share-extension handoff projections.
//!
//! The iOS share extension cannot load the full Rust core just to draw a
//! picker. The main app asks Rust for this small JSON projection and writes
//! the bytes into the App Group handoff store.

use serde::Serialize;

use crate::models::CommunitySummary;

#[derive(Debug, Serialize)]
struct SharedCommunitySummary {
    id: String,
    name: String,
    picture: String,
}

pub fn communities_snapshot_json(communities: Vec<CommunitySummary>) -> Vec<u8> {
    let rows: Vec<SharedCommunitySummary> = communities
        .into_iter()
        .map(|community| SharedCommunitySummary {
            id: community.id,
            name: community.name,
            picture: community.picture,
        })
        .collect();
    serde_json::to_vec(&rows).unwrap_or_else(|_| b"[]".to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn community_snapshot_schema_matches_extension_decoder() {
        let json = communities_snapshot_json(vec![CommunitySummary {
            id: "group-a".into(),
            name: "Readers".into(),
            about: "about".into(),
            picture: "https://example.com/a.jpg".into(),
            access: "open".into(),
            visibility: "public".into(),
            admin_pubkeys: vec!["pubkey".into()],
            member_count: Some(42),
            relay_url: "wss://relay.example.com".into(),
            metadata_event_id: "event".into(),
            created_at: Some(1),
        }]);

        assert_eq!(
            String::from_utf8(json).unwrap(),
            r#"[{"id":"group-a","name":"Readers","picture":"https://example.com/a.jpg"}]"#
        );
    }
}
