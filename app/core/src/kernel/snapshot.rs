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
// Boxing is not an option here because uniffi::Enum requires variants to be
// uniffi-compatible (Box<T> is not). The size difference is accepted because
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
