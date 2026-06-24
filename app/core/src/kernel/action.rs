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

/// Thin envelope that carries actions across the UniFFI boundary.
///
/// Replaces the direct `#[uniffi::Enum] AppAction` export. The `namespace`
/// keys a typed serde payload in `json`; the kernel router decodes each
/// namespace to the correct domain reducer. Unknown namespaces produce an
/// invalid-action toast (D6 — never a panic).
#[derive(Debug, Clone, uniffi::Record)]
pub struct AppActionEnvelope {
    pub namespace: String,
    pub json: String,
}

// ── Typed serde payload structs for the envelope router ──────────────────────
//
// One struct per namespace that carries >0 fields. Zero-field actions use
// `serde_json::Value::Null` and need no struct.

#[derive(Debug, serde::Deserialize)]
pub(crate) struct SelectRootTabPayload {
    pub tab: u8,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct PresentSheetPayload {
    pub sheet_id: String,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct SignInNsecPayload {
    pub nsec: String,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct PairBunkerPayload {
    pub uri: String,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct CreateAccountPayload {
    pub profile_name: String,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct AddRelayPayload {
    pub url: String,
    pub role: String,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct RemoveRelayPayload {
    pub url: String,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct SetRelayRolePayload {
    pub url: String,
    pub role: String,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct FollowPayload {
    pub pubkey: String,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct UnfollowPayload {
    pub pubkey: String,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct StartRoomDiscoveryPayload {
    pub relay_url: String,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct ClaimProfilePayload {
    pub pubkey: String,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct ReleaseProfilePayload {
    pub pubkey: String,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct JoinRoomPayload {
    pub group_id: String,
    pub host_relay_url: String,
    pub invite_code: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct LeaveRoomPayload {
    pub(crate) group_id: String,
    pub(crate) host_relay_url: String,
    pub(crate) reason: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct CreateRoomPayload {
    pub group_id: String,
    pub host_relay_url: String,
    pub name: String,
    pub about: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct AddRoomMemberPayload {
    pub group_id: String,
    pub host_relay_url: String,
    pub pubkey: String,
    pub role: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct CreateRoomInvitesPayload {
    pub group_id: String,
    pub host_relay_url: String,
    pub codes: Vec<String>,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct ShareToRoomPayload {
    pub group_id: String,
    pub host_relay_url: String,
    pub target_event_id: String,
    pub target_author_pubkey: Option<String>,
    pub repost: bool,
}

// ── #21 share-flow payloads ───────────────────────────────────────────────────
#[derive(Debug, serde::Deserialize)]
pub(crate) struct ShareArtifactToRoomPayload {
    pub group_id: String,
    pub host_relay_url: String,
    pub preview: crate::kernel::models::ArtifactPreview,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct ShareHighlightToRoomPayload {
    pub group_id: String,
    pub host_relay_url: String,
    pub highlight_event_id: String,
    pub highlight_author_pubkey: String,
    #[serde(default)]
    pub relay_hint: String,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct ShareMintInvitePayload {
    pub group_id: String,
    pub host_relay_url: String,
    #[serde(default = "one")]
    pub count: u32,
}

fn one() -> u32 {
    1
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct AddBookmarkPayload {
    pub item: crate::kernel::snapshot::BookmarkRow,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct RemoveBookmarkPayload {
    pub item: crate::kernel::snapshot::BookmarkRow,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct ReactPayload {
    pub target_event_id: String,
    pub reaction: String,
    pub target_author_pubkey: Option<String>,
}

/// `hl.reaction.toggle` envelope payload — like-or-unlike a target by id.
///
/// The kernel decides react-vs-unreact from its own viewer-reaction tracking
/// (`AppState::viewer_reaction_ids`); the reaction kind:7 event id never crosses
/// FFI. Optional `target_author_pubkey` is used only on the react path (`["p"]`).
#[derive(Debug, serde::Deserialize)]
pub(crate) struct ToggleReactionPayload {
    pub target_event_id: String,
    pub target_author_pubkey: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct UnreactPayload {
    pub reaction_event_id: String,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct RunSearchPayload {
    pub query: String,
    pub scope: String,
}

/// `hl.search.omnibox` envelope payload — one raw input string for the
/// input-intent resolver (`#1865`). No scope: the omnibox declares its own
/// scope allow-list in `omnibox::omnibox_scopes`.
#[derive(Debug, serde::Deserialize)]
pub(crate) struct RunOmniboxPayload {
    pub query: String,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct MarkWhatsNewSeenPayload {
    pub shipped_at_unix: u64,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct LookupIsbnPayload {
    pub isbn: String,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct SetBookPickerQueryPayload {
    pub query: String,
    #[serde(default = "default_recent_limit")]
    pub recent_limit: u32,
    #[serde(default = "default_search_limit")]
    pub search_limit: u32,
}

fn default_recent_limit() -> u32 {
    24
}
fn default_search_limit() -> u32 {
    20
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct PublishHighlightPayload {
    pub content: String,
    pub source_reference: String,
    pub relay_hint: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub context: Option<String>,
}

// ── Phase 5H payload structs ─────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub(crate) struct AudioPlayPayload {
    pub url: String,
    pub guid: String,
    pub artifact_json: String,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct AudioSeekPayload {
    pub seconds: f64,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct AudioSetResumePayload {
    pub seconds: f64,
}

// ── Phase 5I payload structs ─────────────────────────────────────────────────

/// `hl.transcript.load` envelope payload — no fields needed (URL comes from
/// the already-loaded `AppState::podcast.current.artifact`).
#[allow(dead_code)]
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct TranscriptLoadPayload {}

/// `hl.audio.clip_mark_in` payload.
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct ClipMarkInPayload {
    pub current_time: f64,
}

/// `hl.audio.clip_mark_out` payload.
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct ClipMarkOutPayload {
    pub current_time: f64,
}

/// `hl.audio.clip_extend_segment` payload.
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct ClipExtendSegmentPayload {
    pub segment_id: String,
}

/// `hl.audio.clip_set_start` payload.
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct ClipSetStartPayload {
    pub value: f64,
}

/// `hl.audio.clip_set_end` payload.
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct ClipSetEndPayload {
    pub value: f64,
    pub duration_seconds: f64,
}

// ── Phase 5J payload structs ─────────────────────────────────────────────────

/// `hl.podcast.publish_clip` envelope payload.
///
/// Swift sends this after the user confirms the clip on the composer screen.
/// The `artifact_json` carries the full `ArtifactRecord` so the kernel can
/// build the NIP-73 i-tag without a separate round-trip. `note` is the
/// optional comment to attach to the kind:9802.
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct PublishClipPayload {
    pub artifact_json: String,
    pub note: Option<String>,
}

// ── Phase 5D additions (append-only) ─────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub(crate) struct OcrRecognizePayload {
    pub image_handle: String,
}

// ── Phase 5F payload structs ─────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub(crate) struct CaptureSetQuotePayload {
    pub quote: String,
}
#[derive(Debug, serde::Deserialize)]
pub(crate) struct CaptureSetContextPayload {
    pub context: String,
}
#[derive(Debug, serde::Deserialize)]
pub(crate) struct CaptureSetNotePayload {
    pub note: String,
}
#[derive(Debug, serde::Deserialize)]
pub(crate) struct CaptureSelectWordPayload {
    pub word_index: u64,
}
#[derive(Debug, serde::Deserialize)]
pub(crate) struct CaptureSetTargetGroupPayload {
    pub group_id: String,
}
#[derive(Debug, serde::Deserialize)]
pub(crate) struct CaptureSetArtifactRecordPayload {
    /// serde-JSON of an `ArtifactRecord` (an already-published kind:11 book the
    /// highlight/picture references). Mirrors the audio/clip `artifact_json` pattern.
    pub artifact_json: String,
}
#[derive(Debug, serde::Deserialize)]
pub(crate) struct CaptureSetArtifactPreviewPayload {
    /// serde-JSON of an `ArtifactPreview` (a pending book — published kind:11-first
    /// on the pending-book path).
    pub preview_json: String,
}

// ── Phase 5G payload structs ─────────────────────────────────────────────────

/// `hl.blossom.upload { image_handle, servers }` — upload a locally-written
/// JPEG to the configured Blossom server(s).
///
/// `image_handle` is the temp-file path on disk. `servers` is the ordered
/// BUD-02 server list; if empty the kernel falls back to a hard-coded default
/// (`DEFAULT_BLOSSOM_SERVER`). The payload is validated in the reducer (no raw
/// image bytes cross FFI — D5 / Non-Negotiable #7).
#[derive(Debug, serde::Deserialize)]
pub(crate) struct BlossomUploadPayload {
    /// Local disk path of the image written by the iOS capture/camera pipeline.
    pub image_handle: String,
    /// BUD-02 upload server URLs. Empty list falls back to the kernel default.
    #[serde(default)]
    pub servers: Vec<String>,
}

// ── Phase 7 payload structs (append-only) ────────────────────────────────────

/// `hl.discussion.post` envelope payload — publish a kind:11 discussion thread
/// into a NIP-29 room. Fire-and-forget (D6, Non-Negotiable #3).
///
/// Tag shape (mirroring live `discussions.rs::build_event`):
///   `["h", group_id]`     — NIP-29 h-tag routing
///   `["t", "discussion"]` — discussion marker
///   `["title", title]`    — discussion title (required, non-empty)
///   `["r", attachment_url]`  — optional URL attachment
///
/// `body` is the event `content` field (may be empty). `attachment_url` adds
/// an `["r", url]` tag only when non-empty.
#[derive(Debug, serde::Deserialize)]
pub(crate) struct PostDiscussionPayload {
    /// NIP-29 local group id (the `["h", _]` routing tag value). Non-empty.
    pub group_id: String,
    /// Discussion title — required, non-empty (D6: empty → no-op).
    pub title: String,
    /// Discussion body (event `content`). May be empty.
    pub body: String,
    /// Optional URL attachment. When non-empty, adds `["r", url]` tag.
    pub attachment_url: Option<String>,
}

/// `hl.comment.post` envelope payload — post a NIP-22 kind:1111 comment.
///
/// The kernel routes `root_tag_name`/`root_tag_value`/`root_kind` to the
/// `PostCommentAction` wire shape. The anchor resolution (A/E/I selection) is
/// the caller's responsibility — Swift passes the already-resolved uppercase
/// root scope tag name and value. Fire-and-forget (D6, Non-Negotiable #3).
#[derive(Debug, serde::Deserialize)]
pub(crate) struct PostCommentPayload {
    /// Uppercase root scope tag name: `A` (addressable), `E` (event), `I` (external).
    pub root_tag_name: String,
    /// Root scope tag value (address / event-id / external-id). Must be non-empty (D6).
    pub root_tag_value: String,
    /// Root kind for the uppercase `K` tag (e.g. 30023 for articles, 1 for notes).
    pub root_kind: u32,
    /// Optional id of the kind:1111 parent comment being replied to.
    /// `None` → top-level comment (parent mirrors root).
    pub parent_event_id: Option<String>,
    /// Optional hex pubkey of the root author (for the uppercase `P` tag).
    pub root_author_pubkey: Option<String>,
    /// Optional hex pubkey of the parent comment author (for the lowercase `p` tag).
    pub parent_author_pubkey: Option<String>,
    /// Comment body text. Must be non-empty (D6).
    pub content: String,
}

/// `hl.artifact_preview.ensure` envelope payload — ensure a preview row exists
/// for the given coordinate. Idempotent. The reducer calls
/// `artifact_preview::ensure_artifact_preview` and emits the appropriate effect
/// if the coordinate is not yet resolved.
#[derive(Debug, serde::Deserialize)]
pub(crate) struct EnsureArtifactPreviewPayload {
    /// Canonical coordinate key (e.g. `"a:30023:pk:d"`, `"e:<hex>"`,
    /// `"i:isbn:<isbn13>"`, `"r:<url>"`). Must be non-empty (D6: empty → no-op).
    pub coordinate: String,
}

// ── #1653 payload structs ─────────────────────────────────────────────────────

/// `hl.curation.add_to_set` payload.
#[derive(Debug, serde::Deserialize)]
pub(crate) struct AddToSetPayload {
    pub set_coordinate: String,
    pub item_coordinate: String,
}

/// `hl.curation.remove_from_set` payload.
#[derive(Debug, serde::Deserialize)]
pub(crate) struct RemoveFromSetPayload {
    pub set_coordinate: String,
    pub item_coordinate: String,
}

/// `hl.curation.create_and_add` payload.
#[derive(Debug, serde::Deserialize)]
pub(crate) struct CreateAndAddToSetPayload {
    pub title: String,
    pub item_coordinate: String,
}

// ── Phase 7 entity-ref payload structs (append-only) ─────────────────────────

#[derive(Debug, serde::Deserialize)]
pub(crate) struct ResolveEntityRefPayload {
    pub key: String,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct ReleaseEntityRefPayload {
    pub key: String,
}

// ── Phase 7 Part C additions (append-only) ───────────────────────────────────

/// `hl.profile.update` envelope payload — update the active account's kind:0
/// profile metadata.
///
/// All fields are optional — absent/`None` fields are NOT written to the event.
/// `Some("")` clears a field. Unknown kind:0 fields from the existing event are
/// preserved verbatim (round-trip safe). Fire-and-forget (D6, Non-Negotiable #3).
#[derive(Debug, serde::Deserialize)]
pub(crate) struct UpdateProfilePayload {
    pub display_name: Option<String>,
    pub name: Option<String>,
    pub about: Option<String>,
    pub picture_url: Option<String>,
    pub banner_url: Option<String>,
    pub website: Option<String>,
    pub nip05: Option<String>,
    pub lightning_address: Option<String>,
}

/// Every user or platform action the kernel understands.
///
/// Dispatch is fire-and-forget (`dispatch(action)` returns `()`; Non-Negotiable #3).
/// Errors never propagate back as `Result` — they surface as typed `ViewSnapshot` state.
///
/// Append-only: new variants at the bottom keep rebases mechanical.
#[derive(Debug, Clone)]
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

    // ── Phase 7 entity-ref additions (append-only) ────────────────────────────
    /// Resolve an entity ref — triggers `nmp_app_resolve_ref(namespace=1)`.
    /// Sent when `NostrRichText` renders an inline entity embed.
    ResolveEntityRef {
        key: String,
    },

    /// Release an entity ref — triggers `nmp_app_release_ref(namespace=1)`.
    /// Sent when the entity embed view disappears.
    ReleaseEntityRef {
        key: String,
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
    JoinRoom {
        /// NIP-29 local group id.
        group_id: String,
        /// Host relay WebSocket URL (opaque — D3).
        host_relay_url: String,
        /// Optional preauth invite code for closed groups.
        invite_code: Option<String>,
    },

    /// Leave a NIP-29 group by publishing a kind:9022 leave-request via
    /// `"nmp.nip29.leave"`. Fire-and-forget (D6).
    ///
    /// `group_id` is the NIP-29 local group id; `host_relay_url` is the host
    /// relay WebSocket URL (opaque — D3). `reason` is an optional human-readable
    /// reason string; empty/`None` omits the content field.
    LeaveRoom {
        /// NIP-29 local group id.
        group_id: String,
        /// Host relay WebSocket URL (opaque — D3).
        host_relay_url: String,
        /// Optional human-readable reason for leaving.
        reason: Option<String>,
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

    // ── #1653 additions (append-only) ────────────────────────────────────────
    /// Add `item_coordinate` (a NIP-33 address like `"30023:<pk>:<d>"`) to the
    /// kind:30004 curation set identified by `set_coordinate`
    /// (`"30004:<active_pk>:<d>"`).
    ///
    /// Kernel is the sole kind:30004 writer on ported screens (no live-lane
    /// double-publish). Fire-and-forget (D6, Non-Negotiable #3): the updated
    /// set event arrives back as a `BookmarkSetsUpdated` frame once the NMP
    /// relay-echo loop closes and the `SetListProjection` re-snapshots.
    ///
    /// No-op when the set cannot be found in `AppState::all_curation_sets` (D6).
    AddToSet {
        /// NIP-33 address of the curation set to modify (`"30004:<pk>:<d>"`).
        set_coordinate: String,
        /// NIP-33 address of the article to add (`"30023:<pk>:<d>"`).
        item_coordinate: String,
    },

    /// Remove `item_coordinate` from the curation set identified by `set_coordinate`.
    /// Symmetric with `AddToSet`. Kernel sole writer; fire-and-forget (D6).
    /// No-op when the set or the item is not found (D6).
    RemoveFromSet {
        /// NIP-33 address of the curation set to modify.
        set_coordinate: String,
        /// NIP-33 address of the article to remove.
        item_coordinate: String,
    },

    /// Create a brand-new kind:30004 curation set with the given `title` and
    /// immediately add `item_coordinate` as its first member. The `d_tag` is
    /// derived from `title` + the current unix timestamp so it is unique but
    /// human-readable. Kernel sole writer; fire-and-forget (D6). The new set
    /// appears in `myCurationSets` after the NMP relay-echo loop closes and
    /// `SetListProjection` re-snapshots.
    CreateAndAddToSet {
        /// Display title for the new set.
        title: String,
        /// NIP-33 address of the article to add as the first member.
        item_coordinate: String,
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
        /// Optional user note → NIP-84 `comment` tag. Empty/absent = no tag.
        /// Phase 7 (article-reader publish); mirrors build_highlight_event.
        note: Option<String>,
        /// Optional surrounding context → `context` tag. Emitted only when
        /// non-empty AND different from `content` (build_highlight_event fidelity).
        context: Option<String>,
    },

    // ── Phase 5C additions (append-only) ─────────────────────────────────────
    // Doc comment intentionally omitted — AppAction uniffi metadata is near BUF_SIZE
    // limit (5A + 5K pushed it close). Full doc is in kernel/domains/isbn.rs.
    LookupIsbn {
        isbn: String,
    },

    // Doc comment omitted — mirrors LookupIsbn pattern (AppAction uniffi metadata near BUF_SIZE).
    SetBookPickerQuery {
        query: String,
        recent_limit: u32,
        search_limit: u32,
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
    /// (confirmed on pinned nmp d16aea60, `crates/nmp-ffi/src/lib.rs:1852`).
    /// Fire-and-forget (D6, Non-Negotiable #3): search hits arrive as
    /// `KernelEvent::SearchResultsUpdated` via the typed snapshot pipeline.
    RunSearch {
        /// Plain-text search query. Empty string / whitespace → no-op (D6).
        query: String,
        /// Which NIP-50 scope to search in.
        scope: SearchScope,
    },

    /// Classify one omnibox / paste / search input through NMP's input-intent
    /// resolver (`#1865`) and route it.
    ///
    /// The reducer emits `Effect::RunOmnibox`; the effect runner calls
    /// `nmp_app_intent_classify` and, per the classification, either navigates
    /// (pasted ref), enqueues a NIP-05 reverse lookup, opens a NIP-29 group,
    /// runs a multi-kind free-text relay search, or safe-rejects an `nsec`. The
    /// resolved `OmniboxOutcome` arrives back as `KernelEvent::OmniboxResolved`
    /// and is surfaced in `SearchSnapshot::omnibox`.
    ///
    /// Empty / whitespace-only `query` → no-op (D6). Fire-and-forget (D6).
    RunOmnibox {
        /// Raw, untrusted omnibox input (may be a secret — never echoed).
        query: String,
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

    // ── Phase 5H additions (append-only) ─────────────────────────────────────
    /// Load and play a podcast episode.
    ///
    /// The kernel looks up the saved resume position from its in-memory
    /// `PodcastPositionStore` cache (DEVICE-LOCAL — never published to nostr),
    /// emits `CapabilityRequest::Audio(AudioOp::Load { url, resume_at_seconds })`
    /// to the native AVPlayer, and updates `AppState::podcast.current`.
    ///
    /// `artifact_json` is the serde-JSON of `ArtifactRecord` (avoids a nested
    /// uniffi::Record in the envelope shape). The kernel decodes it; on any
    /// parse error the action is a silent no-op (D6).
    ///
    /// Fire-and-forget (D6, Non-Negotiable #3).
    AudioPlay {
        /// HTTP(S) audio URL. Opaque from the caller (D3).
        url: String,
        /// Podcast item GUID — used as the resume-position key.
        guid: String,
        /// serde-JSON of the `ArtifactRecord` for position persistence.
        artifact_json: String,
    },
    /// Pause the native audio player.
    ///
    /// Emits `CapabilityRequest::Audio(AudioOp::Pause)` and updates
    /// `AppState::podcast.current.is_playing = false`.
    /// No-op when no episode is loaded. Fire-and-forget (D6).
    AudioPause,
    /// Seek the native audio player to `seconds`.
    ///
    /// The reducer clamps `seconds` to `[0, duration]` via `seek_projection`
    /// before emitting `CapabilityRequest::Audio(AudioOp::Seek { seconds })`.
    /// No-op when no episode is loaded. Fire-and-forget (D6).
    AudioSeek {
        /// Requested seek position (seconds; may be out of bounds — clamped).
        seconds: f64,
    },
    /// Persist the current resume position immediately (e.g. on app-background).
    ///
    /// Emits `Effect::SavePodcastPosition`. No capability request.
    /// No-op when no episode is loaded or the position is invalid.
    /// DEVICE-LOCAL — never a nostr event (`hl-app-state-vs-nostr-facts`).
    AudioSetResume {
        /// Resume position in seconds (finite, ≥ 0).
        seconds: f64,
    },

    // ── Phase 7 Part C additions (append-only) ─────────────────────────────
    /// Update the active account's kind:0 profile metadata.
    ///
    /// Rust preserves unknown kind:0 fields from the existing event (round-trip
    /// safe). Signs and publishes a new kind:0 replaceable event. Only non-None
    /// fields are written; `Some("")` clears a field.
    /// Fire-and-forget (D6, Non-Negotiable #3).
    UpdateProfile {
        display_name: Option<String>,
        name: Option<String>,
        about: Option<String>,
        picture_url: Option<String>,
        banner_url: Option<String>,
        website: Option<String>,
        nip05: Option<String>,
        lightning_address: Option<String>,
    },

    // ── Phase 7 Part C additions (append-only) ───────────────────────────────
    /// Signal from the iOS NWPathMonitor that the network path changed.
    ///
    /// `is_wifi` is true when the current path is `.satisfied` AND uses a Wi-Fi
    /// interface. `wifi_only` mirrors `UserDefaults["hl.network.wifi_only"]` —
    /// the kernel does not own this preference; Swift reads it and passes it in.
    ///
    /// The effect runner disconnects relay sockets when `wifi_only && !is_wifi`
    /// and reconnects when `wifi_only && is_wifi`. Fire-and-forget (D6).
    ApplyNetworkPath {
        is_wifi: bool,
        wifi_only: bool,
    },
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
    /// kind:30023 articles + kind:9802 highlights in one query — backs the
    /// unified search screen, which renders Articles and Highlights sections
    /// from a single query (Swift buckets the mixed hits by kind). Phase 7.
    ArticlesAndHighlights,
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

    // ── #1653 additions (append-only) ────────────────────────────────────────
    /// The `"hl.bookmark_sets"` typed sidecar was decoded from an NMP snapshot
    /// frame. Carries all observed kind:30003 and kind:30004 set rows (unfiltered
    /// by active pubkey — filtering happens at apply time using `AppState::follows`
    /// and the active session). Produced by
    /// `projections::dispatch_typed_frame` (schema_id `"hl.bookmark_sets"`).
    /// Also directly injectable from tests via `Cmd::Event` (no live NmpApp).
    ///
    /// D1: raw fields only — no "Untitled" fallbacks, no formatted strings.
    BookmarkSetsUpdated {
        /// All kind:30003 bookmark-set rows observed this session (any author).
        all_bookmark_sets: Vec<crate::kernel::snapshot::BookmarkSetRow>,
        /// All kind:30004 curation-set rows observed this session (any author).
        all_curation_sets: Vec<crate::kernel::snapshot::BookmarkSetRow>,
    },

    /// The `"hl.web_bookmarks"` typed sidecar was decoded. Carries all
    /// kind:39701 web-bookmark rows for the active account. Also injectable
    /// from tests via `Cmd::Event`. D1: raw fields only.
    WebBookmarksUpdated(Vec<crate::kernel::snapshot::WebBookmarkRow>),

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
        /// The active viewer's own kind:7 reaction event id for this target, if
        /// any. Kernel-INTERNAL only (this is `KernelEvent`, not an FFI type) —
        /// stored in `AppState::viewer_reaction_ids` so `hl.reaction.toggle` can
        /// emit the unreact effect without ever surfacing the id across FFI.
        viewer_reaction_event_id: Option<String>,
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

    /// The omnibox resolver (`#1865`) classified an input.
    ///
    /// Produced by `omnibox::run_effect_run_omnibox` after calling
    /// `nmp_app_intent_classify` and performing the branch side effect. The
    /// reducer stores the outcome in `AppState::omnibox_outcome`; it is surfaced
    /// in `SearchSnapshot::omnibox` for the shell to route on. Carries no copy of
    /// a rejected secret (`OmniboxOutcome::RejectSecret` is fieldless).
    OmniboxResolved(crate::kernel::snapshot::OmniboxOutcome),

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

    /// Scan of the NMP event store for book records completed.
    ///
    /// Produced by `run_effect_scan_book_picker_recents` and stored in
    /// `AppState::isbn.recents` + `search_results`.
    BookPickerRecentsLoaded {
        recents: Vec<crate::kernel::models::ArtifactRecord>,
        search_results: Vec<crate::kernel::models::ArtifactRecord>,
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

    // ── Phase 5H additions (append-only) ─────────────────────────────────────
    /// A raw `AudioResult` arrived via `CapabilityResult::Audio`.
    ///
    /// Produced by `reduce_event` when it decodes a `CapabilityResult::Audio`
    /// result and routes it to `podcast::reduce_capability_audio`. Carried as
    /// a typed enum rather than raw bytes so the podcast reducer can match
    /// without additional parsing.
    ///
    /// `AudioResult::Progress` is coalesced inside `reduce_capability_audio`
    /// to at most one kernel state update per second (D8 — not per 0.25 s).
    AudioCapabilityResult(crate::capabilities::AudioResult),
    /// A saved resume position was loaded from `PodcastPositionStore`.
    ///
    /// Injected by the effect runner (`Effect::LoadPodcastPosition`) when
    /// `AppAction::AudioPlay` is dispatched and the position store has an entry
    /// for the guid. The reducer caches the position in `AppState::podcast`
    /// so `reduce_action_play` can include it in the `Load` op.
    ///
    /// DEVICE-LOCAL — never published to nostr.
    PodcastPositionLoaded {
        /// Podcast item GUID.
        guid: String,
        /// Saved position in seconds.
        position_seconds: f64,
    },

    // ── Phase 5D additions (append-only) ─────────────────────────────────────
    /// OCR capability result processed: raw lines reconstructed into markdown.
    /// Device-local — never a nostr fact.
    ///
    /// This variant exists for test injection (bypasses the capability round-trip).
    /// In the live path, `CapabilityResult::Ocr(OcrResult::Lines(_))` is handled
    /// by `session::reduce_event_capability_result` which calls
    /// `ocr::reduce_event_ocr_result` directly. The `KernelEvent::OcrRecognitionComplete`
    /// arm in `reduce_event` is therefore a test-only path (same pattern as
    /// `KernelEvent::ShareQueueDrained`).
    OcrRecognitionComplete {
        image_handle: String,
        markdown: String,
        selectable_words: Vec<crate::capabilities::ocr::OcrWord>,
        raw_lines: Vec<crate::capabilities::ocr::OcrLine>,
    },

    // ── Phase 5F additions (append-only) ─────────────────────────────────────
    /// A capture-draft publish round-trip completed.
    ///
    /// Produced by the publish runner after `Effect::PublishHighlightEvent`
    /// (kind:9802) or `Effect::PublishCaptureEvent` (kind:11) is broadcast.
    /// `success` → phase `Done`; otherwise phase `Error { message: error }`.
    /// Injectable directly from tests via `Cmd::Event` (no live NmpApp needed).
    CaptureDraftPublishResult {
        /// `true` when the event was accepted by the relay(s).
        success: bool,
        /// Raw event id of the published event (empty on failure). D1.
        event_id: String,
        /// Raw error message (empty on success). D1.
        error: String,
    },

    // ── Phase 5G additions (append-only) ─────────────────────────────────────
    /// nmp returned a dispatch correlation_id for a Blossom upload that differs
    /// from the placeholder the reducer minted. Sent by `run_effect_blossom_upload`
    /// after `nmp_app_dispatch_action` returns. The actor swaps the placeholder
    /// out of `pending_upload_correlation_ids` and inserts the real nmp id so
    /// `apply_action_result_row` can match the arriving `action_results` row.
    NmpBlossomCorrelationMinted {
        /// The reducer-minted placeholder id to remove from the set.
        placeholder_correlation_id: String,
        /// The real id nmp assigned to the upload (from the dispatch return JSON).
        nmp_correlation_id: String,
    },

    /// nmp returned a dispatch correlation_id for a share-mint invite publish
    /// (`nmp.nip29.create_invite`) that differs from the placeholder the reducer
    /// minted. Sent by `run_effect_dispatch_create_invite_with_correlation` after
    /// `nmp_app_dispatch_action` returns. The actor swaps the placeholder in
    /// `share_publish.pending_correlation_id` for the real nmp id so
    /// `apply_action_result_row` can match the arriving `action_results` row and
    /// drive the share-mint FSM → Done / Error (#21 finding 3).
    SharePublishCorrelationMinted {
        /// The reducer-minted placeholder id to replace.
        placeholder_correlation_id: String,
        /// The real id nmp assigned to the create-invite publish.
        nmp_correlation_id: String,
    },

    /// nmp REJECTED a share-publish / invite-mint dispatch at validation time
    /// (`{"error":..}` from `nmp_app_dispatch_action` — e.g. a reserved kind, or
    /// a null/failed dispatch). Sent by `dispatch_share_publish_action`. Drives
    /// the share FSM → Error (D6) keyed on the in-flight placeholder correlation
    /// id (the publish never reached an `action_results` terminal). #21 finding 1.
    ShareMintDispatchRejected {
        /// The in-flight placeholder correlation id (matched against the FSM).
        correlation_id: String,
        /// Raw nmp error envelope / message. D1.
        error: String,
    },

    /// A Blossom upload action result arrived via the `"action_results"` typed
    /// projection. Routed from `projections::dispatch_typed_frame` by matching
    /// `correlation_id` against
    /// `AppState::capture_draft.pending_upload_correlation_id`.
    ///
    /// On success: sets `has_upload = true` + stores `blob_url` on the capture
    /// draft, unlocking the kind:11 publish path. On failure: clears the pending
    /// upload state so a retry is possible. DEVICE-LOCAL — never a nostr fact.
    BlossomUploadResult {
        /// `true` when nmp reports a `"success"` status.
        success: bool,
        /// The canonical Blossom blob URL on success; empty on failure. D1.
        blob_url: String,
        /// Raw error message on failure; empty on success. D1.
        error: String,
    },

    /// A capture-draft PUBLISH action result arrived via `"action_results"`.
    /// Routed by matching `correlation_id` against
    /// `AppState::capture_draft.pending_publish_correlation_id`.
    ///
    /// Drives `CaptureDraftPhase::Publishing → Done | Error` for REAL (closing
    /// the loop that 5F left open with a clock-timeout fallback). The
    /// `KernelEvent::CaptureDraftPublishResult` variant remains available for
    /// direct test injection; this new variant is the live-lane path.
    CapturePublishActionResult {
        /// `true` when nmp reports a success status.
        success: bool,
        /// Raw error message on failure; empty on success. D1.
        error: String,
    },

    // ── Phase 5I additions (append-only) ─────────────────────────────────────
    /// Transcript fetch+parse completed successfully.
    ///
    /// Injected by `run_effect_fetch_transcript` after HTTP fetch and format
    /// detection/parsing. Routed to `podcast::reduce_event_transcript_ready`.
    /// DEVICE-LOCAL — never a nostr fact.
    TranscriptReady {
        segments: Vec<crate::kernel::domains::podcast::TranscriptSegment>,
    },
    /// Transcript fetch or parse failed. D6 — availability set to Unavailable.
    TranscriptFetchFailed,

    // ── Phase 5E additions (append-only) ─────────────────────────────────────
    /// A raw `CameraResult` arrived via `CapabilityResult::Camera`.
    ///
    /// This variant exists for test injection (bypasses the capability round-trip).
    /// In the live path, `CapabilityResult::Camera(_)` is handled by
    /// `session::reduce_event_capability_result` which calls
    /// `camera::reduce_capability_camera` directly. The `KernelEvent::CameraCapabilityResult`
    /// arm in `reduce_event` is therefore a test-only path (same pattern as
    /// `KernelEvent::OcrRecognitionComplete` and `KernelEvent::ShareQueueDrained`).
    ///
    /// DEVICE-LOCAL — PageImage routes image_handle into the 5D OCR pipeline;
    /// Barcode routes to the 5C ISBN lookup. Neither publishes to nostr here.
    CameraCapabilityResult(crate::capabilities::CameraResult),

    // ── Phase 5J additions (append-only) ─────────────────────────────────────
    /// A podcast-clip PUBLISH action result arrived via the `"action_results"`
    /// typed projection. Routed by matching `correlation_id` against
    /// `AppState::podcast.pending_clip_publish_correlation_id`.
    ///
    /// Drives `PodcastClipPublishPhase::Publishing → Done | Error` for REAL,
    /// closing the loop with the 5G correlation-aware path. Injectable from
    /// tests via `Cmd::Event` (no live NmpApp needed).
    ///
    /// DEVICE-LOCAL — only the published kind:9802 is the nostr fact.
    ClipPublishActionResult {
        /// `true` when nmp reports a `"published"` status.
        success: bool,
        /// Raw error message on failure; empty on success. D1.
        error: String,
    },

    // ── Phase 7 chat additions (append-only) ─────────────────────────────────
    /// A NIP-29 kind:9 chat message was ingested by the `ChatObserver`
    /// (wrapping `GroupChatProjection`) for an open room. Carries the updated
    /// message list (newest-first, bounded) for the affected `group_id`.
    ///
    /// Produced by `ChatObserver::on_kernel_event` after delegating ingest to the
    /// projection, recovering `reply_to_event_id` from raw tags, and snapshotting.
    /// Also injectable directly from tests via `Cmd::Event` (no live NmpApp needed).
    ///
    /// D1: `ChatMessageRawRow` carries raw protocol data only — no formatted strings.
    /// Keyed by `group_id` (NIP-29 local id) in `AppState::chat_rooms`.
    ChatRoomUpdated {
        /// NIP-29 local group id.
        group_id: String,
        /// Updated message list (newest-first, bounded by `MAX_PROJECTION_MESSAGES`).
        messages: Vec<crate::kernel::snapshot::ChatMessageRawRow>,
    },

    // ── Phase 7 additions (append-only) ─────────────────────────────────────
    /// A NIP-22 kind:1111 comment was ingested by the `CommentObserver`
    /// (wrapping `CommentThreadProjection`). Carries the full thread snapshot
    /// for the affected root, ready to be stored in `AppState::comment_threads`.
    ///
    /// Produced by `CommentObserver::on_kernel_event` after delegating ingest
    /// to the projection and calling `snapshot_for(root_tag_value)`. Also
    /// injectable directly from tests via `Cmd::Event` (no live NmpApp needed).
    ///
    /// D1: `CommentThreadSnapshot` is raw protocol data only — no formatted
    /// strings. Swift formats all display labels (timestamps, author bylines).
    /// Keyed by `root_tag_value` (the UPPERCASE root scope value from the
    /// NIP-22 `E`/`A`/`I` tag) in `AppState::comment_threads`.
    CommentThreadUpdated {
        /// Root scope tag value (the `E`/`A`/`I` tag value from NIP-22).
        root_tag_value: String,
        /// Latest comment thread snapshot for this root (all comments + tree).
        snapshot: nmp_nip22::CommentThreadSnapshot,
    },

    // ── Phase 7 discussions additions (append-only) ───────────────────────────
    /// kind:11 discussion rows for a room changed.
    ///
    /// Produced by `DiscussionObserver::on_kernel_event` when it filters a
    /// kind:11+discussion event from the `GroupEventsProjection` for the room
    /// and rebuilds the bounded snapshot. Also injectable directly from tests
    /// via `Cmd::Event` (no live NmpApp needed — same pattern as
    /// `KernelEvent::CommentThreadUpdated`).
    ///
    /// D1: `rows` carry raw protocol data only — no formatted strings.
    /// Keyed by `group_id` in `AppState::room_discussions`.
    RoomDiscussionsUpdated {
        /// NIP-29 local group id (the `["h", _]` tag value).
        group_id: String,
        /// Fresh bounded snapshot of kind:11+discussion rows for this room,
        /// newest-first, at most `ROOM_DISCUSSIONS_CAP` (64) items.
        rows: Vec<crate::kernel::snapshot::DiscussionRow>,
    },

    // ── Phase 7 artifact-preview additions (append-only) ─────────────────────
    /// A coordinate's artifact-preview row was resolved and is ready to fill.
    ///
    /// Produced by the `ResolveArtifactCoordinate` effect runner after it fetches
    /// the underlying event (article-address interest, event-id fetch, or NIP-73
    /// tagged-event interest) and extracts the relevant metadata fields.
    ///
    /// Also injectable directly from tests via `Cmd::Event` (no live NmpApp needed).
    ///
    /// D1: all fields are raw protocol data — no formatted strings, no presentation
    /// fallbacks. The reducer routes to
    /// `artifact_preview::fill_from_artifact_event` which upserts the row and
    /// wires the `e:` alias.
    ArtifactPreviewFilled {
        /// Canonical coordinate key that was resolved (e.g. `"e:<hex>"`,
        /// `"a:30023:pk:d"`, `"i:podcast:item:guid:<guid>"`).
        coordinate: String,
        /// 64-char hex event id of the source event (if available). Used to
        /// install an `e:` alias in `AppState::artifact_previews`.
        event_id: String,
        /// Title tag value from the source event. `None` when absent.
        title: Option<String>,
        /// Cover image URL. `None` when absent.
        image_url: Option<String>,
        /// Author hex pubkey. `None` for non-nostr content.
        author_pubkey: Option<String>,
        /// Summary / description. `None` when absent.
        summary: Option<String>,
    },

    // ── Phase 7 (#1697 gate) additions (append-only) ─────────────────────────
    /// A local kind:0 scan of the kernel-owned `EventStore` completed for the
    /// active search query.
    ///
    /// Produced by the `Effect::RunSearch` runner (`run_effect_run_search`),
    /// which scans the published `EventStore` for kind:0 events via
    /// `EventStore::query(StoreQuery::KindTime { kinds: [0], … })` and decodes each into a
    /// `ProfileSearchRow`. The reducer upserts these into
    /// `AppState::profile_search_cache` (dedup by pubkey, newest wins). This is
    /// the SOLE production driver of the search profiles bucket — relay NIP-50
    /// search runs the articles/highlights scope (no kind:0), so the local store
    /// scan is what populates the people results (replacing the bespoke
    /// `crate::search::search_profiles` nostrdb scan — D4 single source).
    ///
    /// Raw protocol rows only (D1). Bounded by the scan limit
    /// (`PROFILE_SEARCH_CACHE_SCAN_LIMIT`) — never unbounded (Non-Negotiable #7).
    ///
    /// `generation` is captured from `AppState::profile_search_generation` at
    /// the moment `Effect::RunSearch` is dispatched. The reducer drops this event
    /// when `generation != state.profile_search_generation` (stale scan — D5).
    ProfileSearchScanned {
        generation: u64,
        rows: Vec<crate::kernel::snapshot::ProfileSearchRow>,
    },
}
