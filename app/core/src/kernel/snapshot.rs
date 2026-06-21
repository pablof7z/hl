//! View-snapshot types — Phase 1 + Phase 2E.
//!
//! Every variant is a fixed-size, screen-shaped projection of `AppState`
//! for one registered view. Sizes are bounded by construction: structs have
//! fixed fields, no lists that grow with the event store (Non-Negotiable #7).
//!
//! Snapshots are the ONLY data that crosses FFI in the nmp-lane; native
//! renders them without performing any business logic (D4 / D5).

use crate::kernel::domains::relay_diagnostics::RelayDiagRow;

/// Which screen the root shell should display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum RouteKind {
    /// User has not completed onboarding; show the onboarding flow.
    Onboarding,
    /// Onboarding complete but no active session; show the login screen.
    Login,
    /// Session present; show the main tab shell.
    RootShell,
}

/// Snapshot for the `ViewId::AppRoot` projection.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct AppRootSnapshot {
    /// Which top-level screen to render.
    pub route_kind: RouteKind,
    /// Whether a session secret is currently in memory.
    pub session_present: bool,
    /// Whether the user has completed onboarding.
    pub onboarding_complete: bool,

    // ── Phase 2B additions ────────────────────────────────────────────────────
    /// The most recently minted `nostrconnect://` URI, or `None` when no
    /// NostrConnect sign-in is in progress. The iOS QR-code sheet renders this
    /// directly. Cleared when `IdentityChanged` fires or on `Logout`.
    /// Bounded: one string ≤ 512 bytes (NIP-46 spec limit).
    pub nostrconnect_uri: Option<String>,
}

/// A transient toast message visible in the root shell.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ToastSnapshot {
    pub message: String,
    /// UNIX second at which the kernel will auto-dismiss this toast
    /// (clock-driven, D8/D9 — no Swift `Timer` involved).
    pub dismiss_at_unix: u64,
}

/// Snapshot for the `ViewId::RootShell` projection.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct RootShellSnapshot {
    /// Index of the currently selected tab (matches `RootTab` raw values).
    pub selected_tab: u8,
    /// Total number of tabs — fixed in Phase 1.
    pub tab_count: u8,
    /// Active toast, if any. Cleared by the kernel when `clock >= dismiss_at_unix`.
    pub toast: Option<ToastSnapshot>,
    /// ID of the sheet currently covering the root shell, if any.
    pub sheet_id: Option<String>,
}

/// Snapshot for the `ViewId::NetworkSettings` projection.
///
/// Read-side only: raw relay list with URL, role tone, and connection state.
/// Swift shell formats the display strings. Bounded: one entry per configured relay.
///
/// Named `KernelNetworkSettingsSnapshot` to avoid collision with the legacy
/// `NetworkSettingsSnapshot` in `relays.rs` (bespoke live lane — Phase 2E coexists
/// with the live lane until the 2F iOS cutover, Non-Negotiable #6).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct KernelNetworkSettingsSnapshot {
    /// Raw relay diagnostic rows; same data as `RelayDiagnosticsViewSnapshot` but
    /// surfaced under the network-settings ViewId.
    pub relays: Vec<RelayDiagRow>,
}

/// Snapshot for the `ViewId::RelayDiagnostics` projection.
///
/// Raw counters and connection state per relay. Swift shell formats labels
/// and "X ago" timestamp strings. Bounded: one row per configured relay URL.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct RelayDiagnosticsViewSnapshot {
    /// One row per relay the NMP kernel knows about (bounded by relay count).
    pub relays: Vec<RelayDiagRow>,
}

/// Tagged union of all view snapshots — one variant per `ViewRoute`.
///
/// Bounded by open views: the kernel only emits a snapshot for a view that
/// has been registered via `open_view`. Closed views produce nothing
/// (Non-Negotiable #7).
// Phase 3D: ProfileSnapshot is large (13 String fields + Vec<CommunityRow>).
// Phase 3F: KernelRoomHomeSnapshot is similarly sized (11 fields + Vec<String>). Both are accepted because
// ViewSnapshot is passed by value rarely (once per snapshot push, not in hot
// loops) and profiling has not flagged this path.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum ViewSnapshot {
    AppRoot(AppRootSnapshot),
    RootShell(RootShellSnapshot),

    // ── Phase 2E additions ────────────────────────────────────────────────────
    /// Network settings overview — relay list with raw fields (D1).
    NetworkSettings(KernelNetworkSettingsSnapshot),
    /// Relay-diagnostics detail — per-relay raw counters and state (D1).
    RelayDiagnostics(RelayDiagnosticsViewSnapshot),

    // ── Phase 3B additions (append-only) ─────────────────────────────────────
    /// Joined-groups / communities list for the active account.
    Communities(CommunitiesSnapshot),

    // ── Phase 3E additions (append-only) ─────────────────────────────────────
    /// Room explorer / discovery screen.
    RoomExplorer(KernelRoomExplorerSnapshot),

    // ── Phase 3D additions (append-only) ─────────────────────────────────────
    /// Single-profile view — identity + relationship + communities.
    /// Articles/highlights deferred to Phase 4.
    Profile(ProfileSnapshot),

    // ── Phase 3F additions (append-only) ─────────────────────────────────────
    /// Per-room home shell — header + metadata + membership + empty lanes.
    /// Lane bodies (kind:11/9 content feeds) deferred to Phase 4.
    ///
    /// Named `KernelRoomHomeSnapshot` to avoid collision with the legacy
    /// `RoomHomeSnapshot` in `room_home.rs` (bespoke live lane — Phase 3F
    /// coexists with the live lane until the iOS cutover, Non-Negotiable #6).
    RoomHome(KernelRoomHomeSnapshot),

    // ── Phase 4C additions (append-only) ─────────────────────────────────────
    /// Active account's NIP-51 kind:10003 bookmark list.
    /// Raw protocol rows only — no labels or formatted strings (D1).
    /// Swift formats bookmark chrome (toolbar icons, swipe titles, etc.).
    Bookmarks(BookmarksSnapshot),

    // ── Phase 4A additions (append-only) ─────────────────────────────────────
    /// NIP-23 article reader — full `ArticleProjection` raw fields including
    /// `content_tree_bytes` for the article body. D1: no formatted strings
    /// ("Untitled", "min read", hashtag labels) — those are Swift-side concerns.
    ArticleReader(KernelArticleReaderSnapshot),

    // ── Phase 4D additions (append-only) ─────────────────────────────────────
    /// NIP-50 relay search results — bounded raw hit rows.
    /// D1: no "X results" count label, no formatted kind/author strings.
    Search(SearchSnapshot),

    // ── Phase 4G additions (append-only) ─────────────────────────────────────
    /// "Following reads" article feed — kind:30023 events over follow authors,
    /// pulled via the Phase 4F feed-pull engine. Raw rows only (D1: no
    /// formatted strings, no "Untitled" fallback, no "min read" labels).
    /// Bounded by the accumulated feed pages (D5: `FEED_PAGE_SIZE` per drain).
    ArticleFeed(ArticleFeedSnapshot),

    // ── Phase 4H additions (append-only) ─────────────────────────────────────
    /// Home/own highlights feed — kind:9802 rows from the pull cursor.
    /// Sorted newest-first; bounded by accumulated pull pages (Non-Negotiable #7).
    /// D1: no byline formatting, no avatar assembly, no source-kind labels — Swift.
    HighlightFeed(HighlightFeedSnapshot),

    // ── Phase 4J additions (append-only) ─────────────────────────────────────
    /// Home feed — merged view of kind:9802 highlights + kind:30023 article reads.
    /// Highlights are grouped by source reference; reads are suppressed when the
    /// article they reference is highlighted. Sorted by latest activity.
    /// Raw rows only (D1: no bylines, no "min read", no "Untitled" fallback).
    ///
    /// Named `KernelHomeFeedSnapshot` to avoid collision with the legacy
    /// `HomeFeedSnapshot` in `home_feed.rs` (bespoke live lane — Phase 4J
    /// coexists with the live lane until the iOS cutover, Non-Negotiable #6).
    HomeFeed(KernelHomeFeedSnapshot),

    // ── Phase 5A additions (append-only) ─────────────────────────────────────
    /// What's New sheet — device-local seen-state projection.
    /// `should_present` drives the sheet; `entries` lists the unseen items.
    /// D1: no "N new features" count label — raw rows only.
    WhatsNew(WhatsNewSnapshot),

    // ── Phase 5C additions (append-only) ─────────────────────────────────────
    /// Book picker — ISBN lookup state (pending + last result + cache size).
    /// Device-local (no nostr facts). Raw fields only (D1).
    BookPicker(crate::kernel::domains::isbn::BookPickerKernelSnapshot),

    // ── Phase 5K additions (append-only) ─────────────────────────────────────
    /// Share-extension intake composer — pending share item + community picker.
    /// Raw fields only (D1: no formatted strings, no community name fallbacks).
    /// Swift formats display labels and handles the share flow after reading the
    /// raw snapshot.
    ShareComposer(crate::kernel::domains::share::ShareComposerSnapshot),

    // ── Phase 5H additions (append-only) ─────────────────────────────────────
    /// Full-screen podcast player — now-playing fields + position/duration/
    /// is_playing + clip range (empty for 5H; populated by Phase 5I/5J).
    /// Device-local (resume position NEVER published to nostr). Raw f64 fields
    /// only (D1: Swift formats "X:XX" timestamps and progress percentage).
    PodcastListening(PodcastListeningSnapshot),

    // ── Phase 5D additions (append-only) ─────────────────────────────────────
    /// OCR capture state — reconstructed markdown + selectable words + raw lines.
    /// Device-local. D1: raw fields only.
    Capture(KernelCaptureSnapshot),

    // ── Phase 7 additions (append-only) ─────────────────────────────────────
    /// NIP-22 comment thread — flat raw record list for `root_tag_value`.
    /// D1: no formatted timestamps, no tree nesting, no byline strings.
    /// Swift builds the display tree from `parent_tag_value` relationships.
    CommentThread(CommentThreadKernelSnapshot),

    // ── Phase 7 feedback additions (append-only) ──────────────────────────────
    /// Feedback thread list — top-level NIP-22 roots under the Highlighter
    /// project root, filtered to the active account. Sorted by `last_activity_at`
    /// descending; capped at 256. D1: no formatted strings; `title`, `summary`,
    /// `status_label` are `None` without an explicit HL metadata source.
    FeedbackThreads(KernelFeedbackThreadsSnapshot),
    /// Feedback thread detail — root record + descendant replies for one thread,
    /// sorted oldest-first. `show_header` follows the 300s/author grouping rule.
    /// D1: raw fields only; no bylines, no relative-time labels.
    FeedbackThread(KernelFeedbackThreadSnapshot),
    // ── Phase 7 chat additions (append-only) ─────────────────────────────────
    /// NIP-29 kind:9 group chat — bounded raw message rows for one room.
    /// D1: no formatted timestamps, no byline strings, no `is_from_me`.
    /// Swift owns all display formatting and optimistic affordances.
    RoomChat(RoomChatSnapshot),
    // ── Phase 7 discussions additions (append-only) ──────────────────────────
    /// Per-room kind:11 discussions list — raw rows filtered from
    /// `GroupEventsProjection`, bounded at 64 per room, newest-first.
    /// D1: no title fallback, no formatted timestamps.
    RoomDiscussions(RoomDiscussionsSnapshot),
}

// ── Phase 7 artifact-preview additions (append-only) ─────────────────────────

/// Content type of an artifact-preview coordinate.
///
/// Variants are raw protocol-level kinds — no display labels (D1). Swift formats
/// all human-readable strings ("Article", "Book", etc.).
///
/// Append-only: new variants at the bottom keep rebases mechanical.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum ArtifactPreviewKind {
    Article,
    Book,
    Podcast,
    Video,
    Paper,
    Web,
    Unknown,
}

/// Lightweight preview row for a content coordinate.
///
/// Keyed by canonical coordinate string in `AppState::artifact_previews`.
/// Raw protocol fields only (D1): Swift formats all display strings, source
/// labels, author bylines, etc. `pending = true` means the resolution is in
/// flight; the UI may show a placeholder until it becomes `false`.
///
/// `display_url` carries the raw URL for `r:` web coordinates. For all other
/// coordinate types it is `None`.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ArtifactPreviewRow {
    /// Canonical coordinate key (e.g. `"a:30023:pk:d"`, `"e:<hex>"`,
    /// `"i:isbn:9780735211292"`, `"r:https://…"`).
    pub coordinate: String,
    /// Title from the protocol source. `None` when not yet resolved (pending).
    pub title: Option<String>,
    /// Cover / thumbnail URL. `None` when absent or not yet resolved.
    pub image_url: Option<String>,
    /// Author's hex pubkey (kind:30023 `pubkey` field). `None` for non-nostr
    /// content (ISBN, web URL) or when not yet resolved.
    pub author_pubkey: Option<String>,
    /// Short summary / description. `None` when absent or not yet resolved.
    pub summary: Option<String>,
    /// Inferred content type from the coordinate scheme.
    pub kind: ArtifactPreviewKind,
    /// `true` while resolution is in flight; `false` when the row is final
    /// (even if some optional fields remain `None`).
    pub pending: bool,
    /// Raw URL for `r:` web coordinates; `None` for all other kinds.
    pub display_url: Option<String>,
}

// ── Phase 4B additions (append-only) ─────────────────────────────────────────

/// Reaction state for a single target event — raw counts only (D1: no labels).
///
/// Swift owns optimistic UI state (count adjustment, toggled icon) — the kernel
/// exposes only the authoritative nmp-projected values. No `"X likes"` string
/// or count formatting here.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ReactionRow {
    /// The event id that was reacted to (raw 64-char hex).
    pub target_event_id: String,
    /// Total number of `+` (like) reactions from all authors as projected
    /// by `ReactionProjection`. Raw u32 — no labels.
    pub count: u32,
    /// `true` if the active viewer has reacted to this event.
    /// Optimistic toggling lives in Swift (D1).
    pub viewer_reacted: bool,
}

// ── Phase 3B additions (append-only) ─────────────────────────────────────────

/// One joined group as seen by the active account.
///
/// Raw protocol data only (D3 / ADR-0032): Swift formats all display strings
/// (`"{n} members"`, `"Open"/"Closed"`, avatar initials, etc.).
/// Fields mirror `nmp_nip29::projection::DiscoveredGroup` plus membership booleans
/// which arrive via `JoinedGroupsSnapshot` (nmp-nip29 PR #1587/#1588).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct CommunityRow {
    /// NIP-29 local group id (the `["d", _]` tag value).
    pub group_id: String,
    /// Host relay URL. Together with `group_id` forms the stable `GroupId`.
    pub host_relay_url: String,
    /// `["name", _]` tag value from kind:39000, if present.
    pub name: Option<String>,
    /// `["picture", _]` tag value from kind:39000, if present.
    pub picture: Option<String>,
    /// `["about", _]` tag value from kind:39000, if present.
    pub about: Option<String>,
    /// Cardinality of `["p", _]` tags on the latest kind:39002 (member list).
    pub member_count: u32,
    /// `true` iff the latest kind:39000 lacks a `["private"]` tag.
    pub public: bool,
    /// `true` iff the latest kind:39000 lacks a `["closed"]` tag.
    pub open: bool,
    /// `true` iff the active account holds admin rights for this group.
    pub is_admin: bool,
}

/// Snapshot for `ViewId::Communities` — the joined-groups list.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct CommunitiesSnapshot {
    /// Joined groups for the active account. Bounded by the projection
    /// (at most as many entries as the account has joined); never grows
    /// with the event store (Non-Negotiable #7).
    pub groups: Vec<CommunityRow>,
}

// ── Phase 3E additions (append-only) ─────────────────────────────────────────

/// One discovered group row — raw protocol data only (D3 / ADR-0032).
/// Swift shell formats display strings (name fallback, member label, etc.).
/// Fields mirror `nmp_nip29::projection::DiscoveredGroup`.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct DiscoveredRow {
    pub group_id: String,
    pub host_relay_url: String,
    pub name: Option<String>,
    pub picture: Option<String>,
    pub about: Option<String>,
    pub member_count: u32,
    pub public: bool,
    pub open: bool,
}

/// A recommendation row for the friends/authors shelves.
///
/// Raw data only — Swift builds `"@{handle} + N you follow"` from `reason_pubkeys`.
/// `total_reason_count` may exceed `reason_pubkeys.len()` if the vec is capped.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct RecommendationRow {
    pub group_id: String,
    pub host_relay_url: String,
    pub name: Option<String>,
    pub picture: Option<String>,
    pub about: Option<String>,
    /// Pubkeys of the follows who are in this group (capped for snapshot size).
    pub reason_pubkeys: Vec<String>,
    /// Total follows count in this group before capping `reason_pubkeys`.
    pub total_reason_count: u32,
}

/// Snapshot for `ViewId::RoomExplorer` — the discovery screen.
///
/// Named `KernelRoomExplorerSnapshot` to avoid collision with the legacy
/// `RoomExplorerSnapshot` in `room_explorer.rs` (bespoke live lane — Phase 3E
/// coexists with the live lane until the iOS cutover, Non-Negotiable #6).
///
/// Bounded by projection: `featured` is empty until curator logic is wired
/// (Phase 3F); `new_noteworthy` is capped at 256; shelves are empty until
/// Phase 4 feeds.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct KernelRoomExplorerSnapshot {
    /// Featured rooms (curator-selected). Empty in Phase 3 — wired in Phase 3F.
    pub featured: Vec<DiscoveredRow>,
    /// Discovered rooms (public+open, excluding joined), newest-first, cap 256.
    pub new_noteworthy: Vec<DiscoveredRow>,
    /// Rooms with ≥2 follows inside. Empty in Phase 3 (requires Phase 4 member events).
    pub friends_shelf: Vec<RecommendationRow>,
    /// Rooms from authors you read. Empty in Phase 3 (requires Phase 4 feed data).
    pub authors_shelf: Vec<RecommendationRow>,
}

// ── Phase 3D additions (append-only) ─────────────────────────────────────────

/// Snapshot for `ViewId::Profile{pubkey}` — the profile detail view.
///
/// Raw `ProfileCardModel` fields + relationship + communities (Phase 3D scope).
/// Articles and highlights deferred to Phase 4.
///
/// Raw-data doctrine (D3 / ADR-0032): Swift formats ALL display strings from
/// these raw fields. Kernel emits no bech32 (`npub`), no NIP-05 label strip
/// (`"_@example.com"→"example.com"`), no handle fallback, no `"abc123…d789"`
/// short-pubkey abbreviation — those are Swift-side presentation decisions.
///
/// `is_following` is derived from `AppState::is_following(pubkey)` (3C), which
/// reads the active account's `follows` projection.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ProfileSnapshot {
    /// Raw 64-char hex pubkey of the viewed profile.
    pub pubkey: String,
    /// `ProfileCardModel::display_name` — `Some` if the kind:0 has a
    /// non-empty `"display_name"` field. Swift formats fallback order.
    pub display_name: Option<String>,
    /// `ProfileCardModel::name` — `Some` if the kind:0 has a non-empty
    /// `"name"` (username) field.
    pub name: Option<String>,
    /// `ProfileCardModel::raw_display_name` — `Some` if the kind:0 has a
    /// non-empty `"display_name"` without the camelCase normalisation.
    pub raw_display_name: Option<String>,
    /// `ProfileCardModel::picture_url` — `Some` if the kind:0 has a
    /// non-empty `"picture"` URL.
    pub picture_url: Option<String>,
    /// `ProfileCardModel::banner` — `Some` if the kind:0 has a non-empty
    /// `"banner"` URL.
    pub banner: Option<String>,
    /// `ProfileCardModel::website` — `Some` if the kind:0 has a non-empty
    /// `"website"` field.
    pub website: Option<String>,
    /// `ProfileCardModel::nip05` — raw NIP-05 identifier string (e.g.
    /// `"_@example.com"` or `"alice@example.com"`). Swift strips the leading
    /// `"_@"` prefix for display (`"example.com"`). Empty string if absent.
    pub nip05: String,
    /// `ProfileCardModel::about` — raw bio / about text. Empty string if absent.
    pub about: String,
    /// `ProfileCardModel::lud16` — Lightning address, if present.
    pub lud16: Option<String>,
    /// `true` if the active account follows this pubkey (derived from the
    /// `FollowListSnapshot` via `AppState::is_following`). Updated on every
    /// `FollowListUpdated` event — the Profile view reflects follow state
    /// changes without requiring a re-claim.
    pub is_following: bool,
    /// Communities (joined groups) that are known to the active account.
    /// Phase 3D: surfaces the active account's joined-groups list as context.
    /// Phase 4 will add per-pubkey group-membership interests.
    /// Bounded by the `JoinedGroupsSnapshot` (never grows with the event store).
    pub communities: Vec<CommunityRow>,
}

// ── Phase 3F additions (append-only) ─────────────────────────────────────────

/// Snapshot for `ViewId::RoomHome{group_id}` — the per-room home shell.
///
/// Ships the room header (metadata) + membership state + lane bodies.
/// Phase 3F shipped an empty lanes structure; Phase 4I fills lane bodies via
/// the feed-pull engine (ADR-0058).
///
/// Raw-data doctrine (D3 / ADR-0032): Swift formats ALL display strings from
/// these raw fields. Kernel emits no formatted strings (`"{n} members"`, etc.).
///
/// `invite_link_base` is supplied from `AppState::room_policy.invite_link_base`
/// (D3: injected at construction, never hardcoded). Swift composes the full
/// invite URL by appending the invite code to this base.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct KernelRoomHomeSnapshot {
    /// NIP-29 local group id (the `["d", _]` tag value).
    pub group_id: String,
    /// Host relay URL. Together with `group_id` forms the stable `GroupId`.
    pub host_relay_url: String,
    /// `["name", _]` tag value from kind:39000, if present.
    pub name: Option<String>,
    /// `["picture", _]` tag value from kind:39000, if present.
    pub picture: Option<String>,
    /// `["about", _]` tag value from kind:39000, if present.
    pub about: Option<String>,
    /// Cardinality of `["p", _]` tags on the latest kind:39002 (member list).
    pub member_count: u32,
    /// `true` iff the latest kind:39000 lacks a `["private"]` tag.
    pub public: bool,
    /// `true` iff the latest kind:39000 lacks a `["closed"]` tag.
    pub open: bool,
    /// `true` iff the active account holds admin rights for this group.
    pub is_admin: bool,
    /// Lane identifiers for this room (e.g. `"general"`, `"notes"`).
    /// Phase 4I fills these via the feed-pull engine (ADR-0058).
    /// Empty when no feed rows have arrived yet.
    /// Bounded by the number of lanes configured for the room (non-negotiable #7).
    pub lane_ids: Vec<String>,
    /// Base URL for invite links (e.g. `"https://highlighter.com/r"`).
    /// Swift composes the full invite URL: `"{invite_link_base}/{code}"`.
    /// Sourced from `AppState::room_policy.invite_link_base` (D3 — never hardcoded).
    pub invite_link_base: String,

    // ── Phase 4I additions (append-only) ─────────────────────────────────────
    /// Raw event rows from the room-lane feed (kind:9 or kind:11 with `#h` tag).
    /// Populated by the ADR-0058 pull engine when `ViewId::RoomHome` is open.
    /// Bounded at `ROOM_LANE_ROW_CAP` rows per group in `room_home.rs`.
    /// Empty until the first feed page arrives.
    pub lanes: Vec<RoomLaneRow>,

    // ── Room-home aggregation additions (append-only) ─────────────────────────
    /// Artifact library: kind:11 share events (non-discussion) from the lane feed
    /// with artifact coordinates resolved through `AppState::artifact_previews`.
    pub artifact_library: Vec<KernelRoomLibraryRow>,
    /// All highlights (kind:9802) received via the room highlight feed, sorted
    /// newest-first. Bounded at `ROOM_HOME_HIGHLIGHT_CAP` rows.
    pub highlights: Vec<HighlightRow>,
    /// Highlights grouped by artifact coordinate (matching entries in artifact_library).
    pub highlights_by_reference: Vec<KernelHighlightReferenceBucket>,
    /// Comment counts + rows grouped by artifact coordinate.
    pub comments_by_reference: Vec<KernelCommentReferenceBucket>,
}

// ── Room-home aggregation additions (append-only) ─────────────────────────────

/// One artifact in the room's library — a kind:11 share event (not a discussion)
/// whose artifact coordinate is resolved through `AppState::artifact_previews`.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct KernelRoomLibraryRow {
    pub coordinate: String,
    pub share_event_id: String,
    pub preview: Option<ArtifactPreviewRow>,
}

/// Highlights grouped by artifact coordinate.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct KernelHighlightReferenceBucket {
    pub coordinate: String,
    pub highlights: Vec<HighlightRow>,
}

/// Comment counts + rows grouped by artifact coordinate.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct KernelCommentReferenceBucket {
    pub root_tag_value: String,
    pub comments: Vec<CommentRecordRow>,
    pub count: u32,
}

// ── Phase 4I additions (append-only) ─────────────────────────────────────────

/// One raw event row from the room-lane feed (kind:9 or kind:11 with `#h` tag).
///
/// Raw protocol data only (D1): no formatted strings, no labels, no presenter
/// logic. Swift formats author names, timestamps, content previews, etc.
/// Bounded: rows are capped at `ROOM_LANE_ROW_CAP` per group in `room_home.rs`.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct RoomLaneRow {
    /// Raw 64-char hex event id.
    pub event_id: String,
    /// Raw 64-char hex author pubkey.
    pub author_pubkey: String,
    /// Nostr event kind (9 = chat message, 11 = share/artifact).
    pub kind: u32,
    /// Event content field (raw text — no formatting, no HTML; D1).
    pub content: String,
    /// UNIX seconds `created_at` field (integer, no "X ago" format; D1).
    pub created_at: u64,
    /// Raw tag list as delivered by the nmp kernel.
    /// Swift extracts `#h`, `#e`, `#a` tag values for display/routing.
    pub tags: Vec<Vec<String>>,
}

// ── Phase 4C additions (append-only) ─────────────────────────────────────────

/// One bookmark item from the active account's NIP-51 kind:10003 list.
///
/// Raw protocol data only (D1): no presentation strings, no formatted labels,
/// no toolbar chrome. Swift formats all user-visible bookmark UI.
///
/// Mirrors `nmp_nip51::BookmarkItem` but as a `uniffi::Enum` for FFI.
/// Variants match the NIP-51 tag types: `e` (event), `a` (address),
/// `r` (URL), `t` (hashtag).
#[derive(Debug, Clone, PartialEq, serde::Deserialize, uniffi::Enum)]
pub enum BookmarkRow {
    /// `["e", <event-id>, <optional-relay>]` — a bookmarked Nostr event.
    Event {
        /// Raw 64-char lowercase hex event id.
        event_id: String,
        /// Optional relay hint (opaque URL string; D3 — never constructed).
        relay: Option<String>,
    },
    /// `["a", <kind:pubkey:d>, <optional-relay>]` — a bookmarked replaceable event.
    Address {
        /// NIP-19 address coordinate: `"<kind>:<pubkey>:<d-tag>"`.
        coordinate: String,
        /// Optional relay hint.
        relay: Option<String>,
    },
    /// `["r", <url>]` — a bookmarked web URL.
    Url {
        /// Raw HTTP/HTTPS URL string.
        url: String,
    },
    /// `["t", <hashtag>]` — a bookmarked hashtag.
    Hashtag {
        /// Normalised lowercase hashtag string (without `#` prefix).
        hashtag: String,
    },
}

/// Snapshot for `ViewId::Bookmarks` — the active account's NIP-51 kind:10003
/// bookmark list.
///
/// Raw protocol data only (D1): Swift formats all display strings, toolbar
/// icons, swipe actions, empty-state copy, and accessibility labels.
/// Bounded by the bookmark list length (non-negotiable #7: never grows with
/// the unbounded event store — only the latest kind:10003 is projected).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BookmarksSnapshot {
    /// Bookmark items from the active account's kind:10003 list.
    /// Raw `BookmarkRow` values — no labels or presentation formatting (D1).
    pub rows: Vec<BookmarkRow>,
}

// ── Phase 4A additions (append-only) ─────────────────────────────────────────

/// Raw NIP-23 article record stored in `AppState::articles`.
///
/// One entry per decoded `ArticleProjection` from the `"nmp.nip23.articles"`
/// typed sidecar. Raw protocol data only — D1: no `"Untitled"` fallback for
/// absent titles, no `"{minutes} min read"` formatted string, no `"#{tag}"`
/// hashtag labels. Swift / D1 owns all presentation strings.
///
/// `content_tree_bytes` is the re-encoded `ContentTreeWire` opaque bytes for
/// the article body (using `nmp_content::wire::encode_content_tree`). Empty for
/// feed-list-only rows where the full document has not arrived yet.
#[derive(Debug, Clone, PartialEq)]
pub struct ArticleRow {
    /// Addressable coordinate `kind:author_hex:d_tag`.
    pub address: String,
    /// 64-character hex event id of the winning kind:30023 event.
    pub id: String,
    /// 64-character hex author pubkey.
    pub author_pubkey: String,
    /// Optional display name from a kind:0 profile (enriched by the projection).
    pub author_display_name: Option<String>,
    /// Optional author picture URL from a kind:0 profile.
    pub author_picture_url: Option<String>,
    /// `title` tag value, or `None` when absent (D1: no "Untitled" fallback).
    pub title: Option<String>,
    /// `summary` tag value, or `None` when absent.
    pub summary: Option<String>,
    /// `image` (hero) tag value as URL, or `None` when absent.
    pub hero_image_url: Option<String>,
    /// Addressable `d` tag value.
    pub d_tag: String,
    /// Event creation time as Unix seconds.
    pub created_at: u64,
    /// Opaque `ContentTreeWire` bytes for the article body, or empty when
    /// only the feed-list trimmed summary (no body) has arrived.
    pub content_tree_bytes: Vec<u8>,
}

/// Snapshot for `ViewId::ArticleReader{address}` — the article reader view.
///
/// Carries the full article document fields from the `ArticleProjection`
/// (via `AppState::articles`). Raw-data doctrine (D1 / ADR-0032): Swift
/// formats ALL display strings from these raw fields. No `"Untitled"` title
/// fallback, no `"{n} min read"` label, no `"#{tag}"` hashtag formatting.
///
/// `content_tree_bytes` is the opaque serialised `ContentTreeWire` for the
/// article body. Empty until the full article document arrives (the feed-list
/// trimmed summary has no body). Swift / platform layer decodes these bytes
/// using the `nmp-content` wire codec.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct KernelArticleReaderSnapshot {
    /// Addressable coordinate `kind:author_hex:d_tag`.
    pub address: String,
    /// 64-character hex event id.
    pub id: String,
    /// 64-character hex author pubkey. Swift formats bech32 `npub`.
    pub author_pubkey: String,
    /// Optional display name from the author's kind:0 (enriched by projection).
    pub author_display_name: Option<String>,
    /// Optional author picture URL from kind:0.
    pub author_picture_url: Option<String>,
    /// `title` tag, or `None` when absent. D1: no "Untitled" fallback.
    pub title: Option<String>,
    /// `summary` tag, or `None` when absent.
    pub summary: Option<String>,
    /// `image` (hero) tag URL, or `None` when absent.
    pub hero_image_url: Option<String>,
    /// Addressable `d` tag value.
    pub d_tag: String,
    /// Event creation time as Unix seconds. Swift formats the display date.
    pub created_at: u64,
    /// Opaque `ContentTreeWire` bytes for the article body.
    /// Empty until the full document arrives. Swift / platform decodes via
    /// `nmp_content::wire::decode_content_tree`.
    pub content_tree_bytes: Vec<u8>,
    /// The article body content tree as serde-JSON (Phase 7, option β). Swift's
    /// vendored nmp `ContentTreeWire.swift` is JSON-`Decodable` and feeds
    /// `NostrContentRenderer` — this is the body render path (replacing raw
    /// markdown). Empty string until the document arrives / on decode failure.
    pub content_tree_json: String,
    /// Overlay highlights anchored to this article (kind:9802 tagged `#a ==
    /// address`), newest-first, deduped by event id. Carries the SAME enriched
    /// NIP-84/NIP-73 fields as the highlight feed (decoded via the shared
    /// `decode_highlight_row`). Empty in the brief window between OpenView and
    /// the first highlight-feed page (Phase 7).
    pub highlights: Vec<HighlightRow>,
}

// ── Phase 4D additions (append-only) ─────────────────────────────────────────

/// One NIP-50 search hit stored in `AppState::search_results`.
///
/// Raw protocol data only (D1 / ADR-0032): Swift formats all display strings
/// (event-kind labels, author bech32, date, content preview, etc.).
/// Fields mirror `nmp_nip50::SearchHit` — the bounded accumulator in
/// `SearchResultsProjection` (max `DEFAULT_MAX_SEARCH_HITS = 200` entries).
#[derive(Debug, Clone, PartialEq)]
pub struct SearchHitRow {
    /// 64-character hex event id.
    pub id: String,
    /// 64-character hex author pubkey. D1: Swift formats bech32 `npub`.
    pub author: String,
    /// Nostr event kind number (raw u32 — D1: Swift maps to display label).
    pub kind: u32,
    /// Event creation time as Unix seconds. D1: Swift formats the display date.
    pub created_at: u64,
    /// Raw event `content` field.
    pub content: String,
    /// Event tags as a Vec of Vec<String> (same wire shape as Nostr JSON).
    pub tags: Vec<Vec<String>>,
    /// Relay URLs this event was observed on (may be empty for cache hits).
    pub relay_provenance: Vec<String>,
}

/// One local-scan community result row for the Search screen communities bucket.
///
/// Derived from `AppState::discovered_groups` merged with `AppState::communities`.
/// Raw protocol data only (D1 / ADR-0032): Swift formats all display strings
/// (name fallback to group_id, `"{n} members"` label, avatar initials, etc.).
///
/// `member_count` is raw `u64` — no `"N members"` label (D1). Widened from the
/// `u32` in `CommunityRow` / `DiscoveredRow` source types.
/// `public` and `open` are omitted: the kernel already filters to public+open
/// rows before emitting, so Swift never needs to render a closed/private row.
///
/// Append-only: new fields at the bottom keep rebases mechanical.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct CommunitySearchRow {
    /// NIP-29 local group id (the `["d", _]` tag value).
    pub group_id: String,
    /// Host relay URL. Together with `group_id` forms the stable `GroupId`.
    pub host_relay_url: String,
    /// `["name", _]` tag value from kind:39000, if present.
    /// D1: no fallback to group_id — Swift owns the fallback display logic.
    pub name: Option<String>,
    /// `["about", _]` tag value from kind:39000, if present.
    pub about: Option<String>,
    /// `["picture", _]` tag value from kind:39000, if present.
    pub picture: Option<String>,
    /// Cardinality of `["p", _]` tags on the latest kind:39002 (member list).
    /// Raw `u64` — no `"N members"` label (D1).
    pub member_count: u64,
}

/// Snapshot for `ViewId::Search` — NIP-50 relay search results + local buckets.
///
/// Raw protocol data only (D1): Swift formats all display strings.
/// Bounded by `SearchResultsProjection`'s `max_hits` cap
/// (default `DEFAULT_MAX_SEARCH_HITS = 200` from nmp-nip50 — Non-Negotiable #7)
/// for `hits`; `communities` is bounded at 20 (COMMUNITY_SEARCH_CAP in search.rs).
///
/// Append-only: `communities` is the Phase 7 gate #4 addition.
/// Profile rows are deferred to nmp #1697.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct SearchSnapshot {
    /// Ordered (by `created_at` descending, then `id` ascending) search hit rows.
    /// Raw fields only — no "X results" count label, no formatted strings (D1).
    pub hits: Vec<KernelSearchHitRow>,
    // ── Phase 7 (gate #4 communities bucket) additions (append-only) ─────────
    /// Local-scan community results from `AppState::discovered_groups` merged with
    /// `AppState::communities`. Substring-matched by name/about against the
    /// active `search_query`; filtered to public+open; sorted by lowercase name
    /// then host_relay_url then group_id; bounded at 20.
    /// Empty when the query is blank or no public+open communities match.
    /// D1: raw rows only — Swift renders all display strings and fallbacks.
    pub communities: Vec<CommunitySearchRow>,
}

// ── Phase 4G additions (append-only) ─────────────────────────────────────────

/// One kind:30023 article row from the "Following reads" feed.
///
/// Decoded from a raw `KernelEvent` emitted by the Phase 4F feed-pull engine.
/// Raw protocol data only (D1 / ADR-0032): Swift formats ALL display strings
/// from these raw fields. No `"Untitled"` title fallback, no `"{n} min read"`
/// label, no `"#{tag}"` hashtag formatting — those are Swift-side concerns.
///
/// Distinct from `ArticleRow` (Phase 4A): `ArticleRow` is keyed by addressable
/// coordinate in `AppState::articles` and carries `content_tree_bytes` for the
/// full article body. `ArticleFeedRow` is lighter (no body bytes) and is used
/// for feed list display only.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ArticleFeedRow {
    /// Addressable coordinate `kind:author_hex:d_tag` (built from the raw event).
    pub address: String,
    /// 64-character hex event id of the kind:30023 event.
    pub id: String,
    /// 64-character hex author pubkey. Swift formats bech32 `npub`.
    pub author_pubkey: String,
    /// `["title", _]` tag value, or `None` when absent.
    /// D1: no "Untitled" fallback — `None` means genuinely absent.
    pub title: Option<String>,
    /// `["summary", _]` tag value, or `None` when absent.
    pub summary: Option<String>,
    /// `["image", _]` tag value as a URL, or `None` when absent.
    pub hero_image_url: Option<String>,
    /// Addressable `d` tag value.
    pub d_tag: String,
    /// Event creation time as Unix seconds. Swift formats the display date.
    pub created_at: u64,
}

/// Snapshot for `ViewId::ArticleFeed` — the "Following reads" article feed.
///
/// Raw protocol rows only (D1): Swift formats all list-cell display strings
/// (title fallback, author name, date label, read-time estimate, hero image).
/// Bounded by accumulated feed pages (`FEED_PAGE_SIZE` × number of drains,
/// D5 / Non-Negotiable #7).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ArticleFeedSnapshot {
    /// Decoded kind:30023 article rows in ingest-seq order (newest first from
    /// the network, but stored in the order drained from the pager). Raw fields
    /// only — no labels, no formatted strings (D1).
    pub rows: Vec<ArticleFeedRow>,
    /// `true` when `has_more == false` from the last drain (fully caught up).
    /// Swift uses this to decide whether to show a "load more" affordance.
    pub exhausted: bool,
}

/// uniffi-compatible search hit row for FFI.
///
/// Mirrors the Rust-internal `SearchHitRow`. `tags` is the raw NIP-01 tag array
/// (`Vec<Vec<String>>`, which uniffi DOES support — see `KernelEventRow`/
/// `ArtifactPreview` precedents), so Swift can bucket hits by `kind` and extract
/// the fields each result card needs (article title/summary/image/d, highlight
/// a/e/context, etc.) — D1: raw protocol data only, Swift owns all formatting.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct KernelSearchHitRow {
    /// 64-character hex event id.
    pub id: String,
    /// 64-character hex author pubkey. D1: Swift formats bech32 `npub`.
    pub author: String,
    /// Nostr event kind number (raw u32).
    pub kind: u32,
    /// Event creation time as Unix seconds.
    pub created_at: u64,
    /// Raw event `content` field.
    pub content: String,
    /// Raw NIP-01 tags (`[[name, value, ...], ...]`). Swift extracts per-kind
    /// fields from these to hydrate the result-bucket cards.
    pub tags: Vec<Vec<String>>,
    /// Relay URLs this event was observed on.
    pub relay_provenance: Vec<String>,
}

// ── Phase 4H additions (append-only) ─────────────────────────────────────────

/// One NIP-84 kind:9802 highlight row stored in the highlight feed.
///
/// Decoded from a raw `KernelEvent` (kind:9802) pulled via the
/// `"hl.feed.highlights"` feed cursor (ADR-0058). Raw protocol data only
/// (D1 / ADR-0032): Swift formats ALL display strings.
///
/// D1: no byline composition (`"Highlighted by {name}, {name2} and {n} others"`),
/// no avatar URL assembly, no source-kind icon/label, no "share" copy — those
/// are Swift-side presentation decisions. The kernel emits only the raw
/// `content`, `source_reference`, `author_pubkey`, and `created_at`.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct HighlightRow {
    /// 64-character hex event id of the kind:9802 event.
    pub event_id: String,
    /// 64-character hex author pubkey. D1: Swift formats bech32 `npub`.
    pub author_pubkey: String,
    /// The highlighted text content from the kind:9802 event's `content` field.
    /// Raw UTF-8 — no truncation or ellipsis (D1: Swift owns display formatting).
    pub content: String,
    /// NIP-84 source reference extracted from the `a` (addressable) or `e`
    /// (non-addressable) tag of the kind:9802 event, if present.
    ///
    /// - `a` tag: `"<kind>:<pubkey>:<d_tag>"` coordinate (used for NIP-23 articles).
    /// - `e` tag: raw 64-char hex event id.
    /// - `None` when no source tag is present (valid per NIP-84 §3.2 for
    ///   free-standing highlights not anchored to a specific resource).
    ///
    /// D3: opaque string from the protocol — kernel never constructs references.
    pub source_reference: Option<String>,
    /// Optional user note attached to the highlight, from the NIP-84 `comment`
    /// tag of the kind:9802 event. `None` when absent or empty. Raw UTF-8 (D1:
    /// Swift owns blank-note display rules). Mirrors the live lane's `note`.
    pub note: Option<String>,
    /// Event creation time as Unix seconds. D1: Swift formats the display date.
    pub created_at: u64,

    // ── Phase 7 enrichment: NIP-84/NIP-73 source + clip + image fields ────────
    // Mirrors the bespoke `highlights.rs::record_from_cached_event` parse so the
    // highlight CARD (resource header, podcast-clip chrome, page-scan image) can
    // render from kernel rows. Empty string / None when the tag is absent (D1).
    /// `context` tag — the paragraph the quote was lifted from.
    pub context: String,
    /// `a` tag — addressable artifact coordinate `kind:pubkey:d` (NIP-23 etc).
    pub artifact_address: String,
    /// `e` tag — non-addressable source event id.
    pub event_reference: String,
    /// `i` tag — NIP-73 external content id (`podcast:item:guid:…`, `isbn:…`).
    pub external_reference: String,
    /// `r` tag — source URL (web highlight).
    pub source_url: String,
    /// Canonical source key: `a:…` / `e:…` / `i:…` / `r:…` in priority order,
    /// or empty when no source tag is present (mirrors the live lane).
    pub source_reference_key: String,
    /// `start` tag — podcast-clip start in seconds. `None` when absent.
    pub clip_start_seconds: Option<f64>,
    /// `end` tag — podcast-clip end in seconds. `None` when absent.
    pub clip_end_seconds: Option<f64>,
    /// `speaker` tag — podcast-clip speaker. Empty when absent.
    pub clip_speaker: String,
    /// All `segment` tag values — transcript segment ids for a podcast clip.
    pub clip_transcript_segment_ids: Vec<String>,
    /// NIP-92 `imeta` image URL — the page-scan photo. Empty when absent.
    pub image_url: String,
}

/// Snapshot for `ViewId::HighlightFeed` — the home/own highlights feed.
///
/// Carries the decoded highlight rows from `AppState::highlight_feed`, sorted by
/// `created_at` descending (newest first), deduplicated by `event_id`.
///
/// Raw-data doctrine (D1): Swift formats ALL display strings from these raw
/// fields. No `"Highlighted by {name}"` byline, no avatar URLs, no
/// source-kind labels, no share-message composition — those are Swift concerns.
///
/// Bounded by the accumulated pull pages (each page is capped at
/// `feed::FEED_PAGE_SIZE = 20` entries — Non-Negotiable #7). `exhausted`
/// signals when the cursor has caught up to the ingest log head.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct HighlightFeedSnapshot {
    /// Decoded highlight rows, sorted newest-first. Raw fields only (D1).
    pub rows: Vec<HighlightRow>,
    /// `true` when the pull cursor is caught up (`has_more == false` on last drain).
    /// Swift uses this to hide the "load more" affordance.
    pub exhausted: bool,
}

// ── Phase 4J additions (append-only) ─────────────────────────────────────────

/// Discriminant for a row in the merged home feed.
///
/// D1: no user-visible label — Swift maps this to display strings.
/// `Highlight` rows carry one or more highlight event ids grouped by source;
/// `Article` rows carry the article coordinate for a standalone (non-highlighted) read.
///
/// Named `KernelHomeFeedRowKind` to avoid collision with any legacy type in the
/// bespoke live lane (Non-Negotiable #6 coexistence).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum KernelHomeFeedRowKind {
    /// One or more highlights grouped by the same source reference (article/URL).
    Highlight,
    /// A standalone article read (not suppressed by any highlight).
    Article,
}

/// One row in the merged home feed.
///
/// For `KernelHomeFeedRowKind::Highlight`: carries the grouped highlight event ids and
/// the common source_reference (if any).
/// For `KernelHomeFeedRowKind::Article`: carries the article address, event id, and author.
///
/// D1: raw structural fields only — no bylines, no "min read", no "Untitled"
/// fallback, no avatar URLs. Swift owns all presentation.
/// D3: opaque source_reference and article_address strings from the protocol —
/// the kernel never constructs them.
///
/// Named `KernelHomeFeedRow` to avoid collision with any legacy type in the
/// bespoke live lane (Non-Negotiable #6 coexistence).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct KernelHomeFeedRow {
    /// Deterministic structural key for stable SwiftUI list identity.
    /// - `"h:src:<source_reference>"` for highlight groups with a source reference.
    /// - `"h:evt:<event_id>"` for solo highlights without a source reference.
    /// - `"r:<article_address>"` for standalone article rows.
    ///
    /// D1: this is a structural identity key, not a user-visible label.
    pub stable_id: String,

    /// Timestamp used for inter-row sort order (descending: newest first).
    /// For highlight rows: the maximum `created_at` of all highlights in the group.
    /// For article rows: the article's `created_at`.
    pub sort_key: u64,

    /// Row type discriminant. Swift switches on this to decide which cell
    /// template to render.
    pub kind: KernelHomeFeedRowKind,

    // ── Highlight-row fields (populated when kind == Highlight) ───────────────
    /// 64-char hex event ids of the highlights in this group, sorted by
    /// `created_at` ascending (oldest first within the group — matches live lane).
    /// Empty for `KernelHomeFeedRowKind::Article` rows.
    pub highlight_event_ids: Vec<String>,

    /// Author pubkeys of the highlights in this group (parallel to
    /// `highlight_event_ids`). D1: Swift formats bech32 `npub` / display name.
    /// Empty for `KernelHomeFeedRowKind::Article` rows.
    pub highlight_author_pubkeys: Vec<String>,

    /// NIP-84 source reference common to all highlights in this group, if
    /// present. `None` for solo highlights not anchored to a resource.
    /// `a`-tag form: `"kind:pubkey:d_tag"` (addressable).
    /// `e`-tag form: raw 64-char hex event id.
    /// D3: opaque from the protocol.
    pub source_reference: Option<String>,

    // ── Article-row fields (populated when kind == Article) ───────────────────
    /// Addressable coordinate `"kind:author_hex:d_tag"` of the article.
    /// `None` for `KernelHomeFeedRowKind::Highlight` rows.
    pub article_address: Option<String>,

    /// 64-char hex event id of the kind:30023 article event.
    /// `None` for `KernelHomeFeedRowKind::Highlight` rows.
    pub article_id: Option<String>,

    /// 64-char hex author pubkey of the article.
    /// D1: Swift formats bech32 `npub` / display name.
    /// `None` for `KernelHomeFeedRowKind::Highlight` rows.
    pub article_author_pubkey: Option<String>,

    /// Unix-second `created_at` of the article event.
    /// D1: Swift formats the display date.
    /// `None` for `KernelHomeFeedRowKind::Highlight` rows.
    pub article_created_at: Option<u64>,

    /// Canonical artifact-preview coordinate key this row references, if any
    /// (Phase 7 artifact-preview consumer). For Article rows: the `a:` article
    /// coordinate. For Highlight rows: the canonicalized `source_reference`
    /// (`a:`/`e:`/`i:`/`r:`), or `None` for free-standing highlights with no
    /// source. Swift looks this up in `KernelHomeFeedSnapshot.artifact_previews`
    /// to render the resource card (skeleton while the preview is pending). D3.
    pub artifact_coordinate: Option<String>,

    // ── Phase 7 home-feed aggregation additions (append-only) ────────────────
    /// `true` iff the article author is in the active account's follow set
    /// (`state.is_following(article_author_pubkey)`).
    /// D1: a boolean fact, not a display label ("Following" copy is Swift-side).
    /// Always `false` for `KernelHomeFeedRowKind::Highlight` rows (no article author
    /// to surface — may change in a future product iteration).
    pub author_followed: bool,

    /// Hex pubkeys of follows who have interacted with this article (kind 1, 7, 16,
    /// or 1111 with `#k=30023`), deduped and sorted: latest-interaction-time
    /// descending, then pubkey ascending as a tie-break.
    /// D1: raw hex pubkeys — Swift formats bech32 / display names.
    /// Empty for `KernelHomeFeedRowKind::Highlight` rows.
    pub interactor_pubkeys: Vec<String>,

    /// The latest-activity timestamp for this row — max of the article
    /// `created_at` and all matching interaction `created_at` values.
    /// Equals `sort_key` for both row kinds. Unix seconds (D1: no "X ago" label).
    pub latest_activity_at: u64,

    /// Enriched highlight rows for this group, sorted oldest-first within the
    /// group (matches the bespoke live lane ordering). Populated from the same
    /// raw kind:9802 events as `highlight_event_ids`, decoded via the shared
    /// `decode_highlight_row` so they carry the full NIP-84/NIP-73 fields.
    /// Empty for `KernelHomeFeedRowKind::Article` rows.
    pub highlights: Vec<HighlightRow>,
}

/// Snapshot for `ViewId::HomeFeed` — the merged home feed.
///
/// Carries the merged, suppressed, grouped, and sorted list of home-feed rows
/// derived from `AppState::article_feed` (Phase 4G) and
/// `AppState::highlight_feed` (Phase 4H).
///
/// Raw-data doctrine (D1 / ADR-0032): Swift formats ALL display strings from
/// these raw fields. No `"Highlighted by {name}"`, no `"{n} min read"`, no
/// `"#{tag}"` formatting, no `"Untitled"` fallback — those are Swift-side
/// presentation decisions.
///
/// Bounded by the underlying feed pages (`FEED_PAGE_SIZE` × drain calls per
/// feed — Non-Negotiable #7). The number of output rows ≤ sum of both feed
/// page sizes (suppression only reduces the count).
///
/// Named `KernelHomeFeedSnapshot` to avoid collision with the legacy
/// `HomeFeedSnapshot` in `home_feed.rs` (bespoke live lane — Phase 4J
/// coexists with the live lane until the iOS cutover, Non-Negotiable #6).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct KernelHomeFeedSnapshot {
    /// Merged rows sorted by `sort_key` descending. Raw structural fields only (D1).
    pub rows: Vec<KernelHomeFeedRow>,
    /// Artifact-preview rows for the coordinates these feed rows reference
    /// (Phase 7 artifact-preview consumer). Filtered to only the
    /// `artifact_coordinate` values present in `rows`. Swift keys by
    /// `coordinate` to render each row's resource card; a `pending` row (or a
    /// missing coordinate) renders as a skeleton. D1: raw preview fields only.
    pub artifact_previews: Vec<ArtifactPreviewRow>,
}

// ── Phase 5A additions (append-only) ─────────────────────────────────────────

/// One What's New changelog entry from the bundled `resources/whats-new.json`.
///
/// Raw protocol data only (D1): Swift formats all display strings from these
/// raw fields. No bullet formatting, no "New!" badge, no date labels —
/// those are Swift-side presentation decisions.
///
/// Used both in `KernelEvent::WhatsNewLoaded` and in `WhatsNewSnapshot`.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct WhatsNewEntryRow {
    /// ISO-8601 UTC timestamp string as it appears in the bundled JSON
    /// (e.g. `"2026-05-14T21:45:00Z"`). D1: Swift formats the display date.
    pub shipped_at_iso: String,
    /// UNIX seconds parsed from `shipped_at_iso`. D1: no "X ago" label.
    pub shipped_at_unix: u64,
    /// Changelog bullet lines for this release. Raw strings — no `"• "` prefix
    /// or markdown formatting added by the kernel (D1: Swift owns presentation).
    pub lines: Vec<String>,
}

/// Snapshot for `ViewId::WhatsNew` — the What's New sheet.
///
/// Device-local: never derived from or published to Nostr events.
/// Raw-data doctrine (D1): Swift formats all display strings from these raw
/// fields. No `"N new features"` count label, no badge formatting.
///
/// Bounded by the number of entries in the bundled JSON (typically < 20;
/// Non-Negotiable #7 — does not grow with the event store).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct WhatsNewSnapshot {
    /// Unseen What's New entries (filtered: `shipped_at_unix > last_seen_marker`).
    /// Sorted newest-first. Empty when no unseen entries exist.
    pub entries: Vec<WhatsNewEntryRow>,
    /// `true` when `entries` is non-empty and the sheet should be presented.
    /// Swift uses this flag to trigger the sheet presentation.
    pub should_present: bool,
}

// ── Phase 5H additions (append-only) ─────────────────────────────────────────

/// Transcript segment snapshot — raw time-bounded utterance.
///
/// D1: Swift formats timestamps from `start`/`end`. No "X:XX" labels here.
/// DEVICE-LOCAL — fetched per session, never a nostr fact.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct KernelTranscriptSegment {
    pub id: String,
    pub start: f64,
    pub end: f64,
    pub speaker: String,
    pub text: String,
}

/// Availability state for the transcript of the current episode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum KernelTranscriptAvailability {
    /// No `hl.transcript.load` action dispatched yet.
    NotRequested,
    /// `Effect::FetchTranscript` in flight.
    Loading,
    /// Segments parsed and stored.
    Available,
    /// Fetch failed or no transcript URL. UI should hide the transcript panel.
    Unavailable,
}

/// Snapshot for `ViewId::PodcastListening` — the full-screen podcast player.
///
/// All playback facts are **device-local** (resume position NEVER published to
/// nostr — `hl-app-state-vs-nostr-facts`).
///
/// Raw-data doctrine (D1): Swift formats all display strings from these raw
/// fields — "X:XX" timestamps, progress percentage, chapter titles.  The kernel
/// does NOT produce formatted duration strings.
///
/// Bounded: fixed-size record (no lists that grow with the event store —
/// Non-Negotiable #7).  Clip ranges (`clip_start_seconds` / `clip_end_seconds`)
/// are empty in Phase 5H and are populated by Phase 5I/5J.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct PodcastListeningSnapshot {
    /// Podcast item GUID — the stable episode identifier.
    pub guid: String,
    /// HTTP(S) audio URL of the loaded episode.  Opaque from the kernel (D3).
    pub audio_url: String,
    /// Episode title.  Raw string — no truncation or ellipsis (D1).
    pub title: String,
    /// Episode author / host.  Raw string (D1).
    pub author: String,
    /// Episode artwork URL.  Raw string — Swift handles loading/caching (D1).
    pub image_url: String,
    /// Total duration in seconds as reported by `AudioResult::Loaded`.
    /// `0.0` until the native player has fully loaded the item.
    pub duration_seconds: f64,
    /// Current playback position in seconds.  Updated at most once per second
    /// (bounded cadence, D8).  `0.0` at start.
    pub position_seconds: f64,
    /// `true` when the native player is actively playing.  `false` when paused,
    /// buffering, or stopped.  Updated by `AudioResult::Progress`.
    pub is_playing: bool,
    /// Clip start position in seconds for an in-progress clip selection.
    /// `None` when no clip is being assembled.
    pub clip_start_seconds: Option<f64>,
    /// Clip end position in seconds for an in-progress clip selection.
    /// `None` when no clip is being assembled.
    pub clip_end_seconds: Option<f64>,
    /// Transcript segments for the current episode. Raw segments — Swift formats
    /// timestamps and display text (D1). Empty until `hl.transcript.load` completes.
    /// DEVICE-LOCAL — never a nostr fact.
    pub transcript_segments: Vec<KernelTranscriptSegment>,
    /// Transcript availability for the current episode.
    pub transcript_availability: KernelTranscriptAvailability,
    /// Speaker label from the in-progress clip selection (empty when no clip).
    pub clip_speaker: String,
    /// Segment IDs selected for the in-progress clip. Empty when no clip.
    pub clip_selected_segment_ids: Vec<String>,
    // ── Phase 5J additions (append-only) ─────────────────────────────────────
    /// Current clip-publish phase (Idle → Publishing → Done | Error).
    /// Device-local — only the published kind:9802 is a nostr fact.
    pub clip_publish_phase: KernelClipPublishPhase,
}

// ── Phase 5J additions (append-only) ─────────────────────────────────────────

/// FSM phase for a podcast-clip publish round-trip.
///
/// `Idle` before the user triggers publish; `Publishing` once the
/// `Effect::PublishClipWithCorrelation` is in flight; `Done` when the
/// `action_results` projection confirms publish; `Error` on any failure.
///
/// DEVICE-LOCAL — the published kind:9802 is the nostr fact, not this FSM.
///
/// Append-only: new variants at the bottom keep rebases mechanical.
#[derive(Debug, Clone, PartialEq, Eq, Default, uniffi::Enum)]
pub enum KernelClipPublishPhase {
    /// No publish in flight.
    #[default]
    Idle,
    /// `Effect::PublishClipWithCorrelation` is in flight; awaiting
    /// the `action_results` verdict keyed by `correlation_id`.
    Publishing,
    /// The kind:9802 event was accepted by at least one relay. D1: raw.
    Done,
    /// The publish was rejected or the correlation_id never arrived. D1: raw.
    Error {
        /// Raw error message. D1 — Swift formats.
        message: String,
    },
}

// ── Phase 5D additions (append-only) ─────────────────────────────────────────

/// Snapshot for `ViewId::Capture` — OCR capture state.
///
/// Device-local: never derived from or published to Nostr events.
/// Raw-data doctrine (D1): Swift formats all display strings from these raw
/// fields. No formatted markdown preview label, no "Scanning…" copy — those
/// are Swift-side presentation decisions.
///
/// Bounded by the image's text content — `selectable_words` and `raw_lines`
/// are the output of one Vision scan; they do not grow with the event store
/// (Non-Negotiable #7).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct KernelCaptureSnapshot {
    /// Temp-file path of the last captured image, or `None` before first capture.
    pub image_handle: Option<String>,
    /// Reconstructed markdown from the last completed OCR pass. Empty until the
    /// first successful `CapabilityResult::Ocr(OcrResult::Lines)` arrives.
    /// D1: no truncation, no preview label.
    pub markdown: String,
    /// Selectable word projections for drag-selection UI. Empty until OCR completes.
    /// D1: raw `OcrWord` values — Swift owns drag-select geometry rendering.
    pub selectable_words: Vec<crate::capabilities::ocr::OcrWord>,
    /// Raw OCR line observations from the last completed scan. Empty until OCR completes.
    /// D1: raw `OcrLine` values with Vision bounding boxes.
    pub raw_lines: Vec<crate::capabilities::ocr::OcrLine>,
    /// `true` while a `VNRecognizeTextRequest` is in flight. Swift shows a
    /// progress indicator when this is `true`.
    pub pending: bool,

    // ── Phase 5F additions (append-only) ─────────────────────────────────────────
    /// Draft quote text (the highlighted/selected passage). Raw — D1.
    pub draft_quote: String,
    /// Draft source context (the paragraph the quote was lifted from). Raw — D1.
    pub draft_context: String,
    /// Draft user-authored note. Raw — D1.
    pub draft_note: String,
    /// Indices into `selectable_words` for the current drag selection.
    /// `u64` for uniffi compat (kernel stores `usize`).
    pub selected_word_indices: Vec<u64>,
    /// The validated NIP-29 target community id, or `None` for a standalone
    /// capture. D1: raw id only — Swift resolves the display name from the
    /// communities list (no "Optional" fallback here).
    pub target_group_id: Option<String>,
    /// Publish-phase FSM state.
    pub publish_phase: KernelCaptureDraftPhase,
    /// `true` when the draft is publishable (Reviewing + quote, or
    /// Reviewing + markdown + target group). Swift gates the publish button.
    pub can_publish: bool,
    /// Raw publish error message when `publish_phase == Error`, else empty. D1.
    pub publish_error: String,
}

// ── Phase 7 discussions additions (append-only) ──────────────────────────────

/// One kind:11 discussion row from a NIP-29 room — raw protocol data only (D1).
///
/// Filtered from `GroupEventsProjection` rows: kind==11 AND `["t","discussion"]`
/// tag present. `title` comes from the `["title", _]` tag; `body` is the event
/// `content` field. `attachment_url` is extracted from an `["r", url]` tag.
///
/// D1: no `"Untitled discussion"` fallback — Swift owns display fallbacks.
/// D1: no formatted timestamps — `created_at` is raw Unix seconds.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct DiscussionRow {
    /// Raw 64-char hex event id of the kind:11 event.
    pub event_id: String,
    /// Raw 64-char hex author pubkey.
    pub author_pubkey: String,
    /// `["title", _]` tag value, or empty string when absent. D1: no fallback.
    pub title: String,
    /// Event `content` field (discussion body). May be empty.
    pub body: String,
    /// `["r", url]` tag value if present, else `None`.
    pub attachment_url: Option<String>,
    /// Event `created_at` Unix seconds. D1: no "X ago" formatting.
    pub created_at: u64,
}

/// Snapshot for `ViewId::RoomDiscussions{group_id}` — the per-room discussions tab.
///
/// Raw protocol rows only (D1): Swift formats all display strings (title fallback,
/// author name, date label, attachment chip, empty-state copy).
///
/// Bounded at `ROOM_DISCUSSIONS_CAP` (64) rows per room, newest-first.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct RoomDiscussionsSnapshot {
    /// NIP-29 local group id this snapshot is for.
    pub group_id: String,
    /// Filtered kind:11+discussion rows, newest-first. Bounded at 64.
    pub rows: Vec<DiscussionRow>,
}

// ── Phase 7 additions (append-only) ─────────────────────────────────────────

/// A single NIP-22 kind:1111 comment record row — raw protocol data only (D1).
///
/// No formatted timestamps (Swift formats), no tree nesting (Swift builds tree
/// from `parent_tag_value`), no byline strings. `is_top_level` is a pure
/// boolean: `parent_tag_value == root_tag_value` (NIP-22: top-level comment's
/// parent is the root). `comment_count` is derived as `records.len() as u32`.
///
/// Swift reads this flat list and reconstructs the display tree using
/// `parent_tag_value` links without needing a recursive Rust type.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct CommentRecordRow {
    /// kind:1111 event id (raw 64-char hex). D1.
    pub event_id: String,
    /// Comment author pubkey (raw 64-char hex). D1.
    pub author_pubkey: String,
    /// Raw comment body text (`event.content`), untouched. D1.
    pub body: String,
    /// Root scope tag name — uppercase `A`, `E`, or `I`. D1.
    pub root_tag_name: String,
    /// Root scope tag value (address / event-id / external-id). D1.
    pub root_tag_value: String,
    /// Root kind string from the uppercase `K` tag (empty if absent). D1.
    pub root_kind: String,
    /// Parent scope tag name — lowercase `a`, `e`, or `i`. D1.
    pub parent_tag_name: String,
    /// Parent scope tag value. Equals `root_tag_value` for top-level comments. D1.
    pub parent_tag_value: String,
    /// Parent kind string from the lowercase `k` tag (empty if absent). D1.
    pub parent_kind: String,
    /// Event `created_at` unix seconds. D1: no formatting.
    pub created_at: u64,
    /// `true` when `parent_tag_value == root_tag_value` (top-level comment,
    /// per NIP-22 §3). Swift may use this to efficiently partition
    /// root comments from replies without re-computing the comparison.
    pub is_top_level: bool,
    /// Per-comment like count from `AppState::reaction_state` (kind:7 `+`
    /// reactions to this comment's `event_id`), as projected by the global
    /// `ReactionProjection`. Raw u32 — Swift formats any label (D1). `0` when
    /// no reactions are known for this event.
    pub like_count: u32,
    /// `true` when the active viewer has liked this comment (`viewer_reacted`
    /// from `AppState::reaction_state`). Optimistic toggling lives in Swift (D1).
    pub viewer_reacted: bool,
    /// `true` when this comment's `event_id` is in the active account's
    /// kind:10003 bookmark list (`AppState::bookmarks`). D1.
    pub bookmarked: bool,
}

/// Snapshot for `ViewId::CommentThread` — flat raw comment list for one root.
///
/// D1: no formatted strings, no tree nesting in the snapshot. Swift builds the
/// display tree from `parent_tag_value` links. `comment_count` = `records.len()`.
/// Bounded by `MAX_PROJECTION_MESSAGES` from nmp-core (Non-Negotiable #7).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct CommentThreadKernelSnapshot {
    /// Root scope tag value this snapshot is for.
    pub root_tag_value: String,
    /// Flat list of comment records for this root, newest-first. D1.
    pub records: Vec<CommentRecordRow>,
    /// Total comment count (`records.len() as u32`). Raw — no `"N comments"` label.
    pub comment_count: u32,
}

// ── Phase 7 feedback additions (append-only) ─────────────────────────────────

/// One feedback thread root visible in the feedback thread list.
///
/// Derived from top-level NIP-22 kind:1111 records in
/// `AppState::comment_threads[HIGHLIGHTER_PROJECT_COORDINATE]` whose
/// `author_pubkey` matches the active viewer.
///
/// D1: no formatted strings. `title`, `summary`, `status_label` are `None` until
/// an explicit HL metadata source is wired. `preview` is whitespace-collapsed
/// raw body text (≤140 chars). `last_activity_at` = max `created_at` over root
/// + direct replies.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FeedbackThreadRow {
    /// Event id of the top-level root kind:1111 comment (raw 64-char hex). D1.
    pub root_event_id: String,
    /// Author pubkey of the root comment (raw 64-char hex). D1.
    pub author_pubkey: String,
    /// `created_at` of the root comment (unix seconds). D1.
    pub created_at: u64,
    /// Max `created_at` of root + all direct replies (unix seconds). D1.
    pub last_activity_at: u64,
    /// Optional thread title — `None` without an HL metadata source. D1.
    pub title: Option<String>,
    /// Optional thread summary — `None` without an HL metadata source. D1.
    pub summary: Option<String>,
    /// Optional status label — `None` without an HL metadata source. D1.
    pub status_label: Option<String>,
    /// Whitespace-collapsed preview of the root body, capped at 140 chars. D1.
    pub preview: String,
    /// Count of direct replies under this root. Raw u32 — no label. D1.
    pub reply_count: u32,
}

/// Snapshot for `ViewId::FeedbackThreads` — the feedback thread list.
///
/// Bounded at 256 entries (Non-Negotiable #7). Sorted newest activity first.
/// D1: no formatted strings; `is_publishing` and `error` carry publish-FSM state
/// for the composer only (native owns all display formatting).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct KernelFeedbackThreadsSnapshot {
    /// The NIP-22 root scope value (`HIGHLIGHTER_PROJECT_COORDINATE`). D1.
    pub root_tag_value: String,
    /// Thread rows for the active viewer, sorted by `last_activity_at` desc. D1.
    pub threads: Vec<FeedbackThreadRow>,
    /// `true` while a `hl.feedback.post_root` action is in flight.
    pub is_publishing: bool,
    /// Last publish error, if any. `None` when clean. D1: raw error string.
    pub error: Option<String>,
}

/// One message row in a feedback thread detail view.
///
/// Includes the root comment and all descendant replies. Sorted oldest-first.
///
/// D1: no formatted timestamps, no byline strings, no `is_from_me` flag (native
/// computes those from the active session and profile snapshots).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FeedbackMessageRow {
    /// kind:1111 event id (raw 64-char hex). D1.
    pub event_id: String,
    /// Event id of the thread root comment this row belongs to. D1.
    pub root_event_id: String,
    /// Author pubkey (raw 64-char hex). D1.
    pub author_pubkey: String,
    /// `created_at` unix seconds. D1: no formatting.
    pub created_at: u64,
    /// Raw comment body text. D1.
    pub content: String,
    /// Event id of the parent comment, if this is a nested reply. `None` for
    /// direct replies to the root (parent_tag_value == root_event_id). D1.
    pub parent_event_id: Option<String>,
    /// `true` on first row, author change, or gap > 300 seconds.
    /// Swift uses this to show the author/timestamp header cell. D1: boolean only.
    pub show_header: bool,
}

/// Snapshot for `ViewId::FeedbackThread { root_event_id }` — thread detail.
///
/// Contains the root record + all descendant replies (ancestor-chain traversal),
/// sorted oldest-first. Bounded by the NMP `CommentThreadProjection` cap
/// (Non-Negotiable #7).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct KernelFeedbackThreadSnapshot {
    /// The NIP-22 root scope value (`HIGHLIGHTER_PROJECT_COORDINATE`). D1.
    pub root_tag_value: String,
    /// Event id of the thread root comment. D1.
    pub root_event_id: String,
    /// Thread message rows, sorted oldest-first. D1.
    pub rows: Vec<FeedbackMessageRow>,
    /// `true` while a `hl.feedback.post_reply` action is in flight.
    pub is_publishing: bool,
    /// Last publish error, if any. `None` when clean. D1.
    pub error: Option<String>,
}
// ── Phase 7 chat additions (append-only) ─────────────────────────────────────

/// One raw message row in the authoritative chat buffer and in `ChatMessageRow`.
///
/// D1: raw protocol fields only — no formatted timestamps, no byline strings,
/// no `is_from_me` flag. Swift computes all display labels from raw fields.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ChatMessageRawRow {
    /// Event id (raw 64-char hex). Dedupe key.
    pub event_id: String,
    /// Author pubkey (raw 64-char hex).
    pub author_pubkey: String,
    /// Event `content`, verbatim.
    pub content: String,
    /// Event `created_at` (Unix seconds).
    pub created_at: u64,
    /// The event id this message is replying to, if any.
    /// Recovered from `["e", id, "", "reply"]` (preferred) or first `e` tag.
    pub reply_to_event_id: Option<String>,
}

/// One display row in `RoomChatSnapshot` — raw fields + computed `show_header`
/// and resolved `reply_to` preview (only when the parent is in the visible window).
///
/// D1: no formatted strings, no `is_from_me`. Swift formats timestamps, bylines,
/// and profile pictures.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ChatMessageRow {
    /// Event id (raw 64-char hex).
    pub event_id: String,
    /// Author pubkey (raw 64-char hex).
    pub author_pubkey: String,
    /// Event `content`, verbatim.
    pub content: String,
    /// Event `created_at` (Unix seconds).
    pub created_at: u64,
    /// The event id this message is replying to, if any (raw hex).
    pub reply_to_event_id: Option<String>,
    /// Resolved reply preview — `Some` only when the parent event is inside the
    /// bounded visible window. `None` when not a reply or parent is older than
    /// the window.
    pub reply_to: Option<ChatReplyPreview>,
    /// `true` for the first row, on author change, or when `created_at` gap with
    /// the prior row exceeds 300 seconds.
    pub show_header: bool,
}

/// Compact preview of a replied-to message. Only present when the parent event
/// is within the bounded visible window (D5: bounded by open chat window).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct ChatReplyPreview {
    /// Event id of the replied-to message.
    pub event_id: String,
    /// Author pubkey of the replied-to message.
    pub author_pubkey: String,
    /// Content of the replied-to message (verbatim, D1).
    pub content: String,
    /// `created_at` (Unix seconds) of the replied-to message.
    pub created_at: u64,
}

/// Snapshot for `ViewId::RoomChat { group_id }` — bounded raw chat rows for one
/// NIP-29 room.
///
/// Rows are projected oldest-first for the visible window so the chat scrolls
/// downward naturally. Window size is `page_count * 50`, hard-capped at 1000.
///
/// D1: no formatted strings. D5: bounded window. D6: empty when room not open.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct RoomChatSnapshot {
    /// NIP-29 local group id.
    pub group_id: String,
    /// Oldest-first display rows for the visible window.
    pub rows: Vec<ChatMessageRow>,
    /// `true` when more messages exist beyond the visible window.
    pub has_more: bool,
    /// Current page count (1–20). Incremented by `hl.chat.load_more`.
    pub page_count: u32,
    /// `true` when at least one message has been received for this room.
    pub has_activity: bool,
    /// Monotonic revision bumped on every `ChatRoomUpdated` for change detection.
    pub activity_revision: u64,
}

// ── Phase 5F additions (append-only) ─────────────────────────────────────────

/// Publish-phase FSM exposed in `KernelCaptureSnapshot`.
///
/// Mirrors the kernel-internal `CaptureDraftPhase` (in
/// `kernel/domains/capture_draft.rs`); the internal `Error { message }` variant's
/// payload is surfaced via the snapshot's `publish_error` field (uniffi enums
/// stay payload-free here for FFI simplicity).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum KernelCaptureDraftPhase {
    Idle,
    Reviewing,
    Publishing,
    Done,
    Error,
}
