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
}
