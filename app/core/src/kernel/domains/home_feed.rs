//! Home-feed merge domain — Phase 4J.
//!
//! Ports the merge/grouping/suppression logic from the live bespoke
//! `app/core/src/home_feed.rs` (249 lines) as a pure derivation over
//! `AppState::article_feed` (kind:30023 rows from Phase 4G) and
//! `AppState::highlight_feed` (kind:9802 rows from Phase 4H). No new nmp
//! wiring — this is feed COMPOSITION only.
//!
//! ## Merge logic (ported verbatim from live home_feed.rs)
//!
//! 1. Group highlights by `source_reference` (the NIP-84 `a`/`e` tag value).
//!    Highlights with no source reference form a solo group keyed by event_id.
//! 2. Collect the set of highlighted article addresses (`source_reference` values
//!    that look like an addressable coordinate `kind:pubkey:d`).
//! 3. Append article rows for articles NOT in the highlighted-addresses set
//!    (suppression: if an article is highlighted, it appears in the highlight
//!    group rather than as a standalone article row).
//! 4. Sort all rows by `sort_key` descending (newest activity first).
//!
//! ## stable_id structural key
//!
//! `highlight_stable_id` computes a deterministic string from the first
//! highlight in a group:
//!   - `"h:src:<address>"` when `source_reference` looks like an address
//!   - `"h:src:<source_reference>"` otherwise (fallback for URL-anchored highlights)
//!   - `"h:evt:<event_id>"` when no source reference is present
//!
//! Article (read) rows use `"r:<article_address>"`.
//!
//! These are raw structural keys (D1: no user-visible labels).
//!
//! ## View lifecycle
//!
//! `ViewId::HomeFeed` opens/closes both the `ArticleFeed` and `HighlightFeed`
//! views so both underlying pull cursors are registered and draining before
//! the merge snapshot is computed. The merge snapshot is a pure read of the
//! two `FeedState`s — no additional nmp wiring here.
//!
//! ## D1 compliance
//!
//! `HomeFeedRow` carries only raw structural fields (event_ids, pubkeys,
//! source_reference, created_at). No `"Highlighted by …"`, no `"min read"`,
//! no `"#{tag}"`, no stable_id as a user-visible label. Swift owns all
//! presentation.
//!
//! ## Threading
//!
//! All functions here are pure (synchronous, non-blocking, no async). The
//! snapshot projection runs on the actor thread.
//!
//! ## Live lane untouched
//!
//! `app/core/src/home_feed.rs` (bespoke HighlighterCore) is NOT modified.

use std::collections::{HashMap, HashSet};

use crate::kernel::app::AppState;
use crate::kernel::effect::Effect;
use crate::kernel::snapshot::{
    KernelHomeFeedRow, KernelHomeFeedRowKind, KernelHomeFeedSnapshot, ViewSnapshot,
};
use crate::kernel::view::ViewId;

use super::articles_feed::{
    lifecycle_effects_for_view_close as article_feed_close,
    lifecycle_effects_for_view_open as article_feed_open,
};
use super::highlight_feed::{
    lifecycle_effects_for_view_close as highlight_feed_close,
    lifecycle_effects_for_view_open as highlight_feed_open,
};

// ─── Lifecycle effects ────────────────────────────────────────────────────────

/// Return lifecycle effects for `Cmd::OpenView(ViewId::HomeFeed)`.
///
/// Composes the lifecycle effects of both underlying feeds:
/// - `ArticleFeed` open: `RegisterFeedCursor("hl.feed.articles")` + `DrainFeed`
///   (fail-closed when `AppState::follows` is empty — no follows, no cursor).
/// - `HighlightFeed` open: `RegisterFeedCursor("hl.feed.highlights")` + `DrainFeed`.
///
/// This ensures both pull cursors are registered before the first snapshot
/// projection runs. The HomeFeed snapshot is a pure merge over the two
/// already-registered feed states.
pub(crate) fn lifecycle_effects_for_view_open(id: &ViewId, state: &AppState) -> Vec<Effect> {
    if !matches!(id, ViewId::HomeFeed) {
        return vec![];
    }

    let mut effects = article_feed_open(&ViewId::ArticleFeed, state);
    effects.extend(highlight_feed_open(&ViewId::HighlightFeed));
    effects
}

/// Return lifecycle effects for `Cmd::CloseView(ViewId::HomeFeed)`.
///
/// Releases both underlying feed cursors. The `FeedState.rows` buffers are
/// cleared inline by the actor's `ReleaseFeedCursor` inline handler (same
/// pattern as `ReleaseGroupEvents` in Phase 3F).
pub(crate) fn lifecycle_effects_for_view_close(id: &ViewId) -> Vec<Effect> {
    if !matches!(id, ViewId::HomeFeed) {
        return vec![];
    }

    let mut effects = article_feed_close(&ViewId::ArticleFeed);
    effects.extend(highlight_feed_close(&ViewId::HighlightFeed));
    effects
}

// ─── Snapshot projection ─────────────────────────────────────────────────────

/// Project a `ViewSnapshot::HomeFeed(HomeFeedSnapshot)` from `AppState`.
///
/// Merges kind:9802 highlight rows from `AppState::highlight_feed` and
/// kind:30023 article rows from `AppState::article_feed` using the same
/// grouping/suppression logic as the live bespoke `home_feed.rs::build_items`.
///
/// D1: `HomeFeedRow` carries only raw structural fields — no formatted strings.
/// Structural keys (`stable_id`) are deterministic identifiers, not labels.
pub(crate) fn project_home_feed_snapshot(state: &AppState) -> Option<ViewSnapshot> {
    let rows = build_home_feed_rows(state);
    Some(ViewSnapshot::HomeFeed(KernelHomeFeedSnapshot { rows }))
}

/// Build the merged, sorted, suppressed list of `HomeFeedRow`s.
///
/// Ported from `home_feed.rs::build_items` (live bespoke, 249L) with the
/// bespoke domain types replaced by the kernel's raw `FeedState` rows.
///
/// Step 1: group highlights by their source_reference. Order of groups follows
/// insertion order of the first highlight in each group (arrival order from
/// the feed drain — seq-ordered, not created_at-ordered).
///
/// Step 2: collect the set of highlighted addresses (source_references that
/// are addressable coordinates — contains `:` as in `kind:pubkey:d`).
///
/// Step 3: append article rows whose address is NOT in the highlighted set
/// (suppression: an article surfaced by a highlight is shown in the highlight
/// group, not as a duplicate standalone article entry).
///
/// Step 4: sort all rows by `sort_key` descending (newest first). Tie-break
/// by `stable_id` ascending for a deterministic render order.
pub(crate) fn build_home_feed_rows(state: &AppState) -> Vec<KernelHomeFeedRow> {
    // ── Step 1: decode highlights from highlight_feed.rows ───────────────────
    //
    // Each kind:9802 row contributes: source_reference (from `a`/`e`/`r` tag),
    // event_id, author_pubkey, created_at. We group by source_reference.
    //
    // We also separately track `a`-tag values as `highlighted_addresses` for
    // suppression (Step 2). Only `a` tags produce nostr addressable coordinates
    // that can match article addresses — `e` tags are event ids and `r` tags are
    // URLs, neither of which will collide with a `kind:pubkey:d_tag` coordinate.
    // This mirrors the live `home_feed.rs::build_items` which builds
    // `highlighted_addresses` from `highlight.highlight.artifact_address` (the
    // `a` tag value only), NOT from the full `source_reference_key`.

    // group_key → vec of highlights within the group
    let mut group_map: HashMap<String, Vec<RawHighlight>> = HashMap::new();
    // Preserve insertion order of group keys (first encounter of each source).
    let mut group_order: Vec<String> = Vec::new();
    // Only `a`-tag source references — used for article suppression (Step 2).
    let mut highlighted_addresses: HashSet<String> = HashSet::new();

    for ev in &state.highlight_feed.rows {
        if ev.kind != 9802 || ev.content.is_empty() {
            continue; // skip malformed / wrong-kind rows (D6)
        }

        let extracted = extract_source_reference(&ev.tags);

        // Track the raw `a` tag value separately for suppression — only an `a`
        // tag (addressable coordinate) can match a kind:30023 article address.
        // Suppression uses the RAW value (e.g. "30023:pubkey:d"), not the
        // prefixed group key "a:30023:pubkey:d". This mirrors live
        // `build_items` which uses `highlight.highlight.artifact_address` directly.
        if let Some((_, ref raw_val, SourceRefKind::Address)) = extracted {
            highlighted_addresses.insert(raw_val.clone());
        }

        let (group_key_val, source_reference, source_ref_kind) = match extracted {
            Some((gk, raw, kind)) => (Some(gk), Some(raw), Some(kind)),
            None => (None, None, None),
        };

        // Prefixed group key matches live `source_reference_key` format.
        // Solo highlights use "solo:<event_id>" (same as live lane).
        let key = group_key_val.unwrap_or_else(|| format!("solo:{}", ev.id));

        if !group_map.contains_key(&key) {
            group_order.push(key.clone());
            group_map.insert(key.clone(), Vec::new());
        }
        group_map
            .get_mut(&key)
            .expect("key inserted above")
            .push(RawHighlight {
                event_id: ev.id.clone(),
                author_pubkey: ev.author.clone(),
                created_at: ev.created_at,
                source_reference: source_reference.clone(),
                source_ref_kind: source_ref_kind.clone(),
            });
    }

    // ── Step 3: build output rows ────────────────────────────────────────────

    let mut rows: Vec<KernelHomeFeedRow> = Vec::with_capacity(group_order.len());

    // Add highlight groups (insertion-ordered).
    for key in &group_order {
        let mut group = group_map.remove(key).unwrap_or_default();
        // Within a group, sort highlights by created_at ascending (oldest first
        // within the group — matches live home_feed.rs::build_items group sort).
        group.sort_by_key(|h| h.created_at);

        let sort_key = group.iter().map(|h| h.created_at).max().unwrap_or(0);
        let stable_id = highlight_stable_id(&group);
        let highlight_event_ids: Vec<String> = group.iter().map(|h| h.event_id.clone()).collect();
        let highlight_author_pubkeys: Vec<String> =
            group.iter().map(|h| h.author_pubkey.clone()).collect();
        let source_reference = group.first().and_then(|h| h.source_reference.clone());

        rows.push(KernelHomeFeedRow {
            stable_id,
            sort_key,
            kind: KernelHomeFeedRowKind::Highlight,
            highlight_event_ids,
            highlight_author_pubkeys,
            source_reference,
            article_address: None,
            article_id: None,
            article_author_pubkey: None,
            article_created_at: None,
        });
    }

    // Add article rows NOT already surfaced by a highlight group (suppression).
    for ev in &state.article_feed.rows {
        if ev.kind != 30023 {
            continue; // skip wrong-kind rows (D6)
        }

        // Build the addressable coordinate: `kind:author:d_tag`.
        let d_tag = ev
            .tags
            .iter()
            .find(|t| t.first().map(|s| s == "d").unwrap_or(false))
            .and_then(|t| t.get(1))
            .cloned()
            .unwrap_or_default();
        let address = format!("{}:{}:{}", ev.kind, ev.author, d_tag);

        // Suppress: if this article's address appears in the highlight set, skip.
        if highlighted_addresses.contains(&address) {
            continue;
        }

        rows.push(KernelHomeFeedRow {
            stable_id: format!("r:{}", address),
            sort_key: ev.created_at,
            kind: KernelHomeFeedRowKind::Article,
            highlight_event_ids: Vec::new(),
            highlight_author_pubkeys: Vec::new(),
            source_reference: None,
            article_address: Some(address),
            article_id: Some(ev.id.clone()),
            article_author_pubkey: Some(ev.author.clone()),
            article_created_at: Some(ev.created_at),
        });
    }

    // ── Step 4: sort by sort_key descending, stable_id ascending tie-break ───
    rows.sort_by(|a, b| {
        b.sort_key
            .cmp(&a.sort_key)
            .then_with(|| a.stable_id.cmp(&b.stable_id))
    });

    rows
}

// ─── Internal helpers ────────────────────────────────────────────────────────

/// Which NIP-84 tag provided the source reference for a highlight.
///
/// Used by `highlight_stable_id` to decide between `"h:src:*"` (for `a` and `r`
/// tags) and `"h:evt:*"` (for `e`, `i`, and solo highlights). The live bespoke
/// `home_feed.rs::highlight_stable_id` (lines 124-135) only emits `"h:src:*"` when
/// `artifact_address` (`a` tag) or `source_url` (`r` tag) is present; for
/// `e`-only, `i`-only, and solo cases it falls back to
/// `"h:evt:<highlight_event_id>"`.
///
/// Grouping key format (mirrors live `highlights.rs:1442` `source_reference_key`):
///   `"a:<val>"` | `"e:<val>"` | `"i:<val>"` | `"r:<val>"` | `"solo:<event_id>"`
#[derive(Debug, Clone, PartialEq, Eq)]
enum SourceRefKind {
    /// From an `a` tag — addressable coordinate. Group key `"a:<val>"`.
    /// Stable_id uses `"h:src:<val>"` (mirrors live `artifact_address` branch).
    Address,
    /// From an `r` tag — URL anchor. Group key `"r:<val>"`.
    /// Stable_id uses `"h:src:<val>"` (mirrors live `source_url` branch).
    Url,
    /// From an `e` tag — referenced event id. Group key `"e:<val>"`.
    /// Stable_id falls back to `"h:evt:<highlight_event_id>"` (live has no `e`
    /// branch in `highlight_stable_id`; it falls through to the event_id default).
    Event,
    /// From an `i` tag — external identifier (ISBN, podcast, URL-like ref).
    /// Group key `"i:<val>"`. Stable_id falls back to `"h:evt:<highlight_event_id>"`
    /// (live has no `i` branch in `highlight_stable_id` either).
    ExternalRef,
}

/// Minimal highlight record used during merge computation.
struct RawHighlight {
    event_id: String,
    author_pubkey: String,
    created_at: u64,
    source_reference: Option<String>,
    /// Which tag produced `source_reference` — drives `highlight_stable_id`.
    source_ref_kind: Option<SourceRefKind>,
}

/// Compute the stable_id for a highlight group.
///
/// Ported from `home_feed.rs::highlight_stable_id` (lines 124-135). Uses the
/// first highlight in the (created_at-ascending-sorted) group:
///
/// - `"h:src:<val>"` when `source_ref_kind` is `Address` (`a` tag) — mirrors
///   live `artifact_address` branch. The `val` is the raw a-tag value
///   (e.g. `"30023:pubkey:d_tag"`), NOT the prefixed group key `"a:<val>"`.
/// - `"h:src:<val>"` when `source_ref_kind` is `Url` (`r` tag) — mirrors
///   live `source_url` branch. The `val` is the raw r-tag value.
/// - `"h:evt:<highlight_event_id>"` for `e`-only (`Event`), `i`-only
///   (`ExternalRef`), and solo highlights — live `highlight_stable_id` has no
///   branch for these; they fall to `format!("h:evt:{}", first.highlight.event_id)`.
///   This uses the **highlight's own event id**, not the referenced tag value.
///
/// D1: raw structural keys only — never a user-visible label.
fn highlight_stable_id(group: &[RawHighlight]) -> String {
    let Some(first) = group.first() else {
        return "h:empty".to_string();
    };
    match (&first.source_ref_kind, &first.source_reference) {
        (Some(SourceRefKind::Address), Some(ref src)) => format!("h:src:{src}"),
        (Some(SourceRefKind::Url), Some(ref src)) => format!("h:src:{src}"),
        // e-only, i-only, or solo: fall back to the highlight's own event id (live behavior).
        _ => format!("h:evt:{}", first.event_id),
    }
}

/// Extract the NIP-84 source reference from a tag list, returning a
/// `(group_key, raw_val, SourceRefKind)` triple so callers can:
///   - use `group_key` as the grouping map key (prefixed: `"a:<v>"`, `"e:<v>"`,
///     `"i:<v>"`, `"r:<v>"`) — matches live `highlights.rs:1442` `source_reference_key`
///   - use `raw_val` for the `source_reference` field on the output row and for
///     stable_id emission (Address/Url cases emit `"h:src:<raw_val>"`)
///   - use `SourceRefKind` to choose the correct `highlight_stable_id` branch
///
/// Priority order mirrors live `highlights.rs::record_from_cached_event:1442`:
///   a (artifact_address) → e (event_reference) → i (external_reference) → r (source_url)
///
/// Returns `None` when none of the four tags is present (solo highlight).
fn extract_source_reference(tags: &[Vec<String>]) -> Option<(String, String, SourceRefKind)> {
    fn first_tag<'a>(tags: &'a [Vec<String>], name: &str) -> Option<&'a str> {
        tags.iter()
            .find(|t| t.first().map(|s| s == name).unwrap_or(false))
            .and_then(|t| t.get(1))
            .map(String::as_str)
    }

    // 1. `a` tag — addressable coordinate (NIP-23 articles, primary target).
    if let Some(val) = first_tag(tags, "a") {
        return Some((format!("a:{val}"), val.to_string(), SourceRefKind::Address));
    }
    // 2. `e` tag — non-addressable event reference.
    if let Some(val) = first_tag(tags, "e") {
        return Some((format!("e:{val}"), val.to_string(), SourceRefKind::Event));
    }
    // 3. `i` tag — external identifier (ISBN, podcast episode, URL-like ref).
    //    Added in live highlights.rs:1447; stable_id falls to h:evt:* (no i-branch
    //    in live highlight_stable_id).
    if let Some(val) = first_tag(tags, "i") {
        return Some((
            format!("i:{val}"),
            val.to_string(),
            SourceRefKind::ExternalRef,
        ));
    }
    // 4. `r` tag — URL anchor (web-page highlights). Bespoke lane stores as
    //    `HighlightRecord.source_url`; stable_id emits `"h:src:<url>"`.
    first_tag(tags, "r").map(|val| (format!("r:{val}"), val.to_string(), SourceRefKind::Url))
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::action::AppAction;
    use crate::kernel::actor::{reduce, Cmd};
    use crate::kernel::app::AppState;
    use crate::kernel::clock::{Clock, ManualClock};
    use crate::kernel::domains::feed::apply_feed_page;
    use crate::kernel::effect::Effect;
    use crate::kernel::snapshot::KernelHomeFeedRowKind;
    use crate::kernel::view::ViewId;

    fn make_state() -> AppState {
        AppState::default()
    }

    fn step(state: &mut AppState, clock: &ManualClock, cmd: Cmd) -> Vec<Effect> {
        let now = clock.now_unix_seconds();
        reduce(state, cmd, now)
    }

    /// Build a minimal kind:9802 highlight raw event.
    fn highlight_ev(
        id: &str,
        pubkey: &str,
        source: &str,
        created_at: u64,
    ) -> nmp_core::substrate::KernelEvent {
        let mut tags = Vec::new();
        if !source.is_empty() {
            if source.contains(':') {
                tags.push(vec!["a".to_string(), source.to_string()]);
            } else {
                tags.push(vec!["e".to_string(), source.to_string()]);
            }
        }
        nmp_core::substrate::KernelEvent {
            id: id.to_string(),
            author: pubkey.to_string(),
            kind: 9802,
            created_at,
            tags,
            content: "highlighted text".to_string(),
            relay_provenance: vec![],
        }
    }

    /// Build a minimal kind:30023 article raw event.
    fn article_ev(
        id: &str,
        pubkey: &str,
        d_tag: &str,
        created_at: u64,
    ) -> nmp_core::substrate::KernelEvent {
        nmp_core::substrate::KernelEvent {
            id: id.to_string(),
            author: pubkey.to_string(),
            kind: 30023,
            created_at,
            tags: vec![vec!["d".to_string(), d_tag.to_string()]],
            content: String::new(),
            relay_provenance: vec![],
        }
    }

    // 4J-T1: home_feed_merges_articles_and_highlights
    //
    // Both article and highlight rows from the two FeedStates must appear in the
    // merged HomeFeedSnapshot.
    #[test]
    fn home_feed_merges_articles_and_highlights() {
        let mut state = make_state();

        // One highlight (with a source reference to a different article).
        let hl = highlight_ev(
            "hl0000000000000000000000000000000000000000000000000000000000000001",
            "pub0000000000000000000000000000000000000000000000000000000000000001",
            "30023:pub0000000000000000000000000000000000000000000000000000000000000001:d1",
            1_700_000_010,
        );

        // One article that is NOT highlighted (different address).
        let art = article_ev(
            "art000000000000000000000000000000000000000000000000000000000000002",
            "pub0000000000000000000000000000000000000000000000000000000000000002",
            "d2",
            1_700_000_005,
        );

        apply_feed_page(&mut state.highlight_feed, vec![hl], 10, false, None);
        apply_feed_page(&mut state.article_feed, vec![art], 10, false, None);

        let snap = project_home_feed_snapshot(&state).unwrap();
        match snap {
            ViewSnapshot::HomeFeed(ref s) => {
                assert_eq!(s.rows.len(), 2, "one highlight group + one article");
                // Sorted newest-first: highlight (sort_key=10) before article (5).
                assert_eq!(s.rows[0].kind, KernelHomeFeedRowKind::Highlight);
                assert_eq!(s.rows[1].kind, KernelHomeFeedRowKind::Article);
            }
            other => panic!("expected HomeFeed snapshot, got {:?}", other),
        }
    }

    // 4J-T2: read_suppressed_when_article_highlighted
    //
    // When a highlight's source_reference matches an article's addressable
    // coordinate, the article row MUST NOT appear in the merged feed (suppression).
    #[test]
    fn read_suppressed_when_article_highlighted() {
        let mut state = make_state();

        let pubkey = "pub0000000000000000000000000000000000000000000000000000000000000001";
        let d_tag = "my-article";
        let address = format!("30023:{pubkey}:{d_tag}");

        // Highlight pointing at the article.
        let hl = highlight_ev(
            "hl0000000000000000000000000000000000000000000000000000000000000001",
            pubkey,
            &address,
            1_700_000_020,
        );

        // Article that the highlight references.
        let art = article_ev(
            "art000000000000000000000000000000000000000000000000000000000000001",
            pubkey,
            d_tag,
            1_700_000_001,
        );

        apply_feed_page(&mut state.highlight_feed, vec![hl], 10, false, None);
        apply_feed_page(&mut state.article_feed, vec![art], 10, false, None);

        let snap = project_home_feed_snapshot(&state).unwrap();
        match snap {
            ViewSnapshot::HomeFeed(ref s) => {
                assert_eq!(
                    s.rows.len(),
                    1,
                    "highlighted article must be suppressed — only highlight group visible"
                );
                assert_eq!(s.rows[0].kind, KernelHomeFeedRowKind::Highlight);
                let stable = &s.rows[0].stable_id;
                assert!(
                    stable.starts_with("h:src:"),
                    "stable_id must start with 'h:src:' for addressed highlight, got: {stable}"
                );
            }
            other => panic!("expected HomeFeed snapshot, got {:?}", other),
        }
    }

    // 4J-T3: grouping_by_source_reference
    //
    // Multiple highlights sharing the same source_reference must be merged into
    // one HomeFeedRow (same group), not emitted as separate rows.
    #[test]
    fn grouping_by_source_reference() {
        let mut state = make_state();

        let source = "30023:cafe0000000000000000000000000000000000000000000000000000000001:d";
        let hl1 = highlight_ev(
            "hla0000000000000000000000000000000000000000000000000000000000000001",
            "pub0000000000000000000000000000000000000000000000000000000000000001",
            source,
            1_700_000_010,
        );
        let hl2 = highlight_ev(
            "hlb0000000000000000000000000000000000000000000000000000000000000002",
            "pub0000000000000000000000000000000000000000000000000000000000000002",
            source,
            1_700_000_020,
        );

        apply_feed_page(&mut state.highlight_feed, vec![hl1, hl2], 20, false, None);

        let snap = project_home_feed_snapshot(&state).unwrap();
        match snap {
            ViewSnapshot::HomeFeed(ref s) => {
                assert_eq!(
                    s.rows.len(),
                    1,
                    "two highlights on same source must merge into one group"
                );
                let row = &s.rows[0];
                assert_eq!(row.kind, KernelHomeFeedRowKind::Highlight);
                assert_eq!(
                    row.highlight_event_ids.len(),
                    2,
                    "group must contain both highlight event ids"
                );
                // sort_key = max of highlight created_ats.
                assert_eq!(row.sort_key, 1_700_000_020);
            }
            other => panic!("expected HomeFeed snapshot, got {:?}", other),
        }
    }

    // 4J-T4: home_feed_snapshot_raw_no_labels
    //
    // No formatted strings (D1) in the HomeFeedSnapshot — no bylines, no
    // "Highlighted by", no "min read", no "Untitled" fallback.
    #[test]
    fn home_feed_snapshot_raw_no_labels() {
        let mut state = make_state();

        let hl = highlight_ev(
            "hld0000000000000000000000000000000000000000000000000000000000000001",
            "pub0000000000000000000000000000000000000000000000000000000000000001",
            "",
            1_700_000_001,
        );
        let art = article_ev(
            "art000000000000000000000000000000000000000000000000000000000000003",
            "pub0000000000000000000000000000000000000000000000000000000000000003",
            "my-d3",
            1_700_000_002,
        );

        apply_feed_page(&mut state.highlight_feed, vec![hl], 10, false, None);
        apply_feed_page(&mut state.article_feed, vec![art], 10, false, None);

        let snap = project_home_feed_snapshot(&state).unwrap();
        let debug = format!("{:?}", snap);

        // D1: none of these presentation strings must appear in the snapshot.
        assert!(
            !debug.contains("Highlighted by"),
            "D1: no 'Highlighted by' byline"
        );
        assert!(!debug.contains("min read"), "D1: no 'min read' label");
        assert!(!debug.contains("Untitled"), "D1: no 'Untitled' fallback");
        assert!(
            !debug.contains(" others"),
            "D1: no 'N others' overflow string"
        );
    }

    // 4J-T5: home_feed_opens_both_underlying_feeds
    //
    // Opening ViewId::HomeFeed must emit at least RegisterFeedCursor for the
    // highlight feed (hl.feed.highlights is always registered) and optionally
    // for the article feed (fail-closed when follows is empty). With follows
    // seeded, both RegisterFeedCursor effects must be present.
    #[test]
    fn home_feed_opens_both_underlying_feeds() {
        let mut state = make_state();
        // Seed follows so the article feed is not fail-closed.
        state.follows =
            vec!["aabbcc0000000000000000000000000000000000000000000000000000000001".to_string()];

        let effects = lifecycle_effects_for_view_open(&ViewId::HomeFeed, &state);

        let register_count = effects
            .iter()
            .filter(|e| matches!(e, Effect::RegisterFeedCursor { .. }))
            .count();
        assert_eq!(
            register_count, 2,
            "HomeFeed open must emit RegisterFeedCursor for both article and highlight feeds"
        );

        let drain_count = effects
            .iter()
            .filter(|e| matches!(e, Effect::DrainFeed { .. }))
            .count();
        assert_eq!(
            drain_count, 2,
            "HomeFeed open must emit DrainFeed for both feeds"
        );
    }

    // 4J-T6: empty_feeds_empty_home
    //
    // When both FeedStates are empty, the merged snapshot must be empty (no
    // panics, no placeholder rows).
    #[test]
    fn empty_feeds_empty_home() {
        let state = make_state();

        let snap = project_home_feed_snapshot(&state).unwrap();
        match snap {
            ViewSnapshot::HomeFeed(ref s) => {
                assert!(s.rows.is_empty(), "empty feeds must yield empty home feed");
            }
            other => panic!("expected HomeFeed snapshot, got {:?}", other),
        }
    }

    // 4J-T7: identity_loss_clears
    //
    // After Logout, the two underlying FeedStates are cleared so stale rows
    // from the previous session do not leak into the next account's home feed.
    #[test]
    fn identity_loss_clears() {
        let mut state = make_state();
        let clock = ManualClock::default();

        // Seed both feeds.
        apply_feed_page(
            &mut state.highlight_feed,
            vec![highlight_ev(
                "hle0000000000000000000000000000000000000000000000000000000000000001",
                "pub1",
                "",
                1_000,
            )],
            5,
            false,
            None,
        );
        apply_feed_page(
            &mut state.article_feed,
            vec![article_ev(
                "art000000000000000000000000000000000000000000000000000000000000004",
                "pub1",
                "d4",
                2_000,
            )],
            5,
            false,
            None,
        );

        assert!(!state.highlight_feed.rows.is_empty());
        assert!(!state.article_feed.rows.is_empty());

        // Logout clears both feed states.
        step(&mut state, &clock, Cmd::Action(AppAction::Logout));

        assert!(
            state.highlight_feed.rows.is_empty(),
            "highlight_feed must be cleared on Logout"
        );
        assert!(
            state.article_feed.rows.is_empty(),
            "article_feed must be cleared on Logout"
        );

        // Merged snapshot must now be empty.
        let snap = project_home_feed_snapshot(&state).unwrap();
        match snap {
            ViewSnapshot::HomeFeed(ref s) => {
                assert!(
                    s.rows.is_empty(),
                    "home feed snapshot must be empty after identity loss"
                );
            }
            other => panic!("expected HomeFeed snapshot, got {:?}", other),
        }
    }

    // 4J-T8: home_feed_sort_by_latest
    //
    // Rows must be sorted by sort_key descending — the newest activity
    // (highlight created_at or article created_at) comes first.
    #[test]
    fn home_feed_sort_by_latest() {
        let mut state = make_state();

        let hl_old = highlight_ev(
            "hlf0000000000000000000000000000000000000000000000000000000000000001",
            "pub0000000000000000000000000000000000000000000000000000000000000001",
            "",
            1_000,
        );
        let art_new = article_ev(
            "art000000000000000000000000000000000000000000000000000000000000005",
            "pub0000000000000000000000000000000000000000000000000000000000000001",
            "d5",
            2_000,
        );

        apply_feed_page(&mut state.highlight_feed, vec![hl_old], 10, false, None);
        apply_feed_page(&mut state.article_feed, vec![art_new], 10, false, None);

        let snap = project_home_feed_snapshot(&state).unwrap();
        match snap {
            ViewSnapshot::HomeFeed(ref s) => {
                assert_eq!(s.rows.len(), 2);
                // Newer article (sort_key=2000) must precede older highlight (1000).
                assert_eq!(
                    s.rows[0].kind,
                    KernelHomeFeedRowKind::Article,
                    "newer article must sort first"
                );
                assert_eq!(
                    s.rows[1].kind,
                    KernelHomeFeedRowKind::Highlight,
                    "older highlight must sort second"
                );
            }
            other => panic!("expected HomeFeed snapshot, got {:?}", other),
        }
    }

    // 4J-T9: e_only_highlight_uses_evt_stable_id_not_src
    //
    // A highlight whose source reference came from an `e` tag (non-addressable
    // event reference, NOT an article address) must produce a stable_id of the
    // form `"h:evt:<highlight_event_id>"`, NOT `"h:src:<referenced_event_id>"`.
    //
    // Mirrors live `home_feed.rs::highlight_stable_id` lines 133-134:
    //   format!("h:evt:{}", first.highlight.event_id)
    // The fallback uses the HIGHLIGHT's own event_id, not the referenced event.
    #[test]
    fn e_only_highlight_uses_evt_stable_id_not_src() {
        let mut state = make_state();

        let highlight_id = "hlg0000000000000000000000000000000000000000000000000000000000000001";
        let referenced_event_id =
            "ref0000000000000000000000000000000000000000000000000000000000000099";

        // `highlight_ev` uses `e` tag when source contains no `:`.
        let hl = highlight_ev(highlight_id, "pubkey01", referenced_event_id, 1_700_000_100);
        apply_feed_page(&mut state.highlight_feed, vec![hl], 10, false, None);

        let snap = project_home_feed_snapshot(&state).unwrap();
        match snap {
            ViewSnapshot::HomeFeed(ref s) => {
                assert_eq!(s.rows.len(), 1, "one highlight group expected");
                let row = &s.rows[0];
                let stable = &row.stable_id;

                // Must be h:evt:<highlight_event_id> — NOT h:src:<referenced_event_id>.
                assert_eq!(
                    stable,
                    &format!("h:evt:{highlight_id}"),
                    "e-only highlight stable_id must use highlight's own event_id, got: {stable}"
                );
                assert!(
                    !stable.starts_with("h:src:"),
                    "e-only highlight must NOT use h:src: prefix, got: {stable}"
                );
                assert!(
                    !stable.contains(referenced_event_id),
                    "stable_id must NOT embed the referenced event id, got: {stable}"
                );
            }
            other => panic!("expected HomeFeed snapshot, got {:?}", other),
        }
    }

    // 4J-T10: i_tag_highlight_groups_and_stable_id_matches_live
    //
    // A highlight anchored by an `i` tag (external identifier — ISBN, podcast,
    // URL-like ref) must:
    //   1. Group correctly (two highlights with the same `i` value → one row).
    //   2. Emit `"h:evt:<highlight_event_id>"` as stable_id — live
    //      `home_feed.rs::highlight_stable_id` has no `i` branch and falls
    //      through to `format!("h:evt:{}", first.highlight.event_id)`.
    //   3. NOT emit `"h:src:*"` (would be wrong — live only uses h:src for a/r).
    //
    // Also verifies the `i` tag is NOT in `highlighted_addresses`, so i-anchored
    // highlights do not erroneously suppress article reads.
    #[test]
    fn i_tag_highlight_groups_and_stable_id_matches_live() {
        let mut state = make_state();

        // Build kind:9802 events with `i` tag manually (highlight_ev helper uses
        // `a` vs `e` heuristic; `i` requires a direct construction).
        fn i_tag_ev(
            id: &str,
            pubkey: &str,
            i_val: &str,
            created_at: u64,
        ) -> nmp_core::substrate::KernelEvent {
            nmp_core::substrate::KernelEvent {
                id: id.to_string(),
                author: pubkey.to_string(),
                kind: 9802,
                created_at,
                tags: vec![vec!["i".to_string(), i_val.to_string()]],
                content: "highlighted text".to_string(),
                relay_provenance: vec![],
            }
        }

        let i_val = "isbn:9780140449136"; // Iliad ISBN as a realistic i-tag value
        let hl1_id = "hlh0000000000000000000000000000000000000000000000000000000000000001";
        let hl2_id = "hlh0000000000000000000000000000000000000000000000000000000000000002";

        let hl1 = i_tag_ev(hl1_id, "pub01", i_val, 1_700_000_200);
        let hl2 = i_tag_ev(hl2_id, "pub02", i_val, 1_700_000_210);

        // Also add an article to confirm it is NOT suppressed by the i-tag highlight.
        let art = article_ev(
            "art000000000000000000000000000000000000000000000000000000000000006",
            "pub03",
            "d6",
            1_700_000_100,
        );

        apply_feed_page(&mut state.highlight_feed, vec![hl1, hl2], 20, false, None);
        apply_feed_page(&mut state.article_feed, vec![art], 10, false, None);

        let snap = project_home_feed_snapshot(&state).unwrap();
        match snap {
            ViewSnapshot::HomeFeed(ref s) => {
                // Two i-tag highlights with same i-value → one group row + one article.
                assert_eq!(
                    s.rows.len(),
                    2,
                    "one i-tag highlight group + one unsuppressed article"
                );

                // Highlight group must be first (sort_key=1_700_000_210 > article 1_700_000_100).
                let hl_row = &s.rows[0];
                assert_eq!(hl_row.kind, KernelHomeFeedRowKind::Highlight);
                assert_eq!(
                    hl_row.highlight_event_ids.len(),
                    2,
                    "both i-tag highlights must be in the same group"
                );

                let stable = &hl_row.stable_id;
                // Must be h:evt:<first_highlight_event_id> (ascending-sorted within group).
                assert_eq!(
                    stable,
                    &format!("h:evt:{hl1_id}"),
                    "i-tag highlight stable_id must be h:evt:<highlight_id>, got: {stable}"
                );
                assert!(
                    !stable.starts_with("h:src:"),
                    "i-tag highlight must NOT use h:src: prefix, got: {stable}"
                );

                // Article must NOT be suppressed (i-tag refs don't go into highlighted_addresses).
                let art_row = &s.rows[1];
                assert_eq!(
                    art_row.kind,
                    KernelHomeFeedRowKind::Article,
                    "article must be present — i-tag highlights don't suppress articles"
                );
            }
            other => panic!("expected HomeFeed snapshot, got {:?}", other),
        }
    }
}
