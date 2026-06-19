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
}
