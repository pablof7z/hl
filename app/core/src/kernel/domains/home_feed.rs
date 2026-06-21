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

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::kernel::app::AppState;
use crate::kernel::effect::Effect;
use crate::kernel::snapshot::{
    ArtifactPreviewRow, HighlightRow, KernelHomeFeedRow, KernelHomeFeedRowKind,
    KernelHomeFeedSnapshot, ViewSnapshot,
};
use crate::kernel::view::ViewId;

use super::articles_feed::{
    lifecycle_effects_for_view_close as article_feed_close,
    lifecycle_effects_for_view_open as article_feed_open,
};
use super::highlight_feed::{
    decode_highlight_row, lifecycle_effects_for_view_close as highlight_feed_close,
    lifecycle_effects_for_view_open as highlight_feed_open,
};

/// Feed key for the home-feed interaction cursor (kind:1/7/16/1111, follows,
/// `#k=30023`). Phase 7.
pub(crate) const HOME_INTERACTIONS_FEED_KEY: &str = "hl.feed.home_interactions";

// ─── Lifecycle effects ────────────────────────────────────────────────────────

/// Return lifecycle effects for `Cmd::OpenView(ViewId::HomeFeed)`.
///
/// Composes the lifecycle effects of all three underlying feeds:
/// - `ArticleFeed` open: `RegisterFeedCursor("hl.feed.articles")` + `DrainFeed`
///   (fail-closed when `AppState::follows` is empty — no follows, no cursor).
/// - `HighlightFeed` open: `RegisterFeedCursor("hl.feed.highlights")` + `DrainFeed`.
/// - Interaction cursor: `RegisterFeedCursor("hl.feed.home_interactions")` + `DrainFeed`
///   (fail-closed when `AppState::follows` is empty — no follows, no interaction scan).
///
/// This ensures all pull cursors are registered before the first snapshot
/// projection runs. The HomeFeed snapshot is a pure merge over the three
/// already-registered feed states.
pub(crate) fn lifecycle_effects_for_view_open(id: &ViewId, state: &AppState) -> Vec<Effect> {
    if !matches!(id, ViewId::HomeFeed) {
        return vec![];
    }

    let mut effects = article_feed_open(&ViewId::ArticleFeed, state);
    effects.extend(highlight_feed_open(&ViewId::HighlightFeed));

    // Phase 7: interaction cursor — fail-closed when follows is empty.
    if let Some(scope) = super::feed::home_interaction_feed_scope(&state.follows) {
        effects.extend(super::feed::reduce_register_feed_cursor(
            HOME_INTERACTIONS_FEED_KEY.to_string(),
            scope,
        ));
        effects.extend(super::feed::reduce_drain_feed(
            HOME_INTERACTIONS_FEED_KEY.to_string(),
        ));
    }

    effects
}

/// Emit any missing cursor registrations for an already-open `HomeFeed` view.
///
/// Called from the `FollowListUpdated` reducer when `HomeFeed` is open and
/// follows arrive (or change). If a cursor has not been registered yet
/// (cursor_id == 0), emit `RegisterFeedCursor` + `DrainFeed` for it.
///
/// This is also the re-population path after an account SWITCH: the unified
/// teardown wipes ALL of HomeFeed's cursors (article_feed, highlight_feed,
/// home_feed_interactions) to default (cursor_id == 0); the new account's
/// `FollowListUpdated` then re-fires this hook. To leave NO open cursor blank,
/// this re-registers EVERY wiped HomeFeed cursor — not just the follow-scoped
/// ones (#1653 codex r5 HIGH gap #1):
///   - `article_feed` (follow-scoped — fail-closed when follows empty),
///   - `home_feed_interactions` (follow-scoped — fail-closed when follows empty),
///   - `highlight_feed` (NOT follow-scoped: kind:9802 any-author). Because it
///     does not depend on follows it has no other re-register trigger after a
///     switch wiped it — without this branch the highlight cursor stays at 0
///     (never advances) until the view is closed and reopened.
///
/// D8: effect-driven, not polling — triggered by the follow-list update event.
/// Does not ask Swift to close/reopen the view.
pub(crate) fn lifecycle_effects_for_follow_update(state: &AppState) -> Vec<Effect> {
    let mut effects = Vec::new();

    // Re-register article feed if follows arrived after HomeFeed was opened.
    if state.article_feed.cursor_id == 0 {
        if let Some(scope) = super::feed::article_feed_scope(&state.follows) {
            effects.extend(super::feed::reduce_register_feed_cursor(
                crate::kernel::domains::articles_feed::ARTICLE_FEED_KEY.to_string(),
                scope,
            ));
            effects.extend(super::feed::reduce_drain_feed(
                crate::kernel::domains::articles_feed::ARTICLE_FEED_KEY.to_string(),
            ));
        }
    }

    // Re-register the highlight feed if it was wiped (e.g. by an account switch).
    // The highlight feed is NOT follow-scoped — register unconditionally when its
    // cursor is unregistered so a switched-to account's open HomeFeed re-populates
    // its highlight cursor (#1653 codex r5 gap #1). Mirrors the open-time
    // `highlight_feed::lifecycle_effects_for_view_open` registration.
    if state.highlight_feed.cursor_id == 0 {
        effects.extend(highlight_feed_open(&ViewId::HighlightFeed));
    }

    // Register interaction cursor if follows arrived after HomeFeed was opened.
    if state.home_feed_interactions.cursor_id == 0 {
        if let Some(scope) = super::feed::home_interaction_feed_scope(&state.follows) {
            effects.extend(super::feed::reduce_register_feed_cursor(
                HOME_INTERACTIONS_FEED_KEY.to_string(),
                scope,
            ));
            effects.extend(super::feed::reduce_drain_feed(
                HOME_INTERACTIONS_FEED_KEY.to_string(),
            ));
        }
    }

    effects
}

/// Return lifecycle effects for `Cmd::CloseView(ViewId::HomeFeed)`.
///
/// Releases all three underlying feed cursors. The `FeedState.rows` buffers are
/// cleared inline by the actor's `ReleaseFeedCursor` inline handler (same
/// pattern as `ReleaseGroupEvents` in Phase 3F).
pub(crate) fn lifecycle_effects_for_view_close(id: &ViewId) -> Vec<Effect> {
    if !matches!(id, ViewId::HomeFeed) {
        return vec![];
    }

    let mut effects = article_feed_close(&ViewId::ArticleFeed);
    effects.extend(highlight_feed_close(&ViewId::HighlightFeed));
    // Phase 7: release the interaction cursor too.
    effects.extend(super::feed::reduce_release_feed_cursor(
        HOME_INTERACTIONS_FEED_KEY.to_string(),
    ));
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
    // Attach the artifact-preview rows for the coordinates these rows reference,
    // filtered to only the present `artifact_coordinate` values (Phase 7
    // artifact-preview consumer). Previews are populated in `AppState` by the
    // &mut ensure-hook (`ensure_artifact_previews`, run on feed-page apply) and
    // by the article/isbn/artifact fill hooks; here we just read + filter (D1).
    let mut seen = std::collections::BTreeSet::new();
    let artifact_previews: Vec<ArtifactPreviewRow> = rows
        .iter()
        .filter_map(|r| r.artifact_coordinate.as_deref())
        .filter(|coord| seen.insert(coord.to_string()))
        .filter_map(|coord| state.artifact_previews.get(coord).cloned())
        .collect();
    Some(ViewSnapshot::HomeFeed(KernelHomeFeedSnapshot {
        rows,
        artifact_previews,
    }))
}

/// Ensure an `artifact_previews` entry exists (pending or resolved) for every
/// coordinate the current home-feed rows reference. Idempotent. Run from the
/// &mut feed-page apply path so the next `project_home_feed_snapshot` can attach
/// resolved/pending previews; resolution is filled by the article/isbn/artifact
/// hooks. Returns effects (currently none — fills are synchronous from cache).
pub(crate) fn ensure_artifact_previews(state: &mut AppState) -> Vec<Effect> {
    let coords: Vec<String> = build_home_feed_rows(state)
        .into_iter()
        .filter_map(|r| r.artifact_coordinate)
        .collect();
    let mut effects = Vec::new();
    for coord in coords {
        effects.extend(super::artifact_preview::ensure_artifact_preview(
            state, coord,
        ));
    }
    effects
}

/// Build the merged, sorted, suppressed list of `HomeFeedRow`s.
///
/// Ported from `home_feed.rs::build_items` (live bespoke, 249L) with the
/// bespoke domain types replaced by the kernel's raw `FeedState` rows, and
/// extended (Phase 7 aggregation) with social fields and embedded highlights.
///
/// Step 1: decode highlights from `highlight_feed.rows`. Group by source_reference;
///   also decode full `HighlightRow`s (via the shared `decode_highlight_row`) for
///   embedding in the output rows. Track `a`-tag values for suppression.
///
/// Step 2: decode direct articles from `article_feed.rows`. Build a PendingRead map
///   keyed by article coordinate. Compute `author_followed` via `state.is_following`.
///
/// Step 3: decode interactions from `home_feed_interactions.rows`. For each event
///   authored by a follow with a resolvable article coordinate, update the
///   PendingRead's interactor map and latest-activity time.
///
/// Step 4: suppress direct article rows whose coordinate appears in the
///   highlighted-addresses set.
///
/// Step 5: emit highlight rows (with embedded `HighlightRow`s) and article rows
///   (with social fields).
///
/// Step 6: sort all rows by `latest_activity_at` / `sort_key` descending.
///   Tie-break by `stable_id` ascending for a deterministic render order.
pub(crate) fn build_home_feed_rows(state: &AppState) -> Vec<KernelHomeFeedRow> {
    // ── Step 1: decode highlights ────────────────────────────────────────────
    //
    // group_key → (raw structural entries, decoded HighlightRows)
    // Preserve insertion order via group_order.
    let mut group_map: HashMap<String, (Vec<RawHighlight>, Vec<HighlightRow>)> = HashMap::new();
    let mut group_order: Vec<String> = Vec::new();
    // Only `a`-tag source references — used for article suppression (Step 4).
    let mut highlighted_addresses: HashSet<String> = HashSet::new();

    for ev in &state.highlight_feed.rows {
        if ev.kind != 9802 || ev.content.is_empty() {
            continue; // skip malformed / wrong-kind rows (D6)
        }

        let extracted = extract_source_reference(&ev.tags);

        // Track the raw `a` tag value separately for suppression — only an `a`
        // tag (addressable coordinate) can match a kind:30023 article address.
        if let Some((_, ref raw_val, SourceRefKind::Address)) = extracted {
            highlighted_addresses.insert(raw_val.clone());
        }

        let (group_key_val, source_reference, source_ref_kind) = match extracted {
            Some((gk, raw, kind)) => (Some(gk), Some(raw), Some(kind)),
            None => (None, None, None),
        };

        let key = group_key_val.unwrap_or_else(|| format!("solo:{}", ev.id));

        if !group_map.contains_key(&key) {
            group_order.push(key.clone());
            group_map.insert(key.clone(), (Vec::new(), Vec::new()));
        }
        let entry = group_map.get_mut(&key).expect("key inserted above");
        entry.0.push(RawHighlight {
            event_id: ev.id.clone(),
            author_pubkey: ev.author.clone(),
            created_at: ev.created_at,
            source_reference: source_reference.clone(),
            source_ref_kind: source_ref_kind.clone(),
        });
        // Decode the enriched HighlightRow for embedding (Phase 7 aggregation).
        if let Some(hr) = decode_highlight_row(ev) {
            entry.1.push(hr);
        }
    }

    // ── Step 2: build article id→address map + PendingRead map ──────────────
    //
    // PendingRead accumulates article identity, author_followed, and the
    // interactor map (pubkey → latest interaction created_at) keyed by
    // article coordinate.

    // Maps event id → article coordinate for `e`-tag fallback in Step 3.
    let mut article_id_to_address: HashMap<String, String> = HashMap::new();
    // Preserves insertion order of article coordinates.
    let mut article_order: Vec<String> = Vec::new();

    struct PendingRead {
        // article_address is stored but read via the `reads` map key (addr).
        #[allow(dead_code)]
        article_address: String,
        article_id: String,
        article_author_pubkey: String,
        article_created_at: u64,
        author_followed: bool,
        /// pubkey → latest interaction created_at for that pubkey.
        interactors: BTreeMap<String, u64>,
        latest_activity_at: u64,
    }

    let mut reads: HashMap<String, PendingRead> = HashMap::new();

    for ev in &state.article_feed.rows {
        if ev.kind != 30023 {
            continue;
        }
        let d_tag = ev
            .tags
            .iter()
            .find(|t| t.first().map(|s| s == "d").unwrap_or(false))
            .and_then(|t| t.get(1))
            .cloned()
            .unwrap_or_default();
        let address = format!("{}:{}:{}", ev.kind, ev.author, d_tag);

        // Register the event id → address mapping for interaction fallback.
        article_id_to_address.insert(ev.id.clone(), address.clone());

        if !reads.contains_key(&address) {
            article_order.push(address.clone());
            reads.insert(
                address.clone(),
                PendingRead {
                    article_address: address.clone(),
                    article_id: ev.id.clone(),
                    article_author_pubkey: ev.author.clone(),
                    article_created_at: ev.created_at,
                    author_followed: state.is_following(&ev.author),
                    interactors: BTreeMap::new(),
                    latest_activity_at: ev.created_at,
                },
            );
        }
    }

    // ── Step 3: process interactions ─────────────────────────────────────────
    //
    // For each kind:1/7/16/1111 event authored by a follow with `#k=30023`,
    // resolve the target article coordinate and update the PendingRead.
    for ev in &state.home_feed_interactions.rows {
        // Must be authored by a follow.
        if !state.is_following(&ev.author) {
            continue;
        }
        // Must be an interaction kind.
        if !matches!(ev.kind, 1 | 7 | 16 | 1111) {
            continue;
        }
        // Must have `#k=30023` (already filtered by the feed scope, but
        // verify defensively — D6: no panics on unexpected rows).
        let has_k_30023 = ev.tags.iter().any(|t| {
            t.first().map(|s| s == "k").unwrap_or(false)
                && t.get(1).map(|v| v == "30023").unwrap_or(false)
        });
        if !has_k_30023 {
            continue;
        }

        // Resolve article coordinate: `a`/`A` tag first, then `e` fallback.
        let target_address: Option<String> = {
            let a_tag = ev
                .tags
                .iter()
                .find(|t| {
                    t.first().map(|s| s == "a" || s == "A").unwrap_or(false)
                        && t.get(1).map(|v| v.starts_with("30023:")).unwrap_or(false)
                })
                .and_then(|t| t.get(1))
                .cloned();

            if a_tag.is_some() {
                a_tag
            } else {
                // `e`-tag fallback: map known article event id → address.
                ev.tags
                    .iter()
                    .find(|t| t.first().map(|s| s == "e").unwrap_or(false))
                    .and_then(|t| t.get(1))
                    .and_then(|eid| article_id_to_address.get(eid).cloned())
            }
        };

        let Some(addr) = target_address else {
            continue; // no resolvable article target — ignore (D6)
        };

        // Update or create a PendingRead for this coordinate.
        let pr = reads.entry(addr.clone()).or_insert_with(|| {
            if !article_order.contains(&addr) {
                article_order.push(addr.clone());
            }
            // Parse author from the coordinate `30023:<pubkey>:<d>`.
            let author = addr.split(':').nth(1).unwrap_or("").to_string();
            PendingRead {
                article_address: addr.clone(),
                article_id: String::new(),
                article_author_pubkey: author.clone(),
                article_created_at: 0,
                author_followed: state.is_following(&author),
                interactors: BTreeMap::new(),
                latest_activity_at: 0,
            }
        });

        // Upsert interactor: track the max created_at per pubkey.
        let entry = pr.interactors.entry(ev.author.clone()).or_insert(0);
        *entry = (*entry).max(ev.created_at);
        pr.latest_activity_at = pr
            .latest_activity_at
            .max(ev.created_at)
            .max(pr.article_created_at);
    }

    // ── Step 5: emit output rows ─────────────────────────────────────────────

    let mut rows: Vec<KernelHomeFeedRow> = Vec::new();

    // Add highlight groups (insertion-ordered), oldest-first within each group.
    for key in &group_order {
        let (mut group, mut hl_rows) = group_map.remove(key).unwrap_or_default();
        // Sort structural entries by created_at ascending (oldest first within group).
        group.sort_by_key(|h| h.created_at);
        // Sort decoded HighlightRows to match (oldest first — design §128).
        hl_rows.sort_unstable_by(|a, b| {
            a.created_at
                .cmp(&b.created_at)
                .then_with(|| a.event_id.cmp(&b.event_id))
        });
        // Dedup decoded rows by event_id (feed may deliver duplicates).
        {
            let mut seen = HashSet::new();
            hl_rows.retain(|r| seen.insert(r.event_id.clone()));
        }

        let sort_key = group.iter().map(|h| h.created_at).max().unwrap_or(0);
        let latest_activity_at = sort_key;
        let stable_id = highlight_stable_id(&group);
        let highlight_event_ids: Vec<String> = group.iter().map(|h| h.event_id.clone()).collect();
        let highlight_author_pubkeys: Vec<String> =
            group.iter().map(|h| h.author_pubkey.clone()).collect();
        let source_reference = group.first().and_then(|h| h.source_reference.clone());
        let artifact_coordinate = source_reference.as_deref().and_then(|src| {
            let tag = if src.contains(':') { "a" } else { "e" };
            super::artifact_preview::coordinate_key(tag, src)
        });

        rows.push(KernelHomeFeedRow {
            stable_id,
            sort_key,
            kind: KernelHomeFeedRowKind::Highlight,
            highlight_event_ids,
            highlight_author_pubkeys,
            source_reference,
            highlights: hl_rows,
            article_address: None,
            article_id: None,
            article_author_pubkey: None,
            article_created_at: None,
            artifact_coordinate,
            // Highlight rows: author_followed = false, interactor_pubkeys = [] per design §128.
            author_followed: false,
            interactor_pubkeys: Vec::new(),
            latest_activity_at,
        });
    }

    // Add article rows NOT already surfaced by a highlight group (suppression).
    for addr in &article_order {
        if highlighted_addresses.contains(addr) {
            continue;
        }

        let Some(pr) = reads.get(addr) else {
            continue;
        };
        // Skip interaction-only rows (no article_created_at, no article_id) unless
        // the article coordinate is known and a preview exists/can be requested.
        // For now: emit any row where the article_id is non-empty (direct article)
        // or where at least one interactor exists (social-read card).
        if pr.article_id.is_empty() && pr.interactors.is_empty() {
            continue;
        }

        // Derive interactor_pubkeys: sort by latest interaction time desc, then
        // pubkey asc as tie-break (design §191-193).
        let mut interactor_vec: Vec<(&String, &u64)> = pr.interactors.iter().collect();
        interactor_vec.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
        let interactor_pubkeys: Vec<String> = interactor_vec
            .into_iter()
            .map(|(pk, _)| pk.clone())
            .collect();

        let latest_activity_at = pr.latest_activity_at.max(pr.article_created_at);
        let sort_key = latest_activity_at;
        let artifact_coordinate = super::artifact_preview::coordinate_key("a", addr);

        rows.push(KernelHomeFeedRow {
            stable_id: format!("r:{}", addr),
            sort_key,
            kind: KernelHomeFeedRowKind::Article,
            highlight_event_ids: Vec::new(),
            highlight_author_pubkeys: Vec::new(),
            source_reference: None,
            highlights: Vec::new(),
            article_address: Some(addr.clone()),
            article_id: if pr.article_id.is_empty() {
                None
            } else {
                Some(pr.article_id.clone())
            },
            article_author_pubkey: if pr.article_author_pubkey.is_empty() {
                None
            } else {
                Some(pr.article_author_pubkey.clone())
            },
            article_created_at: if pr.article_created_at == 0 {
                None
            } else {
                Some(pr.article_created_at)
            },
            artifact_coordinate,
            author_followed: pr.author_followed,
            interactor_pubkeys,
            latest_activity_at,
        });
    }

    // ── Step 6: sort by sort_key descending, stable_id ascending tie-break ───
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

    // 4J-T5 (updated Phase 7): home_feed_opens_all_underlying_feeds
    //
    // Opening ViewId::HomeFeed must emit RegisterFeedCursor + DrainFeed for all
    // three underlying cursors: article feed, highlight feed, and the Phase 7
    // home-interactions feed. All three require follows (fail-closed when empty).
    // With follows seeded, all three RegisterFeedCursor + DrainFeed effects are present.
    #[test]
    fn home_feed_opens_both_underlying_feeds() {
        let mut state = make_state();
        // Seed follows so the article + interaction feeds are not fail-closed.
        state.follows =
            vec!["aabbcc0000000000000000000000000000000000000000000000000000000001".to_string()];

        let effects = lifecycle_effects_for_view_open(&ViewId::HomeFeed, &state);

        let register_count = effects
            .iter()
            .filter(|e| matches!(e, Effect::RegisterFeedCursor { .. }))
            .count();
        assert_eq!(
            register_count, 3,
            "HomeFeed open must emit RegisterFeedCursor for article, highlight, and home-interaction feeds"
        );

        let drain_count = effects
            .iter()
            .filter(|e| matches!(e, Effect::DrainFeed { .. }))
            .count();
        assert_eq!(
            drain_count, 3,
            "HomeFeed open must emit DrainFeed for all three feeds"
        );

        // Confirm the home-interaction cursor key is registered.
        assert!(
            effects.iter().any(|e| matches!(
                e,
                Effect::RegisterFeedCursor { key, .. } if key == HOME_INTERACTIONS_FEED_KEY
            )),
            "HomeFeed open must register the home-interaction feed cursor"
        );
    }

    // #1653 codex r5 gap #1: after an account switch wipes ALL of HomeFeed's
    // cursors (cursor_id == 0), the new account's FollowListUpdated re-fires the
    // follow-update hook, which MUST re-register EVERY wiped open cursor — including
    // the (non-follow-scoped) highlight feed. Pre-fix the hook re-registered only
    // article_feed + home_interactions, leaving the highlight cursor stuck at 0.
    #[test]
    fn follow_update_reregisters_all_wiped_open_cursors_including_highlight() {
        let mut state = make_state();
        // Post-switch state: follows arrived for the new account; ALL HomeFeed
        // cursors were reset to default (cursor_id == 0) by the teardown.
        state.follows =
            vec!["aabbcc0000000000000000000000000000000000000000000000000000000001".to_string()];
        assert_eq!(state.article_feed.cursor_id, 0);
        assert_eq!(state.highlight_feed.cursor_id, 0);
        assert_eq!(state.home_feed_interactions.cursor_id, 0);

        let effects = lifecycle_effects_for_follow_update(&state);

        // All three wiped cursors must re-register.
        let registered_keys: Vec<&String> = effects
            .iter()
            .filter_map(|e| match e {
                Effect::RegisterFeedCursor { key, .. } => Some(key),
                _ => None,
            })
            .collect();
        assert!(
            registered_keys
                .iter()
                .any(|k| k.as_str() == crate::kernel::domains::articles_feed::ARTICLE_FEED_KEY),
            "article_feed must re-register, got {registered_keys:?}"
        );
        assert!(
            registered_keys
                .iter()
                .any(|k| k.as_str() == crate::kernel::domains::highlight_feed::HIGHLIGHT_FEED_KEY),
            "highlight_feed must re-register after switch (gap #1), got {registered_keys:?}"
        );
        assert!(
            registered_keys
                .iter()
                .any(|k| k.as_str() == HOME_INTERACTIONS_FEED_KEY),
            "home_feed_interactions must re-register, got {registered_keys:?}"
        );
    }

    // Once the highlight feed has been registered (cursor_id != 0), a later
    // follow-update must NOT re-register it (idempotent — no churn).
    #[test]
    fn follow_update_does_not_rereregister_live_highlight_cursor() {
        let mut state = make_state();
        state.follows =
            vec!["aabbcc0000000000000000000000000000000000000000000000000000000001".to_string()];
        state.highlight_feed.cursor_id = 99; // already live

        let effects = lifecycle_effects_for_follow_update(&state);

        assert!(
            !effects.iter().any(|e| matches!(
                e,
                Effect::RegisterFeedCursor { key, .. }
                    if key == crate::kernel::domains::highlight_feed::HIGHLIGHT_FEED_KEY
            )),
            "live highlight cursor must NOT re-register (no churn), got {effects:?}"
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

    // Phase 7 artifact-preview consumer: a standalone article row carries an
    // `a:` artifact_coordinate, and after ensure_artifact_previews the snapshot
    // attaches the resolved (non-pending, titled) preview keyed by that coord.
    #[test]
    fn home_feed_attaches_resolved_article_preview() {
        let mut state = make_state();
        let pubkey = "cccc".repeat(16);
        let d_tag = "preview-article";
        let address = format!("30023:{pubkey}:{d_tag}");
        let coordinate = format!("a:{address}");

        // Standalone article (no highlight → not suppressed).
        let art = article_ev(
            "art000000000000000000000000000000000000000000000000000000000000009",
            &pubkey,
            d_tag,
            1_700_000_005,
        );
        apply_feed_page(&mut state.article_feed, vec![art], 10, false, None);

        // Resolution source: the article is in AppState::articles.
        state.articles.insert(
            address.clone(),
            crate::kernel::snapshot::ArticleRow {
                address: address.clone(),
                id: "dddd".repeat(16),
                author_pubkey: pubkey.clone(),
                author_display_name: None,
                author_picture_url: None,
                title: Some("Preview Title".to_string()),
                summary: Some("Preview summary.".to_string()),
                hero_image_url: Some("https://example.com/p.jpg".to_string()),
                d_tag: d_tag.to_string(),
                created_at: 1_700_000_005,
                content_tree_bytes: vec![],
            },
        );

        // &mut hook (as run on feed-page apply) then immutable projection.
        ensure_artifact_previews(&mut state);
        let snap = project_home_feed_snapshot(&state).unwrap();
        match snap {
            ViewSnapshot::HomeFeed(s) => {
                let row = s
                    .rows
                    .iter()
                    .find(|r| r.kind == KernelHomeFeedRowKind::Article)
                    .expect("article row present");
                assert_eq!(
                    row.artifact_coordinate.as_deref(),
                    Some(coordinate.as_str()),
                    "article row carries its a: coordinate"
                );
                let preview = s
                    .artifact_previews
                    .iter()
                    .find(|p| p.coordinate == coordinate)
                    .expect("preview attached for the referenced coordinate");
                assert!(!preview.pending, "resolved from AppState::articles");
                assert_eq!(preview.title.as_deref(), Some("Preview Title"));
            }
            other => panic!("expected HomeFeed snapshot, got {:?}", other),
        }
    }

    // ── Phase 7 home-feed aggregation tests ──────────────────────────────────

    /// Build a minimal interaction event (kind 1, 7, 16, or 1111).
    fn interaction_ev(
        id: &str,
        pubkey: &str,
        kind: u32,
        article_address: &str,
        created_at: u64,
    ) -> nmp_core::substrate::KernelEvent {
        let tags = vec![
            vec!["a".to_string(), article_address.to_string()],
            vec!["k".to_string(), "30023".to_string()],
        ];
        nmp_core::substrate::KernelEvent {
            id: id.to_string(),
            author: pubkey.to_string(),
            kind,
            created_at,
            tags,
            content: String::new(),
            relay_provenance: vec![],
        }
    }

    // 7-HF1: home_feed_article_author_followed_true
    //
    // When the article author is in the follow set, author_followed must be true
    // and interactor_pubkeys must be empty (no interactions seeded).
    #[test]
    fn home_feed_article_author_followed_true() {
        let mut state = make_state();
        let author = "aabb".repeat(16);
        state.follows = vec![author.clone()];

        let art = article_ev("art1".repeat(16).as_str(), &author, "d1", 1_000);
        apply_feed_page(&mut state.article_feed, vec![art], 10, false, None);

        let snap = project_home_feed_snapshot(&state).unwrap();
        match snap {
            ViewSnapshot::HomeFeed(s) => {
                let row = s
                    .rows
                    .iter()
                    .find(|r| r.kind == KernelHomeFeedRowKind::Article)
                    .expect("article row present");
                assert!(
                    row.author_followed,
                    "author in follows → author_followed == true"
                );
                assert!(
                    row.interactor_pubkeys.is_empty(),
                    "no interactions → interactor_pubkeys empty"
                );
            }
            other => panic!("expected HomeFeed, got {other:?}"),
        }
    }

    // 7-HF2: home_feed_article_author_followed_false_social_surface
    //
    // When a follow (alice) interacts with a non-follow author's article via `a`
    // tag, the row must have author_followed=false and interactor_pubkeys=[alice].
    #[test]
    fn home_feed_article_author_followed_false_social_surface() {
        let mut state = make_state();
        let alice = "aa00".repeat(16);
        let bob = "bb00".repeat(16);
        state.follows = vec![alice.clone()];

        // Bob's article (bob is not followed).
        let art = article_ev("a001".repeat(16).as_str(), &bob, "d-bob", 100);
        apply_feed_page(&mut state.article_feed, vec![art], 10, false, None);

        // Alice interacts with Bob's article.
        let addr = format!("30023:{}:d-bob", bob);
        let interaction = interaction_ev("i001".repeat(16).as_str(), &alice, 7, &addr, 200);
        apply_feed_page(
            &mut state.home_feed_interactions,
            vec![interaction],
            10,
            false,
            None,
        );

        let snap = project_home_feed_snapshot(&state).unwrap();
        match snap {
            ViewSnapshot::HomeFeed(s) => {
                let row = s
                    .rows
                    .iter()
                    .find(|r| r.kind == KernelHomeFeedRowKind::Article)
                    .expect("article row present");
                assert!(
                    !row.author_followed,
                    "bob not followed → author_followed = false"
                );
                assert_eq!(
                    row.interactor_pubkeys,
                    vec![alice.clone()],
                    "alice interacted → interactor_pubkeys=[alice]"
                );
            }
            other => panic!("expected HomeFeed, got {other:?}"),
        }
    }

    // 7-HF3: home_feed_interactors_filter_to_follows
    //
    // Interactions from non-follows must be excluded from interactor_pubkeys.
    #[test]
    fn home_feed_interactors_filter_to_follows() {
        let mut state = make_state();
        let alice = "aa11".repeat(16);
        let mallory = "cc11".repeat(16);
        let bob = "bb11".repeat(16);
        state.follows = vec![alice.clone()]; // mallory is NOT followed

        let art = article_ev("a002".repeat(16).as_str(), &bob, "d-bob2", 100);
        apply_feed_page(&mut state.article_feed, vec![art], 10, false, None);

        let addr = format!("30023:{}:d-bob2", bob);
        // Alice (follow) interacts.
        let ia = interaction_ev("i002".repeat(16).as_str(), &alice, 7, &addr, 200);
        // Mallory (non-follow) interacts — must be filtered.
        let im = interaction_ev("i003".repeat(16).as_str(), &mallory, 7, &addr, 300);
        apply_feed_page(
            &mut state.home_feed_interactions,
            vec![ia, im],
            10,
            false,
            None,
        );

        let snap = project_home_feed_snapshot(&state).unwrap();
        match snap {
            ViewSnapshot::HomeFeed(s) => {
                let row = s
                    .rows
                    .iter()
                    .find(|r| r.kind == KernelHomeFeedRowKind::Article)
                    .expect("article row");
                assert_eq!(row.interactor_pubkeys, vec![alice.clone()]);
                assert!(
                    !row.interactor_pubkeys.contains(&mallory),
                    "mallory not a follow — must be excluded"
                );
            }
            other => panic!("expected HomeFeed, got {other:?}"),
        }
    }

    // 7-HF4: home_feed_interactors_dedupe_and_order
    //
    // Multiple interactions by the same pubkey must be deduped.
    // Sort order: latest-interaction-at desc, then pubkey asc as tie-break.
    #[test]
    fn home_feed_interactors_dedupe_and_order() {
        let mut state = make_state();
        let alice = "aa22".repeat(16);
        let bob_author = "bb22".repeat(16);
        let charlie = "cc22".repeat(16);
        state.follows = vec![alice.clone(), charlie.clone()];

        let art = article_ev("a003".repeat(16).as_str(), &bob_author, "d3", 100);
        apply_feed_page(&mut state.article_feed, vec![art], 10, false, None);

        let addr = format!("30023:{}:d3", bob_author);
        // Alice interacts twice (different events, same pubkey) — only latest counts.
        let ia1 = interaction_ev("i004".repeat(16).as_str(), &alice, 7, &addr, 100);
        let ia2 = interaction_ev("i005".repeat(16).as_str(), &alice, 1, &addr, 150);
        // Charlie interacts once, newer than alice's latest.
        let ic = interaction_ev("i006".repeat(16).as_str(), &charlie, 7, &addr, 200);
        apply_feed_page(
            &mut state.home_feed_interactions,
            vec![ia1, ia2, ic],
            10,
            false,
            None,
        );

        let snap = project_home_feed_snapshot(&state).unwrap();
        match snap {
            ViewSnapshot::HomeFeed(s) => {
                let row = s
                    .rows
                    .iter()
                    .find(|r| r.kind == KernelHomeFeedRowKind::Article)
                    .expect("article row");
                // charlie (latest at 200) before alice (latest at 150), alice deduped.
                assert_eq!(
                    row.interactor_pubkeys,
                    vec![charlie.clone(), alice.clone()],
                    "sort by latest interaction desc; alice deduped"
                );
            }
            other => panic!("expected HomeFeed, got {other:?}"),
        }
    }

    // 7-HF5: home_feed_latest_activity_uses_interaction_max
    //
    // latest_activity_at must be max(article_created_at, max(interaction created_ats)).
    #[test]
    fn home_feed_latest_activity_uses_interaction_max() {
        let mut state = make_state();
        let alice = "aa33".repeat(16);
        let bob_author = "bb33".repeat(16);
        state.follows = vec![alice.clone()];

        let art = article_ev("a004".repeat(16).as_str(), &bob_author, "d4", 100);
        apply_feed_page(&mut state.article_feed, vec![art], 10, false, None);

        let addr = format!("30023:{}:d4", bob_author);
        let i1 = interaction_ev("i007".repeat(16).as_str(), &alice, 7, &addr, 300);
        let i2 = interaction_ev("i008".repeat(16).as_str(), &alice, 16, &addr, 250);
        apply_feed_page(
            &mut state.home_feed_interactions,
            vec![i1, i2],
            10,
            false,
            None,
        );

        let snap = project_home_feed_snapshot(&state).unwrap();
        match snap {
            ViewSnapshot::HomeFeed(s) => {
                let row = s
                    .rows
                    .iter()
                    .find(|r| r.kind == KernelHomeFeedRowKind::Article)
                    .expect("article row");
                assert_eq!(
                    row.latest_activity_at, 300,
                    "max of article(100) + interactions(300, 250) = 300"
                );
                assert_eq!(row.sort_key, 300, "sort_key == latest_activity_at");
            }
            other => panic!("expected HomeFeed, got {other:?}"),
        }
    }

    // 7-HF6: home_feed_direct_article_suppressed_by_highlight (existing 4J-T2 still passes;
    // this adds Phase 7 field assertions to the suppression case)
    #[test]
    fn home_feed_highlight_rows_have_empty_social_fields() {
        let mut state = make_state();
        let pubkey = "ee44".repeat(16);
        state.follows = vec![pubkey.clone()];

        // Highlight with a source reference.
        let source = format!("30023:{}:d-hl", pubkey);
        let hl = highlight_ev("hl01".repeat(16).as_str(), &pubkey, &source, 500);
        apply_feed_page(&mut state.highlight_feed, vec![hl], 10, false, None);

        let snap = project_home_feed_snapshot(&state).unwrap();
        match snap {
            ViewSnapshot::HomeFeed(s) => {
                let row = s
                    .rows
                    .iter()
                    .find(|r| r.kind == KernelHomeFeedRowKind::Highlight)
                    .expect("highlight row");
                assert!(
                    !row.author_followed,
                    "highlight rows: author_followed always false"
                );
                assert!(
                    row.interactor_pubkeys.is_empty(),
                    "highlight rows: interactor_pubkeys always empty"
                );
            }
            other => panic!("expected HomeFeed, got {other:?}"),
        }
    }

    // 7-HF7: home_feed_embeds_highlight_rows
    //
    // A highlight group row must embed decoded HighlightRows matching the raw events.
    // Two highlights on the same source → one row with highlights.len()==2, oldest-first.
    #[test]
    fn home_feed_embeds_highlight_rows() {
        let mut state = make_state();
        let pk = "ff55".repeat(16);
        let source = format!("30023:{}:d-embed", pk);

        let hl1 = highlight_ev("hl1a".repeat(16).as_str(), &pk, &source, 100);
        let hl2 = highlight_ev("hl1b".repeat(16).as_str(), &pk, &source, 200);
        let hl1_id = hl1.id.clone();
        let hl2_id = hl2.id.clone();
        apply_feed_page(&mut state.highlight_feed, vec![hl1, hl2], 10, false, None);

        let snap = project_home_feed_snapshot(&state).unwrap();
        match snap {
            ViewSnapshot::HomeFeed(s) => {
                let row = s
                    .rows
                    .iter()
                    .find(|r| r.kind == KernelHomeFeedRowKind::Highlight)
                    .expect("highlight group row");
                assert_eq!(row.highlights.len(), 2, "two highlights embedded");
                // Oldest-first within the group.
                assert_eq!(row.highlights[0].event_id, hl1_id, "oldest highlight first");
                assert_eq!(
                    row.highlights[1].event_id, hl2_id,
                    "newest highlight second"
                );
                // Content must match what the highlight_ev helper sets.
                assert_eq!(row.highlights[0].content, "highlighted text");
                assert_eq!(row.highlights[1].content, "highlighted text");
            }
            other => panic!("expected HomeFeed, got {other:?}"),
        }
    }

    // 7-HF8: home_feed_highlight_content_matches_highlight_feed_decoder
    //
    // The HighlightRow embedded in HomeFeed must equal the row emitted by
    // `project_highlight_feed_snapshot` for the same event.
    #[test]
    fn home_feed_highlight_content_matches_highlight_feed_decoder() {
        use super::super::highlight_feed::decode_highlight_row as direct_decode;

        let mut state = make_state();
        let pk = "aa66".repeat(16);
        let source = format!("30023:{}:d-parity", pk);

        let ev = highlight_ev("hl2a".repeat(16).as_str(), &pk, &source, 400);
        let expected_row = direct_decode(&ev).expect("decodes directly");
        apply_feed_page(&mut state.highlight_feed, vec![ev], 10, false, None);

        let snap = project_home_feed_snapshot(&state).unwrap();
        match snap {
            ViewSnapshot::HomeFeed(s) => {
                let row = s
                    .rows
                    .iter()
                    .find(|r| r.kind == KernelHomeFeedRowKind::Highlight)
                    .expect("highlight row");
                assert_eq!(row.highlights.len(), 1);
                let embedded = &row.highlights[0];
                assert_eq!(
                    *embedded, expected_row,
                    "embedded HighlightRow must equal direct decode"
                );
            }
            other => panic!("expected HomeFeed, got {other:?}"),
        }
    }

    // 7-HF9: home_feed_ignores_interaction_without_article_target
    //
    // An interaction with no `a`/`A` tag starting with "30023:" and no resolvable
    // `e` article id must be silently ignored (no panic, no spurious row).
    #[test]
    fn home_feed_ignores_interaction_without_article_target() {
        let mut state = make_state();
        let alice = "aa77".repeat(16);
        state.follows = vec![alice.clone()];

        // kind:7 with no `a` tag and no known article `e` target.
        let ev = nmp_core::substrate::KernelEvent {
            id: "i009".repeat(16),
            author: alice.clone(),
            kind: 7,
            created_at: 500,
            tags: vec![vec!["k".to_string(), "30023".to_string()]],
            content: "+".to_string(),
            relay_provenance: vec![],
        };
        apply_feed_page(&mut state.home_feed_interactions, vec![ev], 10, false, None);

        // Must not panic and must produce no social-read row.
        let snap = project_home_feed_snapshot(&state).unwrap();
        match snap {
            ViewSnapshot::HomeFeed(s) => {
                assert!(
                    s.rows.is_empty(),
                    "interaction with unresolvable target must not produce a row"
                );
            }
            other => panic!("expected HomeFeed, got {other:?}"),
        }
    }

    // 7-HF10: home_feed_registers_interaction_cursor_for_open_home
    //
    // lifecycle_effects_for_view_open(HomeFeed) with follows seeded must register
    // the home-interaction cursor (RegisterFeedCursor key == HOME_INTERACTIONS_FEED_KEY).
    #[test]
    fn home_feed_registers_interaction_cursor_for_open_home() {
        let mut state = make_state();
        state.follows = vec!["aa88".repeat(16)];

        let effects = lifecycle_effects_for_view_open(&ViewId::HomeFeed, &state);

        assert!(
            effects.iter().any(|e| matches!(
                e,
                Effect::RegisterFeedCursor { key, .. } if key == HOME_INTERACTIONS_FEED_KEY
            )),
            "open HomeFeed with follows must register interaction cursor"
        );
        assert!(
            effects.iter().any(|e| matches!(
                e,
                Effect::DrainFeed { key } if key == HOME_INTERACTIONS_FEED_KEY
            )),
            "open HomeFeed with follows must drain interaction cursor"
        );
    }

    // 7-HF11: home_feed_fail_closed_no_follows_for_interactions
    //
    // With no follows, neither the article feed nor the interaction feed cursor
    // should be registered (fail-closed D5). Only the highlight cursor is always
    // registered (it's global, not follow-scoped).
    #[test]
    fn home_feed_fail_closed_no_follows_for_interactions() {
        let state = make_state(); // follows is empty

        let effects = lifecycle_effects_for_view_open(&ViewId::HomeFeed, &state);

        // Highlight cursor: always registered.
        use crate::kernel::domains::highlight_feed::HIGHLIGHT_FEED_KEY;
        assert!(
            effects.iter().any(|e| matches!(
                e,
                Effect::RegisterFeedCursor { key, .. } if key == HIGHLIGHT_FEED_KEY
            )),
            "highlight cursor must register even with no follows"
        );

        // Interaction cursor: must NOT be registered (fail-closed).
        assert!(
            !effects.iter().any(|e| matches!(
                e,
                Effect::RegisterFeedCursor { key, .. } if key == HOME_INTERACTIONS_FEED_KEY
            )),
            "interaction cursor must NOT register when follows is empty"
        );
    }

    // 7-HF12: home_feed_follow_update_registers_missing_cursors
    //
    // `lifecycle_effects_for_follow_update` emits RegisterFeedCursor for the
    // interaction cursor when follows arrive and cursor_id is still 0.
    #[test]
    fn home_feed_follow_update_registers_missing_cursors() {
        let mut state = make_state();
        // Simulate: HomeFeed opened while follows was empty (cursor_id stays 0).
        // Now follows have arrived.
        state.follows = vec!["aa99".repeat(16)];
        // home_feed_interactions.cursor_id is 0 (not yet registered).
        assert_eq!(state.home_feed_interactions.cursor_id, 0);

        let effects = lifecycle_effects_for_follow_update(&state);

        assert!(
            effects.iter().any(|e| matches!(
                e,
                Effect::RegisterFeedCursor { key, .. } if key == HOME_INTERACTIONS_FEED_KEY
            )),
            "follow update must register missing interaction cursor"
        );
    }

    // 7-HF13: home_feed_interaction_cleared_on_logout
    //
    // After Logout, home_feed_interactions must be cleared so stale follow-authored
    // interactions from the prior account do not surface under a new account.
    #[test]
    fn home_feed_interaction_cleared_on_logout() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let alice = "aa00".repeat(16);
        let bob = "bb00".repeat(16);
        state.follows = vec![alice.clone()];

        // Seed the interaction feed.
        let art = article_ev("a010".repeat(16).as_str(), &bob, "d10", 100);
        apply_feed_page(&mut state.article_feed, vec![art], 10, false, None);
        let addr = format!("30023:{}:d10", bob);
        let ia = interaction_ev("i010".repeat(16).as_str(), &alice, 7, &addr, 200);
        apply_feed_page(&mut state.home_feed_interactions, vec![ia], 10, false, None);

        assert!(
            !state.home_feed_interactions.rows.is_empty(),
            "interactions seeded"
        );

        step(&mut state, &clock, Cmd::Action(AppAction::Logout));

        assert!(
            state.home_feed_interactions.rows.is_empty(),
            "home_feed_interactions must be cleared on Logout"
        );
    }
}
