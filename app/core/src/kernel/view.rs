//! View registry — maps open `ViewId`s to their `ViewRoute` and the last
//! emitted snapshot. The kernel only recomputes and emits snapshots for
//! currently-open views (Non-Negotiable #7 / D5).

use std::collections::HashMap;

use crate::kernel::snapshot::ViewSnapshot;

/// Stable identifier for an open view instance.
///
/// In Phase 1 there are two: the app-root decision surface and the root shell.
/// Later phases add per-screen identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum ViewId {
    /// The root entry point that decides which top-level route to show
    /// (`Onboarding`, `Login`, or `RootShell`).
    AppRoot,
    /// The main tab shell (visible only when session is present).
    RootShell,

    // ── Phase 2E additions ────────────────────────────────────────────────────
    /// Network settings overview screen (relay list + role configuration).
    NetworkSettings,
    /// Relay-diagnostics detail screen (connection stats, counters, sub list).
    RelayDiagnostics,

    // ── Phase 3B additions (append-only) ─────────────────────────────────────
    /// Joined-groups / communities list for the active account.
    Communities,

    // ── Phase 3E additions (append-only) ─────────────────────────────────────
    /// Room explorer / discovery screen.
    RoomExplorer,

    // ── Phase 3D additions (append-only) ─────────────────────────────────────
    /// Profile detail view for a specific pubkey.
    ///
    /// `pubkey` is a raw 64-char lowercase hex pubkey. The view is opened by
    /// `AppAction::ClaimProfile{pubkey}` (which also sends `Effect::ClaimProfile`
    /// to register the NMP interest) and closed by `AppAction::ReleaseProfile`.
    Profile {
        /// Raw 64-char hex pubkey of the profile being viewed.
        pubkey: String,
    },

    // ── Phase 3F additions (append-only) ─────────────────────────────────────
    /// Per-room home shell view for a specific NIP-29 group.
    ///
    /// `group_id` is the NIP-29 local group id (the `["d", _]` tag value).
    /// Opened by the native UI when the user taps on a room; the actor wires
    /// the `GroupEventsProjection` via `Effect::WireGroupEvents` on view open
    /// and releases the buffer via `Effect::ReleaseGroupEvents` on close.
    RoomHome {
        /// NIP-29 local group id.
        group_id: String,
    },

    // ── Phase 4C additions (append-only) ─────────────────────────────────────
    /// Bookmark library view — the active account's NIP-51 kind:10003 list.
    /// Kind:10003 article-bookmark toggle only (sets/web/curation stay bespoke).
    Bookmarks,

    // ── Phase 4A additions (append-only) ─────────────────────────────────────
    /// NIP-23 article reader view for a specific kind:30023 article.
    ///
    /// `address` is the addressable coordinate `kind:author_hex:d_tag` that
    /// identifies the article. Opened by `AppAction::OpenArticle{address}`;
    /// closed by `AppAction::CloseArticle{address}`. The snapshot is computed
    /// directly from `AppState::articles` — no NMP claim is needed because the
    /// longform typed projection already carries full `ArticleProjection`
    /// documents (including `content_tree`) for every article seen this session.
    ArticleReader {
        /// Addressable coordinate: `kind:author_hex:d_tag`.
        address: String,
    },

    // ── Phase 4D additions (append-only) ─────────────────────────────────────
    /// NIP-50 relay search results view.
    ///
    /// Opened when the user submits a search query; closed when the user
    /// navigates away. On open (lifecycle): no projection wiring needed — the
    /// `SearchResultsProjection` is registered when `AppAction::RunSearch` is
    /// dispatched. On close: `AppState::search_results` is cleared to bound
    /// memory. The snapshot is `ViewSnapshot::Search(SearchSnapshot)`.
    Search,

    // ── Phase 4G additions (append-only) ─────────────────────────────────────
    /// "Following reads" article feed — kind:30023 events over the active
    /// account's follow authors, pulled via the Phase 4F feed-pull engine.
    ///
    /// On open: `articles_feed::lifecycle_effects_for_view_open` registers the
    /// `"hl.feed.articles"` pull cursor (fail-closed when follows is empty) and
    /// emits an initial `DrainFeed` for the first page.
    /// On close: `articles_feed::lifecycle_effects_for_view_close` releases the
    /// cursor. The snapshot is `ViewSnapshot::ArticleFeed(ArticleFeedSnapshot)`.
    ///
    /// Pagination: the UI dispatches `AppAction::LoadMoreArticles` on
    /// scroll-to-end; the reducer emits `DrainFeed` (D8: no polling).
    ArticleFeed,
}

/// Which projection to compute for a registered view.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum ViewRoute {
    AppRoot,
    RootShell,

    // ── Phase 2E additions ────────────────────────────────────────────────────
    /// Network settings projection — relay list with raw role/status data.
    NetworkSettings,
    /// Relay-diagnostics projection — raw counters and connection state per relay.
    RelayDiagnostics,

    // ── Phase 3B additions (append-only) ─────────────────────────────────────
    Communities,

    // ── Phase 3E additions (append-only) ─────────────────────────────────────
    /// Room explorer / discovery screen.
    RoomExplorer,

    // ── Phase 3D additions (append-only) ─────────────────────────────────────
    /// Profile detail projection — `ProfileSnapshot` (identity + relationship +
    /// communities). Articles/highlights deferred to Phase 4.
    Profile {
        /// Raw 64-char hex pubkey of the profile to project.
        pubkey: String,
    },

    // ── Phase 3F additions (append-only) ─────────────────────────────────────
    /// Room-home projection — `RoomHomeSnapshot` (header + metadata + membership
    /// + empty lanes). Lane bodies deferred to Phase 4.
    RoomHome {
        /// NIP-29 local group id.
        group_id: String,
    },

    // ── Phase 4C additions (append-only) ─────────────────────────────────────
    /// Bookmarks projection — `BookmarksSnapshot` (raw kind:10003 rows).
    /// Kind:10003 article-bookmark toggle only.
    Bookmarks,

    // ── Phase 4A additions (append-only) ─────────────────────────────────────
    /// NIP-23 article reader projection — `ArticleReaderSnapshot` (raw fields
    /// from `ArticleProjection` including `content_tree_bytes`). D1: no
    /// formatted strings ("Untitled", "min read", etc.) — Swift owns those.
    ArticleReader {
        /// Addressable coordinate: `kind:author_hex:d_tag`.
        address: String,
    },

    // ── Phase 4D additions (append-only) ─────────────────────────────────────
    /// NIP-50 relay search results projection — `SearchSnapshot` (bounded raw
    /// hit rows). D1: no "X results" label, no formatted kind/author strings.
    Search,

    // ── Phase 4G additions (append-only) ─────────────────────────────────────
    /// "Following reads" article feed projection — `ArticleFeedSnapshot`
    /// (raw kind:30023 rows from the 4F feed-pull engine). D1: no formatted
    /// strings, no "Untitled" fallback, no "min read" labels.
    ArticleFeed,
}

/// Tracks open views and their last-emitted snapshots.
#[derive(Debug, Default)]
pub struct ViewRegistry {
    views: HashMap<ViewId, (ViewRoute, Option<ViewSnapshot>)>,
}

impl ViewRegistry {
    /// Register a view. If it was already open, the existing snapshot is kept.
    pub fn open(&mut self, id: ViewId, route: ViewRoute) {
        self.views.entry(id).or_insert((route, None));
    }

    /// Deregister a view and discard its last snapshot.
    pub fn close(&mut self, id: &ViewId) {
        self.views.remove(id);
    }

    pub fn is_open(&self, id: &ViewId) -> bool {
        self.views.contains_key(id)
    }

    pub fn open_ids(&self) -> impl Iterator<Item = &ViewId> {
        self.views.keys()
    }

    /// The last snapshot that was stored for this view, if any.
    pub fn last_snapshot(&self, id: &ViewId) -> Option<&ViewSnapshot> {
        self.views.get(id)?.1.as_ref()
    }

    /// Overwrite the stored snapshot for an open view. No-op for closed views.
    pub fn update_snapshot(&mut self, id: &ViewId, snapshot: ViewSnapshot) {
        if let Some(entry) = self.views.get_mut(id) {
            entry.1 = Some(snapshot);
        }
    }

    /// Current snapshot for pull access (`current_snapshot` FFI method).
    pub fn current_snapshot(&self, id: &ViewId) -> Option<ViewSnapshot> {
        self.views.get(id)?.1.clone()
    }

    /// The route registered for this view.
    pub fn route(&self, id: &ViewId) -> Option<&ViewRoute> {
        Some(&self.views.get(id)?.0)
    }

    /// Number of open views.
    pub fn open_count(&self) -> usize {
        self.views.len()
    }
}
