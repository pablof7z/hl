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
