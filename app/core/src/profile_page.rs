//! Profile page read model. Rust owns the tab data queries, limits, follow
//! state read, and partial-failure fallback for the native profile surface.

use nostrdb::Ndb;

use crate::errors::CoreError;
use crate::models::{ArticleRecord, CommunitySummary, HighlightRecord, ProfileMetadata};
use crate::{articles, follows, groups, highlights, profile};

pub const PROFILE_PAGE_ARTICLE_LIMIT: u32 = 32;
pub const PROFILE_PAGE_HIGHLIGHT_LIMIT: u32 = 64;

#[derive(Debug, Clone, uniffi::Record)]
pub struct ProfilePageSnapshot {
    pub profile: Option<ProfileMetadata>,
    pub articles: Vec<ArticleRecord>,
    pub highlights: Vec<HighlightRecord>,
    pub communities: Vec<CommunitySummary>,
    pub is_following: bool,
}

impl ProfilePageSnapshot {
    fn empty() -> Self {
        Self {
            profile: None,
            articles: Vec::new(),
            highlights: Vec::new(),
            communities: Vec::new(),
            is_following: false,
        }
    }
}

/// Full profile page snapshot for one pubkey. Cache failures in one section do
/// not blank unrelated sections: Rust logs the failed section and returns the
/// same fallback the native UI already expects for a cold cache.
pub fn query_profile_page_snapshot(
    ndb: &Ndb,
    profile_pubkey: &str,
    viewer_pubkey: Option<&str>,
) -> ProfilePageSnapshot {
    let pubkey = profile_pubkey.trim();
    if pubkey.is_empty() {
        return ProfilePageSnapshot::empty();
    }
    let relationship =
        profile::profile_relationship_projection(profile::ProfileRelationshipProjectionInput {
            profile_pubkey: pubkey.to_string(),
            viewer_pubkey: viewer_pubkey
                .map(str::trim)
                .filter(|viewer| !viewer.is_empty())
                .map(str::to_string),
        });

    ProfilePageSnapshot {
        profile: optional_section_or_none("profile", profile::query_profile_from_ndb(ndb, pubkey)),
        articles: list_section_or_empty(
            "articles",
            articles::query_articles_by_author(ndb, pubkey, PROFILE_PAGE_ARTICLE_LIMIT),
        ),
        highlights: list_section_or_empty(
            "highlights",
            highlights::query_highlights_by_author(ndb, pubkey, PROFILE_PAGE_HIGHLIGHT_LIMIT),
        ),
        communities: list_section_or_empty(
            "communities",
            groups::query_joined_communities_from_ndb(ndb, pubkey),
        ),
        is_following: if relationship.should_refresh_follow_state {
            bool_section_or_false(
                "follow_state",
                follows::is_following(ndb, viewer_pubkey.unwrap_or_default(), pubkey),
            )
        } else {
            false
        },
    }
}

fn optional_section_or_none<T>(
    section: &'static str,
    result: Result<Option<T>, CoreError>,
) -> Option<T> {
    match result {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(section, error = %error, "profile page snapshot section failed");
            None
        }
    }
}

fn list_section_or_empty<T>(section: &'static str, result: Result<Vec<T>, CoreError>) -> Vec<T> {
    match result {
        Ok(values) => values,
        Err(error) => {
            tracing::warn!(section, error = %error, "profile page snapshot section failed");
            Vec::new()
        }
    }
}

fn bool_section_or_false(section: &'static str, result: Result<bool, CoreError>) -> bool {
    match result {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(section, error = %error, "profile page snapshot section failed");
            false
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
    fn profile_page_snapshot_hydrates_tabs_and_follow_state() {
        let (ndb, _tmp) = fresh_ndb();
        let profile_keys = Keys::generate();
        let viewer_keys = Keys::generate();
        let room_owner = Keys::generate();
        let profile_pubkey = profile_keys.public_key().to_hex();

        let profile = EventBuilder::new(Kind::Custom(0), r#"{"display_name":"Profile Name"}"#)
            .custom_created_at(Timestamp::from(1_000))
            .sign_with_keys(&profile_keys)
            .unwrap();
        let article = EventBuilder::new(Kind::Custom(articles::KIND_LONG_FORM), "article body")
            .tags(vec![Tag::identifier("essay"), named("title", "Essay")])
            .custom_created_at(Timestamp::from(1_100))
            .sign_with_keys(&profile_keys)
            .unwrap();
        let highlight = EventBuilder::new(Kind::Custom(9802), "profile quote")
            .tags(vec![named("a", "30023:author:essay")])
            .custom_created_at(Timestamp::from(1_200))
            .sign_with_keys(&profile_keys)
            .unwrap();
        let contact_list = EventBuilder::new(Kind::Custom(3), "")
            .tags(vec![Tag::public_key(profile_keys.public_key())])
            .custom_created_at(Timestamp::from(1_300))
            .sign_with_keys(&viewer_keys)
            .unwrap();
        let room_metadata = EventBuilder::new(Kind::Custom(groups::KIND_GROUP_METADATA), "")
            .tags(vec![Tag::identifier("room-a"), named("name", "Room A")])
            .custom_created_at(Timestamp::from(1_400))
            .sign_with_keys(&room_owner)
            .unwrap();
        let room_membership = EventBuilder::new(Kind::Custom(groups::KIND_GROUP_MEMBERS), "")
            .tags(vec![
                Tag::identifier("room-a"),
                Tag::public_key(profile_keys.public_key()),
            ])
            .custom_created_at(Timestamp::from(1_500))
            .sign_with_keys(&room_owner)
            .unwrap();

        for event in [
            &profile,
            &article,
            &highlight,
            &contact_list,
            &room_metadata,
            &room_membership,
        ] {
            process(&ndb, event);
        }

        let snapshot = query_profile_page_snapshot(
            &ndb,
            &profile_pubkey,
            Some(&viewer_keys.public_key().to_hex()),
        );

        assert_eq!(
            snapshot.profile.as_ref().map(|p| p.display_name.as_str()),
            Some("Profile Name")
        );
        assert_eq!(snapshot.articles.len(), 1);
        assert_eq!(snapshot.articles[0].title, "Essay");
        assert_eq!(snapshot.highlights.len(), 1);
        assert_eq!(snapshot.highlights[0].quote, "profile quote");
        assert_eq!(snapshot.communities.len(), 1);
        assert_eq!(snapshot.communities[0].name, "Room A");
        assert!(snapshot.is_following);
    }

    #[test]
    fn profile_page_snapshot_does_not_follow_without_distinct_viewer() {
        let (ndb, _tmp) = fresh_ndb();
        let profile_keys = Keys::generate();
        let pubkey = profile_keys.public_key().to_hex();

        let own = query_profile_page_snapshot(&ndb, &pubkey, Some(&pubkey));
        assert!(!own.is_following);

        let logged_out = query_profile_page_snapshot(&ndb, &pubkey, None);
        assert!(!logged_out.is_following);

        let blank = query_profile_page_snapshot(&ndb, "", Some(&pubkey));
        assert!(blank.profile.is_none());
        assert!(blank.articles.is_empty());
        assert!(blank.highlights.is_empty());
        assert!(blank.communities.is_empty());
        assert!(!blank.is_following);
    }
}
