//! `AppState` — the single app model owned by the kernel actor.
//!
//! Rust is the single writer for all app facts (Non-Negotiable #2).
//! The state is split into sub-models by concern; all live as fields on
//! `AppState` so the reducer can read and write the full picture atomically.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::kernel::action::{SignInMethod, SignerKind};
use crate::kernel::domains::relay_diagnostics::RelayDiagnosticsState;

/// Session state machine.
///
/// Append-only: new variants at the bottom keep rebases mechanical.
/// Signer policy lives here in Rust; native never mutates session facts.
#[derive(Debug, Clone, PartialEq)]
pub enum SessionState {
    /// Not yet attempted to restore a session.
    Unknown,
    /// Restore in progress: waiting for a `CapabilityResult::Keychain`.
    Restoring {
        /// UNIX second when restoration was started (for timeout).
        started_at: u64,
    },
    /// A session secret is in memory; `pubkey` is the decoded hex public key
    /// and `signer_kind` records which backend is active.
    Present {
        pubkey: String,
        signer_kind: SignerKind,
    },
    /// No session secret found (user never logged in or has logged out).
    Absent,
    /// Restore attempt failed — diagnostic carried as state (D6).
    RestoreFailed { error: String },

    // ── Phase 2A additions (append-only) ─────────────────────────────────────
    /// `add_signer` / `add_bunker` call dispatched; waiting for the
    /// identity-change observer to fire. The clock-driven timeout (30 s) will
    /// transition to `SignInFailed` if the observer never fires.
    SigningIn {
        method: SignInMethod,
        /// UNIX second when sign-in was started (for timeout, future phases).
        started_at: u64,
    },
    /// Sign-in attempt failed — method and error carried as state (D6).
    /// Never returned across the dispatch boundary as a `Result`.
    SignInFailed { method: SignInMethod, error: String },
}

/// Durable onboarding completion flag.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct OnboardingState {
    /// Whether the user has completed onboarding. Read from `OnboardingStore`
    /// on startup via `Effect::LoadOnboardingFlag`.
    pub complete: bool,
    /// Whether the flag has been loaded yet (distinguishes "false by default"
    /// from "loaded and genuinely false").
    pub loaded: bool,
}

/// Navigation route and sheet stack.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RouteState {
    /// Index of the selected root tab (matches `RootTab` raw values).
    pub root_tab: u8,
    /// ID of the sheet currently presented over the root shell, if any.
    pub sheet_id: Option<String>,
}

/// In-memory toast state.
#[derive(Debug, Clone, PartialEq)]
pub struct ToastState {
    pub message: String,
    /// UNIX second at which the kernel should auto-dismiss (clock-driven, D8).
    pub dismiss_at_unix: u64,
}

/// App chrome (toast, feedback sheet).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ChromeState {
    pub toast: Option<ToastState>,
    pub feedback_presented: bool,
}

/// The single app model. Every field is a fact owned exclusively by the
/// Rust actor; no native store writes here (Non-Negotiable #2).
#[derive(Debug, Clone)]
pub struct AppState {
    pub session: SessionState,
    pub onboarding: OnboardingState,
    pub route: RouteState,
    pub chrome: ChromeState,
    /// Monotonic counter bumped on every logout. Effects keyed to the current
    /// epoch are silently dropped when the epoch has advanced (idempotent
    /// cancellation — plan line 157).
    pub session_epoch: u64,

    // ── Phase 2B additions ────────────────────────────────────────────────────
    /// The most recently minted `nostrconnect://` URI, if any. Cleared when
    /// sign-in completes (IdentityChanged fires) or on Logout. Bounded: one
    /// String ≤ 512 bytes per NIP-46 spec. Exposed via `AppRootSnapshot` so
    /// the iOS QR-code sheet can render it without polling.
    pub nostrconnect_uri: Option<String>,

    // ── Phase 2E additions ────────────────────────────────────────────────────
    /// Relay-diagnostics sidecar state. Populated from the `"relay_diagnostics"`
    /// typed-projection frame on every NMP snapshot tick. `None` until the first
    /// valid frame arrives. Bounded: one `RelayDiagRow` per configured relay URL.
    pub relay_diagnostics: RelayDiagnosticsState,

    // ── Phase 3B additions ────────────────────────────────────────────────────
    /// Current joined groups for the active account, decoded from the
    /// `"nmp.nip29.joined_groups"` typed sidecar. Cleared on `IdentityChanged(None)`
    /// and replaced on `IdentityChanged(Some)` when the new account's
    /// projection arrives. Bounded by the number of groups joined.
    pub communities: Vec<crate::kernel::snapshot::CommunityRow>,

    // ── Phase 3C additions ────────────────────────────────────────────────────
    /// Raw hex pubkeys in the active account's NIP-02 follow set (kind:3).
    ///
    /// Updated by `KernelEvent::FollowListUpdated` — a decoded
    /// `"nmp.nip02.follow_list"` typed sidecar from the NMP update callback.
    /// Empty until the first follow-list frame arrives. No formatting — the
    /// presentation layer (Swift) handles bech32/abbreviation/avatar.
    ///
    /// Phase 3D `Profile` snapshots and 3E recommendation heuristics read
    /// this field via `AppState::is_following(pubkey)` — the single query
    /// point so no caller duplicates the Vec scan.
    pub follows: Vec<String>,

    // ── Phase 3E additions ────────────────────────────────────────────────────
    /// Discovered groups from the active discovery relay, decoded from the
    /// `"nmp.nip29.discovered_groups"` typed sidecar. Empty until
    /// `AppAction::StartRoomDiscovery` is dispatched and the projection frame
    /// arrives. Bounded by the discovery relay's group catalog (cap at 256
    /// per §2.2 of the 3E spec).
    pub discovered_groups: Vec<crate::kernel::snapshot::DiscoveredRow>,

    /// Room policy injected at construction time (D3: no wss-scheme literals
    /// in kernel logic). Sourced from `AppConfig`-adjacent bootstrap code.
    pub room_policy: crate::kernel::app::RoomPolicy,

    // ── Phase 3D additions ────────────────────────────────────────────────────
    /// The active account's own profile card, decoded from the `"profile"`
    /// built-in typed sidecar. `None` until the first `"profile"` frame arrives
    /// from the NMP update callback. No `ClaimProfile` call is needed for the
    /// own account — the kernel projects it automatically.
    ///
    /// Used as the fallback source for a `ViewId::Profile{pubkey}` where
    /// `pubkey` equals the active account (before a `claimed_profiles` entry
    /// arrives, or if the view is the own-profile screen).
    pub own_profile: Option<nmp_core::typed_projections::ProfileCardModel>,

    /// Profiles for visited pubkeys, decoded from the `"claimed_profiles"`
    /// typed sidecar. Keyed by raw hex pubkey (64 lowercase chars). Populated
    /// by `nmp_app_claim_profile` / released by `nmp_app_release_profile`
    /// (driven by `Effect::ClaimProfile` / `Effect::ReleaseProfile`).
    ///
    /// Bounded by the number of concurrently open `Profile` views — never
    /// grows with the unbounded event store (Non-Negotiable #7). Cleared on
    /// `IdentityChanged(None)` and `Logout`.
    pub claimed_profiles: HashMap<String, nmp_core::typed_projections::ProfileCardModel>,

    // ── Phase 3F additions ────────────────────────────────────────────────────
    /// Raw group events for open RoomHome views, decoded from the
    /// `"nmp.nip29.group_events"` typed sidecar. Keyed by `group_id` (local id).
    ///
    /// Populated when a `ViewId::RoomHome{group_id}` is opened (via
    /// `Effect::WireGroupEvents`) and cleared when the view is closed (via
    /// `Effect::ReleaseGroupEvents`). Bounded per-group at 256 rows
    /// (`ROOM_HOME_EVENTS_CAP` in `room_home.rs`). Lane bodies are empty in
    /// Phase 3F — the events are buffered so Phase 4 can decode feed content
    /// without re-opening a subscription (Non-Negotiable #7).
    pub room_home_events: HashMap<String, Vec<nmp_nip29::GroupEventRow>>,

    // ── Phase 4C additions ────────────────────────────────────────────────────
    /// Active account's NIP-51 kind:10003 bookmark list, decoded from the
    /// `"hl.bookmarks"` typed sidecar (hl-owned JSON projection).
    ///
    /// Updated by `KernelEvent::BookmarksUpdated` — produced when the hl-owned
    /// `BookmarkListProjection` emits a snapshot frame. Empty until the first
    /// kind:10003 event arrives for the active account. No presentation strings
    /// — raw `BookmarkRow` values only (D1). Cleared on `IdentityChanged(None)`
    /// and `Logout` to prevent stale bookmarks from the previous account leaking
    /// into the next session.
    ///
    /// Bounded by the kind:10003 list length (the latest replaceable event;
    /// never grows with the unbounded event store — Non-Negotiable #7).
    pub bookmarks: Vec<crate::kernel::snapshot::BookmarkRow>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            session: SessionState::Unknown,
            onboarding: OnboardingState::default(),
            route: RouteState::default(),
            chrome: ChromeState::default(),
            session_epoch: 0,
            nostrconnect_uri: None,
            relay_diagnostics: RelayDiagnosticsState::default(),
            communities: Vec::new(),
            follows: Vec::new(),
            discovered_groups: Vec::new(),
            room_policy: RoomPolicy::default(),
            own_profile: None,
            claimed_profiles: HashMap::new(),
            // ── Phase 3F additions ────────────────────────────────────────────
            room_home_events: HashMap::new(),
            // ── Phase 4C additions ────────────────────────────────────────────
            bookmarks: Vec::new(),
        }
    }
}

// ── Phase 2C additions ────────────────────────────────────────────────────────

/// A single relay entry in the `CreateAccount` seed relay policy.
///
/// Values come from `AppConfig::relay_policy` (injected at construction time),
/// never from hardcoded literals inside kernel logic (D3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedRelay {
    /// WebSocket URL (e.g. a `wss`-scheme relay URL). Stored opaquely; the
    /// kernel never constructs relay URLs itself (D3).
    pub url: String,
    /// NIP-65 / kind:10002 role string expected by `ActorCommand::CreateAccount`.
    /// One of `"read"`, `"write"`, `"both"`, `"indexer"`, or a composite.
    pub role: String,
}

/// Account-creation policy injected into `AppConfig` (D3: no hardcoded
/// relay or follow literals in kernel logic).
///
/// `seed_relays` is passed verbatim to `ActorCommand::CreateAccount.relays`.
/// `initial_follows` governs kind:3 publication per ADR-0059: an empty vec
/// means no kind:3 is published (the account starts with no contacts).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CreateAccountPolicy {
    /// Relays to register and publish to a fresh account's kind:10002.
    /// Sourced from `relay_policy.json` seed_defaults at app construction.
    pub seed_relays: Vec<SeedRelay>,
    /// Hex pubkeys the new account auto-follows (kind:3 prepopulate).
    /// ADR-0059 §5: empty → no kind:3 published.
    pub initial_follows: Vec<String>,
}

/// Configuration passed to `HighlighterApp::new`.
#[derive(Debug, Clone, uniffi::Record)]
pub struct AppConfig {
    /// Application-support / documents directory for this app instance.
    /// The kernel will create `<data_dir>/nmp-lane/` for its own storage.
    pub data_dir: String,
}

// ── Phase 2D additions ────────────────────────────────────────────────────────

/// Relay-management policy injected into the kernel (D3: no wss-scheme literals
/// in kernel logic — all relay seed URLs live here, not in relays.rs or
/// any other kernel module).
///
/// Used by `AppAction::AddRelay` / `RemoveRelay` / `SetRelayRole` to validate
/// that relay URLs are non-empty before forwarding to nmp. The rooms d-tag is
/// a product constant owned by Highlighter; it is NOT a relay URL and is kept
/// in kernel code rather than here (a relay URL injected from outside would
/// be more complicated than embedding a fixed product-identifier string).
#[derive(Debug, Clone, Default)]
pub struct RelayPolicy {
    /// Default relay set surfaced to the UI for onboarding / settings
    /// (informational only; kernel does not auto-add these — host triggers
    /// explicit `AppAction::AddRelay` calls for each entry at the right time).
    ///
    /// Values come from `relay_policy.json` seed_defaults. Opaque strings;
    /// the kernel never constructs or validates relay URLs beyond checking
    /// for non-empty (nmp validates the actual wss-scheme scheme / parse).
    pub seed_relay_urls: Vec<String>,
}

/// Runtime policy injected into the kernel that cannot cross uniffi (not a
/// `uniffi::Record`) because it holds Rust-only types. Constructed by the
/// `HighlighterApp` bootstrap from `relay_policy.json` seed defaults.
///
/// Kept separate from `AppConfig` (which is a uniffi Record) so native callers
/// don't need to supply or understand relay/follow policy — it's Rust-owned.
#[derive(Debug, Clone, Default)]
pub struct KernelPolicy {
    /// Policy for `AppAction::CreateAccount`.
    pub create_account: CreateAccountPolicy,
    /// Policy for relay-management actions (Phase 2D).
    pub relay: RelayPolicy,
    /// Room discovery and curator policy (Phase 3G).
    /// Copied into `AppState::room_policy` at actor boot so the kernel can
    /// auto-start discovery when `ViewId::RoomExplorer` is opened.
    pub room: RoomPolicy,
}

/// UNIX seconds after dispatch of `RestoreSession` before the kernel
/// transitions to `SessionState::Absent` (no response from native keychain).
pub const SESSION_RESTORE_TIMEOUT_SECS: u64 = 30;

/// Duration in seconds after presentation before the kernel auto-dismisses
/// a chrome toast (clock-driven, no Swift Timer).
pub const TOAST_DISMISS_SECS: u64 = 3;

/// UNIX seconds after dispatch of a sign-in action before the kernel
/// transitions to `SessionState::SignInFailed`.
///
/// NMP handles parse errors internally (`set_last_error_toast`) without firing
/// the identity-change observer. This clock-driven timeout ensures an invalid
/// nsec — or any other case where the observer never fires — surfaces in
/// `SessionState` rather than leaving the UI stuck in `SigningIn` forever (D6).
pub const SIGN_IN_TIMEOUT_SECS: u64 = 30;

impl AppState {
    /// Storage sub-directory the new lane's `NmpApp` will use.
    pub fn nmp_storage_path(data_dir: &str) -> PathBuf {
        PathBuf::from(data_dir).join("nmp-lane")
    }

    // ── Phase 3C additions ────────────────────────────────────────────────────

    /// Return `true` if `pubkey` (raw 64-char hex) is in the active account's
    /// follow set as last projected from the `"nmp.nip02.follow_list"` sidecar.
    ///
    /// Phase 3D `ProfileSnapshot::is_following` and Phase 3E friends-shelf
    /// recommendation heuristics call this rather than scanning `follows`
    /// directly — single query point, logic in one place.
    pub fn is_following(&self, pubkey: &str) -> bool {
        self.follows.iter().any(|pk| pk == pubkey)
    }
}

// ── Phase 3E additions ────────────────────────────────────────────────────────

/// Room discovery and curator policy (D3: no hardcoded relays in kernel).
///
/// Injected at kernel construction time from `AppConfig`-adjacent bootstrap
/// code. The kernel reads `room_policy.discovery_relay` when wiring the
/// `DiscoveredGroupsProjection`; it never constructs relay URLs itself (D3).
#[derive(Debug, Clone, Default)]
pub struct RoomPolicy {
    /// Relay to discover groups on. Empty = no active discovery.
    /// Set via `AppConfig`/`KernelPolicy`-adjacent bootstrap, not hardcoded.
    pub discovery_relay: String,
    /// Optional curator pubkey for featured rooms (NIP-11 curator pattern).
    /// Empty until Phase 3F wires curator filtering.
    pub curator_pubkey: Option<String>,
    /// Base URL for invite links (e.g. `"https://highlighter.com/r"`).
    /// The kernel supplies raw group_id+code; Swift composes the full URL.
    pub invite_link_base: String,
}
