//! `AppState` — the single app model owned by the kernel actor.
//!
//! Rust is the single writer for all app facts (Non-Negotiable #2).
//! The state is split into sub-models by concern; all live as fields on
//! `AppState` so the reducer can read and write the full picture atomically.

use std::collections::{BTreeMap, HashMap};
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

    // ── Phase 4A additions ────────────────────────────────────────────────────
    /// Raw NIP-23 article records, decoded from the `"nmp.nip23.articles"`
    /// typed sidecar. Keyed by addressable coordinate `kind:author_hex:d_tag`.
    ///
    /// Populated on every `KernelEvent::ArticlesUpdated` (i.e. every NMP
    /// snapshot tick that carries the longform projection). The map is the
    /// complete live snapshot — the sidecar carries all articles the kernel
    /// has seen this session (parameterized-replaceable supersession already
    /// resolved by nmp-content's `LongformProjection`).
    ///
    /// Cleared on `Logout` / `IdentityChanged(None)` so stale data from the
    /// previous session never leaks to the next account. Bounded by the number
    /// of kind:30023 events the kernel has seen — never grows beyond what the
    /// session subscriptions pull in (Non-Negotiable #7).
    ///
    /// D1: `ArticleRow` is raw protocol data only — no formatted strings.
    pub articles: BTreeMap<String, crate::kernel::snapshot::ArticleRow>,

    // ── Phase 4B additions ────────────────────────────────────────────────────
    /// Reaction state for target events, decoded from the `"hl.reactions"`
    /// wrapped typed-snapshot projection. Keyed by target event id (raw
    /// 64-char hex). Values carry raw count + viewer-reacted bool (D1: no
    /// labels or formatting; optimistic UI state lives in Swift).
    ///
    /// Cleared on `Logout` and `IdentityChanged(None)` so stale reaction
    /// counts from a prior account never surface under a new identity.
    /// Bounded by the number of events whose reaction projection has fired —
    /// in practice bounded by opened views (Non-Negotiable #7).
    pub reaction_state: HashMap<String, crate::kernel::snapshot::ReactionRow>,

    // ── Phase 4D additions ────────────────────────────────────────────────────
    /// NIP-50 relay search results for the current active search query.
    ///
    /// Updated by `KernelEvent::SearchResultsUpdated` — produced when the
    /// hl-owned `SearchResultsProjection` emits a snapshot tick. Empty until
    /// `AppAction::RunSearch` is dispatched and hits arrive from relays.
    ///
    /// Cleared when:
    ///   - `AppAction::RunSearch` with a new query replaces the projection
    ///     (the new projection starts empty; the first tick sets `[]`).
    ///   - `ViewId::Search` is closed (lifecycle effect clears the vec to bound
    ///     memory between search sessions).
    ///   - `Logout` / `IdentityChanged(None)`.
    ///
    /// Bounded by `SearchResultsProjection::max_hits` (default 200,
    /// hard cap 500 from nmp-nip50 — Non-Negotiable #7).
    /// D1: `SearchHitRow` is raw protocol data only — no formatted strings.
    pub search_results: Vec<crate::kernel::snapshot::SearchHitRow>,

    // ── Phase 4F additions ────────────────────────────────────────────────────
    /// Pull-cursor state for the article feed (kind:30023 over follows).
    ///
    /// Registered by Phase 4G when the article-feed view opens (via
    /// `lifecycle_effects_for_view_open` emitting `Effect::RegisterFeedCursor`
    /// with key `"hl.feed.articles"`). Cleared on `Logout` / `IdentityChanged(None)`.
    ///
    /// `cursor_id` is minted by `feed::mint_cursor_id("hl.feed.articles")`.
    /// `rows` holds raw `KernelEvent`s projected to `ArticleRow`s by 4G.
    pub article_feed: crate::kernel::domains::feed::FeedState,

    /// Pull-cursor state for the highlights feed (kind:9802, any author).
    ///
    /// Registered by Phase 4H when the highlights feed view opens.
    /// Cleared on `Logout` / `IdentityChanged(None)`.
    pub highlight_feed: crate::kernel::domains::feed::FeedState,

    /// Pull-cursor state per room-lane feed, keyed by `group_id`.
    ///
    /// Each entry is registered by Phase 4I when a `ViewId::RoomHome{group_id}`
    /// opens. Key is `"hl.feed.room.<group_id>"`. Bounded by open room views.
    /// Cleared on `Logout` / `IdentityChanged(None)`.
    pub room_lanes: HashMap<String, crate::kernel::domains::feed::FeedState>,

    // ── Phase 5A additions ────────────────────────────────────────────────────
    /// What's New seen-state — device-local, never published to Nostr.
    ///
    /// Populated by `KernelEvent::WhatsNewLoaded` after `Effect::LoadWhatsNewState`
    /// resolves the bundled JSON + seen-marker file. `should_present` drives the
    /// sheet presentation; `entries` is filtered to unseen items only.
    ///
    /// The seen marker is monotonic: `MarkWhatsNewSeen` never moves it backward.
    /// NOT cleared on `Logout` — the marker is per-device, not per-account.
    /// Bounded: entries come from the bundled JSON (typically < 20 items;
    /// Non-Negotiable #7 — never grows with the event store).
    pub whats_new: crate::kernel::domains::whats_new::WhatsNewState,

    // ── Phase 5C additions ────────────────────────────────────────────────────
    /// ISBN preview cache and lookup state. Device-local — never published.
    /// Cache file: `{data_dir}/isbn-preview-cache-v1.json`.
    pub isbn: crate::kernel::domains::isbn::IsbnState,

    // ── Phase 5K additions ────────────────────────────────────────────────────
    /// Transient share-queue state drained from the iOS App Group.
    ///
    /// DEVICE-LOCAL — never a nostr fact. Cleared on `Logout` /
    /// `IdentityChanged(None)` so a stale queue from a prior account cannot
    /// leak into the next session. The App Group file is the durable handoff
    /// store; this field is the in-kernel working set for the current session.
    pub share_queue: crate::kernel::domains::share::ShareQueueState,

    // ── Phase 5H additions ────────────────────────────────────────────────────
    /// Podcast playback state — transient, DEVICE-LOCAL.
    ///
    /// Holds the currently loaded episode (guid, artifact, position, duration,
    /// is_playing) and waveform cache key. Updated by `AudioResult::Progress` /
    /// `Loaded` / `Ended` events from the native capability bridge.
    ///
    /// NOT cleared on `Logout` / `IdentityChanged(None)` — podcast playback
    /// is per-device, not per-account. The resume position file is also
    /// device-local (never published to nostr).
    ///
    /// Bounded: fixed-size `Option<LoadedEpisode>` — never grows with the
    /// event store (Non-Negotiable #7).
    pub podcast: crate::kernel::domains::podcast::PodcastState,

    /// In-memory saved resume position cache keyed by guid.
    ///
    /// Populated by `KernelEvent::PodcastPositionLoaded` (which is emitted by
    /// `run_effect_load_podcast_position` when `AudioPlay` is dispatched).
    /// This cache lets `reduce_action_play` include `resume_at_seconds` in the
    /// `AudioOp::Load` without blocking the reducer on an I/O read.
    ///
    /// Bounded: one entry per guid encountered this session (cleared on logout).
    pub podcast_resume_cache: std::collections::HashMap<String, f64>,

    // ── Phase 5D additions (append-only) ─────────────────────────────────────
    /// OCR capture state — device-local, never published to Nostr.
    ///
    /// Holds the last-captured image handle, reconstructed markdown,
    /// selectable words, and raw Vision lines from the most recent OCR pass.
    /// NOT cleared on Logout — the last capture is per-device, not per-account.
    /// Bounded by the image's text content (Non-Negotiable #7).
    pub ocr: crate::kernel::domains::ocr::OcrState,
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
            // ── Phase 4A additions ────────────────────────────────────────────
            articles: BTreeMap::new(),
            // ── Phase 4B additions ────────────────────────────────────────────
            reaction_state: HashMap::new(),
            // ── Phase 4D additions ────────────────────────────────────────────
            search_results: Vec::new(),
            // ── Phase 4F additions ────────────────────────────────────────────
            article_feed: crate::kernel::domains::feed::FeedState::default(),
            highlight_feed: crate::kernel::domains::feed::FeedState::default(),
            room_lanes: HashMap::new(),
            // ── Phase 5A additions ────────────────────────────────────────────
            whats_new: crate::kernel::domains::whats_new::WhatsNewState::default(),
            // ── Phase 5C additions ────────────────────────────────────────────
            isbn: crate::kernel::domains::isbn::IsbnState::default(),
            // ── Phase 5K additions ────────────────────────────────────────────
            share_queue: crate::kernel::domains::share::ShareQueueState::default(),
            // ── Phase 5H additions ────────────────────────────────────────────
            podcast: crate::kernel::domains::podcast::PodcastState::default(),
            podcast_resume_cache: std::collections::HashMap::new(),
            // ── Phase 5D additions ────────────────────────────────────────────
            ocr: crate::kernel::domains::ocr::OcrState::default(),
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

    // ── Phase 5A additions ────────────────────────────────────────────────────
    /// Application data directory — same value as `AppConfig::data_dir`.
    /// Used by `Effect::LoadWhatsNewState` and `Effect::PersistWhatsNewSeen`
    /// to locate `{data_dir}/whats-new-state-v1.json`. Empty string = no-op
    /// (test mode; tests inject `KernelEvent::WhatsNewLoaded` directly).
    pub data_dir: String,
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
