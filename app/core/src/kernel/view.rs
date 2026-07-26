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

    // ── Phase 4H additions (append-only) ─────────────────────────────────────
    /// Home/own highlights feed — kind:9802 events via the `"hl.feed.highlights"`
    /// pull cursor (ADR-0058). On open: emits `RegisterFeedCursor` + `DrainFeed`.
    /// On close: emits `ReleaseFeedCursor`. The snapshot is
    /// `ViewSnapshot::HighlightFeed(HighlightFeedSnapshot)`.
    HighlightFeed,

    // ── Phase 4J additions (append-only) ─────────────────────────────────────
    /// Merged home feed — composition of the article feed (Phase 4G, kind:30023)
    /// and highlight feed (Phase 4H, kind:9802). On open: emits lifecycle effects
    /// for both underlying feeds. On close: releases both cursors. The snapshot
    /// is `ViewSnapshot::HomeFeed(HomeFeedSnapshot)`.
    HomeFeed,

    // ── Phase 5A additions (append-only) ─────────────────────────────────────
    /// What's New sheet — device-local seen-state (no nostr publish).
    WhatsNew,

    // ── Phase 5C additions (append-only) ─────────────────────────────────────
    /// Book picker / ISBN preview — shows pending lookup + last result.
    /// Snapshot: `ViewSnapshot::BookPicker(BookPickerKernelSnapshot)`.
    BookPicker,

    // ── Phase 5K additions (append-only) ─────────────────────────────────────
    /// Share-extension intake composer.
    ///
    /// Opened by the main app when it receives a `highlighter://process-share`
    /// URL open (after the share extension wrote `pending-shares-v1.json` to the
    /// App Group and dispatched `AppAction::DrainShareQueue`). The snapshot
    /// is `ViewSnapshot::ShareComposer(ShareComposerSnapshot)` — raw fields
    /// for the pending share item and the available community picker rows (D1).
    ShareComposer,

    /// In-flight share-to-room / drain / invite publish status (#21).
    ///
    /// Snapshot: `ViewSnapshot::SharePublish(SharePublishSnapshot)` — the
    /// publishing / done / error phase the iOS share sheet renders, plus any
    /// invite codes minted by `hl.share.mint_invite` (D1: Swift composes the
    /// share link). Always present (never `None`).
    SharePublish,

    // ── Phase 5H additions (append-only) ─────────────────────────────────────
    /// Full-screen podcast player view.
    ///
    /// Opened when the user taps a podcast episode to play. Snapshot:
    /// `ViewSnapshot::PodcastListening(PodcastListeningSnapshot)` — raw position,
    /// duration, is_playing, and clip range fields (D1: Swift formats timestamps).
    PodcastListening,

    // ── Phase 5D additions (append-only) ─────────────────────────────────────
    /// OCR capture view. Snapshot: `ViewSnapshot::Capture(KernelCaptureSnapshot)`.
    ///
    /// Device-local: `pending` drives a progress indicator while
    /// `VNRecognizeTextRequest` is in flight; `markdown` and `selectable_words`
    /// are available once the OCR round-trip completes.
    Capture,

    // ── Phase 7 additions (append-only) ─────────────────────────────────────
    /// NIP-22 comment thread view for a specific root anchor.
    ///
    /// `root_tag_value` is the UPPERCASE root scope value from the NIP-22 `E`/`A`/`I`
    /// tag (e.g. a 64-char hex event id for `E`, an addressable coord for `A`,
    /// or an external URI for `I`). The snapshot is
    /// `ViewSnapshot::CommentThread(CommentThreadKernelSnapshot)` — flat raw record
    /// list + `comment_count` (D1: no formatted strings; Swift builds display tree).
    ///
    /// On open: no lifecycle wiring is needed — the `CommentObserver` registered at
    /// boot automatically routes all kind:1111 events to `AppState::comment_threads`.
    /// On close: the `comment_threads` entry is NOT cleared (content is session-scoped
    /// and bounded by the NMP projection's `MAX_PROJECTION_MESSAGES` cap).
    CommentThread {
        /// Root scope tag value that anchors this thread (opaque from caller — D3).
        root_tag_value: String,
    },

    // ── Phase 7 feedback additions (append-only) ──────────────────────────────
    /// Feedback thread list — top-level NIP-22 kind:1111 roots under the
    /// Highlighter project coordinate, filtered to the active account.
    ///
    /// On open: no extra observer wiring — the global `CommentObserver` already
    /// routes all kind:1111 events. `hl.feedback.open_list` dispatches this view.
    /// On close: UI flags in `AppState::feedback` are cleared; the underlying
    /// `comment_threads` entry is not cleared (content-addressed).
    FeedbackThreads,

    /// Feedback thread detail for one root comment and its descendants.
    ///
    /// `root_event_id` identifies the top-level kind:1111 comment whose ancestor
    /// chain to project. Snapshot: `ViewSnapshot::FeedbackThread(FeedbackThreadSnapshot)`.
    FeedbackThread {
        /// Event id of the feedback thread's root comment (raw 64-char hex). D3.
        root_event_id: String,
    },
    // ── Phase 7 chat additions (append-only) ─────────────────────────────────
    /// NIP-29 group chat view for a specific room.
    ///
    /// `group_id` is the NIP-29 local group id. The snapshot is
    /// `ViewSnapshot::RoomChat(RoomChatSnapshot)` — bounded raw kind:9 message
    /// rows, oldest-first in the visible window (D1: no formatted strings).
    ///
    /// On open: `hl.chat.open` dispatches `Effect::WireGroupChat`, starting one
    /// host-scoped new-NMP observation. On close: `hl.chat.close` dispatches
    /// `Effect::ReleaseChatRoom`, cancelling it and removing the room buffer.
    RoomChat {
        /// NIP-29 local group id.
        group_id: String,
    },
    // ── Phase 7 discussions additions (append-only) ──────────────────────────
    /// Per-room discussions tab view for a specific NIP-29 group.
    ///
    /// The snapshot is `ViewSnapshot::RoomDiscussions(RoomDiscussionsSnapshot)` —
    /// raw kind:11+discussion rows filtered from `AppState::room_discussions`,
    /// bounded at 64, newest-first (D1: no formatted strings; Swift formats all
    /// title fallbacks, timestamps, and attachment chips).
    RoomDiscussions {
        /// NIP-29 local group id.
        group_id: String,
    },

    // ── Phase 7 additions (append-only) ─────────────────────────────────────
    /// Entity ref view for an inline event card — event id, coordinate, or
    /// external id. Opened by `NostrRichText` when rendering an entity embed.
    /// Key format: 64-char hex event id, `"kind:pubkey:d"` coordinate, or
    /// `"i:<external-id>"`.
    EntityRef {
        /// Entity key (event id, NIP-19 coordinate, or external id).
        key: String,
    },
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

    // ── Phase 4H additions (append-only) ─────────────────────────────────────
    /// Home/own highlights feed projection — `HighlightFeedSnapshot` (kind:9802
    /// rows from the pull cursor, sorted newest-first). D1: no byline formatting.
    HighlightFeed,

    // ── Phase 4J additions (append-only) ─────────────────────────────────────
    /// Merged home feed projection — `HomeFeedSnapshot` (merged, suppressed,
    /// grouped, sorted rows from the article + highlight feeds). D1: raw rows
    /// only — no bylines, no "min read", no "Untitled" fallback.
    HomeFeed,

    // ── Phase 5A additions (append-only) ─────────────────────────────────────
    /// What's New sheet projection — `WhatsNewSnapshot` (unseen entries, should_present flag).
    WhatsNew,

    // ── Phase 5C additions (append-only) ─────────────────────────────────────
    /// Book picker projection — pending isbn, last result, cache size.
    BookPicker,

    // ── Phase 5K additions (append-only) ─────────────────────────────────────
    /// Share-extension intake composer projection — `ShareComposerSnapshot`
    /// (raw pending share item fields + community picker rows). D1: no formatted
    /// strings, no community name fallbacks, no URL validation strings.
    ShareComposer,

    // ── #21 share-flow additions (append-only) ───────────────────────────────
    /// In-flight share-to-room / drain / invite publish status route.
    /// Snapshot: `ViewSnapshot::SharePublish(SharePublishSnapshot)`.
    SharePublish,

    // ── Phase 5H additions (append-only) ─────────────────────────────────────
    /// Podcast player projection — `PodcastListeningSnapshot` (now-playing +
    /// position/duration/is_playing + clip range). D1: raw f64 seconds, no
    /// "X:XX" formatted strings.
    PodcastListening,

    // ── Phase 5D additions (append-only) ─────────────────────────────────────
    /// OCR capture projection — `KernelCaptureSnapshot`
    /// (reconstructed markdown + selectable words + raw lines + pending flag). D1.
    Capture,

    // ── Phase 7 additions (append-only) ─────────────────────────────────────
    /// NIP-22 comment thread projection — `CommentThreadKernelSnapshot` (flat
    /// raw record list keyed by `root_tag_value`). D1: no formatted strings,
    /// no tree nesting in snapshot — Swift builds the display tree from
    /// `parent_tag_value` relationships.
    CommentThread {
        /// Root scope tag value that anchors this thread.
        root_tag_value: String,
    },

    // ── Phase 7 feedback additions (append-only) ──────────────────────────────
    /// Feedback thread list projection — `FeedbackThreadsSnapshot` (top-level
    /// NIP-22 roots under the Highlighter project coordinate, filtered to the
    /// active viewer). D1: no formatted strings, `None` metadata fields.
    FeedbackThreads,

    /// Feedback thread detail projection — `FeedbackThreadSnapshot` (root +
    /// descendants for `root_event_id`, oldest-first). D1: raw fields only.
    FeedbackThread {
        /// Event id of the feedback thread's root comment.
        root_event_id: String,
    },
    // ── Phase 7 chat additions (append-only) ─────────────────────────────────
    /// NIP-29 group chat projection — `RoomChatSnapshot` (bounded raw kind:9
    /// message rows, oldest-first in the visible window). D1: no formatted strings.
    RoomChat {
        /// NIP-29 local group id.
        group_id: String,
    },
    // ── Phase 7 discussions additions (append-only) ──────────────────────────
    /// Per-room discussions tab projection — `RoomDiscussionsSnapshot` (raw
    /// kind:11+discussion rows, bounded at 64, newest-first). D1: no formatted
    /// strings, no title fallbacks — Swift owns all display formatting.
    RoomDiscussions {
        /// NIP-29 local group id.
        group_id: String,
    },

    // ── Phase 7 additions (append-only) ─────────────────────────────────────
    /// Entity ref projection — `KernelEntitySnapshot` (raw event fields for
    /// inline event cards). D1: no formatted strings; Swift formats author
    /// bylines, timestamps, content truncation.
    EntityRef {
        /// Entity key (event id, NIP-19 coordinate, or external id).
        key: String,
    },
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
