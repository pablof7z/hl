//! Effect types — what the actor asks the effect runner to do after a
//! reduce pass. Effects are idempotent and cancellable by view/session-epoch
//! (plan line 157; Non-Negotiable #4).

use crate::capabilities::CapabilityRequest;

/// An instruction from the reducer to the async effect runner.
///
/// Effects are pure data — the reducer never `.await`s anything
/// (Non-Negotiable #2 / plan line 156). The actor's tokio side executes
/// each effect and feeds results back as `KernelEvent`s.
///
/// Append-only: new variants at the bottom keep rebases mechanical.
#[derive(Debug, Clone)]
pub enum Effect {
    /// Read `OnboardingStore::is_complete()` and feed back
    /// `KernelEvent::OnboardingStateLoaded(bool)`.
    LoadOnboardingFlag,
    /// Ask the native shell for the persisted session secret by emitting
    /// `CapabilityRequest::Keychain(KeychainOp::LoadSession)`.
    RestoreSessionSecret,
    /// Ask the native shell to delete the persisted session secret.
    ClearSession,
    /// Forward a capability request to the registered observer.
    EmitCapabilityRequest(CapabilityRequest),

    // ── Phase 2A additions (append-only) ─────────────────────────────────────
    /// Call `nmp.add_signer(LocalNsec(nsec), make_active: true)`.
    ///
    /// NMP auto-persists the nsec to its keyring when `make_active` is true
    /// and the source is `LocalNsec` — hl does NOT separately store the nsec
    /// after this call. Success is signalled by the identity-change observer
    /// firing `KernelEvent::IdentityChanged(Some(pubkey))`. Errors are fed
    /// back as `KernelEvent::SignInFailed` (never as a `Result`).
    AddNsecSigner { nsec: String },

    /// Read the active pubkey from `nmp.active_account_handle()` then call
    /// `nmp.remove_account(pubkey)`. Fire-and-forget — success is observed
    /// via `KernelEvent::IdentityChanged(None)`.
    RemoveActiveAccount,

    // ── Phase 2B additions (append-only) ─────────────────────────────────────
    /// Call `nmp.add_signer(BunkerUri(uri), make_active: true)` which routes
    /// through the NIP-46 broker state machine. Requires
    /// `nmp_signer_broker_init` to have run at boot. Fire-and-forget: the
    /// broker resolves the signer async; success arrives as
    /// `KernelEvent::IdentityChanged(Some(pubkey))`.
    AddBunkerSigner { uri: String },
    /// Call `nmp_app_nostrconnect_uri` to mint a fresh `nostrconnect://` URI.
    /// The raw URI is fed back as `KernelEvent::NostrConnectUriReady` so the
    /// snapshot can expose it to the iOS QR sheet. The broker then awaits the
    /// remote signer to connect; success arrives as `IdentityChanged(Some)`.
    MintNostrConnectUri,
    /// Call `nmp_app_signin_nip55` to begin a NIP-55 external-signer sign-in.
    /// Fire-and-forget: the host capability bridge exchanges with the signer
    /// app async; success arrives as `KernelEvent::IdentityChanged(Some)`.
    StartNip55SignIn,

    // ── Phase 2C additions (append-only) ─────────────────────────────────────
    /// Call `nmp.actor_sender().send(ActorCommand::CreateAccount{...})`.
    ///
    /// Profile metadata, relays, and initial_follows come from the kernel's
    /// injected `KernelPolicy` — never from hardcoded literals (D3). Bootstrap
    /// publish semantics follow ADR-0059: kind:0 and kind:10002 are published;
    /// kind:3 is skipped when `initial_follows` is empty. Fire-and-forget:
    /// success arrives via `KernelEvent::IdentityChanged(Some(pubkey))`.
    /// The 2A clock-driven timeout (SIGN_IN_TIMEOUT_SECS) covers SigningIn.
    CreateAccount {
        /// Display name for the fresh account's kind:0 profile.
        profile_name: String,
    },

    // ── Phase 2D additions (append-only) ─────────────────────────────────────
    /// Call `nmp.actor_sender().send(ActorCommand::AddRelay { url, role })`.
    ///
    /// `role` is the canonical wire string produced by `RelayRole::normalize()`
    /// (e.g. `"both,indexer"`). Fire-and-forget: nmp updates the active
    /// account's kind:10002 relay list asynchronously. D3: no wss-scheme literals
    /// in the kernel — the URL comes from the caller, the role from the
    /// normalized `RelayRole` variant.
    AddRelay { url: String, role: String },
    /// Call `nmp.actor_sender().send(ActorCommand::RemoveRelay { url })`.
    ///
    /// Fire-and-forget: nmp removes the relay from the active account's
    /// kind:10002 list. D6: no-op if the relay is not present.
    RemoveRelay { url: String },
    /// Change role: equivalent to a `RemoveRelay` followed by `AddRelay` in
    /// nmp's T66a relay-edit model. Implemented by the effect runner as a
    /// single `ActorCommand::AddRelay` with the new role (nmp upserts).
    /// Fire-and-forget. D3: no wss-scheme literals.
    SetRelayRole { url: String, role: String },
    /// Sign and publish a kind:30078 app-data event via
    /// `ActorCommand::PublishRawEvent`.
    ///
    /// Used to persist the hl rooms relay list under the hl-owned d-tag
    /// `"com.highlighter.relays"`. The kernel builds the JSON `content` and
    /// the `["d", "com.highlighter.relays"]` tag; the active signer signs it
    /// through nmp's standard publish path. Fire-and-forget: nmp handles relay
    /// routing via `PublishTarget::Auto` (NIP-65 outbox; D3). No wss-scheme
    /// literals in the kernel — relay URLs are embedded inside `content` only.
    PublishRoomsRelayList {
        /// JSON-serialized rooms relay list to embed in the event content.
        content: String,
    },

    // ── Phase 3B additions (append-only) ─────────────────────────────────────
    /// Call `nmp_nip29::register::wire_joined_groups(nmp_ref, pubkey, "")`.
    ///
    /// Registers (or re-registers) the `JoinedGroupsProjection` event observer
    /// and typed snapshot closure under `"nmp.nip29.joined_groups"`. Must be
    /// emitted at boot (via `start_nmp_app`) and on every
    /// `IdentityChanged(Some(pubkey))` so the projection follows account switches.
    /// Fire-and-forget: the snapshot update arrives via the NMP update callback
    /// as `KernelEvent::NmpSnapshotFrame` on the next projection tick.
    WireJoinedGroups {
        /// Hex pubkey of the account whose joined groups to project.
        pubkey: String,
    },

    // ── Phase 3C additions (append-only) ─────────────────────────────────────
    /// Call `nmp_app_dispatch_action` with `"nmp.follow"` or `"nmp.unfollow"`
    /// namespace and `{"pubkey":"<hex>"}` JSON. Fire-and-forget (D6, Non-
    /// Negotiable #3): the updated follow list arrives back through the
    /// `FollowListUpdated` projection event (via the NMP update callback).
    ///
    /// The `nmp.follow` / `nmp.unfollow` action namespaces (via
    /// `nmp_nip02::FollowModule` / `UnfollowModule`) enqueue
    /// `ActorCommand::Follow` / `Unfollow` on the nmp actor thread which
    /// rebuilds + re-publishes kind:3.
    DispatchFollowAction {
        /// `true` → `"nmp.follow"` namespace; `false` → `"nmp.unfollow"`.
        follow: bool,
        /// Raw 64-char lowercase hex pubkey to follow or unfollow.
        pubkey: String,
    },

    // ── Phase 3E additions (append-only) ─────────────────────────────────────
    /// Call `nmp_app_dispatch_action` with the given namespace and JSON payload.
    ///
    /// Used for NIP-29 write/subscribe actions (discover, join, create, etc.).
    /// Fire-and-forget (D6): the returned correlation_id JSON is freed and
    /// discarded. Results arrive via the relevant `KernelEvent::*Updated` event.
    DispatchNip29Action {
        /// NIP-29 action namespace (e.g. `"nmp.nip29.discover"`).
        namespace: String,
        /// JSON payload for the action (e.g. `{"relay_url":"..."}`).
        json: String,
    },
    /// Wire the `DiscoveredGroupsProjection` event observer + typed snapshot
    /// projection for `relay_url` into the live `NmpApp`.
    ///
    /// Called when `AppAction::StartRoomDiscovery` is dispatched. Registers the
    /// observer that accumulates kind:39000/39001/39002 events from the relay.
    /// Fire-and-forget: the snapshot arrives via the NMP update callback as
    /// `KernelEvent::NmpSnapshotFrame` on the next projection tick.
    WireGroupDiscovery {
        /// The discovery relay URL (opaque string; kernel never constructs URLs, D3).
        relay_url: String,
    },

    // ── Phase 3D additions (append-only) ─────────────────────────────────────
    /// Call `nmp_app_claim_profile(raw_ptr, pubkey, "hl.profile.<pubkey>",
    /// force:0, liveness:Live)`.
    ///
    /// Sent when the UI opens a `ViewId::Profile{pubkey}` view (triggered by
    /// `AppAction::ClaimProfile`). `Live` liveness (`c_int = 1`) keeps a
    /// `Tailing` kind:0 subscription open so profile edits arrive reactively
    /// while the view is on screen. The updated card arrives back through the
    /// `"claimed_profiles"` typed sidecar as `KernelEvent::ProfileCardUpdated`.
    /// Fire-and-forget (D6, Non-Negotiable #3): nmp handles the claim async.
    ClaimProfile {
        /// Raw 64-char lowercase hex pubkey to claim.
        pubkey: String,
    },

    /// Call `nmp_app_release_profile(raw_ptr, pubkey, "hl.profile.<pubkey>")`.
    ///
    /// Sent when the UI closes a `ViewId::Profile{pubkey}` view (triggered by
    /// `AppAction::ReleaseProfile`). Decrements the per-consumer refcount;
    /// when zero, NMP cancels the Tailing kind:0 subscription and removes the
    /// card from `claimed_profiles`. Fire-and-forget (D6).
    ReleaseProfile {
        /// Raw 64-char lowercase hex pubkey to release.
        pubkey: String,
    },
}
