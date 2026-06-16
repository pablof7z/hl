//! Share-extension handoff projections.
//!
//! The iOS share extension cannot load the full Rust core just to draw a
//! picker. The main app asks Rust for this small JSON projection and writes
//! the bytes into the App Group handoff store.

use serde::Serialize;

use crate::errors::CoreError;
use crate::models::CommunitySummary;

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ShareQueueItem {
    pub id: String,
    pub group_id: String,
    pub url: String,
    pub note: String,
    pub created_at_unix_seconds: f64,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ShareQueueAttempt {
    pub item: ShareQueueItem,
    pub succeeded: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ShareQueueDrainProjectionInput {
    pub attempts: Vec<ShareQueueAttempt>,
    pub communities: Vec<CommunitySummary>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ShareQueueDrainProjection {
    pub requeue: Vec<ShareQueueItem>,
    pub success_count: u64,
    pub toast: Option<String>,
}

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

pub fn share_queue_drain_projection(
    input: ShareQueueDrainProjectionInput,
) -> ShareQueueDrainProjection {
    let mut requeue = Vec::new();
    let mut success_count = 0u64;
    let mut last_success_community = None;

    for attempt in input.attempts {
        if attempt.succeeded {
            success_count += 1;
            last_success_community = Some(community_label(
                &attempt.item.group_id,
                input.communities.as_slice(),
            ));
        } else {
            requeue.push(attempt.item);
        }
    }

    ShareQueueDrainProjection {
        requeue,
        success_count,
        toast: if success_count == 0 {
            None
        } else if success_count == 1 {
            Some(format!(
                "Shared to {}",
                last_success_community.unwrap_or_else(|| "community".into())
            ))
        } else {
            Some(format!("Shared {success_count} items"))
        },
    }
}

pub fn share_queue_attempt(
    item: ShareQueueItem,
    result: Result<(), CoreError>,
) -> ShareQueueAttempt {
    ShareQueueAttempt {
        item,
        succeeded: result.is_ok(),
    }
}

fn community_label(group_id: &str, communities: &[CommunitySummary]) -> String {
    communities
        .iter()
        .find(|community| community.id == group_id)
        .map(|community| community.name.clone())
        .unwrap_or_else(|| group_id.to_string())
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

    #[test]
    fn queue_drain_projection_requeues_failures_and_names_single_success() {
        let succeeded = share("ok", "group-a");
        let failed = share("retry", "group-b");

        let projection = share_queue_drain_projection(ShareQueueDrainProjectionInput {
            attempts: vec![
                ShareQueueAttempt {
                    item: succeeded,
                    succeeded: true,
                },
                ShareQueueAttempt {
                    item: failed.clone(),
                    succeeded: false,
                },
            ],
            communities: vec![community("group-a", "Readers")],
        });

        assert_eq!(projection.success_count, 1);
        assert_eq!(projection.toast.as_deref(), Some("Shared to Readers"));
        assert_eq!(projection.requeue, vec![failed]);
    }

    #[test]
    fn queue_drain_projection_counts_multiple_successes() {
        let projection = share_queue_drain_projection(ShareQueueDrainProjectionInput {
            attempts: vec![
                ShareQueueAttempt {
                    item: share("one", "group-a"),
                    succeeded: true,
                },
                ShareQueueAttempt {
                    item: share("two", "group-b"),
                    succeeded: true,
                },
            ],
            communities: vec![],
        });

        assert_eq!(projection.success_count, 2);
        assert_eq!(projection.toast.as_deref(), Some("Shared 2 items"));
        assert!(projection.requeue.is_empty());
    }

    #[test]
    fn queue_attempt_projects_publish_result() {
        let item = share("one", "group-a");
        assert!(share_queue_attempt(item.clone(), Ok(())).succeeded);

        let failed = share_queue_attempt(item.clone(), Err(CoreError::Network("offline".into())));
        assert_eq!(failed.item, item);
        assert!(!failed.succeeded);
    }

    fn share(id: &str, group_id: &str) -> ShareQueueItem {
        ShareQueueItem {
            id: id.into(),
            group_id: group_id.into(),
            url: "https://example.com".into(),
            note: "note".into(),
            created_at_unix_seconds: 42.0,
        }
    }

    fn community(id: &str, name: &str) -> CommunitySummary {
        CommunitySummary {
            id: id.into(),
            name: name.into(),
            about: String::new(),
            picture: String::new(),
            access: "open".into(),
            visibility: "public".into(),
            admin_pubkeys: Vec::new(),
            member_count: None,
            relay_url: String::new(),
            metadata_event_id: String::new(),
            created_at: None,
        }
    }
}
