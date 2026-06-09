//! NIP-51 Bookmark sets (kind:30003), Curation sets (kind:30004), and
//! NIP-B0 Web bookmarks (kind:39701).
//!
//! These are all parameterized replaceable events (NIP-33), so one event
//! exists per (author, d-tag) pair. Reads come straight from NostrDB;
//! writes go through the runtime's signer.

use std::collections::HashMap;

use ::url::Url;
use nostr_sdk::prelude::*;
use nostrdb::{Filter as NdbFilter, Ndb, Transaction};

use crate::articles;
use crate::artifacts::first_tag_value;
use crate::clock::Clock;
use crate::errors::CoreError;
use crate::models::{ArticleRecord, BookmarkSetRecord, CurationMenuItem, WebBookmarkRecord};
use crate::nostr_runtime::NostrRuntime;

pub const KIND_BOOKMARK_SETS: u16 = 30003;
pub const KIND_CURATION_SETS: u16 = 30004;
pub const KIND_WEB_BOOKMARK: u16 = 39701;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum BookmarkLibraryScope {
    Mine,
    Explore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum BookmarkLibraryFilter {
    Articles,
    Collections,
    Web,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum BookmarkLibraryPane {
    Articles,
    Collections,
    Web,
    Explore,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BookmarkLibraryProjectionInput {
    pub scope: BookmarkLibraryScope,
    pub selected_filter: BookmarkLibraryFilter,
    pub article_count: u64,
    pub collection_count: u64,
    pub web_bookmark_count: u64,
    pub explore_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BookmarkLibraryScopeOptionProjection {
    pub scope: BookmarkLibraryScope,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BookmarkLibraryFilterChipProjection {
    pub filter: BookmarkLibraryFilter,
    pub label: String,
    pub icon_system_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BookmarkLibraryProjection {
    pub scope_options: Vec<BookmarkLibraryScopeOptionProjection>,
    pub filter_chips: Vec<BookmarkLibraryFilterChipProjection>,
    pub selected_pane: BookmarkLibraryPane,
    pub is_empty: bool,
    pub empty_icon_system_name: String,
    pub empty_title: String,
    pub empty_message: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BookmarkLibrarySnapshot {
    pub my_articles: Vec<ArticleRecord>,
    pub my_bookmark_sets: Vec<BookmarkSetRecord>,
    pub my_curation_sets: Vec<BookmarkSetRecord>,
    pub my_web_bookmarks: Vec<WebBookmarkRecord>,
    pub following_curation_sets: Vec<BookmarkSetRecord>,
}

impl BookmarkLibrarySnapshot {
    fn empty() -> Self {
        Self {
            my_articles: Vec::new(),
            my_bookmark_sets: Vec::new(),
            my_curation_sets: Vec::new(),
            my_web_bookmarks: Vec::new(),
            following_curation_sets: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BookmarkedArticleRowProjectionInput {
    pub article: ArticleRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BookmarkedArticleRowProjection {
    pub title: String,
    pub summary: Option<String>,
    pub image_url: Option<String>,
    pub display_unix_seconds: Option<u64>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BookmarkSetRowProjectionInput {
    pub record: BookmarkSetRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BookmarkSetRowProjection {
    pub display_title: String,
    pub kind_label: String,
    pub kind_icon_system_name: String,
    pub item_count_label: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct BookmarkSetDetailProjectionInput {
    pub record: BookmarkSetRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BookmarkSetDetailProjection {
    pub display_title: String,
}

/// Native create-collection sheet input. Rust owns title normalization.
#[derive(Debug, Clone, uniffi::Record)]
pub struct CurationSetCreateProjectionInput {
    pub title: String,
}

/// Native create-collection sheet projection. Rust owns submit eligibility.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct CurationSetCreateProjection {
    pub submit_title: String,
    pub can_create: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct WebBookmarkRowProjectionInput {
    pub bookmark: WebBookmarkRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct WebBookmarkRowProjection {
    pub display_title: String,
    pub host: Option<String>,
    pub description: Option<String>,
    pub display_unix_seconds: Option<u64>,
}

// -- Public query API --------------------------------------------------------

/// Full bookmark-library read model. Rust owns the current user's bookmark
/// address lookup, section query ordering, explore-set filtering, and fallback
/// policy when one cached section fails to read.
pub fn query_bookmark_library_snapshot(ndb: &Ndb, user_hex: &str) -> BookmarkLibrarySnapshot {
    let user_hex = user_hex.trim();
    if user_hex.is_empty() {
        return BookmarkLibrarySnapshot::empty();
    }

    let bookmarked_addresses = match crate::bookmarks::query_bookmarks(ndb, user_hex) {
        Ok(list) => list.addresses,
        Err(error) => {
            tracing::warn!(error = %error, "failed to query bookmark addresses");
            Vec::new()
        }
    };
    let my_articles = bookmark_library_section_or_empty(
        "bookmarked articles",
        articles::query_articles_for_addresses(ndb, &bookmarked_addresses),
    );
    let my_bookmark_sets = bookmark_library_section_or_empty(
        "bookmark sets",
        query_user_sets(ndb, user_hex, KIND_BOOKMARK_SETS),
    );
    let my_curation_sets = bookmark_library_section_or_empty(
        "curation sets",
        query_user_sets(ndb, user_hex, KIND_CURATION_SETS),
    );
    let my_web_bookmarks =
        bookmark_library_section_or_empty("web bookmarks", query_user_web_bookmarks(ndb, user_hex));

    let follows =
        bookmark_library_section_or_empty("follows", crate::follows::query_follows(ndb, user_hex));
    let following_curation_sets = bookmark_library_section_or_empty(
        "following curation sets",
        query_following_curation_sets(ndb, &follows),
    );

    BookmarkLibrarySnapshot {
        my_articles,
        my_bookmark_sets,
        my_curation_sets,
        my_web_bookmarks,
        following_curation_sets: explorable_curation_sets(ndb, following_curation_sets),
    }
}

/// Return all kind:30003 or kind:30004 sets authored by `user_hex`, newest
/// first. Deduplicates in Rust so callers always get one record per d-tag.
pub fn query_user_sets(
    ndb: &Ndb,
    user_hex: &str,
    kind: u16,
) -> Result<Vec<BookmarkSetRecord>, CoreError> {
    if user_hex.is_empty() {
        return Ok(Vec::new());
    }
    let author = PublicKey::from_hex(user_hex)
        .map_err(|e| CoreError::InvalidInput(format!("invalid pubkey: {e}")))?;
    let pk_bytes: [u8; 32] = author.to_bytes();

    let txn = Transaction::new(ndb).map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;
    let filter = NdbFilter::new()
        .kinds([kind as u64])
        .authors([&pk_bytes])
        .build();
    let results = ndb
        .query(&txn, &[filter], 256)
        .map_err(|e| CoreError::Cache(format!("query user sets: {e}")))?;

    let mut by_d: HashMap<String, Event> = HashMap::new();
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        let d = first_tag_value(&event, "d").unwrap_or("").to_string();
        let entry = by_d.entry(d).or_insert_with(|| event.clone());
        if event.created_at > entry.created_at {
            *entry = event;
        }
    }

    let mut records: Vec<BookmarkSetRecord> = by_d
        .into_values()
        .map(|ev| parse_set_event(ev, kind))
        .collect();
    records.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(records)
}

/// Return kind:30004 curation sets authored by any of `follow_hexes`, newest
/// first per (author, d-tag). Used for the Explore mode.
pub fn query_following_curation_sets(
    ndb: &Ndb,
    follow_hexes: &[String],
) -> Result<Vec<BookmarkSetRecord>, CoreError> {
    if follow_hexes.is_empty() {
        return Ok(Vec::new());
    }
    let pks: Vec<PublicKey> = follow_hexes
        .iter()
        .filter_map(|h| PublicKey::from_hex(h).ok())
        .collect();
    if pks.is_empty() {
        return Ok(Vec::new());
    }
    let pk_bytes: Vec<[u8; 32]> = pks.iter().map(|pk| pk.to_bytes()).collect();
    let pk_refs: Vec<&[u8; 32]> = pk_bytes.iter().collect();

    let txn = Transaction::new(ndb).map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;
    let filter = NdbFilter::new()
        .kinds([KIND_CURATION_SETS as u64])
        .authors(pk_refs.iter().copied())
        .build();
    let results = ndb
        .query(&txn, &[filter], 512)
        .map_err(|e| CoreError::Cache(format!("query following curation sets: {e}")))?;

    let mut by_key: HashMap<(String, String), Event> = HashMap::new();
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        let d = first_tag_value(&event, "d").unwrap_or("").to_string();
        let pk = event.pubkey.to_hex();
        let key = (pk, d);
        let entry = by_key.entry(key).or_insert_with(|| event.clone());
        if event.created_at > entry.created_at {
            *entry = event;
        }
    }

    let mut records: Vec<BookmarkSetRecord> = by_key
        .into_values()
        .map(|ev| parse_set_event(ev, KIND_CURATION_SETS))
        .collect();
    records.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(records)
}

fn bookmark_library_section_or_empty<T>(
    section: &'static str,
    result: Result<Vec<T>, CoreError>,
) -> Vec<T> {
    match result {
        Ok(values) => values,
        Err(error) => {
            tracing::warn!(section, error = %error, "bookmark library section failed");
            Vec::new()
        }
    }
}

/// Return all NIP-B0 kind:39701 web bookmarks authored by `user_hex`,
/// newest first. The `url` field is prefixed with `https://`.
pub fn query_user_web_bookmarks(
    ndb: &Ndb,
    user_hex: &str,
) -> Result<Vec<WebBookmarkRecord>, CoreError> {
    if user_hex.is_empty() {
        return Ok(Vec::new());
    }
    let author = PublicKey::from_hex(user_hex)
        .map_err(|e| CoreError::InvalidInput(format!("invalid pubkey: {e}")))?;
    let pk_bytes: [u8; 32] = author.to_bytes();

    let txn = Transaction::new(ndb).map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;
    let filter = NdbFilter::new()
        .kinds([KIND_WEB_BOOKMARK as u64])
        .authors([&pk_bytes])
        .build();
    let results = ndb
        .query(&txn, &[filter], 256)
        .map_err(|e| CoreError::Cache(format!("query web bookmarks: {e}")))?;

    let mut by_d: HashMap<String, Event> = HashMap::new();
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        let d = first_tag_value(&event, "d").unwrap_or("").to_string();
        let entry = by_d.entry(d).or_insert_with(|| event.clone());
        if event.created_at > entry.created_at {
            *entry = event;
        }
    }

    let mut records: Vec<WebBookmarkRecord> =
        by_d.into_values().map(parse_web_bookmark_event).collect();
    records.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(records)
}

/// Project curation sets into menu rows for a single article address.
/// Preserves the order already established by `query_user_sets`.
pub fn curation_menu_items_for_address(
    sets: Vec<BookmarkSetRecord>,
    address: &str,
) -> Vec<CurationMenuItem> {
    let address = address.trim();
    sets.into_iter()
        .map(|set| CurationMenuItem {
            id: set.id.clone(),
            title: curation_set_display_title(&set),
            is_member: !address.is_empty() && set.article_addresses.iter().any(|a| a == address),
        })
        .collect()
}

pub fn bookmark_library_projection(
    input: BookmarkLibraryProjectionInput,
) -> BookmarkLibraryProjection {
    let selected_pane = match input.scope {
        BookmarkLibraryScope::Mine => match input.selected_filter {
            BookmarkLibraryFilter::Articles => BookmarkLibraryPane::Articles,
            BookmarkLibraryFilter::Collections => BookmarkLibraryPane::Collections,
            BookmarkLibraryFilter::Web => BookmarkLibraryPane::Web,
        },
        BookmarkLibraryScope::Explore => BookmarkLibraryPane::Explore,
    };
    let item_count = match selected_pane {
        BookmarkLibraryPane::Articles => input.article_count,
        BookmarkLibraryPane::Collections => input.collection_count,
        BookmarkLibraryPane::Web => input.web_bookmark_count,
        BookmarkLibraryPane::Explore => input.explore_count,
    };
    let empty = bookmark_library_empty_state(selected_pane);

    BookmarkLibraryProjection {
        scope_options: vec![
            BookmarkLibraryScopeOptionProjection {
                scope: BookmarkLibraryScope::Mine,
                label: "Mine".into(),
            },
            BookmarkLibraryScopeOptionProjection {
                scope: BookmarkLibraryScope::Explore,
                label: "Explore".into(),
            },
        ],
        filter_chips: vec![
            bookmark_library_filter_chip(BookmarkLibraryFilter::Articles),
            bookmark_library_filter_chip(BookmarkLibraryFilter::Collections),
            bookmark_library_filter_chip(BookmarkLibraryFilter::Web),
        ],
        selected_pane,
        is_empty: item_count == 0,
        empty_icon_system_name: empty.0.into(),
        empty_title: empty.1.into(),
        empty_message: empty.2.into(),
    }
}

pub fn bookmarked_article_row_projection(
    input: BookmarkedArticleRowProjectionInput,
) -> BookmarkedArticleRowProjection {
    let article = input.article;
    BookmarkedArticleRowProjection {
        title: title_or_untitled(article.title),
        summary: non_empty_string(article.summary),
        image_url: non_empty_string(article.image),
        display_unix_seconds: article.published_at.or(article.created_at),
    }
}

pub fn bookmark_set_row_projection(
    input: BookmarkSetRowProjectionInput,
) -> BookmarkSetRowProjection {
    let record = input.record;
    let item_count = record.article_addresses.len() + record.note_ids.len();
    let is_bookmark_set = record.kind == KIND_BOOKMARK_SETS as u32;
    BookmarkSetRowProjection {
        display_title: bookmark_set_display_title(&record, "Untitled"),
        kind_label: if is_bookmark_set {
            "Bookmarks".to_string()
        } else {
            "Curation".to_string()
        },
        kind_icon_system_name: if is_bookmark_set {
            "bookmark.fill".to_string()
        } else {
            "rectangle.stack.fill".to_string()
        },
        item_count_label: if item_count == 0 {
            None
        } else {
            Some(format!(
                "{item_count} item{}",
                if item_count == 1 { "" } else { "s" }
            ))
        },
    }
}

pub fn bookmark_set_detail_projection(
    input: BookmarkSetDetailProjectionInput,
) -> BookmarkSetDetailProjection {
    BookmarkSetDetailProjection {
        display_title: bookmark_set_display_title(&input.record, "Collection"),
    }
}

/// Project the create-collection sheet. Native shells render controls and pass
/// the returned `submit_title` to the publish action.
pub fn curation_set_create_projection(
    input: CurationSetCreateProjectionInput,
) -> CurationSetCreateProjection {
    let submit_title = input.title.trim().to_string();
    CurationSetCreateProjection {
        can_create: !submit_title.is_empty(),
        submit_title,
    }
}

pub fn web_bookmark_row_projection(
    input: WebBookmarkRowProjectionInput,
) -> WebBookmarkRowProjection {
    let bookmark = input.bookmark;
    WebBookmarkRowProjection {
        display_title: if bookmark.title.is_empty() {
            bookmark.url.clone()
        } else {
            bookmark.title
        },
        host: web_bookmark_host(&bookmark.url),
        description: non_empty_string(bookmark.description),
        display_unix_seconds: bookmark.published_at.or(bookmark.created_at),
    }
}

fn bookmark_library_filter_chip(
    filter: BookmarkLibraryFilter,
) -> BookmarkLibraryFilterChipProjection {
    let (label, icon_system_name) = match filter {
        BookmarkLibraryFilter::Articles => ("Articles", "doc.text"),
        BookmarkLibraryFilter::Collections => ("Collections", "rectangle.stack"),
        BookmarkLibraryFilter::Web => ("Web", "globe"),
    };
    BookmarkLibraryFilterChipProjection {
        filter,
        label: label.into(),
        icon_system_name: icon_system_name.into(),
    }
}

fn bookmark_library_empty_state(
    pane: BookmarkLibraryPane,
) -> (&'static str, &'static str, &'static str) {
    match pane {
        BookmarkLibraryPane::Articles => (
            "bookmark",
            "No bookmarks yet",
            "Save articles from anywhere in Highlighter to find them here.",
        ),
        BookmarkLibraryPane::Collections => (
            "rectangle.stack",
            "No collections yet",
            "Create bookmark or curation sets to organise your saved content.",
        ),
        BookmarkLibraryPane::Web => (
            "globe",
            "No web bookmarks yet",
            "Web pages you bookmark via Nostr will appear here.",
        ),
        BookmarkLibraryPane::Explore => (
            "rectangle.stack",
            "Nothing to explore",
            "People you follow haven't created any curation sets yet.",
        ),
    }
}

/// Keep only curation sets that can render a visible Explore row.
///
/// Note-backed sets are visible even when no article is cached. Address-only
/// sets need at least one locally resolvable NIP-23 article; malformed,
/// uncached, or storage-failed address sets are hidden, matching the previous
/// iOS Explore behavior without making Swift fan out per set.
pub fn explorable_curation_sets(ndb: &Ndb, sets: Vec<BookmarkSetRecord>) -> Vec<BookmarkSetRecord> {
    filter_explorable_curation_sets(sets, |set| {
        match articles::query_articles_for_addresses(ndb, &set.article_addresses) {
            Ok(articles) => !articles.is_empty(),
            Err(error) => {
                tracing::warn!(
                    set_id = %set.id,
                    error = %error,
                    "failed to resolve curation set articles"
                );
                false
            }
        }
    })
}

fn filter_explorable_curation_sets<F>(
    sets: Vec<BookmarkSetRecord>,
    mut resolves_article: F,
) -> Vec<BookmarkSetRecord>
where
    F: FnMut(&BookmarkSetRecord) -> bool,
{
    sets.into_iter()
        .filter(|set| {
            !set.note_ids.is_empty() || (!set.article_addresses.is_empty() && resolves_article(set))
        })
        .collect()
}

fn curation_set_display_title(set: &BookmarkSetRecord) -> String {
    bookmark_set_display_title(set, "Untitled")
}

fn bookmark_set_display_title(set: &BookmarkSetRecord, empty_fallback: &str) -> String {
    if !set.title.is_empty() {
        return set.title.clone();
    }
    if !set.id.is_empty() {
        return set.id.clone();
    }
    empty_fallback.to_string()
}

fn title_or_untitled(title: String) -> String {
    if title.is_empty() {
        "Untitled".to_string()
    } else {
        title
    }
}

fn non_empty_string(value: String) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn web_bookmark_host(url: &str) -> Option<String> {
    Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(str::to_string))
}

// -- Publish API (curation sets) --------------------------------------------

/// Create a brand-new empty kind:30004 curation set authored by the
/// current user. `title` is the user-supplied display name; description /
/// image stay empty (those are layered later via the editor). Returns the
/// freshly published record so the caller can optimistically insert it
/// into a list and immediately use its `id` (d-tag) for further edits.
pub async fn create_curation_set(
    runtime: &NostrRuntime,
    user_hex: &str,
    title: &str,
    clock: &dyn Clock,
) -> Result<BookmarkSetRecord, CoreError> {
    let title = title.trim();
    if title.is_empty() {
        return Err(CoreError::InvalidInput(
            "collection title must not be empty".into(),
        ));
    }

    // Stable identifier — UNIX nanoseconds, unique-per-user since each
    // author generates their own. NIP-33 only requires uniqueness within
    // the (author, d-tag) keyspace, not globally.
    let nanos = clock.now_unix_nanos();
    let d_tag = format!("c-{nanos:x}");

    let tags = vec![
        Tag::parse(vec!["d".to_string(), d_tag.clone()])
            .map_err(|e| CoreError::Other(format!("build d tag: {e}")))?,
        Tag::parse(vec!["title".to_string(), title.to_string()])
            .map_err(|e| CoreError::Other(format!("build title tag: {e}")))?,
    ];

    let builder = EventBuilder::new(Kind::Custom(KIND_CURATION_SETS), "").tags(tags);
    let client = runtime.client();
    let event = client
        .sign_event_builder(builder)
        .await
        .map_err(|e| CoreError::Signer(format!("sign curation set: {e}")))?;
    client
        .send_event(&event)
        .await
        .map_err(|e| CoreError::Relay(format!("publish curation set: {e}")))?;

    let _ = user_hex; // unused — pubkey comes from the signer
    Ok(BookmarkSetRecord {
        id: d_tag,
        pubkey: event.pubkey.to_hex(),
        kind: KIND_CURATION_SETS as u32,
        title: title.to_string(),
        description: String::new(),
        image: String::new(),
        article_addresses: Vec::new(),
        note_ids: Vec::new(),
        created_at: Some(event.created_at.as_secs()),
    })
}

/// Toggle an `a`-tag (NIP-33 article address) in the curation set keyed
/// by `(user_hex, d_tag)`. Reads the newest cached version, mutates the
/// membership, re-publishes the full set preserving every other tag.
/// Returns the new membership state.
pub async fn toggle_address_in_curation_set(
    runtime: &NostrRuntime,
    user_hex: &str,
    d_tag: &str,
    address: &str,
) -> Result<bool, CoreError> {
    update_address_in_curation_set(
        runtime,
        user_hex,
        d_tag,
        address,
        CurationMembershipChange::Toggle,
    )
    .await
}

/// Idempotently add or remove an `a`-tag (NIP-33 article address) from
/// the curation set keyed by `(user_hex, d_tag)`. Reads the newest
/// cached version, mutates the membership, re-publishes the full set
/// preserving every other tag. Returns the new membership state.
///
/// `member == true` ensures the address is present; `member == false`
/// ensures it's absent. No-op if already in the desired state — still
/// returns the current state without re-publishing.
pub async fn set_address_in_curation_set(
    runtime: &NostrRuntime,
    user_hex: &str,
    d_tag: &str,
    address: &str,
    member: bool,
) -> Result<bool, CoreError> {
    update_address_in_curation_set(
        runtime,
        user_hex,
        d_tag,
        address,
        CurationMembershipChange::Set(member),
    )
    .await
}

#[derive(Clone, Copy)]
enum CurationMembershipChange {
    Set(bool),
    Toggle,
}

async fn update_address_in_curation_set(
    runtime: &NostrRuntime,
    user_hex: &str,
    d_tag: &str,
    address: &str,
    change: CurationMembershipChange,
) -> Result<bool, CoreError> {
    let d_tag = d_tag.trim();
    let address = address.trim();
    if d_tag.is_empty() {
        return Err(CoreError::InvalidInput(
            "curation d-tag must not be empty".into(),
        ));
    }
    if address.is_empty() {
        return Err(CoreError::InvalidInput("address must not be empty".into()));
    }

    let event = newest_set_event(runtime.ndb(), user_hex, KIND_CURATION_SETS, d_tag)?
        .ok_or_else(|| CoreError::Other(format!("curation set not found: {d_tag}")))?;

    // Walk the existing tags so we can preserve everything we don't
    // touch (description, image, e-tags, custom tags from other
    // clients). We rebuild the `a`-tag list with the membership flip.
    let mut a_addresses: Vec<String> = Vec::new();
    let mut other_tags: Vec<Vec<String>> = Vec::new();
    for tag in event.tags.iter() {
        let s = tag.as_slice();
        match s.first().map(String::as_str) {
            Some("a") => {
                if let Some(v) = s.get(1) {
                    a_addresses.push(v.clone());
                }
            }
            _ => other_tags.push(s.to_vec()),
        }
    }

    let was_present = a_addresses.iter().any(|a| a == address);
    let next_member = next_curation_address_membership(was_present, change);
    if was_present == next_member {
        return Ok(next_member);
    }
    if next_member {
        a_addresses.push(address.to_string());
    } else {
        a_addresses.retain(|a| a != address);
    }

    let mut tags: Vec<Tag> = Vec::with_capacity(other_tags.len() + a_addresses.len());
    for raw in other_tags {
        if let Ok(t) = Tag::parse(raw) {
            tags.push(t);
        }
    }
    for addr in &a_addresses {
        tags.push(
            Tag::parse(vec!["a".to_string(), addr.clone()])
                .map_err(|e| CoreError::Other(format!("build a tag: {e}")))?,
        );
    }

    let builder =
        EventBuilder::new(Kind::Custom(KIND_CURATION_SETS), event.content.clone()).tags(tags);
    let client = runtime.client();
    let new_event = client
        .sign_event_builder(builder)
        .await
        .map_err(|e| CoreError::Signer(format!("sign curation set: {e}")))?;
    client
        .send_event(&new_event)
        .await
        .map_err(|e| CoreError::Relay(format!("publish curation set: {e}")))?;

    Ok(next_member)
}

fn next_curation_address_membership(was_present: bool, change: CurationMembershipChange) -> bool {
    match change {
        CurationMembershipChange::Set(member) => member,
        CurationMembershipChange::Toggle => !was_present,
    }
}

/// Read the newest cached event for `(user_hex, kind, d_tag)`. Used by
/// the publish path to do read-modify-write without round-tripping a
/// relay first.
fn newest_set_event(
    ndb: &Ndb,
    user_hex: &str,
    kind: u16,
    d_tag: &str,
) -> Result<Option<Event>, CoreError> {
    let author = PublicKey::from_hex(user_hex)
        .map_err(|e| CoreError::InvalidInput(format!("invalid pubkey: {e}")))?;
    let pk_bytes: [u8; 32] = author.to_bytes();

    let txn = Transaction::new(ndb).map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;
    let filter = NdbFilter::new()
        .kinds([kind as u64])
        .authors([&pk_bytes])
        .tags([d_tag], 'd')
        .build();
    let results = ndb
        .query(&txn, &[filter], 16)
        .map_err(|e| CoreError::Cache(format!("query set: {e}")))?;

    let mut newest: Option<Event> = None;
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        newest = Some(match newest {
            Some(prev) if prev.created_at >= event.created_at => prev,
            _ => event,
        });
    }
    Ok(newest)
}

// -- Parsing -----------------------------------------------------------------

fn parse_set_event(event: Event, kind: u16) -> BookmarkSetRecord {
    let mut article_addresses = Vec::new();
    let mut note_ids = Vec::new();

    for tag in event.tags.iter() {
        let s = tag.as_slice();
        match s.first().map(String::as_str) {
            Some("a") => {
                if let Some(v) = s.get(1) {
                    article_addresses.push(v.clone());
                }
            }
            Some("e") => {
                if let Some(v) = s.get(1) {
                    note_ids.push(v.clone());
                }
            }
            _ => {}
        }
    }

    BookmarkSetRecord {
        id: first_tag_value(&event, "d").unwrap_or("").to_string(),
        pubkey: event.pubkey.to_hex(),
        kind: kind as u32,
        title: first_tag_value(&event, "title").unwrap_or("").to_string(),
        description: first_tag_value(&event, "description")
            .unwrap_or("")
            .to_string(),
        image: first_tag_value(&event, "image").unwrap_or("").to_string(),
        article_addresses,
        note_ids,
        created_at: Some(event.created_at.as_secs()),
    }
}

fn parse_web_bookmark_event(event: Event) -> WebBookmarkRecord {
    let d = first_tag_value(&event, "d").unwrap_or("").to_string();
    let url = if d.is_empty() {
        String::new()
    } else {
        format!("https://{d}")
    };

    let topics: Vec<String> = event
        .tags
        .iter()
        .filter_map(|tag| {
            let s = tag.as_slice();
            if s.first().map(String::as_str) == Some("t") {
                s.get(1).cloned()
            } else {
                None
            }
        })
        .collect();

    let published_at = first_tag_value(&event, "published_at").and_then(|v| v.parse::<u64>().ok());

    WebBookmarkRecord {
        url,
        pubkey: event.pubkey.to_hex(),
        title: first_tag_value(&event, "title").unwrap_or("").to_string(),
        description: event.content.clone(),
        topics,
        published_at,
        created_at: Some(event.created_at.as_secs()),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        bookmark_library_projection, bookmark_set_detail_projection, bookmark_set_row_projection,
        bookmarked_article_row_projection, curation_menu_items_for_address,
        curation_set_create_projection, filter_explorable_curation_sets,
        next_curation_address_membership, query_bookmark_library_snapshot,
        web_bookmark_row_projection, BookmarkLibraryFilter, BookmarkLibraryFilterChipProjection,
        BookmarkLibraryPane, BookmarkLibraryProjectionInput, BookmarkLibraryScope,
        BookmarkLibraryScopeOptionProjection, BookmarkSetDetailProjectionInput,
        BookmarkSetRowProjectionInput, BookmarkedArticleRowProjectionInput,
        CurationMembershipChange, CurationSetCreateProjectionInput, WebBookmarkRowProjectionInput,
        KIND_BOOKMARK_SETS, KIND_CURATION_SETS, KIND_WEB_BOOKMARK,
    };
    use crate::models::{ArticleRecord, BookmarkSetRecord, WebBookmarkRecord};
    use nostr_sdk::prelude::*;
    use nostrdb::{Config, Ndb};
    use tempfile::TempDir;

    fn fresh_ndb() -> (Ndb, TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = Config::new();
        let ndb = Ndb::new(tmp.path().to_str().unwrap(), &cfg).unwrap();
        (ndb, tmp)
    }

    fn process(ndb: &Ndb, event: &Event) {
        let line = format!("[\"EVENT\",\"sub\",{}]", event.as_json());
        ndb.process_event(&line).unwrap();
    }

    fn set(id: &str, title: &str, article_addresses: Vec<&str>) -> BookmarkSetRecord {
        set_with_notes(id, title, article_addresses, Vec::new())
    }

    fn set_with_notes(
        id: &str,
        title: &str,
        article_addresses: Vec<&str>,
        note_ids: Vec<&str>,
    ) -> BookmarkSetRecord {
        BookmarkSetRecord {
            id: id.to_string(),
            pubkey: "author".to_string(),
            kind: KIND_CURATION_SETS as u32,
            title: title.to_string(),
            description: String::new(),
            image: String::new(),
            article_addresses: article_addresses.into_iter().map(str::to_string).collect(),
            note_ids: note_ids.into_iter().map(str::to_string).collect(),
            created_at: Some(1),
        }
    }

    #[test]
    fn curation_membership_toggle_flips_current_state() {
        assert!(next_curation_address_membership(
            false,
            CurationMembershipChange::Toggle
        ));
        assert!(!next_curation_address_membership(
            true,
            CurationMembershipChange::Toggle
        ));
    }

    #[test]
    fn curation_membership_set_uses_requested_state() {
        assert!(next_curation_address_membership(
            false,
            CurationMembershipChange::Set(true)
        ));
        assert!(next_curation_address_membership(
            true,
            CurationMembershipChange::Set(true)
        ));
        assert!(!next_curation_address_membership(
            false,
            CurationMembershipChange::Set(false)
        ));
        assert!(!next_curation_address_membership(
            true,
            CurationMembershipChange::Set(false)
        ));
    }

    #[test]
    fn curation_menu_items_project_exact_membership() {
        let items = curation_menu_items_for_address(
            vec![
                set("first", "First", vec!["30023:abc:one"]),
                set("second", "Second", vec!["30023:abc:one-two"]),
            ],
            "30023:abc:one",
        );

        assert_eq!(items.len(), 2);
        assert!(items[0].is_member);
        assert!(!items[1].is_member);
    }

    #[test]
    fn curation_menu_items_apply_title_fallbacks() {
        let items = curation_menu_items_for_address(
            vec![
                set("with-id", "", Vec::new()),
                set("", "", Vec::new()),
                set("with-title", "Named", Vec::new()),
            ],
            "",
        );

        assert_eq!(items[0].title, "with-id");
        assert_eq!(items[1].title, "Untitled");
        assert_eq!(items[2].title, "Named");
        assert!(items.iter().all(|item| !item.is_member));
    }

    #[test]
    fn bookmark_library_projection_projects_mine_articles_chrome() {
        let projection = bookmark_library_projection(BookmarkLibraryProjectionInput {
            scope: BookmarkLibraryScope::Mine,
            selected_filter: BookmarkLibraryFilter::Articles,
            article_count: 0,
            collection_count: 2,
            web_bookmark_count: 3,
            explore_count: 4,
        });

        assert_eq!(projection.selected_pane, BookmarkLibraryPane::Articles);
        assert!(projection.is_empty);
        assert_eq!(
            projection.scope_options,
            vec![
                BookmarkLibraryScopeOptionProjection {
                    scope: BookmarkLibraryScope::Mine,
                    label: "Mine".into()
                },
                BookmarkLibraryScopeOptionProjection {
                    scope: BookmarkLibraryScope::Explore,
                    label: "Explore".into()
                },
            ]
        );
        assert_eq!(
            projection.filter_chips,
            vec![
                BookmarkLibraryFilterChipProjection {
                    filter: BookmarkLibraryFilter::Articles,
                    label: "Articles".into(),
                    icon_system_name: "doc.text".into()
                },
                BookmarkLibraryFilterChipProjection {
                    filter: BookmarkLibraryFilter::Collections,
                    label: "Collections".into(),
                    icon_system_name: "rectangle.stack".into()
                },
                BookmarkLibraryFilterChipProjection {
                    filter: BookmarkLibraryFilter::Web,
                    label: "Web".into(),
                    icon_system_name: "globe".into()
                },
            ]
        );
        assert_eq!(projection.empty_icon_system_name, "bookmark");
        assert_eq!(projection.empty_title, "No bookmarks yet");
        assert_eq!(
            projection.empty_message,
            "Save articles from anywhere in Highlighter to find them here."
        );
    }

    #[test]
    fn bookmark_library_projection_projects_mine_selected_counts() {
        let collections = bookmark_library_projection(BookmarkLibraryProjectionInput {
            scope: BookmarkLibraryScope::Mine,
            selected_filter: BookmarkLibraryFilter::Collections,
            article_count: 0,
            collection_count: 1,
            web_bookmark_count: 0,
            explore_count: 0,
        });
        let web = bookmark_library_projection(BookmarkLibraryProjectionInput {
            scope: BookmarkLibraryScope::Mine,
            selected_filter: BookmarkLibraryFilter::Web,
            article_count: 0,
            collection_count: 0,
            web_bookmark_count: 0,
            explore_count: 0,
        });

        assert_eq!(collections.selected_pane, BookmarkLibraryPane::Collections);
        assert!(!collections.is_empty);
        assert_eq!(collections.empty_icon_system_name, "rectangle.stack");
        assert_eq!(collections.empty_title, "No collections yet");

        assert_eq!(web.selected_pane, BookmarkLibraryPane::Web);
        assert!(web.is_empty);
        assert_eq!(web.empty_icon_system_name, "globe");
        assert_eq!(web.empty_title, "No web bookmarks yet");
        assert_eq!(
            web.empty_message,
            "Web pages you bookmark via Nostr will appear here."
        );
    }

    #[test]
    fn bookmark_library_projection_explore_overrides_selected_filter() {
        let projection = bookmark_library_projection(BookmarkLibraryProjectionInput {
            scope: BookmarkLibraryScope::Explore,
            selected_filter: BookmarkLibraryFilter::Articles,
            article_count: 10,
            collection_count: 10,
            web_bookmark_count: 10,
            explore_count: 0,
        });

        assert_eq!(projection.selected_pane, BookmarkLibraryPane::Explore);
        assert!(projection.is_empty);
        assert_eq!(projection.empty_icon_system_name, "rectangle.stack");
        assert_eq!(projection.empty_title, "Nothing to explore");
        assert_eq!(
            projection.empty_message,
            "People you follow haven't created any curation sets yet."
        );
    }

    #[test]
    fn bookmark_library_snapshot_reads_user_sections() {
        let (ndb, _tmp) = fresh_ndb();
        let user = Keys::generate();
        let address = format!("30023:{}:essay", user.public_key().to_hex());

        let article = EventBuilder::new(Kind::Custom(crate::articles::KIND_LONG_FORM), "body")
            .tags([
                Tag::parse(vec!["d".to_string(), "essay".to_string()]).unwrap(),
                Tag::parse(vec!["title".to_string(), "Saved Essay".to_string()]).unwrap(),
            ])
            .sign_with_keys(&user)
            .unwrap();
        let bookmarks = EventBuilder::new(Kind::Custom(crate::bookmarks::KIND_BOOKMARKS), "")
            .tags([Tag::parse(vec!["a".to_string(), address]).unwrap()])
            .sign_with_keys(&user)
            .unwrap();
        let bookmark_set = EventBuilder::new(Kind::Custom(KIND_BOOKMARK_SETS), "")
            .tags([
                Tag::parse(vec!["d".to_string(), "saved".to_string()]).unwrap(),
                Tag::parse(vec!["title".to_string(), "Saved Set".to_string()]).unwrap(),
            ])
            .sign_with_keys(&user)
            .unwrap();
        let curation_set = EventBuilder::new(Kind::Custom(KIND_CURATION_SETS), "")
            .tags([
                Tag::parse(vec!["d".to_string(), "curated".to_string()]).unwrap(),
                Tag::parse(vec!["title".to_string(), "Curated Set".to_string()]).unwrap(),
            ])
            .sign_with_keys(&user)
            .unwrap();
        let web_bookmark = EventBuilder::new(Kind::Custom(KIND_WEB_BOOKMARK), "Page summary")
            .tags([
                Tag::parse(vec!["d".to_string(), "example.com/page".to_string()]).unwrap(),
                Tag::parse(vec!["title".to_string(), "Example Page".to_string()]).unwrap(),
            ])
            .sign_with_keys(&user)
            .unwrap();

        for event in [
            &article,
            &bookmarks,
            &bookmark_set,
            &curation_set,
            &web_bookmark,
        ] {
            process(&ndb, event);
        }
        std::thread::sleep(std::time::Duration::from_millis(50));

        let snapshot = query_bookmark_library_snapshot(&ndb, &user.public_key().to_hex());
        assert_eq!(snapshot.my_articles.len(), 1);
        assert_eq!(snapshot.my_articles[0].title, "Saved Essay");
        assert_eq!(snapshot.my_bookmark_sets.len(), 1);
        assert_eq!(snapshot.my_bookmark_sets[0].title, "Saved Set");
        assert_eq!(snapshot.my_curation_sets.len(), 1);
        assert_eq!(snapshot.my_curation_sets[0].title, "Curated Set");
        assert_eq!(snapshot.my_web_bookmarks.len(), 1);
        assert_eq!(snapshot.my_web_bookmarks[0].title, "Example Page");
        assert!(snapshot.following_curation_sets.is_empty());
    }

    #[test]
    fn bookmark_article_rows_project_display_fallbacks() {
        let projection = bookmarked_article_row_projection(BookmarkedArticleRowProjectionInput {
            article: article("", "", "", None, Some(42)),
        });

        assert_eq!(projection.title, "Untitled");
        assert_eq!(projection.summary, None);
        assert_eq!(projection.image_url, None);
        assert_eq!(projection.display_unix_seconds, Some(42));

        let projection = bookmarked_article_row_projection(BookmarkedArticleRowProjectionInput {
            article: article(
                "Essay",
                "Summary",
                "https://img.example/essay.jpg",
                Some(100),
                Some(42),
            ),
        });

        assert_eq!(projection.title, "Essay");
        assert_eq!(projection.summary, Some("Summary".into()));
        assert_eq!(
            projection.image_url,
            Some("https://img.example/essay.jpg".into())
        );
        assert_eq!(projection.display_unix_seconds, Some(100));
    }

    #[test]
    fn bookmark_set_rows_project_title_kind_and_count() {
        let mut record = set("with-id", "", vec!["30023:author:one"]);
        record.kind = KIND_BOOKMARK_SETS as u32;

        let projection = bookmark_set_row_projection(BookmarkSetRowProjectionInput { record });

        assert_eq!(projection.display_title, "with-id");
        assert_eq!(projection.kind_label, "Bookmarks");
        assert_eq!(projection.kind_icon_system_name, "bookmark.fill");
        assert_eq!(projection.item_count_label, Some("1 item".into()));

        let mut record = set_with_notes("", "", Vec::new(), vec!["note-1", "note-2"]);
        record.kind = KIND_CURATION_SETS as u32;

        let projection = bookmark_set_row_projection(BookmarkSetRowProjectionInput { record });

        assert_eq!(projection.display_title, "Untitled");
        assert_eq!(projection.kind_label, "Curation");
        assert_eq!(projection.kind_icon_system_name, "rectangle.stack.fill");
        assert_eq!(projection.item_count_label, Some("2 items".into()));
    }

    #[test]
    fn bookmark_set_detail_uses_collection_empty_fallback() {
        let projection = bookmark_set_detail_projection(BookmarkSetDetailProjectionInput {
            record: set("", "", Vec::new()),
        });

        assert_eq!(projection.display_title, "Collection");
    }

    #[test]
    fn curation_set_create_projection_trims_and_requires_title() {
        let projection = curation_set_create_projection(CurationSetCreateProjectionInput {
            title: "  Essays  ".into(),
        });
        let blank = curation_set_create_projection(CurationSetCreateProjectionInput {
            title: " \n ".into(),
        });

        assert_eq!(projection.submit_title, "Essays");
        assert!(projection.can_create);
        assert_eq!(blank.submit_title, "");
        assert!(!blank.can_create);
    }

    #[test]
    fn web_bookmark_rows_project_title_host_description_and_date() {
        let projection = web_bookmark_row_projection(WebBookmarkRowProjectionInput {
            bookmark: web_bookmark("", "", "https://example.com/path", Some(5), Some(4)),
        });

        assert_eq!(projection.display_title, "https://example.com/path");
        assert_eq!(projection.host, Some("example.com".into()));
        assert_eq!(projection.description, None);
        assert_eq!(projection.display_unix_seconds, Some(5));

        let projection = web_bookmark_row_projection(WebBookmarkRowProjectionInput {
            bookmark: web_bookmark("Page title", "Description", "not a url", None, Some(4)),
        });

        assert_eq!(projection.display_title, "Page title");
        assert_eq!(projection.host, None);
        assert_eq!(projection.description, Some("Description".into()));
        assert_eq!(projection.display_unix_seconds, Some(4));
    }

    #[test]
    fn explorable_curation_sets_keep_only_visible_explore_rows() {
        let sets = vec![
            set("empty", "Empty", Vec::new()),
            set("unresolved", "Unresolved", vec!["30023:author:missing"]),
            set_with_notes("notes", "Notes", Vec::new(), vec!["note-id"]),
            set("article", "Article", vec!["30023:author:cached"]),
        ];

        let rows = filter_explorable_curation_sets(sets, |set| set.id == "article");
        let ids = rows.into_iter().map(|set| set.id).collect::<Vec<_>>();

        assert_eq!(ids, vec!["notes", "article"]);
    }

    fn article(
        title: &str,
        summary: &str,
        image: &str,
        published_at: Option<u64>,
        created_at: Option<u64>,
    ) -> ArticleRecord {
        ArticleRecord {
            event_id: "article".into(),
            address: "30023:author:d".into(),
            pubkey: "author".into(),
            identifier: "d".into(),
            title: title.into(),
            summary: summary.into(),
            image: image.into(),
            content: String::new(),
            hashtags: Vec::new(),
            published_at,
            created_at,
        }
    }

    fn web_bookmark(
        title: &str,
        description: &str,
        url: &str,
        published_at: Option<u64>,
        created_at: Option<u64>,
    ) -> WebBookmarkRecord {
        WebBookmarkRecord {
            url: url.into(),
            pubkey: "author".into(),
            title: title.into(),
            description: description.into(),
            topics: Vec::new(),
            published_at,
            created_at,
        }
    }
}
