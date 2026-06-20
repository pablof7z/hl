//! Artifact-preview kernel projection — Phase 7 (artifact-preview slice).
//!
//! ## Responsibilities
//!
//! Maintains `AppState::artifact_previews` — a coordinate-keyed map of
//! lightweight preview rows used by 6 content screens (HomeFeed, ArticleFeed,
//! HighlightDetail, RoomHome article cards, ShareComposer, ArticleReader header).
//!
//! ## Coordinate key scheme (D1 / canonical form)
//!
//! | Source tag     | Coordinate key           | Example                                  |
//! |----------------|--------------------------|------------------------------------------|
//! | `a:kind:pk:d`  | `a:kind:pk:d`            | `a:30023:deadbeef:my-article`            |
//! | `e:<id>`       | `e:<64-char hex>`        | `e:deadbeef...`                          |
//! | `i:isbn:…`     | `i:isbn:9780735211292`   | `i:isbn:9780735211292`                   |
//! | `i:podcast:…`  | `i:podcast:item:guid:…`  | `i:podcast:item:guid:abc123`             |
//! | `r:<url>`      | `r:<url>`                | `r:https://example.com/article`          |
//!
//! ## Resolution strategy (D8: effect-driven, no polling)
//!
//! 1. `ensure_artifact_preview(state, coordinate)` is the public entry point.
//!    Callers (other domain reducers or action arms) call this when they need a
//!    preview row and do not yet have one. It is idempotent — safe to call
//!    repeatedly for the same coordinate.
//!
//! 2. For `a:` coordinates whose kind is 30023:
//!    if the article already lives in `AppState::articles`, build a non-pending
//!    row immediately from the `ArticleRow` fields. Otherwise insert a pending row
//!    and emit `Effect::ResolveArtifactCoordinate` (deduped via
//!    `AppState::artifact_preview_requests`).
//!
//! 3. For `i:isbn:…`:
//!    reuse the existing ISBN domain — emit `Effect::LookupIsbn` and let the ISBN
//!    event cycle (`KernelEvent::IsbnPreviewReady`) fill the preview row via
//!    `fill_from_isbn_result`. No second ISBN fetcher (D4).
//!
//! 4. For everything else (`e:`, `i:podcast:…`, `r:`):
//!    pending row + deduplicated `Effect::ResolveArtifactCoordinate`. The effect
//!    runner in actor.rs lowers this to the appropriate nmp interest or no-op.
//!    Web URLs get a minimal non-pending row from the URL itself (no web scraper —
//!    real web-metadata fetch is product-gated and DEFERRED).
//!
//! 5. When `AppState::articles` is updated (`KernelEvent::ArticlesUpdated`) or a
//!    kind:11 artifact-metadata event arrives (`KernelEvent::ArtifactPreviewFilled`),
//!    we upsert matching preview rows as non-pending and clear their request entries.
//!
//! ## D-rules satisfied
//!
//! * D1 — `ArtifactPreviewRow` carries raw protocol fields only (no formatted strings).
//! * D4 — reuses `AppState::articles` and the ISBN domain; no duplication.
//! * D6 — malformed coordinates are a no-op; effect runner never panics.
//! * D8 — one effect per missing coordinate, no polling.
//! * D9 — `created_at` comes from the nostr event; kernel never stamps time.
//!
//! ## Logout / identity change
//!
//! `clear_on_identity_lost` wipes both `artifact_previews` and
//! `artifact_preview_requests` so stale data never leaks across sessions.

use std::collections::BTreeMap;

use crate::kernel::app::AppState;
use crate::kernel::domains::isbn::KernelArtifactPreview;
use crate::kernel::effect::Effect;
use crate::kernel::snapshot::{ArticleRow, ArtifactPreviewKind, ArtifactPreviewRow};

// ─── Coordinate key helpers ──────────────────────────────────────────────────

/// Parse a raw source tag pair (`tag_name`, `tag_value`) into a canonical
/// coordinate key string for `AppState::artifact_previews`.
///
/// Supported canonical forms (per design §Coordinate Keys):
/// - `("a", "30023:pk:d")` → `"a:30023:pk:d"`
/// - `("e", "<hex>")` → `"e:<hex>"`
/// - `("i", "isbn:<isbn13>")` → `"i:isbn:<isbn13>"`
/// - `("i", "podcast:item:guid:<guid>")` → `"i:podcast:item:guid:<guid>"`
/// - `("r", "<url>")` → `"r:<url>"`
///
/// Returns `None` for empty tag values (D6 no-op).
///
/// Public API for future per-screen consumers that receive raw nostr tag pairs.
#[allow(dead_code)]
pub(crate) fn coordinate_key(tag_name: &str, tag_value: &str) -> Option<String> {
    let v = tag_value.trim();
    if v.is_empty() {
        return None;
    }
    match tag_name {
        "a" | "e" | "i" | "r" => Some(format!("{tag_name}:{v}")),
        _ => None,
    }
}

/// Parse a coordinate key string back into its tag-name / tag-value components.
///
/// Returns `None` for keys that do not start with a known tag prefix.
pub(crate) fn parse_coordinate_key(coordinate: &str) -> Option<(&str, &str)> {
    let (tag, rest) = coordinate.split_once(':')?;
    match tag {
        "a" | "e" | "i" | "r" => Some((tag, rest)),
        _ => None,
    }
}

// ─── Kind inference ──────────────────────────────────────────────────────────

/// Infer the `ArtifactPreviewKind` from a canonical coordinate key.
pub(crate) fn kind_from_coordinate(coordinate: &str) -> ArtifactPreviewKind {
    // Use parse_coordinate_key to extract the tag so we're consistent with
    // the canonical parsing logic.
    let Some((tag, rest)) = parse_coordinate_key(coordinate) else {
        return ArtifactPreviewKind::Unknown;
    };
    match tag {
        "a" => {
            // kind:30023 → Article; other addressable kinds are Unknown for now.
            if rest.starts_with("30023:") {
                ArtifactPreviewKind::Article
            } else {
                ArtifactPreviewKind::Unknown
            }
        }
        "i" => {
            if rest.starts_with("isbn:") {
                ArtifactPreviewKind::Book
            } else if rest.starts_with("podcast:") {
                ArtifactPreviewKind::Podcast
            } else {
                ArtifactPreviewKind::Unknown
            }
        }
        "r" => ArtifactPreviewKind::Web,
        _ => ArtifactPreviewKind::Unknown,
    }
}

/// Build a canonical coordinate key from a tag name + value pair and immediately
/// call `ensure_artifact_preview` on it. Convenience wrapper for callers that
/// receive raw nostr tags (tag name + tag value) rather than a pre-built key.
///
/// Returns empty effects for unknown tag names or empty tag values (D6 no-op).
///
/// Public API for future per-screen consumers that receive raw nostr tag pairs.
#[allow(dead_code)]
pub(crate) fn ensure_artifact_preview_from_tag(
    state: &mut AppState,
    tag_name: &str,
    tag_value: &str,
) -> Vec<Effect> {
    match coordinate_key(tag_name, tag_value) {
        Some(coord) => ensure_artifact_preview(state, coord),
        None => vec![],
    }
}

// ─── Public entry point ──────────────────────────────────────────────────────

/// Ensure a preview row exists for `coordinate`.
///
/// - If an entry already exists (pending or not), return empty effects (idempotent).
/// - For `a:30023:pk:d` coordinates whose article lives in `AppState::articles`,
///   upsert a non-pending row immediately from the `ArticleRow` (no effect needed).
/// - For `i:isbn:…` coordinates: insert pending row + emit `Effect::LookupIsbn`
///   (reuses the ISBN domain — D4 no duplication).
/// - For all other coordinates: insert pending row + emit
///   `Effect::ResolveArtifactCoordinate` (deduped via
///   `AppState::artifact_preview_requests`).
/// - Web `r:` URLs: build a minimal non-pending row from the URL itself (no scraper).
///
/// D8: one effect per missing coordinate; D6: no-op on empty / malformed input.
pub(crate) fn ensure_artifact_preview(state: &mut AppState, coordinate: String) -> Vec<Effect> {
    if coordinate.is_empty() {
        return vec![];
    }

    // Already have a row (pending or resolved) — idempotent.
    if state.artifact_previews.contains_key(&coordinate) {
        return vec![];
    }

    // ── a:30023 — article coordinate ─────────────────────────────────────────
    if let Some(article_key) = article_key_from_coordinate(&coordinate) {
        if let Some(row) = state.articles.get(article_key) {
            // Article already in memory — build non-pending row immediately.
            let preview = article_row_to_preview(coordinate.clone(), row);
            state.artifact_previews.insert(coordinate, preview);
            return vec![];
        }
    }

    // ── r: URL — minimal non-pending row, no scraper ─────────────────────────
    if coordinate.starts_with("r:") {
        let url = coordinate.trim_start_matches("r:").to_string();
        state.artifact_previews.insert(
            coordinate.clone(),
            ArtifactPreviewRow {
                coordinate: coordinate.clone(),
                title: None,
                image_url: None,
                author_pubkey: None,
                summary: None,
                kind: ArtifactPreviewKind::Web,
                pending: false,
                // Store the URL as the title fallback hint in the url field.
                // Real web-metadata fetch is deferred / product-gated.
                display_url: Some(url),
            },
        );
        return vec![];
    }

    // ── i:isbn: — reuse ISBN domain ──────────────────────────────────────────
    if let Some(isbn13) = isbn13_from_coordinate(&coordinate) {
        state.artifact_previews.insert(
            coordinate.clone(),
            ArtifactPreviewRow {
                coordinate: coordinate.clone(),
                title: None,
                image_url: None,
                author_pubkey: None,
                summary: None,
                kind: ArtifactPreviewKind::Book,
                pending: true,
                display_url: None,
            },
        );
        // Reuse the existing ISBN domain effect — it will emit IsbnPreviewReady
        // which we handle in fill_from_isbn_result. D4: no second fetcher.
        return vec![Effect::LookupIsbn { isbn13 }];
    }

    // ── e: / i:podcast: / a:(non-30023) — ResolveArtifactCoordinate ─────────
    let kind = kind_from_coordinate(&coordinate);
    state.artifact_previews.insert(
        coordinate.clone(),
        ArtifactPreviewRow {
            coordinate: coordinate.clone(),
            title: None,
            image_url: None,
            author_pubkey: None,
            summary: None,
            kind,
            pending: true,
            display_url: None,
        },
    );

    // Emit a resolve effect only if not already in flight (D8: dedupe).
    if state.artifact_preview_requests.contains(&coordinate) {
        return vec![];
    }
    state.artifact_preview_requests.insert(coordinate.clone());
    vec![Effect::ResolveArtifactCoordinate { coordinate }]
}

// ─── Observer / fill paths ───────────────────────────────────────────────────

/// Called when `AppState::articles` is replaced (from `ArticlesUpdated`).
///
/// For every `a:30023:pk:d` coordinate in `artifact_previews` that is still
/// pending, check the updated articles map and fill if the article is now present.
/// Also handles the reverse: if an article just arrived that matches a pending
/// preview, upsert immediately.
///
/// Returns empty Vec (no side-effects needed — preview updates are purely in-memory).
pub(crate) fn on_articles_updated(
    state: &mut AppState,
    articles: &BTreeMap<String, ArticleRow>,
) -> Vec<Effect> {
    // Collect coordinates that need filling to avoid borrow-conflict.
    let to_fill: Vec<(String, ArtifactPreviewRow)> = state
        .artifact_previews
        .iter()
        .filter(|(_, row)| row.pending)
        .filter_map(|(coord, _)| {
            let article_key = article_key_from_coordinate(coord)?;
            let article = articles.get(article_key)?;
            Some((
                coord.clone(),
                article_row_to_preview(coord.clone(), article),
            ))
        })
        .collect();

    for (coord, preview) in to_fill {
        state.artifact_preview_requests.remove(&coord);
        state.artifact_previews.insert(coord, preview);
    }

    vec![]
}

/// Called when a kind:11 artifact-metadata event fills a preview row.
///
/// Upserts a non-pending row for `coordinate` using the parsed event fields.
/// Also sets an `e:` alias if `event_id` is non-empty (allowing e-tag lookups
/// to resolve to the same preview). Clears the pending request.
///
/// D6: no-op on empty coordinate. D1: raw fields only.
pub(crate) fn fill_from_artifact_event(
    state: &mut AppState,
    coordinate: String,
    event_id: String,
    title: Option<String>,
    image_url: Option<String>,
    author_pubkey: Option<String>,
    summary: Option<String>,
) -> Vec<Effect> {
    if coordinate.is_empty() {
        return vec![];
    }

    let kind = kind_from_coordinate(&coordinate);
    let row = ArtifactPreviewRow {
        coordinate: coordinate.clone(),
        title,
        image_url,
        author_pubkey,
        summary,
        kind,
        pending: false,
        display_url: None,
    };
    state.artifact_preview_requests.remove(&coordinate);
    state.artifact_previews.insert(coordinate, row.clone());

    // Wire an e: alias so event-id-based lookups resolve too.
    if !event_id.is_empty() {
        let e_key = format!("e:{event_id}");
        state
            .artifact_previews
            .entry(e_key)
            .or_insert_with(|| ArtifactPreviewRow {
                coordinate: row.coordinate.clone(),
                ..row
            });
    }

    vec![]
}

/// Called when `KernelEvent::IsbnPreviewReady` fires for an isbn coordinate.
///
/// Fills the `i:isbn:<isbn13>` preview row from the ISBN domain result and
/// clears the pending request. D6: no-op if the coordinate is not tracked.
pub(crate) fn fill_from_isbn_result(
    state: &mut AppState,
    isbn13: &str,
    preview: Option<&KernelArtifactPreview>,
) -> Vec<Effect> {
    let coordinate = format!("i:isbn:{isbn13}");
    if !state.artifact_previews.contains_key(&coordinate) {
        return vec![];
    }
    let row = ArtifactPreviewRow {
        coordinate: coordinate.clone(),
        title: preview.and_then(|p| {
            if p.title.is_empty() {
                None
            } else {
                Some(p.title.clone())
            }
        }),
        image_url: preview.and_then(|p| {
            if p.image.is_empty() {
                None
            } else {
                Some(p.image.clone())
            }
        }),
        author_pubkey: None, // ISBN has no nostr pubkey
        summary: preview.and_then(|p| {
            if p.description.is_empty() {
                None
            } else {
                Some(p.description.clone())
            }
        }),
        kind: ArtifactPreviewKind::Book,
        pending: false,
        display_url: None,
    };
    state.artifact_preview_requests.remove(&coordinate);
    state.artifact_previews.insert(coordinate, row);
    vec![]
}

// ─── Logout / identity change ────────────────────────────────────────────────

/// Clear all artifact-preview state on logout or `IdentityChanged(None)`.
///
/// Both maps must be wiped — stale previews from a prior account's nostr
/// subscriptions must not surface under a new identity.
pub(crate) fn clear_on_identity_lost(state: &mut AppState) {
    state.artifact_previews.clear();
    state.artifact_preview_requests.clear();
}

// ─── Private helpers ─────────────────────────────────────────────────────────

/// If `coordinate` is an `a:30023:pk:d` key, return the articles-map key
/// (`"30023:pk:d"`). Returns `None` for other coordinate kinds.
fn article_key_from_coordinate(coordinate: &str) -> Option<&str> {
    let rest = coordinate.strip_prefix("a:")?;
    // Only kind:30023 articles are in AppState::articles.
    if rest.starts_with("30023:") {
        Some(rest)
    } else {
        None
    }
}

/// Extract a 13-digit ISBN from an `i:isbn:<isbn13>` coordinate.
fn isbn13_from_coordinate(coordinate: &str) -> Option<String> {
    let rest = coordinate.strip_prefix("i:isbn:")?;
    if rest.len() == 13 && rest.chars().all(|c| c.is_ascii_digit()) {
        Some(rest.to_string())
    } else {
        None
    }
}

/// Build a non-pending `ArtifactPreviewRow` from an existing `ArticleRow`.
///
/// D1: raw fields only — no "Untitled" fallback, no formatted strings.
fn article_row_to_preview(coordinate: String, row: &ArticleRow) -> ArtifactPreviewRow {
    ArtifactPreviewRow {
        coordinate,
        title: row.title.clone(),
        image_url: row.hero_image_url.clone(),
        author_pubkey: Some(row.author_pubkey.clone()),
        summary: row.summary.clone(),
        kind: ArtifactPreviewKind::Article,
        pending: false,
        display_url: None,
    }
}

// ─── Unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::action::{AppAction, KernelEvent};
    use crate::kernel::actor::{reduce, Cmd};
    use crate::kernel::app::AppState;
    use crate::kernel::clock::Clock;
    use crate::kernel::clock::ManualClock;
    use crate::kernel::effect::Effect;
    use crate::kernel::snapshot::{ArticleRow, ArtifactPreviewKind};

    fn t() -> u64 {
        ManualClock::new(0).now_unix_seconds()
    }

    fn blank_article(address: &str, d_tag: &str) -> ArticleRow {
        ArticleRow {
            address: address.to_string(),
            id: "aaaa".repeat(16),
            author_pubkey: "bbbb".repeat(16),
            author_display_name: None,
            author_picture_url: None,
            title: Some("Test Article".to_string()),
            summary: Some("A summary.".to_string()),
            hero_image_url: Some("https://example.com/cover.jpg".to_string()),
            d_tag: d_tag.to_string(),
            created_at: 1_700_000_000,
            content_tree_bytes: vec![],
        }
    }

    // ── Test 1: a: coordinate resolves from existing AppState::articles ────────

    #[test]
    fn article_coordinate_resolves_from_existing_articles() {
        let mut state = AppState::default();
        let now = t();

        let address = format!("30023:{}:{}", "b".repeat(64), "my-article");
        let coordinate = format!("a:{address}");

        // Inject the article into state first (simulating an ArticlesUpdated).
        let article = blank_article(&address, "my-article");
        state.articles.insert(address.clone(), article);

        // Now ensure preview — should resolve immediately without effect.
        let effects = ensure_artifact_preview(&mut state, coordinate.clone());
        assert!(
            effects.is_empty(),
            "should produce no effects when article is already in state"
        );
        let row = state
            .artifact_previews
            .get(&coordinate)
            .expect("preview row must exist");
        assert!(!row.pending, "row must be non-pending");
        assert_eq!(row.kind, ArtifactPreviewKind::Article);
        assert_eq!(row.title.as_deref(), Some("Test Article"));
    }

    // ── Test 2: missing coordinate inserts pending + emits ResolveArtifactCoordinate once ──

    #[test]
    fn missing_coordinate_inserts_pending_and_emits_resolve_once() {
        let mut state = AppState::default();

        let coordinate = "e:".to_string() + &"dead".repeat(16);

        // First request.
        let effects1 = ensure_artifact_preview(&mut state, coordinate.clone());
        assert_eq!(effects1.len(), 1, "first request must emit one effect");
        assert!(
            matches!(&effects1[0], Effect::ResolveArtifactCoordinate { coordinate: c } if c == &coordinate),
            "must emit ResolveArtifactCoordinate"
        );
        assert!(state.artifact_previews[&coordinate].pending);
        assert!(state.artifact_preview_requests.contains(&coordinate));

        // Second request for the same coordinate — must NOT emit a second effect (dedupe).
        let effects2 = ensure_artifact_preview(&mut state, coordinate.clone());
        assert!(
            effects2.is_empty(),
            "second request for same coordinate must be deduped (no second effect)"
        );
    }

    // ── Test 3: isbn coordinate reuses the isbn domain ────────────────────────

    #[test]
    fn isbn_coordinate_reuses_isbn_domain() {
        let mut state = AppState::default();

        let coordinate = "i:isbn:9780735211292".to_string();
        let effects = ensure_artifact_preview(&mut state, coordinate.clone());

        // Should emit LookupIsbn, NOT ResolveArtifactCoordinate.
        assert_eq!(effects.len(), 1);
        assert!(
            matches!(&effects[0], Effect::LookupIsbn { isbn13 } if isbn13 == "9780735211292"),
            "isbn coordinate must emit LookupIsbn, got {:?}",
            effects
        );
        assert!(state.artifact_previews[&coordinate].pending);
        assert_eq!(
            state.artifact_previews[&coordinate].kind,
            ArtifactPreviewKind::Book
        );
    }

    // ── Test 4: kind:11 artifact-metadata event fills preview + e: alias ──────

    #[test]
    fn kind11_metadata_event_fills_preview_and_e_alias() {
        let mut state = AppState::default();
        let coordinate = "a:30023:".to_string() + &"c".repeat(64) + ":article-d";
        let event_id = "e".repeat(64);

        // Insert pending row.
        ensure_artifact_preview(&mut state, coordinate.clone());
        // Simulate coordinate was not in articles — should still be pending.
        assert!(state.artifact_previews[&coordinate].pending);

        // Fill via kind:11 artifact event.
        let effects = fill_from_artifact_event(
            &mut state,
            coordinate.clone(),
            event_id.clone(),
            Some("Article Title".to_string()),
            Some("https://img.example.com/cover.jpg".to_string()),
            Some("a1b2".repeat(16)),
            Some("A short summary.".to_string()),
        );
        assert!(effects.is_empty());

        let row = &state.artifact_previews[&coordinate];
        assert!(!row.pending, "row must be non-pending after fill");
        assert_eq!(row.title.as_deref(), Some("Article Title"));

        // e: alias must also exist.
        let e_key = format!("e:{event_id}");
        assert!(
            state.artifact_previews.contains_key(&e_key),
            "e: alias must be set"
        );
        // Request must be cleared.
        assert!(!state.artifact_preview_requests.contains(&coordinate));
    }

    // ── Test 5: web URL gets minimal non-pending row immediately ──────────────

    #[test]
    fn web_url_minimal_nonpending() {
        let mut state = AppState::default();
        let url = "https://example.com/article";
        let coordinate = format!("r:{url}");

        let effects = ensure_artifact_preview(&mut state, coordinate.clone());
        assert!(effects.is_empty(), "web URL must not emit any effect");

        let row = &state.artifact_previews[&coordinate];
        assert!(!row.pending, "web URL row must be non-pending");
        assert_eq!(row.kind, ArtifactPreviewKind::Web);
        assert_eq!(row.display_url.as_deref(), Some(url));
    }

    // ── Test 6: cleared on logout ─────────────────────────────────────────────

    #[test]
    fn cleared_on_logout() {
        let mut state = AppState::default();

        // Add some previews.
        ensure_artifact_preview(&mut state, "r:https://example.com".to_string());
        ensure_artifact_preview(&mut state, "e:".to_string() + &"d".repeat(64));

        assert!(!state.artifact_previews.is_empty());

        // Simulate logout (dispatch Logout action through the full reducer).
        // Using clear_on_identity_lost directly since we don't need full actor setup.
        clear_on_identity_lost(&mut state);

        assert!(
            state.artifact_previews.is_empty(),
            "artifact_previews must be empty after logout"
        );
        assert!(
            state.artifact_preview_requests.is_empty(),
            "artifact_preview_requests must be empty after logout"
        );
    }

    // ── Test 7: canonical coordinate key parsing round-trips ─────────────────

    #[test]
    fn canonical_coordinate_key_parsing() {
        // a: round-trip.
        let coord_a = "a:30023:deadbeef:my-article";
        let (tag, rest) = parse_coordinate_key(coord_a).unwrap();
        assert_eq!(tag, "a");
        assert_eq!(rest, "30023:deadbeef:my-article");

        // e: round-trip.
        let coord_e = "e:abcdef1234567890";
        let (tag, rest) = parse_coordinate_key(coord_e).unwrap();
        assert_eq!(tag, "e");
        assert_eq!(rest, "abcdef1234567890");

        // i: round-trip.
        let coord_i = "i:isbn:9780735211292";
        let (tag, rest) = parse_coordinate_key(coord_i).unwrap();
        assert_eq!(tag, "i");
        assert_eq!(rest, "isbn:9780735211292");

        // r: round-trip.
        let coord_r = "r:https://example.com/article";
        let (tag, rest) = parse_coordinate_key(coord_r).unwrap();
        assert_eq!(tag, "r");
        assert_eq!(rest, "https://example.com/article");

        // coordinate_key helper.
        assert_eq!(
            coordinate_key("a", "30023:deadbeef:x").unwrap(),
            "a:30023:deadbeef:x"
        );
        assert_eq!(
            coordinate_key("i", "isbn:9780735211292").unwrap(),
            "i:isbn:9780735211292"
        );
        assert_eq!(coordinate_key("x", "whatever"), None);
        assert_eq!(coordinate_key("e", ""), None);
    }

    // ── Test 8: ArticlesUpdated fills pending a: rows ─────────────────────────

    #[test]
    fn articles_updated_fills_pending_a_rows() {
        let mut state = AppState::default();

        let pk = "b".repeat(64);
        let address = format!("30023:{pk}:article-xyz");
        let coordinate = format!("a:{address}");

        // Insert as pending (article not yet in state).
        let effects = ensure_artifact_preview(&mut state, coordinate.clone());
        // No article in state → ResolveArtifactCoordinate emitted.
        assert!(!effects.is_empty());
        assert!(state.artifact_previews[&coordinate].pending);

        // Now simulate ArticlesUpdated arriving.
        let article = blank_article(&address, "article-xyz");
        let mut new_articles = BTreeMap::new();
        new_articles.insert(address.clone(), article);

        // Update articles in state (as the domain would do).
        state.articles = new_articles.clone();

        // Call the observer.
        let fill_effects = on_articles_updated(&mut state, &new_articles);
        assert!(fill_effects.is_empty());

        let row = &state.artifact_previews[&coordinate];
        assert!(
            !row.pending,
            "row must be non-pending after articles updated"
        );
        assert_eq!(row.title.as_deref(), Some("Test Article"));
        assert!(!state.artifact_preview_requests.contains(&coordinate));
    }
}
