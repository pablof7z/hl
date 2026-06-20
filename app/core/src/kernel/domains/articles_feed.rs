//! Article feed domain — Phase 4G (ADR-0058 consumer slice).
//!
//! Registers a pull-cursor for the "Following reads" article feed (kind:30023
//! over the active account's follow authors). Builds on the Phase 4F feed-pull
//! core (`feed.rs`) and the Phase 4A `ArticleRow` type.
//!
//! ## Responsibilities
//!
//! * **LIFECYCLE** — emit `Effect::RegisterFeedCursor` + `Effect::DrainFeed`
//!   on `ViewId::ArticleFeed` open; `Effect::ReleaseFeedCursor` on close.
//!   Wired via `lifecycle_effects_for_view_open` / `lifecycle_effects_for_view_close`.
//!
//! * **ACTION** — `AppAction::LoadMoreArticles` emits `Effect::DrainFeed` to
//!   pull the next page (pagination on scroll-to-end, D8: no polling).
//!
//! * **EVENT** — `KernelEvent::FeedPage` with key `"hl.feed.articles"` is
//!   routed by the actor's existing `reduce_event` arm into `AppState::article_feed`
//!   via `feed::apply_feed_page`. No additional arm needed here.
//!
//! * **SNAPSHOT** — `project_article_feed_snapshot` converts the raw
//!   `KernelEvent` rows in `AppState::article_feed.rows` into `ArticleFeedRow`
//!   values for `ViewSnapshot::ArticleFeed`. D1: raw fields only — no formatted
//!   strings, no presentation labels.
//!
//! ## Feed key
//!
//! `ARTICLE_FEED_KEY = "hl.feed.articles"` — must match the routing arm in
//! `actor.rs::reduce_event::KernelEvent::FeedPage` and the helpers in `feed.rs`.
//!
//! ## Fail-closed on empty follows
//!
//! When `AppState::follows` is empty (account has no follows yet, or the
//! projection has not arrived) `article_feed_scope` returns `None` and
//! `lifecycle_effects_for_view_open` emits NO cursor registration — the view
//! opens but remains empty until follows arrive (D5: never broad-scan).
//!
//! ## Threading
//!
//! All public functions here are pure (synchronous, no `async`) and run on the
//! actor thread. The effect runner is async in `actor_task` but that is the
//! generic 4F path (`feed.rs::run_effect_register_feed_cursor` etc.) — not here.
//!
//! ## Live lane untouched
//!
//! `HighlighterCore` / `nostr_runtime.rs` are NOT modified by this slice.
//! The new kernel feed path coexists with the live lane (reads only).

use crate::kernel::app::AppState;
use crate::kernel::domains::feed::{
    article_feed_scope, reduce_drain_feed, reduce_register_feed_cursor, reduce_release_feed_cursor,
};
use crate::kernel::effect::Effect;
use crate::kernel::snapshot::{ArticleFeedRow, ArticleFeedSnapshot, ViewSnapshot};
use crate::kernel::view::ViewId;

/// Stable feed key for the article feed.
///
/// Must match the routing arm in `actor.rs::reduce_event::KernelEvent::FeedPage`
/// (already present in 4F: `"hl.feed.articles" => Some(&mut state.article_feed)`).
pub(crate) const ARTICLE_FEED_KEY: &str = "hl.feed.articles";

// ─── Lifecycle effects ────────────────────────────────────────────────────────

/// Return effects for `Cmd::OpenView(ViewId::ArticleFeed)`.
///
/// When the active account has follows: emits `RegisterFeedCursor` (with
/// kind:30023/follow-authors scope) followed by `DrainFeed` (initial fill).
///
/// When `AppState::follows` is empty: returns no effects — fail-closed per D5
/// (never register a broad-scan cursor). The view remains empty until a
/// `FollowListUpdated` event populates follows and the UI re-opens the view.
///
/// Called by the actor loop for `Cmd::OpenView(ViewId::ArticleFeed)`.
pub(crate) fn lifecycle_effects_for_view_open(id: &ViewId, state: &AppState) -> Vec<Effect> {
    if !matches!(id, ViewId::ArticleFeed) {
        return vec![];
    }

    let Some(scope) = article_feed_scope(&state.follows) else {
        // Fail-closed: no follows → no cursor, no scan.
        tracing::debug!("ArticleFeed open: no follows — cursor registration skipped (fail-closed)");
        return vec![];
    };

    let mut effects = reduce_register_feed_cursor(ARTICLE_FEED_KEY.to_string(), scope);
    // Immediately drain so the first page fills without a separate user action.
    effects.extend(reduce_drain_feed(ARTICLE_FEED_KEY.to_string()));
    effects
}

/// Return effects for `Cmd::CloseView(ViewId::ArticleFeed)`.
///
/// Emits `ReleaseFeedCursor` to unregister the pull cursor from the kernel.
/// The `article_feed.rows` buffer is cleared inline by the actor's existing
/// `ReleaseFeedCursor` inline handler (same pattern as `ReleaseGroupEvents`).
///
/// Called by the actor loop for `Cmd::CloseView(ViewId::ArticleFeed)`.
pub(crate) fn lifecycle_effects_for_view_close(id: &ViewId) -> Vec<Effect> {
    if !matches!(id, ViewId::ArticleFeed) {
        return vec![];
    }
    reduce_release_feed_cursor(ARTICLE_FEED_KEY.to_string())
}

// ─── Action reducer ───────────────────────────────────────────────────────────

/// Emit `Effect::DrainFeed` for scroll-to-end pagination.
///
/// Called from `actor.rs::reduce_action` for `AppAction::LoadMoreArticles`.
/// D8: no polling — a single `DrainFeed` effect per action; the actor's inline
/// handler calls `nmp_app_pull_page` once and feeds the result back as
/// `KernelEvent::FeedPage` which updates `AppState::article_feed`. The UI
/// emits another `LoadMoreArticles` to pull the next page (pull-model pagination).
///
/// Returns `[]` when `AppState::article_feed.exhausted == true` (no more pages).
pub(crate) fn reduce_action_load_more_articles(state: &AppState) -> Vec<Effect> {
    if state.article_feed.exhausted {
        // Already caught up — no further pages available.
        return vec![];
    }
    reduce_drain_feed(ARTICLE_FEED_KEY.to_string())
}

// ─── Snapshot projection ─────────────────────────────────────────────────────

/// Project an `ArticleFeedSnapshot` from `AppState::article_feed`.
///
/// Converts raw `KernelEvent` rows (kind:30023 entries from the feed pager)
/// into `ArticleFeedRow` values. Returns `None` only when the view is not open
/// (callers check view registry first — this function always returns `Some`
/// when called for an open `ArticleFeed` view).
///
/// D1: raw protocol fields only — no `"Untitled"` fallback, no `"{n} min read"`,
/// no `"#tag"` formatting, no truncated previews. Swift owns all presentation.
///
/// D5: row count is bounded by `AppState::article_feed.rows.len()` which is
/// itself bounded by the sum of `FEED_PAGE_SIZE` (20) across all drained pages.
pub(crate) fn project_article_feed_snapshot(state: &AppState) -> Option<ViewSnapshot> {
    let rows: Vec<ArticleFeedRow> = state
        .article_feed
        .rows
        .iter()
        .filter(|ev| ev.kind == 30023)
        .map(|ev| {
            // Extract the `d` tag value for the addressable coordinate.
            let d_tag = ev
                .tags
                .iter()
                .find(|t| t.first().map(|s| s == "d").unwrap_or(false))
                .and_then(|t| t.get(1))
                .cloned()
                .unwrap_or_default();

            // Build the addressable coordinate: `kind:author_hex:d_tag`.
            let address = format!("{}:{}:{}", ev.kind, ev.author, d_tag);

            // Extract the `title` tag value (NIP-23 uses a `["title", _]` tag).
            let title = ev
                .tags
                .iter()
                .find(|t| t.first().map(|s| s == "title").unwrap_or(false))
                .and_then(|t| t.get(1))
                .cloned();

            // Extract the `summary` tag value.
            let summary = ev
                .tags
                .iter()
                .find(|t| t.first().map(|s| s == "summary").unwrap_or(false))
                .and_then(|t| t.get(1))
                .cloned();

            // Extract the `image` tag value (hero image URL).
            let hero_image_url = ev
                .tags
                .iter()
                .find(|t| t.first().map(|s| s == "image").unwrap_or(false))
                .and_then(|t| t.get(1))
                .cloned();

            ArticleFeedRow {
                address,
                id: ev.id.clone(),
                author_pubkey: ev.author.clone(),
                title,
                summary,
                hero_image_url,
                d_tag,
                created_at: ev.created_at,
            }
        })
        .collect();

    Some(ViewSnapshot::ArticleFeed(ArticleFeedSnapshot {
        rows,
        exhausted: state.article_feed.exhausted,
    }))
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::action::KernelEvent;
    use crate::kernel::actor::{reduce, Cmd};
    use crate::kernel::app::AppState;
    use crate::kernel::clock::{Clock, ManualClock};
    use crate::kernel::domains::feed::{apply_feed_page, FeedState};
    use crate::kernel::effect::Effect;
    use crate::kernel::view::{ViewId, ViewRoute};

    fn make_state() -> AppState {
        AppState::default()
    }

    fn step(state: &mut AppState, clock: &ManualClock, cmd: Cmd) -> Vec<Effect> {
        let now = clock.now_unix_seconds();
        reduce(state, cmd, now)
    }

    fn dummy_kernel_event(
        id: &str,
        pubkey: &str,
        d_tag: &str,
        title: &str,
    ) -> nmp_core::substrate::KernelEvent {
        nmp_core::substrate::KernelEvent {
            id: id.to_string(),
            author: pubkey.to_string(),
            kind: 30023,
            created_at: 1_700_000_000,
            tags: vec![
                vec!["d".to_string(), d_tag.to_string()],
                vec!["title".to_string(), title.to_string()],
            ],
            content: String::new(),
            relay_provenance: vec![],
        }
    }

    // 4G-T1: article_feed_registers_cursor_with_follow_scope
    //
    // When ArticleFeed view opens with a non-empty follow set,
    // lifecycle_effects_for_view_open must emit RegisterFeedCursor (with the
    // correct feed key and a non-zero cursor_id) followed by DrainFeed.
    #[test]
    fn article_feed_registers_cursor_with_follow_scope() {
        let mut state = make_state();
        state.follows =
            vec!["aabbcc0000000000000000000000000000000000000000000000000000000001".to_string()];

        let effects = lifecycle_effects_for_view_open(&ViewId::ArticleFeed, &state);

        // Must emit at least RegisterFeedCursor + DrainFeed.
        assert!(
            effects.len() >= 2,
            "expected RegisterFeedCursor + DrainFeed, got {} effects",
            effects.len()
        );

        let has_register = effects.iter().any(|e| {
            matches!(
                e,
                Effect::RegisterFeedCursor { key, cursor_id, .. }
                if key == ARTICLE_FEED_KEY && *cursor_id != 0
            )
        });
        assert!(
            has_register,
            "must emit RegisterFeedCursor with correct key and non-zero cursor_id"
        );

        let has_drain = effects
            .iter()
            .any(|e| matches!(e, Effect::DrainFeed { key } if key == ARTICLE_FEED_KEY));
        assert!(has_drain, "must emit DrainFeed for initial fill");
    }

    // 4G-T2: article_feed_fail_closed_empty_follows
    //
    // When the follow set is empty, lifecycle_effects_for_view_open must return
    // no effects (fail-closed: never register a broad-scan cursor, D5).
    #[test]
    fn article_feed_fail_closed_empty_follows() {
        let state = make_state(); // follows is empty by default

        let effects = lifecycle_effects_for_view_open(&ViewId::ArticleFeed, &state);

        assert!(
            effects.is_empty(),
            "must fail closed (no effects) when follow set is empty (D5)"
        );
    }

    // 4G-T3: feedpage_appends_article_rows_raw
    //
    // Injecting a KernelEvent::FeedPage with key "hl.feed.articles" via Cmd::Event
    // must append rows to AppState::article_feed and the snapshot must expose them
    // as ArticleFeedRow values with raw fields (D1: no formatted strings).
    #[test]
    fn feedpage_appends_article_rows_raw() {
        let mut state = make_state();
        let clock = ManualClock::default();

        let pubkey = "aabb000000000000000000000000000000000000000000000000000000000001";
        let rows = vec![
            dummy_kernel_event("id0001", pubkey, "my-article", "My Article Title"),
            dummy_kernel_event("id0002", pubkey, "another", "Another Article"),
        ];

        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::FeedPage {
                key: ARTICLE_FEED_KEY.to_string(),
                cursor_id: 1,
                rows,
                next_after_seq: 42,
                exhausted: false,
                gap_rebased_to: None,
            }),
        );

        assert_eq!(
            state.article_feed.rows.len(),
            2,
            "two rows appended to article_feed"
        );
        assert_eq!(state.article_feed.after_seq, 42);
        assert!(!state.article_feed.exhausted);

        // Snapshot projection.
        let snap = project_article_feed_snapshot(&state).expect("snapshot present");
        match snap {
            ViewSnapshot::ArticleFeed(s) => {
                assert_eq!(s.rows.len(), 2);
                // Verify raw fields — no formatted strings (D1).
                let row = &s.rows[0];
                assert_eq!(row.author_pubkey, pubkey);
                assert_eq!(row.d_tag, "my-article");
                assert_eq!(row.title.as_deref(), Some("My Article Title"));
                // D1: title must not be a fallback "Untitled".
                assert!(
                    row.title.as_deref().unwrap_or("") != "Untitled",
                    "D1: no Untitled fallback in ArticleFeedRow"
                );
                assert!(!s.exhausted);
            }
            other => panic!("expected ArticleFeed snapshot, got {:?}", other),
        }
    }

    // 4G-T4: scroll_end_emits_drain
    //
    // AppAction::LoadMoreArticles on a non-exhausted feed must emit DrainFeed.
    // After the feed is exhausted, LoadMoreArticles must be a no-op.
    #[test]
    fn scroll_end_emits_drain() {
        let mut state = make_state();
        let clock = ManualClock::default();

        // Seed a non-exhausted feed state.
        state.article_feed.cursor_id = 99;
        state.article_feed.exhausted = false;

        // LoadMoreArticles should emit DrainFeed.
        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(crate::kernel::action::AppAction::LoadMoreArticles),
        );
        let has_drain = effects
            .iter()
            .any(|e| matches!(e, Effect::DrainFeed { key } if key == ARTICLE_FEED_KEY));
        assert!(
            has_drain,
            "LoadMoreArticles must emit DrainFeed on non-exhausted feed"
        );

        // Mark exhausted.
        state.article_feed.exhausted = true;
        let effects2 = step(
            &mut state,
            &clock,
            Cmd::Action(crate::kernel::action::AppAction::LoadMoreArticles),
        );
        let has_drain2 = effects2
            .iter()
            .any(|e| matches!(e, Effect::DrainFeed { key } if key == ARTICLE_FEED_KEY));
        assert!(
            !has_drain2,
            "LoadMoreArticles must be no-op when feed is exhausted"
        );
    }

    // 4G-T5: feed_cleared_on_identity_loss
    //
    // On Logout, AppState::article_feed must be cleared so stale articles from
    // the previous session do not leak to the next account.
    #[test]
    fn feed_cleared_on_identity_loss() {
        let mut state = make_state();
        let clock = ManualClock::default();

        // Seed article feed with some rows.
        state
            .article_feed
            .rows
            .push(dummy_kernel_event("aaaa0001", "pubkey1", "d1", "Title 1"));
        state.article_feed.after_seq = 55;
        state.article_feed.cursor_id = 1234;

        // Logout must clear article_feed rows and reset state.
        step(
            &mut state,
            &clock,
            Cmd::Action(crate::kernel::action::AppAction::Logout),
        );

        assert!(
            state.article_feed.rows.is_empty(),
            "article_feed rows must be cleared on Logout"
        );
        assert_eq!(
            state.article_feed.after_seq, 0,
            "article_feed after_seq must reset on Logout"
        );
    }

    // 4G-T6: malformed_entry_no_op
    //
    // A FeedPage event with rows of an unexpected kind (not 30023) must be
    // stored in article_feed.rows (the engine is type-agnostic at the store
    // level) but the snapshot must filter them out, yielding zero ArticleFeedRows.
    #[test]
    fn malformed_entry_no_op() {
        let mut state = make_state();
        let clock = ManualClock::default();

        // A row with kind:1 (short note — not an article).
        let bad_row = nmp_core::substrate::KernelEvent {
            id: "bad001".to_string(),
            author: "pubkey".to_string(),
            kind: 1, // wrong kind
            created_at: 1_000_000,
            tags: vec![],
            content: "Hello, world".to_string(),
            relay_provenance: vec![],
        };

        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::FeedPage {
                key: ARTICLE_FEED_KEY.to_string(),
                cursor_id: 1,
                rows: vec![bad_row],
                next_after_seq: 10,
                exhausted: true,
                gap_rebased_to: None,
            }),
        );

        // The raw engine stores it, but the snapshot filters it out.
        assert_eq!(state.article_feed.rows.len(), 1, "engine stores all rows");

        let snap = project_article_feed_snapshot(&state).expect("snapshot always Some");
        match snap {
            ViewSnapshot::ArticleFeed(s) => {
                assert!(
                    s.rows.is_empty(),
                    "snapshot must filter out non-30023 rows (malformed entry no-op)"
                );
            }
            other => panic!("expected ArticleFeed snapshot, got {:?}", other),
        }
    }

    // 4G-T7: lifecycle_effects_for_view_close_emits_release
    //
    // Closing ArticleFeed view must emit ReleaseFeedCursor.
    #[test]
    fn lifecycle_effects_for_view_close_emits_release() {
        let effects = lifecycle_effects_for_view_close(&ViewId::ArticleFeed);
        assert_eq!(effects.len(), 1, "exactly one effect on close");
        match &effects[0] {
            Effect::ReleaseFeedCursor { key } => {
                assert_eq!(
                    key, ARTICLE_FEED_KEY,
                    "must release the article feed cursor"
                );
            }
            other => panic!("expected ReleaseFeedCursor, got {:?}", other),
        }
    }

    // 4G-T8: non_article_feed_view_ids_ignored
    //
    // lifecycle_effects_for_view_open and lifecycle_effects_for_view_close must
    // return empty vecs for any ViewId other than ArticleFeed.
    #[test]
    fn non_article_feed_view_ids_ignored() {
        let state = make_state();
        let other_ids = [
            ViewId::AppRoot,
            ViewId::RootShell,
            ViewId::Bookmarks,
            ViewId::Search,
        ];
        for id in &other_ids {
            let open_effects = lifecycle_effects_for_view_open(id, &state);
            assert!(
                open_effects.is_empty(),
                "lifecycle_effects_for_view_open must ignore {:?}",
                id
            );
            let close_effects = lifecycle_effects_for_view_close(id);
            assert!(
                close_effects.is_empty(),
                "lifecycle_effects_for_view_close must ignore {:?}",
                id
            );
        }
    }
}
