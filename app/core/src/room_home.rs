//! Room home read model. Rust owns the bounded community-home cache reads,
//! per-artifact reference buckets, comment buckets, and visible lane assembly.

use nostrdb::Ndb;

use crate::errors::CoreError;
use crate::models::{
    ArtifactRecord, CommentReferenceBucket, HighlightReferenceBucket, HydratedHighlight, RoomLane,
};
use crate::{artifacts, comments, highlights, reference_targets, room_lanes};

pub const ROOM_HOME_ARTIFACT_LIMIT: u32 = 32;
pub const ROOM_HOME_HIGHLIGHT_LIMIT: u32 = 64;
pub const ROOM_HOME_REFERENCE_HIGHLIGHT_LIMIT: u32 = 128;
pub const ROOM_HOME_COMMENT_LIMIT: u32 = 128;

#[derive(Debug, Clone, uniffi::Record)]
pub struct RoomHomeSnapshot {
    pub artifacts: Vec<ArtifactRecord>,
    pub highlights: Vec<HydratedHighlight>,
    pub highlights_by_reference: Vec<HighlightReferenceBucket>,
    pub comments_by_reference: Vec<CommentReferenceBucket>,
    pub lanes: Vec<RoomLane>,
}

impl RoomHomeSnapshot {
    fn empty() -> Self {
        Self {
            artifacts: Vec::new(),
            highlights: Vec::new(),
            highlights_by_reference: Vec::new(),
            comments_by_reference: Vec::new(),
            lanes: Vec::new(),
        }
    }
}

/// Full community home snapshot for one NIP-29 group id. Top-level artifact
/// and group-highlight failures become empty sections; per-reference failures
/// omit that bucket. Native shells render the bounded read model directly.
pub(crate) fn query_room_home_snapshot(ndb: &Ndb, group_id: &str) -> RoomHomeSnapshot {
    let group_id = group_id.trim();
    if group_id.is_empty() {
        return RoomHomeSnapshot::empty();
    }

    let artifacts = list_section_or_empty(
        "artifacts",
        artifacts::query_for_group(ndb, group_id, ROOM_HOME_ARTIFACT_LIMIT),
    );
    let highlights = list_section_or_empty(
        "highlights",
        highlights::query_for_group(ndb, group_id, ROOM_HOME_HIGHLIGHT_LIMIT),
    );
    let (highlights_by_reference, comments_by_reference) = reference_buckets(ndb, &artifacts);
    let lanes = room_lanes::build_visible_room_lanes(
        &artifacts,
        &highlights,
        &highlights_by_reference,
        &comments_by_reference,
    );

    RoomHomeSnapshot {
        artifacts,
        highlights,
        highlights_by_reference,
        comments_by_reference,
        lanes,
    }
}

fn reference_buckets(
    ndb: &Ndb,
    artifacts: &[ArtifactRecord],
) -> (Vec<HighlightReferenceBucket>, Vec<CommentReferenceBucket>) {
    let mut highlight_buckets = Vec::new();
    let mut comment_buckets = Vec::new();

    for artifact in artifacts {
        let Some(target) = reference_targets::artifact_reference_target(artifact) else {
            continue;
        };
        if let Some(tag) = target.lowercase_tag.chars().next() {
            let highlights = list_section_or_empty(
                "reference_highlights",
                highlights::query_for_reference(
                    ndb,
                    tag,
                    &target.value,
                    ROOM_HOME_REFERENCE_HIGHLIGHT_LIMIT,
                ),
            );
            highlight_buckets.push(HighlightReferenceBucket {
                lookup_key: target.lookup_key.clone(),
                highlights,
            });
        }
        if let Some(scope) = target.comment_scope.as_ref() {
            if !target.comment_key.is_empty() {
                let comments = list_section_or_empty(
                    "reference_comments",
                    comments::query_for_scope(ndb, scope, ROOM_HOME_COMMENT_LIMIT),
                );
                comment_buckets.push(CommentReferenceBucket {
                    comment_key: target.comment_key,
                    comments,
                });
            }
        }
    }

    (highlight_buckets, comment_buckets)
}

fn list_section_or_empty<T>(section: &'static str, result: Result<Vec<T>, CoreError>) -> Vec<T> {
    match result {
        Ok(values) => values,
        Err(error) => {
            tracing::warn!(section, error = %error, "room home snapshot section failed");
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_ndb::process_event_and_wait;
    use nostr_sdk::prelude::*;
    use tempfile::TempDir;

    fn fresh_ndb() -> (Ndb, TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = nostrdb::Config::new().set_mapsize(64 * 1024 * 1024);
        let ndb = Ndb::new(tmp.path().to_str().unwrap(), &cfg).unwrap();
        (ndb, tmp)
    }

    fn process(ndb: &Ndb, event: &Event) {
        process_event_and_wait(ndb, event);
    }

    fn named(name: &str, value: &str) -> Tag {
        Tag::parse(vec![name.to_string(), value.to_string()]).unwrap()
    }

    #[test]
    fn room_home_snapshot_hydrates_reference_buckets_and_lanes() {
        let (ndb, _tmp) = fresh_ndb();
        let keys = Keys::generate();
        let article_address = "30023:author:essay";

        let artifact = EventBuilder::new(Kind::Custom(11), "shared")
            .tags(vec![
                named("h", "room-a"),
                Tag::identifier("artifact-a"),
                named("title", "Essay"),
                named("source", "article"),
                named("a", article_address),
                named("k", "30023"),
            ])
            .custom_created_at(Timestamp::from(1_000))
            .sign_with_keys(&keys)
            .unwrap();
        let highlight = EventBuilder::new(Kind::Custom(9802), "quote")
            .tags(vec![
                named("h", "room-a"),
                named("a", article_address),
                named("comment", "note"),
            ])
            .custom_created_at(Timestamp::from(1_100))
            .sign_with_keys(&keys)
            .unwrap();
        let comment = EventBuilder::new(Kind::Custom(1111), "comment")
            .tags(vec![
                named("A", article_address),
                named("a", article_address),
                named("K", "30023"),
                named("k", "30023"),
            ])
            .custom_created_at(Timestamp::from(1_200))
            .sign_with_keys(&keys)
            .unwrap();

        for event in [&artifact, &highlight, &comment] {
            process(&ndb, event);
        }

        let snapshot = query_room_home_snapshot(&ndb, "room-a");

        assert_eq!(snapshot.artifacts.len(), 1);
        assert_eq!(snapshot.highlights.len(), 1);
        assert_eq!(snapshot.highlights_by_reference.len(), 1);
        assert_eq!(
            snapshot.highlights_by_reference[0].lookup_key,
            "a:30023:author:essay"
        );
        assert_eq!(snapshot.highlights_by_reference[0].highlights.len(), 1);
        assert_eq!(snapshot.comments_by_reference.len(), 1);
        assert_eq!(
            snapshot.comments_by_reference[0].comment_key,
            "A:30023:author:essay"
        );
        assert_eq!(snapshot.comments_by_reference[0].comments.len(), 1);
        assert_eq!(snapshot.lanes.len(), 1);
        assert_eq!(snapshot.lanes[0].highlights.len(), 1);
        assert_eq!(snapshot.lanes[0].comments.len(), 1);
    }

    #[test]
    fn room_home_snapshot_is_empty_for_blank_group() {
        let (ndb, _tmp) = fresh_ndb();
        let snapshot = query_room_home_snapshot(&ndb, "");

        assert!(snapshot.artifacts.is_empty());
        assert!(snapshot.highlights.is_empty());
        assert!(snapshot.highlights_by_reference.is_empty());
        assert!(snapshot.comments_by_reference.is_empty());
        assert!(snapshot.lanes.is_empty());
    }
}
