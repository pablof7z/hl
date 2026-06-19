//! Kernel input types: `AppAction` (from native UI) and `KernelEvent`
//! (from async Rust work or native capability results).
//!
//! Both live here so all inputs to the reducer sit in one place.
//! `KernelEvent` is never exposed across FFI — native dispatches only
//! `AppAction`. The kernel feeds events back to itself internally.

use crate::capabilities::CapabilityResult;

/// The active root tab. Tab index matches the Swift `MainTabView` order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum RootTab {
    Feed = 0,
    Discover = 1,
    Capture = 2,
    Notifications = 3,
    Settings = 4,
}

/// How a sign-in was initiated — carried in failure and in-progress state.
/// Append-only: adding a variant is non-breaking to existing match arms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignInMethod {
    Nsec,
    Bunker,
    NostrConnect,
    Nip55,
    CreateAccount,
}

/// Which signing backend is active for the current session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignerKind {
    /// A raw nsec stored in (and recalled from) the nmp keyring.
    LocalNsec,
    /// A NIP-46 remote bunker.
    Nip46,
    /// An external NIP-55 signer app.
    Nip55,
}

/// Every user or platform action the kernel understands.
///
/// Dispatch is fire-and-forget (`dispatch(action)` returns `()`; Non-Negotiable #3).
/// Errors never propagate back as `Result` — they surface as typed `ViewSnapshot` state.
///
/// Append-only: new variants at the bottom keep rebases mechanical.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum AppAction {
    /// Attempt to restore a prior session from the native keychain.
    RestoreSession,
    /// Retry a failed restore (same effect as `RestoreSession`; separate
    /// variant for UI affordance clarity).
    RetryRestore,
    /// Clear the active session, emit a `ClearSession` capability request,
    /// bump the session epoch to cancel in-flight view-scoped effects.
    Logout,
    /// Mark onboarding as complete in the durable `OnboardingStore`.
    CompleteOnboarding,
    /// Switch the active root tab.
    SelectRootTab { tab: RootTab },
    /// Present a named sheet over the root shell.
    PresentSheet { sheet_id: String },
    /// Dismiss the topmost sheet.
    DismissSheet,

    // ── Phase 2A additions (append-only) ─────────────────────────────────────
    /// Sign in with a raw nsec (bech32 `nsec1…` or hex).
    ///
    /// The reducer transitions to `SessionState::SigningIn` and emits
    /// `Effect::AddNsecSigner`. Success is signalled by the identity-change
    /// observer firing `KernelEvent::IdentityChanged(Some(pubkey))`. Failure
    /// surfaces as `SessionState::SignInFailed` (D6 — never a `Result`).
    SignInNsec { nsec: String },

    // ── Phase 2B additions (append-only) ─────────────────────────────────────
    /// Sign in via NIP-46 bunker URI (e.g. `bunker://pubkey?relay=…`).
    ///
    /// Requires `nmp_signer_broker_init` to have been called at boot. The
    /// reducer transitions to `SessionState::SigningIn{Bunker}` and emits
    /// `Effect::AddBunkerSigner`. The broker completes the NIP-46 handshake
    /// async; success arrives as `KernelEvent::IdentityChanged(Some(pubkey))`.
    PairBunker { uri: String },
    /// Request a NostrConnect URI so the user can scan it on a remote signer.
    ///
    /// Requires `nmp_signer_broker_init` to have been called at boot. The
    /// reducer transitions to `SessionState::SigningIn{NostrConnect}` and emits
    /// `Effect::MintNostrConnectUri`. The URI is delivered back via
    /// `KernelEvent::NostrConnectUriReady`; completion arrives as
    /// `KernelEvent::IdentityChanged(Some(pubkey))` once the remote signer
    /// scans the QR and completes the handshake.
    StartNostrConnect,
    /// Sign in via NIP-55 external signer app (e.g. Amber on Android).
    ///
    /// Requires `nmp_external_signer_init` to have been called at boot. The
    /// reducer transitions to `SessionState::SigningIn{Nip55}` and emits
    /// `Effect::StartNip55SignIn`. Success arrives via the identity-change
    /// observer as `KernelEvent::IdentityChanged(Some(pubkey))`.
    SignInNip55,

    // ── Phase 2C additions (append-only) ─────────────────────────────────────
    /// Create a fresh Nostr account with the supplied display name.
    ///
    /// Relays and initial follows are Rust POLICY injected from `AppConfig` —
    /// they are NOT caller arguments (D3: no hardcoded relay literals in
    /// kernel logic). Bootstrap publish semantics follow ADR-0059: kind:0 and
    /// kind:10002 are published; kind:3 is skipped when `initial_follows` is
    /// empty (per ADR-0059 §5).
    ///
    /// The reducer transitions to `SessionState::SigningIn{CreateAccount}` and
    /// emits `Effect::CreateAccount`. Success arrives via the existing
    /// `IdentityChanged(Some(pubkey))` observer → `SessionState::Present`.
    /// The 2A clock timeout (SIGN_IN_TIMEOUT_SECS) covers the SigningIn period.
    CreateAccount { profile_name: String },

    // ── Phase 2D additions (append-only) ─────────────────────────────────────
    /// Add a relay to the active account's NIP-65 relay list.
    ///
    /// `url` is the WebSocket relay URL (opaque string — kernel never
    /// constructs URLs; D3). `role` is the NIP-65 / kind:10002 role for the
    /// relay; the kernel normalises it via `RelayRole::normalize` before
    /// forwarding to nmp. Fire-and-forget: emits `Effect::AddRelay`.
    AddRelay { url: String, role: RelayRole },
    /// Remove a relay from the active account's NIP-65 relay list.
    ///
    /// Fire-and-forget: emits `Effect::RemoveRelay`. No-op if the relay is
    /// not present (nmp is idempotent here; D6).
    RemoveRelay { url: String },
    /// Change the role of an already-configured relay.
    ///
    /// Semantically equivalent to `RemoveRelay` + `AddRelay` in nmp's relay
    /// edit model (T66a). Fire-and-forget: emits `Effect::SetRelayRole`.
    SetRelayRole { url: String, role: RelayRole },
    /// Persist the rooms relay list (relays that host NIP-29 rooms) as a
    /// kind:30078 app-data event with d-tag `"com.highlighter.relays"`.
    ///
    /// `relay_urls` is the ordered list of room relay WebSocket URLs to store.
    /// The kernel builds the JSON payload and publishes via
    /// `ActorCommand::PublishRawEvent`. No wss-scheme literals are hardcoded here;
    /// the hl-owned d-tag string `"com.highlighter.relays"` is the only
    /// constant (it is product-controlled, not a relay URL).
    /// Fire-and-forget: emits `Effect::PublishRoomsRelayList`.
    SetRoomsRelayList { relay_urls: Vec<String> },

    // ── Phase 3C additions (append-only) ─────────────────────────────────────
    /// Follow a pubkey — appends it to the active account's kind:3 follow set
    /// and republishes. Fire-and-forget (D6, Non-Negotiable #3): the updated
    /// follow list arrives back via the `FollowListUpdated` projection frame.
    ///
    /// `pubkey` is a raw 64-char lowercase hex pubkey. Hex-shape validation
    /// lives in the nmp-nip02 action module; semantic errors surface as NMP
    /// toasts rather than crossing the dispatch boundary.
    Follow { pubkey: String },

    /// Unfollow a pubkey — removes it from the active account's kind:3 follow
    /// set and republishes. Symmetric with `Follow`; fire-and-forget (D6).
    Unfollow { pubkey: String },
}

/// NIP-65 / kind:10002 role for a configured relay.
///
/// Maps to the composite token strings that `nmp-core` accepts in
/// `ActorCommand::AddRelay { role }`. `normalize()` produces the canonical
/// wire string; `Nip65Role::parse` in nmp validates / rejects unknown tokens.
///
/// Append-only: new variants at the bottom keep rebases mechanical.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum RelayRole {
    /// Read-only relay (kind:10002 `"read"`).
    Read,
    /// Write-only relay (kind:10002 `"write"`).
    Write,
    /// Read + write relay (kind:10002 `"both"`).
    Both,
    /// Indexer relay (kind:10002 `"indexer"`).
    Indexer,
    /// Read + indexer composite (kind:10002 `"read,indexer"`).
    ReadIndexer,
    /// Write + indexer composite (kind:10002 `"write,indexer"`).
    WriteIndexer,
    /// Read + write + indexer composite (kind:10002 `"both,indexer"`).
    BothIndexer,
}

impl RelayRole {
    /// Produce the canonical wire-string expected by nmp's `Nip65Role::parse`.
    ///
    /// These token strings match `nmp-core`'s `relay_roles.rs` exactly —
    /// verified against origin/master. No wss-scheme literals; only the role
    /// vocabulary is embedded here (D3 compliant: role strings are kernel
    /// policy, not relay URLs).
    pub fn normalize(&self) -> &'static str {
        match self {
            RelayRole::Read => "read",
            RelayRole::Write => "write",
            RelayRole::Both => "both",
            RelayRole::Indexer => "indexer",
            RelayRole::ReadIndexer => "read,indexer",
            RelayRole::WriteIndexer => "write,indexer",
            RelayRole::BothIndexer => "both,indexer",
        }
    }
}

/// Internal kernel event — produced by async effects and native capability
/// results, fed back into the actor's command channel. Never crosses FFI.
///
/// Append-only: new variants at the bottom keep rebases mechanical.
#[derive(Debug, Clone)]
pub enum KernelEvent {
    /// A session-restore capability round-trip completed.
    /// `present` = a secret was found; `pubkey` = the hex pubkey decoded from it
    /// (Phase 2 — Phase 1 just records presence/absence).
    SessionRestored {
        present: bool,
        pubkey: Option<String>,
    },
    /// The `LoadOnboardingFlag` effect completed; `bool` = `is_complete()`.
    OnboardingStateLoaded(bool),
    /// A native capability result was delivered via `provide_capability_result`.
    CapabilityResult(CapabilityResult),
    /// NMP identity-change observer fired (active account changed or cleared).
    IdentityChanged(Option<String>),
    /// Clock-driven periodic tick — used for toast dismiss, session-restore
    /// timeout, and snapshot coalescing cadence (D8: no wall-clock reads,
    /// no sleeps; time is injected via the `Clock` abstraction, D9).
    ClockTick,

    // ── Phase 2A additions (append-only) ─────────────────────────────────────
    /// `add_signer` returned an error; the effect runner converts the error
    /// into this event so it surfaces in `SessionState` rather than crossing
    /// the dispatch boundary as a `Result` (D6).
    SignInFailed { method: SignInMethod, error: String },

    // ── Phase 2B additions (append-only) ─────────────────────────────────────
    /// `nmp_app_nostrconnect_uri` produced a URI the UI can display as a QR
    /// code. Carried into the snapshot so the iOS sheet can render it without
    /// polling. The URI is bounded (one string ≤ 512 bytes per NIP-46 spec).
    NostrConnectUriReady { uri: String },
    /// The NIP-46 broker reported handshake progress. `stage` is an opaque
    /// label (e.g. `"connecting"`, `"authenticating"`); `message` is a
    /// human-readable description for debug/diagnostics — not shown in UI.
    BunkerHandshakeState { stage: String, message: String },

    // ── Phase 3A additions (append-only) ─────────────────────────────────────
    /// The NMP update callback received a raw snapshot frame.
    ///
    /// The frame is forwarded from the callback thread into the actor channel
    /// without decoding (callback must be non-blocking; decode happens in the
    /// actor). The actor's `reduce_event` arm dispatches to
    /// `projections::dispatch_typed_frame` which decodes the sidecar and
    /// produces the appropriate `*Updated` events.
    NmpSnapshotFrame(Vec<u8>),

    // ── Phase 3B additions (append-only) ─────────────────────────────────────
    /// The `"nmp.nip29.joined_groups"` typed sidecar was decoded.
    ///
    /// Produced by `projections::dispatch_typed_frame` when the schema_id
    /// `"nmp.nip29.joined_groups"` sidecar arrives (requires nmp-nip29 PR
    /// #1587/#1588 to be pinned). Stored in `AppState.communities` by
    /// `communities::reduce_event_joined_groups_updated`.
    JoinedGroupsUpdated(Vec<crate::kernel::snapshot::CommunityRow>),

    // ── Phase 3C additions (append-only) ─────────────────────────────────────
    /// The `"nmp.nip02.follow_list"` typed sidecar was decoded from an NMP
    /// snapshot frame. Carries the raw hex pubkeys from the active account's
    /// kind:3 follow set. The reducer stores them in `AppState::follows` so
    /// the `is_following` query and the `Profile` view snapshot (Phase 3D) can
    /// consult the set without re-parsing the projection.
    FollowListUpdated(Vec<String>),
}
