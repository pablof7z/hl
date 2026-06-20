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
#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
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

/// Snapshot for `ViewId::Search` — NIP-50 relay search results view.
///
/// Raw protocol data only (D1): Swift formats all display strings.
/// Bounded by `SearchResultsProjection`'s `max_hits` cap
/// (default `DEFAULT_MAX_SEARCH_HITS = 200` from nmp-nip50 — Non-Negotiable #7).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct SearchSnapshot {
    /// Ordered (by `created_at` descending, then `id` ascending) search hit rows.
    /// Raw fields only — no "X results" count label, no formatted strings (D1).
    pub hits: Vec<KernelSearchHitRow>,
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

/// uniffi-compatible search hit row for FFI (uniffi::Record requires simple types).
///
/// Mirrors `SearchHitRow` with `tags` flattened to `Vec<String>` (uniffi
/// does not support `Vec<Vec<String>>`). The Rust-internal `SearchHitRow`
/// uses the native 2D Vec; this struct is for the snapshot FFI boundary only.
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
    /// Event creation time as Unix seconds. D1: Swift formats the display date.
    pub created_at: u64,
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
}
