//! Search across locally cached nostrdb content and (via NIP-50) across
//! relay-hosted long-form articles.
//!
//! Local scans are synchronous ndb reads — cheap, case-insensitive substring
//! matches over the fields a user would reasonably search for:
//!
//! - kind:9802 highlights — quote + note
//! - kind:30023 articles — title + summary + hashtags
//! - kind:39000 communities — name + about
//! - kind:0 profiles — name + display_name + nip05
//!
//! Relay-side search is a `SubscriptionKind::SearchArticles` in
//! `subscriptions.rs`; this module only provides the local reads and the
//! helper that resolves the user's kind:10007 NIP-51 search relay list
//! (merged with `wss://relay.highlighter.com` as a default).

use std::collections::{BTreeMap, HashSet};

use nostr_sdk::prelude::*;
use nostrdb::{Filter as NdbFilter, Ndb, Transaction};

use crate::articles::KIND_LONG_FORM;
use crate::errors::CoreError;
use crate::groups::KIND_GROUP_METADATA;
use crate::models::{
    ArticleReaderRoute, ArticleRecord, CommunitySummary, HighlightRecord, ProfileMetadata,
};
use crate::profile;
use crate::relays::highlighter_relay;

/// NIP-51 kind for the user's curated list of search relays.
pub const KIND_SEARCH_RELAYS: u16 = 10007;
/// kind:9802 NIP-84 highlight.
const KIND_HIGHLIGHT: u16 = 9802;
/// kind:0 NIP-01 profile metadata.
const KIND_METADATA: u16 = 0;

/// Main search screen section limits. Native shells render these buckets but
/// do not choose limits or per-section fallback policy.
pub const SEARCH_HIGHLIGHT_RESULTS_LIMIT: u32 = 30;
pub const SEARCH_ARTICLE_RESULTS_LIMIT: u32 = 30;
pub const SEARCH_COMMUNITY_RESULTS_LIMIT: u32 = 20;
pub const SEARCH_PROFILE_RESULTS_LIMIT: u32 = 20;

#[derive(Debug, Clone, uniffi::Record)]
pub struct SearchResultsSnapshot {
    pub highlights: Vec<HighlightRecord>,
    pub articles: Vec<ArticleRecord>,
    pub communities: Vec<CommunitySummary>,
    pub profiles: Vec<ProfileMetadata>,
}

impl SearchResultsSnapshot {
    fn empty() -> Self {
        Self {
            highlights: Vec::new(),
            articles: Vec::new(),
            communities: Vec::new(),
            profiles: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct SearchArticleResultsSnapshot {
    pub articles: Vec<ArticleRecord>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct SearchChromeSnapshot {
    pub recent_queries: Vec<String>,
    pub search_relays: Vec<String>,
    pub error: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct SearchQueryProjectionInput {
    pub query: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SearchQueryProjection {
    pub search_query: String,
    pub has_query: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct SearchSuggestionsProjectionInput {
    pub joined_communities: Vec<CommunitySummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SearchSuggestionsProjection {
    pub queries: Vec<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct SearchHighlightRowProjectionInput {
    pub highlight: HighlightRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SearchHighlightRowProjection {
    pub article_route: Option<ArticleReaderRoute>,
    pub page_image_url: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct SearchCommunityRowProjectionInput {
    pub community: CommunitySummary,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SearchCommunityRowProjection {
    pub display_name: String,
    pub about: Option<String>,
    pub visibility_label: String,
    pub access_label: String,
    pub member_count_label: Option<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct SearchTextMatchesProjectionInput {
    pub text: String,
    pub query: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SearchTextMatchSpan {
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SearchTextMatchesProjection {
    pub spans: Vec<SearchTextMatchSpan>,
}

/// How many candidate notes to pull from ndb before filtering. Higher than the
/// final `limit` so substring matches still surface when the candidate set is
/// dominated by non-matching notes.
const LOCAL_SCAN_MULTIPLIER: i32 = 8;
const LOCAL_SCAN_FLOOR: i32 = 256;
const LOCAL_SCAN_CEILING: i32 = 4096;
const EVERGREEN_SUGGESTED_QUERIES: [&str; 5] =
    ["Dostoevsky", "Bitcoin", "Attention", "Borges", "Philosophy"];

pub fn search_query_projection(input: SearchQueryProjectionInput) -> SearchQueryProjection {
    let search_query = input.query.trim().to_string();
    SearchQueryProjection {
        has_query: !search_query.is_empty(),
        search_query,
    }
}

pub fn search_suggestions_projection(
    input: SearchSuggestionsProjectionInput,
) -> SearchSuggestionsProjection {
    let mut queries = Vec::new();
    let mut seen = HashSet::new();
    for community in input.joined_communities.into_iter().take(4) {
        push_suggestion(&mut queries, &mut seen, community.name);
    }
    for fallback in EVERGREEN_SUGGESTED_QUERIES {
        if queries.len() >= 8 {
            break;
        }
        push_suggestion(&mut queries, &mut seen, fallback.to_string());
    }
    queries.truncate(8);
    SearchSuggestionsProjection { queries }
}

fn push_suggestion(queries: &mut Vec<String>, seen: &mut HashSet<String>, value: String) {
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        return;
    }
    if seen.insert(trimmed.to_lowercase()) {
        queries.push(trimmed);
    }
}

pub fn search_highlight_row_projection(
    input: SearchHighlightRowProjectionInput,
) -> SearchHighlightRowProjection {
    SearchHighlightRowProjection {
        article_route: crate::articles::article_reader_route_from_address(
            &input.highlight.artifact_address,
        ),
        page_image_url: page_image_url(&input.highlight.image_url),
    }
}

pub fn search_community_row_projection(
    input: SearchCommunityRowProjectionInput,
) -> SearchCommunityRowProjection {
    let community = input.community;
    SearchCommunityRowProjection {
        display_name: community.name,
        about: non_empty_string(&community.about),
        visibility_label: capitalize_first(&community.visibility),
        access_label: capitalize_first(&community.access),
        member_count_label: community
            .member_count
            .map(|count| format!("{count} members")),
    }
}

pub fn search_text_matches_projection(
    input: SearchTextMatchesProjectionInput,
) -> SearchTextMatchesProjection {
    let query = input.query.trim().to_lowercase();
    if query.is_empty() {
        return SearchTextMatchesProjection { spans: Vec::new() };
    }

    let lower_text = input.text.to_lowercase();
    let mut spans = Vec::new();
    let mut search_start = 0usize;
    while search_start < lower_text.len() {
        let Some(relative_start) = lower_text[search_start..].find(&query) else {
            break;
        };
        let start_byte = search_start + relative_start;
        let end_byte = start_byte + query.len();
        let start = lower_text[..start_byte].chars().count() as u32;
        let end = lower_text[..end_byte].chars().count() as u32;
        if start < end {
            spans.push(SearchTextMatchSpan { start, end });
        }
        search_start = end_byte;
    }

    SearchTextMatchesProjection { spans }
}

fn page_image_url(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let parsed = url::Url::parse(trimmed).ok()?;
    match parsed.scheme() {
        "http" | "https" => Some(trimmed.to_string()),
        _ => None,
    }
}

fn capitalize_first(value: &str) -> String {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    format!(
        "{}{}",
        first.to_uppercase().collect::<String>(),
        chars.as_str()
    )
}

fn non_empty_string(value: &str) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn scan_cap(limit: u32) -> i32 {
    let raw = (limit as i32).saturating_mul(LOCAL_SCAN_MULTIPLIER);
    raw.clamp(LOCAL_SCAN_FLOOR, LOCAL_SCAN_CEILING)
}

/// Case-insensitive `needle in haystack`, ignoring leading/trailing whitespace
/// on the query.
fn contains_ci(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    haystack.to_lowercase().contains(&needle.to_lowercase())
}

// -- Highlights --------------------------------------------------------------

pub fn search_highlights(
    ndb: &Ndb,
    query: &str,
    limit: u32,
) -> Result<Vec<HighlightRecord>, CoreError> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let txn = Transaction::new(ndb).map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;
    let filter = NdbFilter::new().kinds([KIND_HIGHLIGHT as u64]).build();
    let results = ndb
        .query(&txn, &[filter], scan_cap(limit))
        .map_err(|e| CoreError::Cache(format!("query highlights: {e}")))?;

    let mut records: Vec<HighlightRecord> = Vec::new();
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        let note_text = first_tag_value(&event, "comment").unwrap_or("");
        if !(contains_ci(&event.content, q) || contains_ci(note_text, q)) {
            continue;
        }
        if let Some(rec) = highlight_record_from_event(&event) {
            records.push(rec);
        }
    }

    records.sort_by(|a, b| b.created_at.unwrap_or(0).cmp(&a.created_at.unwrap_or(0)));
    records.truncate(limit as usize);
    Ok(records)
}

// -- Articles ----------------------------------------------------------------

pub fn search_articles(
    ndb: &Ndb,
    query: &str,
    limit: u32,
) -> Result<Vec<ArticleRecord>, CoreError> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let txn = Transaction::new(ndb).map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;
    let filter = NdbFilter::new().kinds([KIND_LONG_FORM as u64]).build();
    let results = ndb
        .query(&txn, &[filter], scan_cap(limit))
        .map_err(|e| CoreError::Cache(format!("query articles: {e}")))?;

    // Collect into addressable-event dedupe map (newest per (pubkey, d) wins).
    let mut best_per_addr: BTreeMap<(String, String), Event> = BTreeMap::new();
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        let title = first_tag_value(&event, "title").unwrap_or("");
        let summary = first_tag_value(&event, "summary").unwrap_or("");
        let d_tag = first_tag_value(&event, "d").unwrap_or("");
        let hashtags_match = event.tags.iter().any(|t| {
            let s = t.as_slice();
            s.first().map(String::as_str) == Some("t")
                && s.get(1).map(|v| contains_ci(v, q)).unwrap_or(false)
        });
        if !(contains_ci(title, q) || contains_ci(summary, q) || hashtags_match) {
            continue;
        }
        let key = (event.pubkey.to_hex(), d_tag.to_string());
        match best_per_addr.get(&key) {
            Some(prev) if prev.created_at >= event.created_at => {}
            _ => {
                best_per_addr.insert(key, event);
            }
        }
    }

    let mut records: Vec<ArticleRecord> = best_per_addr
        .into_values()
        .filter_map(|ev| article_record_from_event(&ev))
        .collect();

    records.sort_by(|a, b| {
        b.published_at
            .or(b.created_at)
            .unwrap_or(0)
            .cmp(&a.published_at.or(a.created_at).unwrap_or(0))
    });
    records.truncate(limit as usize);
    Ok(records)
}

// -- Communities -------------------------------------------------------------

pub fn search_communities(
    ndb: &Ndb,
    query: &str,
    limit: u32,
) -> Result<Vec<CommunitySummary>, CoreError> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let txn = Transaction::new(ndb).map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;
    let filter = NdbFilter::new().kinds([KIND_GROUP_METADATA as u64]).build();
    let results = ndb
        .query(&txn, &[filter], scan_cap(limit))
        .map_err(|e| CoreError::Cache(format!("query communities: {e}")))?;

    // Dedupe per `d` tag — kind:39000 is replaceable; newest wins.
    let mut best_per_d: BTreeMap<String, Event> = BTreeMap::new();
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        let name = first_tag_value(&event, "name").unwrap_or("");
        let about = first_tag_value(&event, "about").unwrap_or("");
        let d_tag = first_tag_value(&event, "d").unwrap_or("");
        if !(contains_ci(name, q) || contains_ci(about, q)) {
            continue;
        }
        match best_per_d.get(d_tag) {
            Some(prev) if prev.created_at >= event.created_at => {}
            _ => {
                best_per_d.insert(d_tag.to_string(), event);
            }
        }
    }

    let mut records: Vec<CommunitySummary> = best_per_d
        .into_values()
        .filter_map(|ev| crate::groups::build_community_summary(&ev).ok())
        .filter(crate::groups::is_public_open_room)
        .collect();
    records.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    records.truncate(limit as usize);
    Ok(records)
}

// -- Profiles ----------------------------------------------------------------

pub fn search_profiles(
    ndb: &Ndb,
    query: &str,
    limit: u32,
) -> Result<Vec<ProfileMetadata>, CoreError> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let txn = Transaction::new(ndb).map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;
    let filter = NdbFilter::new().kinds([KIND_METADATA as u64]).build();
    let results = ndb
        .query(&txn, &[filter], scan_cap(limit))
        .map_err(|e| CoreError::Cache(format!("query profiles: {e}")))?;

    // Dedupe per author — kind:0 is replaceable; newest wins.
    let mut best_per_author: BTreeMap<String, Event> = BTreeMap::new();
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        let author = event.pubkey.to_hex();
        match best_per_author.get(&author) {
            Some(prev) if prev.created_at >= event.created_at => {}
            _ => {
                best_per_author.insert(author, event);
            }
        }
    }

    let mut records: Vec<ProfileMetadata> = Vec::new();
    for event in best_per_author.into_values() {
        let meta = profile::parse_metadata(&event);
        let matches = contains_ci(&meta.name, q)
            || contains_ci(&meta.display_name, q)
            || contains_ci(&meta.nip05, q)
            || contains_ci(&meta.about, q);
        if matches {
            records.push(meta);
        }
    }

    // Rank by "does the name start with the query" first, then alphabetical.
    let q_lower = q.to_lowercase();
    records.sort_by(|a, b| {
        let a_prefix = a.display_name.to_lowercase().starts_with(&q_lower)
            || a.name.to_lowercase().starts_with(&q_lower);
        let b_prefix = b.display_name.to_lowercase().starts_with(&q_lower)
            || b.name.to_lowercase().starts_with(&q_lower);
        match (a_prefix, b_prefix) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => {
                let a_label = primary_label(a).to_lowercase();
                let b_label = primary_label(b).to_lowercase();
                a_label.cmp(&b_label)
            }
        }
    });
    records.truncate(limit as usize);
    Ok(records)
}

pub fn search_results_snapshot(ndb: &Ndb, query: &str) -> SearchResultsSnapshot {
    let q = query.trim();
    if q.is_empty() {
        return SearchResultsSnapshot::empty();
    }

    SearchResultsSnapshot {
        highlights: search_section_or_empty(
            "highlights",
            search_highlights(ndb, q, SEARCH_HIGHLIGHT_RESULTS_LIMIT),
        ),
        articles: search_section_or_empty(
            "articles",
            search_articles(ndb, q, SEARCH_ARTICLE_RESULTS_LIMIT),
        ),
        communities: search_section_or_empty(
            "communities",
            search_communities(ndb, q, SEARCH_COMMUNITY_RESULTS_LIMIT),
        ),
        profiles: search_section_or_empty(
            "profiles",
            search_profiles(ndb, q, SEARCH_PROFILE_RESULTS_LIMIT),
        ),
    }
}

pub fn search_article_results_snapshot(ndb: &Ndb, query: &str) -> SearchArticleResultsSnapshot {
    let q = query.trim();
    if q.is_empty() {
        return SearchArticleResultsSnapshot {
            articles: Vec::new(),
        };
    }

    SearchArticleResultsSnapshot {
        articles: search_section_or_empty(
            "articles",
            search_articles(ndb, q, SEARCH_ARTICLE_RESULTS_LIMIT),
        ),
    }
}

pub fn search_chrome_snapshot(
    recent_queries: Vec<String>,
    search_relays: Vec<String>,
    error: impl ToString,
) -> SearchChromeSnapshot {
    SearchChromeSnapshot {
        recent_queries,
        search_relays,
        error: error.to_string(),
    }
}

fn search_section_or_empty<T>(section: &'static str, result: Result<Vec<T>, CoreError>) -> Vec<T> {
    match result {
        Ok(values) => values,
        Err(error) => {
            tracing::warn!(section, error = %error, "search snapshot section failed");
            Vec::new()
        }
    }
}

fn primary_label(p: &ProfileMetadata) -> &str {
    if !p.display_name.is_empty() {
        &p.display_name
    } else if !p.name.is_empty() {
        &p.name
    } else {
        &p.nip05
    }
}

// -- NIP-51 kind:10007 search relays ----------------------------------------

/// Resolve the set of relays to hit with NIP-50 queries. Always includes
/// `wss://relay.highlighter.com` (the app's default search host); additionally
/// includes every `relay` tag from the newest cached kind:10007 for `user_hex`.
/// Output is deduped, order-preserving (default first, then user list in tag
/// order).
pub fn query_search_relays(ndb: &Ndb, user_hex: &str) -> Result<Vec<String>, CoreError> {
    let mut out: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    let push = |url: String, out: &mut Vec<String>, seen: &mut HashSet<String>| {
        let trimmed = url.trim().trim_end_matches('/').to_string();
        if trimmed.is_empty() {
            return;
        }
        if seen.insert(trimmed.clone()) {
            out.push(trimmed);
        }
    };

    push(highlighter_relay().to_string(), &mut out, &mut seen);

    if user_hex.is_empty() {
        return Ok(out);
    }

    let Ok(author) = PublicKey::from_hex(user_hex) else {
        return Ok(out);
    };
    let txn = Transaction::new(ndb).map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;
    let pk_bytes: [u8; 32] = author.to_bytes();
    let filter = NdbFilter::new()
        .kinds([KIND_SEARCH_RELAYS as u64])
        .authors([&pk_bytes])
        .build();
    let results = ndb
        .query(&txn, &[filter], 8)
        .map_err(|e| CoreError::Cache(format!("query search relays: {e}")))?;

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

    if let Some(event) = newest {
        for tag in event.tags.iter() {
            let s = tag.as_slice();
            if s.first().map(String::as_str) != Some("relay") {
                continue;
            }
            if let Some(url) = s.get(1) {
                push(url.to_string(), &mut out, &mut seen);
            }
        }
    }

    Ok(out)
}

// -- Event → record helpers --------------------------------------------------

fn first_tag_value<'a>(event: &'a Event, name: &str) -> Option<&'a str> {
    for tag in event.tags.iter() {
        let slice = tag.as_slice();
        if slice.first().map(String::as_str) == Some(name) {
            return slice.get(1).map(String::as_str);
        }
    }
    None
}

fn highlight_record_from_event(event: &Event) -> Option<HighlightRecord> {
    let quote = event.content.clone();
    let context = first_tag_value(event, "context").unwrap_or("").to_string();
    let comment = first_tag_value(event, "comment").unwrap_or("").to_string();
    let artifact_address = first_tag_value(event, "a").unwrap_or("").to_string();
    let event_reference = first_tag_value(event, "e").unwrap_or("").to_string();
    let source_url = first_tag_value(event, "r").unwrap_or("").to_string();

    let source_reference_key = if !artifact_address.is_empty() {
        format!("a:{artifact_address}")
    } else if !event_reference.is_empty() {
        format!("e:{event_reference}")
    } else if !source_url.is_empty() {
        format!("r:{source_url}")
    } else {
        String::new()
    };

    Some(HighlightRecord {
        event_id: event.id.to_hex(),
        pubkey: event.pubkey.to_hex(),
        quote,
        context,
        note: comment,
        artifact_address,
        event_reference,
        external_reference: String::new(),
        source_url,
        source_reference_key,
        clip_start_seconds: first_tag_value(event, "start").and_then(|s| s.parse().ok()),
        clip_end_seconds: first_tag_value(event, "end").and_then(|s| s.parse().ok()),
        clip_speaker: first_tag_value(event, "speaker").unwrap_or("").to_string(),
        clip_transcript_segment_ids: event
            .tags
            .iter()
            .filter_map(|t| {
                let s = t.as_slice();
                if s.first().map(String::as_str) == Some("segment") {
                    s.get(1).map(|v| v.to_string())
                } else {
                    None
                }
            })
            .collect(),
        image_url: crate::highlights::imeta_image_url(event),
        created_at: Some(event.created_at.as_secs()),
    })
}

fn article_record_from_event(event: &Event) -> Option<ArticleRecord> {
    let identifier = first_tag_value(event, "d").unwrap_or("").to_string();
    if identifier.is_empty() {
        return None;
    }
    let title = first_tag_value(event, "title").unwrap_or("").to_string();
    let summary = first_tag_value(event, "summary").unwrap_or("").to_string();
    let image = first_tag_value(event, "image").unwrap_or("").to_string();
    let published_at = first_tag_value(event, "published_at").and_then(|v| v.parse::<u64>().ok());
    let hashtags: Vec<String> = event
        .tags
        .iter()
        .filter_map(|t| {
            let s = t.as_slice();
            if s.first().map(String::as_str) == Some("t") {
                s.get(1).map(|v| v.to_string())
            } else {
                None
            }
        })
        .collect();

    Some(ArticleRecord {
        event_id: event.id.to_hex(),
        address: crate::articles::article_address(&event.pubkey.to_hex(), &identifier),
        pubkey: event.pubkey.to_hex(),
        identifier,
        title,
        summary,
        image,
        content: event.content.clone(),
        hashtags,
        published_at,
        created_at: Some(event.created_at.as_secs()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_ndb::process_event_and_wait;
    use tempfile::TempDir;

    fn fresh_ndb() -> (Ndb, TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = nostrdb::Config::new();
        let ndb = Ndb::new(tmp.path().to_str().unwrap(), &cfg).unwrap();
        (ndb, tmp)
    }

    fn process(ndb: &Ndb, event: &Event) {
        process_event_and_wait(ndb, event);
    }

    fn community(name: &str) -> CommunitySummary {
        CommunitySummary {
            id: format!("id-{name}"),
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

    fn highlight(artifact_address: &str, image_url: &str) -> HighlightRecord {
        HighlightRecord {
            event_id: "event".into(),
            pubkey: "pubkey".into(),
            quote: "quote".into(),
            context: String::new(),
            note: String::new(),
            artifact_address: artifact_address.into(),
            event_reference: String::new(),
            external_reference: String::new(),
            source_url: String::new(),
            source_reference_key: String::new(),
            clip_start_seconds: None,
            clip_end_seconds: None,
            clip_speaker: String::new(),
            clip_transcript_segment_ids: Vec::new(),
            image_url: image_url.into(),
            created_at: None,
        }
    }

    #[test]
    fn search_query_projection_trims_and_blocks_blank_queries() {
        let ready = search_query_projection(SearchQueryProjectionInput {
            query: "  nostr books\n".into(),
        });
        let blank = search_query_projection(SearchQueryProjectionInput {
            query: " \n\t ".into(),
        });

        assert_eq!(ready.search_query, "nostr books");
        assert!(ready.has_query);
        assert_eq!(blank.search_query, "");
        assert!(!blank.has_query);
    }

    #[test]
    fn search_chrome_snapshot_preserves_recent_queries_relays_and_error() {
        let snapshot = search_chrome_snapshot(
            vec!["nostr".into(), "books".into()],
            vec!["wss://relay.highlighter.com".into()],
            "cache unavailable",
        );

        assert_eq!(snapshot.recent_queries, vec!["nostr", "books"]);
        assert_eq!(snapshot.search_relays, vec!["wss://relay.highlighter.com"]);
        assert_eq!(snapshot.error, "cache unavailable");
    }

    #[test]
    fn search_text_matches_projection_returns_case_insensitive_character_spans() {
        let projection = search_text_matches_projection(SearchTextMatchesProjectionInput {
            text: "Alpha beta ALPHA".into(),
            query: " alpha ".into(),
        });
        let blank = search_text_matches_projection(SearchTextMatchesProjectionInput {
            text: "Alpha".into(),
            query: " ".into(),
        });

        assert_eq!(
            projection.spans,
            vec![
                SearchTextMatchSpan { start: 0, end: 5 },
                SearchTextMatchSpan { start: 11, end: 16 },
            ]
        );
        assert!(blank.spans.is_empty());
    }

    #[test]
    fn search_suggestions_projection_dedupes_rooms_and_fills_fallbacks() {
        let projection = search_suggestions_projection(SearchSuggestionsProjectionInput {
            joined_communities: vec![
                community("  Bitcoin  "),
                community(""),
                community("Sci-Fi"),
                community("sci-fi"),
                community("Ignored fifth room"),
            ],
        });

        assert_eq!(
            projection.queries,
            vec![
                "Bitcoin",
                "Sci-Fi",
                "Dostoevsky",
                "Attention",
                "Borges",
                "Philosophy"
            ]
        );
        assert!(projection.queries.len() <= 8);
    }

    #[test]
    fn search_community_row_projection_preserves_row_copy() {
        let mut community_record = community("Readers");
        community_record.about = "Books and notes".into();
        community_record.visibility = "private".into();
        community_record.access = "closed".into();
        community_record.member_count = Some(1);

        let projection = search_community_row_projection(SearchCommunityRowProjectionInput {
            community: community_record,
        });

        assert_eq!(projection.display_name, "Readers");
        assert_eq!(projection.about, Some("Books and notes".into()));
        assert_eq!(projection.visibility_label, "Private");
        assert_eq!(projection.access_label, "Closed");
        assert_eq!(projection.member_count_label, Some("1 members".into()));

        let projection = search_community_row_projection(SearchCommunityRowProjectionInput {
            community: community("Writers"),
        });

        assert_eq!(projection.about, None);
        assert_eq!(projection.visibility_label, "Public");
        assert_eq!(projection.access_label, "Open");
        assert_eq!(projection.member_count_label, None);
    }

    #[test]
    fn search_highlight_row_projection_projects_route_and_page_image() {
        let pubkey = "a".repeat(64);
        let projection = search_highlight_row_projection(SearchHighlightRowProjectionInput {
            highlight: highlight(
                &format!("  30023:{pubkey}:essay\n"),
                " https://example.com/page.jpg ",
            ),
        });
        let invalid = search_highlight_row_projection(SearchHighlightRowProjectionInput {
            highlight: highlight("bad address", "ftp://example.com/page.jpg"),
        });

        let route = projection.article_route.expect("article route");
        assert_eq!(route.address, format!("30023:{pubkey}:essay"));
        assert_eq!(
            projection.page_image_url.as_deref(),
            Some("https://example.com/page.jpg")
        );
        assert!(invalid.article_route.is_none());
        assert!(invalid.page_image_url.is_none());
    }

    #[test]
    fn search_highlights_matches_quote_and_note_case_insensitive() {
        let (ndb, _tmp) = fresh_ndb();
        let keys = Keys::generate();

        let match_quote = EventBuilder::new(Kind::Custom(KIND_HIGHLIGHT), "The Brothers Karamazov")
            .sign_with_keys(&keys)
            .unwrap();
        let match_note = EventBuilder::new(Kind::Custom(KIND_HIGHLIGHT), "an unrelated quote")
            .tags([Tag::parse(vec![
                "comment".to_string(),
                "dostoevsky fan club".to_string(),
            ])
            .unwrap()])
            .sign_with_keys(&keys)
            .unwrap();
        let no_match = EventBuilder::new(Kind::Custom(KIND_HIGHLIGHT), "Proust is the best")
            .sign_with_keys(&keys)
            .unwrap();

        process(&ndb, &match_quote);
        process(&ndb, &match_note);
        process(&ndb, &no_match);

        let hits = search_highlights(&ndb, "DOSTOEVSKY", 20).unwrap();
        assert!(hits.iter().any(|h| h.note.contains("dostoevsky")));
        let kara = search_highlights(&ndb, "karamazov", 20).unwrap();
        assert!(kara.iter().any(|h| h.quote.contains("Karamazov")));
    }

    #[test]
    fn search_articles_matches_title_and_hashtag_and_dedupes_by_address() {
        let (ndb, _tmp) = fresh_ndb();
        let keys = Keys::generate();

        let older = EventBuilder::new(Kind::Custom(KIND_LONG_FORM), "old body")
            .tags([
                Tag::parse(vec!["d".to_string(), "essay".to_string()]).unwrap(),
                Tag::parse(vec!["title".to_string(), "On Attention".to_string()]).unwrap(),
            ])
            .custom_created_at(Timestamp::from(1_000u64))
            .sign_with_keys(&keys)
            .unwrap();
        let newer = EventBuilder::new(Kind::Custom(KIND_LONG_FORM), "new body")
            .tags([
                Tag::parse(vec!["d".to_string(), "essay".to_string()]).unwrap(),
                Tag::parse(vec![
                    "title".to_string(),
                    "On Attention (revised)".to_string(),
                ])
                .unwrap(),
            ])
            .custom_created_at(Timestamp::from(2_000u64))
            .sign_with_keys(&keys)
            .unwrap();
        let hashtag_match = EventBuilder::new(Kind::Custom(KIND_LONG_FORM), "body")
            .tags([
                Tag::parse(vec!["d".to_string(), "other".to_string()]).unwrap(),
                Tag::parse(vec!["title".to_string(), "Entirely Unrelated".to_string()]).unwrap(),
                Tag::parse(vec!["t".to_string(), "attention".to_string()]).unwrap(),
            ])
            .sign_with_keys(&keys)
            .unwrap();

        process(&ndb, &older);
        process(&ndb, &newer);
        process(&ndb, &hashtag_match);

        let hits = search_articles(&ndb, "attention", 20).unwrap();
        assert_eq!(hits.len(), 2, "dedupe by (pubkey, d): 2 distinct addresses");
        let essay = hits.iter().find(|a| a.identifier == "essay").unwrap();
        assert_eq!(essay.title, "On Attention (revised)", "newest wins");
    }

    #[test]
    fn search_snapshots_read_all_sections_and_article_refresh() {
        let (ndb, _tmp) = fresh_ndb();
        let keys = Keys::generate();

        let highlight = EventBuilder::new(Kind::Custom(KIND_HIGHLIGHT), "attention quote")
            .sign_with_keys(&keys)
            .unwrap();
        let article = EventBuilder::new(Kind::Custom(KIND_LONG_FORM), "body")
            .tags([
                Tag::parse(vec!["d".to_string(), "attention-essay".to_string()]).unwrap(),
                Tag::parse(vec!["title".to_string(), "Attention Essay".to_string()]).unwrap(),
            ])
            .sign_with_keys(&keys)
            .unwrap();
        let community = EventBuilder::new(Kind::Custom(KIND_GROUP_METADATA), "")
            .tags([
                Tag::identifier("attention-room"),
                Tag::parse(vec!["name".to_string(), "Attention Room".to_string()]).unwrap(),
                Tag::parse(vec!["public".to_string()]).unwrap(),
                Tag::parse(vec!["open".to_string()]).unwrap(),
            ])
            .sign_with_keys(&keys)
            .unwrap();
        let profile = EventBuilder::new(
            Kind::Custom(KIND_METADATA),
            r#"{"name":"attentionist","display_name":"Attentionist"}"#,
        )
        .sign_with_keys(&keys)
        .unwrap();

        for event in [&highlight, &article, &community, &profile] {
            process(&ndb, event);
        }

        let snapshot = search_results_snapshot(&ndb, " attention ");
        assert_eq!(snapshot.highlights.len(), 1);
        assert_eq!(snapshot.articles.len(), 1);
        assert_eq!(snapshot.communities.len(), 1);
        assert_eq!(snapshot.profiles.len(), 1);

        let article_snapshot = search_article_results_snapshot(&ndb, "attention");
        assert_eq!(
            article_snapshot.articles[0].address,
            snapshot.articles[0].address
        );

        let blank = search_results_snapshot(&ndb, " ");
        assert!(blank.highlights.is_empty());
        assert!(blank.articles.is_empty());
        assert!(blank.communities.is_empty());
        assert!(blank.profiles.is_empty());
    }

    #[test]
    fn search_communities_filters_private_or_closed_rooms() {
        let (ndb, _tmp) = fresh_ndb();
        let keys = Keys::generate();
        for (id, visibility, access) in [
            ("private", "private", "open"),
            ("closed", "public", "closed"),
            ("alpha", "public", "open"),
        ] {
            let event = EventBuilder::new(Kind::Custom(KIND_GROUP_METADATA), "")
                .tags([
                    Tag::identifier(id),
                    Tag::parse(vec!["name".to_string(), "Reader Room".to_string()]).unwrap(),
                    Tag::parse(vec![visibility.to_string()]).unwrap(),
                    Tag::parse(vec![access.to_string()]).unwrap(),
                ])
                .sign_with_keys(&keys)
                .unwrap();
            process(&ndb, &event);
        }

        let hits = search_communities(&ndb, "reader", 20).unwrap();
        let ids: Vec<_> = hits.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, vec!["alpha"]);
    }

    #[test]
    fn search_profiles_ranks_prefix_matches_first() {
        let (ndb, _tmp) = fresh_ndb();
        let a = Keys::generate();
        let b = Keys::generate();

        let contains_only = EventBuilder::new(
            Kind::Custom(KIND_METADATA),
            r#"{"name":"Prof. Aldous Huxley","display_name":"Aldous Huxley"}"#,
        )
        .sign_with_keys(&a)
        .unwrap();
        let prefix_match = EventBuilder::new(
            Kind::Custom(KIND_METADATA),
            r#"{"name":"huxley-fan","display_name":"Huxley's Fan"}"#,
        )
        .sign_with_keys(&b)
        .unwrap();

        process(&ndb, &contains_only);
        process(&ndb, &prefix_match);

        let hits = search_profiles(&ndb, "huxley", 20).unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(
            hits[0].display_name, "Huxley's Fan",
            "prefix match ranks first"
        );
    }

    #[test]
    fn query_search_relays_always_includes_default_and_dedupes() {
        let (ndb, _tmp) = fresh_ndb();
        let user = Keys::generate();

        let event = EventBuilder::new(Kind::Custom(KIND_SEARCH_RELAYS), "")
            .tags([
                Tag::parse(vec![
                    "relay".to_string(),
                    "wss://relay.nostr.band".to_string(),
                ])
                .unwrap(),
                Tag::parse(vec!["relay".to_string(), highlighter_relay().to_string()]).unwrap(),
            ])
            .sign_with_keys(&user)
            .unwrap();
        process(&ndb, &event);

        let relays = query_search_relays(&ndb, &user.public_key().to_hex()).unwrap();
        assert_eq!(
            relays.first().map(String::as_str),
            Some(highlighter_relay())
        );
        assert!(relays.iter().any(|r| r == "wss://relay.nostr.band"));
        // No duplicates for the default relay even though the user also listed it.
        let hl_count = relays
            .iter()
            .filter(|r| r.as_str() == highlighter_relay())
            .count();
        assert_eq!(hl_count, 1);
    }
}
