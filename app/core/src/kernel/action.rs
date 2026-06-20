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
    SelectRootTab {
        tab: RootTab,
    },
    /// Present a named sheet over the root shell.
    PresentSheet {
        sheet_id: String,
    },
    /// Dismiss the topmost sheet.
    DismissSheet,

    // ── Phase 2A additions (append-only) ─────────────────────────────────────
    /// Sign in with a raw nsec (bech32 `nsec1…` or hex).
    ///
    /// The reducer transitions to `SessionState::SigningIn` and emits
    /// `Effect::AddNsecSigner`. Success is signalled by the identity-change
    /// observer firing `KernelEvent::IdentityChanged(Some(pubkey))`. Failure
    /// surfaces as `SessionState::SignInFailed` (D6 — never a `Result`).
    SignInNsec {
        nsec: String,
    },

    // ── Phase 2B additions (append-only) ─────────────────────────────────────
    /// Sign in via NIP-46 bunker URI (e.g. `bunker://pubkey?relay=…`).
    ///
    /// Requires `nmp_signer_broker_init` to have been called at boot. The
    /// reducer transitions to `SessionState::SigningIn{Bunker}` and emits
    /// `Effect::AddBunkerSigner`. The broker completes the NIP-46 handshake
    /// async; success arrives as `KernelEvent::IdentityChanged(Some(pubkey))`.
    PairBunker {
        uri: String,
    },
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
    CreateAccount {
        profile_name: String,
    },

    // ── Phase 2D additions (append-only) ─────────────────────────────────────
    /// Add a relay to the active account's NIP-65 relay list.
    ///
    /// `url` is the WebSocket relay URL (opaque string — kernel never
    /// constructs URLs; D3). `role` is the NIP-65 / kind:10002 role for the
    /// relay; the kernel normalises it via `RelayRole::normalize` before
    /// forwarding to nmp. Fire-and-forget: emits `Effect::AddRelay`.
    AddRelay {
        url: String,
        role: RelayRole,
    },
    /// Remove a relay from the active account's NIP-65 relay list.
    ///
    /// Fire-and-forget: emits `Effect::RemoveRelay`. No-op if the relay is
    /// not present (nmp is idempotent here; D6).
    RemoveRelay {
        url: String,
    },
    /// Change the role of an already-configured relay.
    ///
    /// Semantically equivalent to `RemoveRelay` + `AddRelay` in nmp's relay
    /// edit model (T66a). Fire-and-forget: emits `Effect::SetRelayRole`.
    SetRelayRole {
        url: String,
        role: RelayRole,
    },
    /// Persist the rooms relay list (relays that host NIP-29 rooms) as a
    /// kind:30078 app-data event with d-tag `"com.highlighter.relays"`.
    ///
    /// `relay_urls` is the ordered list of room relay WebSocket URLs to store.
    /// The kernel builds the JSON payload and publishes via
    /// `ActorCommand::PublishRawEvent`. No wss-scheme literals are hardcoded here;
    /// the hl-owned d-tag string `"com.highlighter.relays"` is the only
    /// constant (it is product-controlled, not a relay URL).
    /// Fire-and-forget: emits `Effect::PublishRoomsRelayList`.
    SetRoomsRelayList {
        relay_urls: Vec<String>,
    },

    // ── Phase 3C additions (append-only) ─────────────────────────────────────
    /// Follow a pubkey — appends it to the active account's kind:3 follow set
    /// and republishes. Fire-and-forget (D6, Non-Negotiable #3): the updated
    /// follow list arrives back via the `FollowListUpdated` projection frame.
    ///
    /// `pubkey` is a raw 64-char lowercase hex pubkey. Hex-shape validation
    /// lives in the nmp-nip02 action module; semantic errors surface as NMP
    /// toasts rather than crossing the dispatch boundary.
    Follow {
        pubkey: String,
    },

    /// Unfollow a pubkey — removes it from the active account's kind:3 follow
    /// set and republishes. Symmetric with `Follow`; fire-and-forget (D6).
    Unfollow {
        pubkey: String,
    },

    // ── Phase 3E additions (append-only) ─────────────────────────────────────
    /// Start room discovery on a relay — dispatches `"nmp.nip29.discover"` action
    /// (pushes the relay_discovery_interest) and wires the DiscoveredGroupsProjection.
    /// `relay_url` is the WebSocket relay URL to discover rooms on (opaque string;
    /// kernel never constructs relay URLs — D3). Fire-and-forget: the discovered
    /// groups catalog arrives via the `DiscoveredGroupsUpdated` projection event.
    StartRoomDiscovery {
        relay_url: String,
    },

    // ── Phase 3D additions (append-only) ─────────────────────────────────────
    /// Open a profile view for `pubkey` — triggers `nmp_app_claim_profile` via
    /// `Effect::ClaimProfile`. The profile card arrives back as
    /// `KernelEvent::ProfileCardUpdated` via the `"claimed_profiles"` typed
    /// sidecar on the NMP update callback.
    ///
    /// `pubkey` is a raw 64-char lowercase hex pubkey. The kernel uses a stable
    /// consumer-id (`"hl.profile.<pubkey>"`) so the refcount is scoped to this
    /// view instance. Fire-and-forget (D6, Non-Negotiable #3).
    ClaimProfile {
        pubkey: String,
    },

    /// Close a profile view — triggers `nmp_app_release_profile`. Decrements the
    /// per-consumer refcount; when it reaches zero NMP cancels the kind:0
    /// subscription. Fire-and-forget (D6).
    ReleaseProfile {
        pubkey: String,
    },

    // ── Phase 3F additions (append-only) ─────────────────────────────────────
    /// Join a NIP-29 group by publishing a kind:9021 join-request via
    /// `"nmp.nip29.join"`. The relay's response arrives as a joined-groups
    /// projection update (`KernelEvent::JoinedGroupsUpdated`). Fire-and-forget.
    ///
    /// `group_id` is the NIP-29 local group id; `host_relay_url` is the relay
    /// URL (opaque string — kernel never constructs URLs, D3). `invite_code`
    /// is required for closed groups, optional for open groups.
    ///
    /// NOTE: LeaveRoom (kind:9022) is NOT implemented — there is no
    /// `nmp.nip29.leave` action on pinned nmp b4404159. See nmp issue #1598.
    JoinRoom {
        /// NIP-29 local group id.
        group_id: String,
        /// Host relay WebSocket URL (opaque — D3).
        host_relay_url: String,
        /// Optional preauth invite code for closed groups.
        invite_code: Option<String>,
    },

    /// Create a new public NIP-29 group by publishing kind:9007 + kind:9002
    /// via `"nmp.nip29.create_public_group"`. Fire-and-forget.
    ///
    /// `group_id` is the desired local group id (`[a-z0-9-_]+`). `name` is
    /// the human-readable display name (required). `about` is optional.
    CreateRoom {
        /// NIP-29 local group id (must match `[a-z0-9-_]+`).
        group_id: String,
        /// Host relay WebSocket URL (opaque — D3).
        host_relay_url: String,
        /// Human-readable room name (required, non-empty).
        name: String,
        /// Optional description.
        about: Option<String>,
    },

    /// Add a member to a NIP-29 group by publishing kind:9000 via
    /// `"nmp.nip29.put_user"`. Requires admin rights on the target relay.
    /// Fire-and-forget.
    ///
    /// `pubkey` is a raw 64-char lowercase hex pubkey. `role` is an optional
    /// role string (e.g. `"admin"`) or `None` for a plain member.
    AddRoomMember {
        /// NIP-29 local group id.
        group_id: String,
        /// Host relay WebSocket URL (opaque — D3).
        host_relay_url: String,
        /// Raw 64-char lowercase hex pubkey of the user to add.
        pubkey: String,
        /// Optional role (e.g. `"admin"`). `None` = plain member.
        role: Option<String>,
    },

    /// Mint one or more invite codes for a NIP-29 group by publishing kind:9009
    /// via `"nmp.nip29.create_invite"`. Requires admin rights. Fire-and-forget.
    ///
    /// `codes` must be non-empty; nmp fans out into multiple kind:9009 events
    /// if more than 10 codes are supplied (MAX_CODES_PER_INVITE_EVENT).
    /// The invite_link_base URL is NOT a kernel concern — Swift composes the
    /// full invite URL from `AppState::room_policy.invite_link_base` + code (D3).
    CreateRoomInvites {
        /// NIP-29 local group id.
        group_id: String,
        /// Host relay WebSocket URL (opaque — D3).
        host_relay_url: String,
        /// Invite codes (≥1 required; max 128 chars each; printable ASCII only).
        codes: Vec<String>,
    },

    // ── Phase 4E additions (append-only) ─────────────────────────────────────
    /// Share an existing event into a NIP-29 group.
    ///
    /// When `repost` is `false`, dispatches `"nmp.nip29.share_event_in_group"`
    /// which publishes a kind:11 artifact event tagged with `#h` for the group.
    /// When `repost` is `true`, dispatches `"nmp.nip29.repost_in_group"` which
    /// publishes a kind:16 repost event tagged with `#h`.
    ///
    /// Verified action namespaces on pinned nmp b4404159
    /// (`crates/nmp-nip29/src/action/group_event.rs:101,124`):
    /// - `ShareEventInGroupInput { group: GroupId, target: GroupEventTarget { event_id, author_pubkey? }, content, additional_tags }`
    /// - `RepostInGroupInput { group: GroupId, target: GroupEventTarget { event_id, author_pubkey? }, content, additional_tags }`
    ///
    /// Kernel is the sole writer for these events on ported screens — no
    /// double-publish with the bespoke lane. Fire-and-forget (D6).
    /// D3: `group_id` + `host_relay_url` are opaque strings from the caller.
    ShareToRoom {
        /// NIP-29 local group id (the `h` tag value).
        group_id: String,
        /// Host relay WebSocket URL for this group (opaque — D3).
        host_relay_url: String,
        /// The Nostr event id of the event being shared.
        target_event_id: String,
        /// Optional hex pubkey of the event's author (populates the `p` tag).
        target_author_pubkey: Option<String>,
        /// When `true` → kind:16 repost (`"nmp.nip29.repost_in_group"`).
        /// When `false` → kind:11 share (`"nmp.nip29.share_event_in_group"`).
        repost: bool,
    },

    // ── Phase 4C additions (append-only) ─────────────────────────────────────
    /// Add a bookmark item to the active account's NIP-51 kind:10003 list by
    /// dispatching `"nmp.nip51.add_bookmark"` with a `BookmarkUpdateInput`
    /// JSON payload. Fire-and-forget (D6, Non-Negotiable #3): the updated
    /// kind:10003 list arrives back via the `BookmarksUpdated` projection event.
    ///
    /// Kernel is the SOLE writer for kind:10003 on ported screens — no live-lane
    /// double-publish. No optimistic update: raw state reflects on-chain list.
    AddBookmark {
        /// The bookmark item to add (raw protocol data — D1).
        item: crate::kernel::snapshot::BookmarkRow,
    },

    /// Remove a bookmark item from the active account's NIP-51 kind:10003 list
    /// by dispatching `"nmp.nip51.remove_bookmark"`. Symmetric with `AddBookmark`.
    /// Fire-and-forget (D6, Non-Negotiable #3).
    RemoveBookmark {
        /// The bookmark item to remove (raw protocol data — D1).
        item: crate::kernel::snapshot::BookmarkRow,
    },

    // ── Phase 4A additions (append-only) ─────────────────────────────────────
    /// Open an article reader view for the given NIP-23 address.
    ///
    /// `address` is the addressable coordinate `kind:author_hex:d_tag` that
    /// uniquely identifies the parameterized-replaceable kind:30023 event.
    /// Fire-and-forget: the kernel registers `ViewId::ArticleReader{address}`
    /// and emits a snapshot once the article is present in `AppState::articles`
    /// (populated by the nmp.nip23.articles typed projection). No NMP action is
    /// dispatched — the longform projection populates the state automatically.
    OpenArticle {
        address: String,
    },

    /// Close an article reader view — deregisters `ViewId::ArticleReader{address}`.
    /// Fire-and-forget. No NMP release is needed (longform projection is session-scoped).
    CloseArticle {
        address: String,
    },

    // ── Phase 4B additions (append-only) ─────────────────────────────────────
    /// React to an event with a NIP-25 kind:7 reaction.
    ///
    /// Dispatches `"nmp.nip25.react"` via `nmp_app_dispatch_action`. The wire
    /// payload is `ReactAction { target_event_id, reaction, target_author_pubkey? }`.
    /// `reaction` defaults to `"+"` (like) when not supplied; any non-empty
    /// emoji or custom string is accepted.
    ///
    /// The kernel is the sole kind:7 writer for ported screens (no live-lane
    /// double-publish). Fire-and-forget (D6, Non-Negotiable #3): the updated
    /// reaction count arrives back via `KernelEvent::ReactionStateUpdated` once
    /// the `ReactionProjection` tick fires.
    React {
        /// Target event id (raw 64-char hex).
        target_event_id: String,
        /// Reaction content — defaults to `"+"`. Must be non-empty.
        reaction: String,
        /// Optional author pubkey for the `["p", _]` tag (raw 64-char hex).
        target_author_pubkey: Option<String>,
    },

    /// Remove a prior reaction by publishing a kind:5 deletion via
    /// `"nmp.nip25.unreact"`.
    ///
    /// The wire payload is `UnreactAction { reaction_event_id, reason }`.
    /// `reaction_event_id` must be the raw 64-char hex id of the kind:7 event
    /// to delete. `reason` is an optional freetext string (sent as empty when
    /// absent). Fire-and-forget (D6).
    Unreact {
        /// The kind:7 reaction event id to delete (raw 64-char hex).
        reaction_event_id: String,
    },

    // ── Phase 4G additions (append-only) ─────────────────────────────────────
    /// Pull the next page of the "Following reads" article feed.
    ///
    /// Dispatched by the UI on scroll-to-end of the `ViewId::ArticleFeed`
    /// view. The reducer emits `Effect::DrainFeed { key: "hl.feed.articles" }`
    /// which calls `nmp_app_pull_page` once and feeds the result back as
    /// `KernelEvent::FeedPage` (D8: no polling — one pull per action).
    ///
    /// No-op when `AppState::article_feed.exhausted == true` (fully caught up).
    /// Fire-and-forget (D6, Non-Negotiable #3).
    LoadMoreArticles,

    // ── Phase 4H additions (append-only) ─────────────────────────────────────
    /// Request the next page from the `"hl.feed.highlights"` pull cursor.
    ///
    /// Emitted by the UI at scroll-to-end. Produces `Effect::DrainFeed{key:
    /// "hl.feed.highlights"}`. Fire-and-forget (D6, Non-Negotiable #3): the next
    /// page arrives as `KernelEvent::FeedPage` once the drain completes.
    ///
    /// No-op when the cursor is exhausted (`FeedState.exhausted == true`) —
    /// the reducer checks exhaustion before emitting the effect to avoid
    /// redundant drain calls against a caught-up cursor (D8 — no polling).
    DrainHighlightFeed,

    /// Publish a new NIP-84 kind:9802 highlight event.
    ///
    /// There is no dedicated nmp action namespace for kind:9802 at pinned nmp
    /// b4404159 (verified by grep — see §6 of the Phase 4 spec). The kernel
    /// publishes via `ActorCommand::PublishRawEvent` — the same write path
    /// Phase 2D uses for the rooms relay list. Fire-and-forget (D6).
    ///
    /// Kernel is the sole kind:9802 writer on ported screens — no live-lane
    /// double-publish. The new highlight will appear in the feed on the next
    /// `DrainHighlightFeed` once the relay echoes the event back (normal
    /// pub-echo cycle, no optimistic insert in the kernel — D1).
    ///
    /// D1: `content` and `source_reference` are raw strings. The kernel does
    /// NOT format the NIP-84 `a`/`e` tag value — it receives the already-resolved
    /// coordinate from the caller and embeds it verbatim (D3: no URL construction
    /// or address normalization in kernel logic).
    PublishHighlight {
        /// The highlighted text passage. Must be non-empty (D6: empty content
        /// is a no-op at the reducer level — no event is published).
        content: String,
        /// NIP-84 source reference: `"<kind>:<pubkey>:<d_tag>"` for addressable
        /// (kind:30023 articles), or a raw 64-char hex event id for non-addressable.
        /// D3: opaque string from the caller.
        source_reference: String,
        /// Optional relay URL hint for the `a`/`e` tag (D3: opaque from caller).
        relay_hint: Option<String>,
    },

    // ── Phase 5C additions (append-only) ─────────────────────────────────────
    // Doc comment intentionally omitted — AppAction uniffi metadata is near BUF_SIZE
    // limit (5A + 5K pushed it close). Full doc is in kernel/domains/isbn.rs.
    LookupIsbn {
        isbn: String,
    },

    // ── Phase 4D additions (append-only) ─────────────────────────────────────
    /// Run a NIP-50 relay search for `query` with the given `scope`.
    ///
    /// The reducer emits `Effect::RunSearch` which:
    ///   1. Pushes a `LogicalInterest` with `InterestShape.search = Some(query)`
    ///      via `NmpApp::push_interest`, causing the planner to issue NIP-50 REQs
    ///      on all connected search-capable relays.
    ///   2. Replaces the hl-owned `SearchResultsProjection` (registered under
    ///      the typed snapshot key `"hl.search"`) with a fresh instance seeded
    ///      from the new `SearchRequest`, clearing stale results.
    ///
    /// `query` is a trimmed plain-text search term (empty / whitespace-only is
    /// a no-op at the effect-runner level via `SearchRequest::new`). `scope`
    /// selects which event kinds to search (e.g. `LongForm`, `Users`).
    ///
    /// nmp-nip50 has NO action namespace — submission is via `push_interest`
    /// (confirmed on pinned nmp b4404159, `crates/nmp-ffi/src/lib.rs:1828`).
    /// Fire-and-forget (D6, Non-Negotiable #3): search hits arrive as
    /// `KernelEvent::SearchResultsUpdated` via the typed snapshot pipeline.
    RunSearch {
        /// Plain-text search query. Empty string / whitespace → no-op (D6).
        query: String,
        /// Which NIP-50 scope to search in.
        scope: SearchScope,
    },

    // ── Phase 5A additions (append-only) ─────────────────────────────────────
    /// Prepare the What's New sheet — load entries and seen marker from disk.
    /// Device-local (never published to nostr — `hl-app-state-vs-nostr-facts`).
    PrepareWhatsNew,
    /// Advance the seen marker to `shipped_at_unix`. Monotonic — never moves backward.
    /// Device-local: persists to `{data_dir}/whats-new-state-v1.json`.
    MarkWhatsNewSeen {
        shipped_at_unix: u64,
    },

    // ── Phase 5K additions (append-only) ─────────────────────────────────────
    DrainShareQueue,
}

/// NIP-50 search scope — which event kinds the relay-search targets.
///
/// Serialised into the `LogicalInterest`'s `InterestShape.kinds` field via
/// `nmp_nip50::SearchScope::interest_shape()`. Append-only.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum SearchScope {
    /// kind:0 profile metadata — search for users.
    Users,
    /// kind:30023 long-form articles — search for articles.
    LongForm,
    /// kind:1 short text notes — search for notes.
    Notes,
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
// Phase 5C adds `KernelArtifactPreview` to `IsbnPreviewReady` which carries
// 12 String fields. ProfileCardUpdated (Phase 3D) was already the large variant;
// this allow was not needed until 5C pushed the size gap above threshold.
#[allow(clippy::large_enum_variant)]
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

    // ── Phase 3E additions (append-only) ─────────────────────────────────────
    /// The `"nmp.nip29.discovered_groups"` typed sidecar was decoded.
    ///
    /// Produced by `projections::dispatch_typed_frame` when the schema_id
    /// `"nmp.nip29.discovered_groups"` sidecar arrives. Stored in
    /// `AppState.discovered_groups` by
    /// `discovery::reduce_event_discovered_groups_updated`.
    DiscoveredGroupsUpdated(Vec<crate::kernel::snapshot::DiscoveredRow>),

    // ── Phase 3D additions (append-only) ─────────────────────────────────────
    /// A profile card from the `"claimed_profiles"` typed sidecar was decoded.
    ///
    /// Produced by `projections::dispatch_typed_frame` when the
    /// `"claimed_profiles"` schema_id sidecar arrives. Carries one updated
    /// `ProfileCardModel` for the given `pubkey`. The reducer stores it in
    /// `AppState::claimed_profiles` so the `ViewId::Profile{pubkey}` snapshot
    /// can read it directly (non-blocking HashMap lookup).
    ///
    /// Also produced when the `"profile"` (own-account) sidecar arrives for
    /// the active account's pubkey, in case the Profile view is open for the
    /// own account. In that case `pubkey` matches the active account.
    ProfileCardUpdated {
        /// Raw 64-char hex pubkey.
        pubkey: String,
        /// Decoded `ProfileCardModel` — raw fields, no presentation formatting.
        /// Boxed to keep `KernelEvent` size balanced (ProfileCardModel is large).
        card: Box<nmp_core::typed_projections::ProfileCardModel>,
    },

    // ── Phase 4C additions (append-only) ─────────────────────────────────────
    /// The `"hl.bookmarks"` typed sidecar was decoded from an NMP snapshot frame.
    ///
    /// Produced by `projections::dispatch_typed_frame` when the hl-owned
    /// `"hl.bookmarks"` JSON-serde sidecar arrives (registered by
    /// `bookmarks::register_bookmark_list_projection`). Carries raw
    /// `BookmarkRow` items from the active account's kind:10003 list.
    /// The reducer stores them in `AppState::bookmarks`.
    ///
    /// No labels or formatted strings — Swift formats all bookmark UI (D1).
    BookmarksUpdated(Vec<crate::kernel::snapshot::BookmarkRow>),

    // ── Phase 4A additions (append-only) ─────────────────────────────────────
    /// The `"nmp.nip23.articles"` typed sidecar was decoded.
    ///
    /// Produced by `projections::dispatch_typed_frame` when the schema_id
    /// `"nmp.nip23.articles"` sidecar arrives. Carries all decoded article rows
    /// (raw fields — D1: no formatted strings). The reducer replaces
    /// `AppState::articles` with the new set. Also injectable directly from
    /// tests via `Cmd::Event` (no live NmpApp needed).
    ArticlesUpdated(Vec<crate::kernel::snapshot::ArticleRow>),

    // ── Phase 4B additions (append-only) ─────────────────────────────────────
    /// The `"hl.reactions"` wrapped typed-snapshot was decoded from the NMP
    /// update callback and applied to `AppState::reaction_state`.
    ///
    /// Produced by `projections::dispatch_typed_frame` when the `"hl.reactions"`
    /// schema_id sidecar arrives. Carries raw count + viewer-reacted bool for
    /// one target event — no formatted strings (D1). The reducer stores it in
    /// `AppState::reaction_state` keyed by `target_event_id`.
    ///
    /// Optimistic UI state (count delta, toggled icon) lives in Swift (D1).
    /// The kernel exposes only the authoritative `ReactionProjection` values.
    ReactionStateUpdated {
        /// Target event id that was reacted to (raw 64-char hex).
        target_event_id: String,
        /// Total reaction count from all authors (authoritative from projection).
        count: u32,
        /// `true` if the active viewer has reacted.
        viewer_reacted: bool,
    },

    // ── Phase 4D additions (append-only) ─────────────────────────────────────
    /// The `"hl.search"` typed sidecar was decoded from an NMP snapshot frame.
    ///
    /// Produced by `projections::dispatch_typed_frame` when the hl-owned
    /// `"hl.search"` JSON-serde sidecar arrives (registered and replaced by
    /// `search::register_search_projection` / `search::replace_search_projection`
    /// on each `RunSearch` dispatch). Carries raw `SearchHitRow` items from the
    /// active NIP-50 search. The reducer stores them in `AppState::search_results`.
    ///
    /// Bounded by `DEFAULT_MAX_SEARCH_HITS` (200) from nmp-nip50 — never
    /// unbounded (Non-Negotiable #7). No labels or formatted strings — Swift
    /// formats all search result UI (D1).
    SearchResultsUpdated(Vec<crate::kernel::snapshot::SearchHitRow>),

    // ── Phase 4F additions (append-only) ─────────────────────────────────────
    /// `nmp_app_pull_page` returned a decoded page (or a Gap rebase) for the
    /// named feed cursor.
    ///
    /// Produced by the `DrainFeed` effect runner after decoding the binary Page
    /// wire from `nmp_ffi::pull::nmp_app_pull_page`. The reducer routes on `key`
    /// to the correct `FeedState` in `AppState` and calls
    /// `feed::apply_feed_page`.
    ///
    /// - `rows`: positive (Inserted/Replaced) entries decoded into raw
    ///   `KernelEvent`s in ingest-seq order. Empty on a Gap (clear-and-rebase).
    /// - `next_after_seq`: the kernel's `next_after_seq` from the wire — the
    ///   cursor should advance to this value.
    /// - `exhausted`: `true` when `has_more == false` (fully caught up).
    /// - `gap_rebased_to`: `Some(seq)` on a Gap; the reducer clears `rows` and
    ///   resets `after_seq` to `seq` (ADR-0058 §10).
    ///
    /// D1: `KernelEvent` fields are raw protocol data — no formatted strings.
    /// D5: rows are bounded by `FEED_PAGE_SIZE` per drain call.
    /// D8: no polling — this event arrives once per `DrainFeed` effect,
    ///     not from a timer.
    FeedPage {
        /// Stable feed key matching the `RegisterFeedCursor.key`.
        key: String,
        /// Cursor id that was drained (for the actor to call `AdvancePullCursor`).
        cursor_id: u64,
        /// Decoded positive-log rows from this page (ingest-seq order).
        rows: Vec<nmp_core::substrate::KernelEvent>,
        /// The kernel's `next_after_seq` — advance the cursor here.
        next_after_seq: u64,
        /// `true` when `has_more == false` (the feed is fully caught up).
        exhausted: bool,
        /// `Some(seq)` on a Gap — the reducer must clear rows and rebase to seq.
        gap_rebased_to: Option<u64>,
    },

    // ── Phase 5A additions (append-only) ─────────────────────────────────────
    /// What's New entries and presentation flag loaded from bundled JSON + state file.
    WhatsNewLoaded {
        entries: Vec<crate::kernel::snapshot::WhatsNewEntryRow>,
        should_present: bool,
    },

    // ── Phase 5C additions (append-only) ─────────────────────────────────────
    /// The Open Library HTTP lookup for `isbn13` completed (or was served from
    /// the in-memory cache). `preview` is `Some` on success; `error` is non-empty
    /// on failure. The reducer stores the result in `AppState::isbn` and emits
    /// `Effect::PersistIsbnCache` when a new entry was fetched (not from cache).
    ///
    /// Device-local — never triggers a nostr publish (memory hl-app-state-vs-nostr-facts).
    IsbnPreviewReady {
        /// Normalized 13-digit Bookland ISBN.
        isbn13: String,
        /// Fetched or cached preview. `None` only if normalization was impossible.
        preview: Option<crate::kernel::domains::isbn::KernelArtifactPreview>,
        /// Non-empty if the HTTP fetch failed; empty on success/cache-hit.
        error: String,
    },
    /// The in-memory ISBN cache was loaded from `isbn-preview-cache-v1.json`.
    ///
    /// Produced by `run_effect_load_isbn_cache` on first lookup after the cache
    /// file is available. Carries the raw deserialized entries so the reducer can
    /// populate `AppState::isbn.cache` and set `cache_loaded = true`.
    IsbnCacheLoaded {
        /// Deserialized (isbn13, entry) pairs from the JSON cache file.
        entries: Vec<(String, crate::kernel::domains::isbn::CachedIsbnEntry)>,
    },

    // ── Phase 5K additions (append-only) ─────────────────────────────────────
    /// The App Group share queue was drained by the native capability bridge.
    ///
    /// Produced when `CapabilityResult::Share(ShareResult::Pending(payloads))`
    /// arrives via `provide_capability_result` in response to a
    /// `CapabilityRequest::Share(ShareOp::DrainQueue)`. Carries the raw payloads
    /// the iOS share extension wrote to `pending-shares-v1.json` before the
    /// native bridge deleted the file.
    ///
    /// The reducer deduplicates by `(group_id, url)` and appends new items to
    /// `AppState::share_queue.pending`. Device-local — NOT a nostr fact.
    ShareQueueDrained(Vec<crate::capabilities::share::RawSharePayload>),
}
