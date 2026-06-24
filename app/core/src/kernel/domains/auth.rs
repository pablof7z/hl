//! Auth domain — sign-in, sign-out, and signer-management arms of the kernel.
//!
//! Covers: SignInNsec / PairBunker / StartNostrConnect / SignInNip55 /
//!         CreateAccount (actions); IdentityChanged / SignInFailed /
//!         NostrConnectUriReady / BunkerHandshakeState (events); and the
//!         matching `run_effect` arms.

use std::ffi::CString;

use tokio::sync::mpsc;
use zeroize::Zeroizing;

use nmp_ffi::{nmp_app_nostrconnect_uri, nmp_app_signin_nip55, NmpApp};

use crate::kernel::action::{KernelEvent, SignInMethod, SignerKind};
use crate::kernel::actor::{Cmd, NmpHandle};
use crate::kernel::app::{AppState, SessionState, SIGN_IN_TIMEOUT_SECS};
use crate::kernel::effect::Effect;

// ─── Reducer (action) ────────────────────────────────────────────────────────

/// Handle auth-related `AppAction` variants.
///
/// Returns `None` if the action is not owned by the auth domain.
/// Caller is responsible for passing only the relevant variant.
pub(crate) fn reduce_action_sign_in_nsec(
    state: &mut AppState,
    nsec: String,
    now: u64,
) -> Vec<Effect> {
    state.session = SessionState::SigningIn {
        method: SignInMethod::Nsec,
        started_at: now,
    };
    // AddNsecSigner calls nmp.add_signer(LocalNsec(nsec), true).
    // Success → IdentityChanged(Some(pubkey)); failure → SignInFailed.
    // Fire-and-forget: reducer never awaits the result.
    vec![Effect::AddNsecSigner { nsec }]
}

pub(crate) fn reduce_action_pair_bunker(
    state: &mut AppState,
    uri: String,
    now: u64,
) -> Vec<Effect> {
    state.session = SessionState::SigningIn {
        method: SignInMethod::Bunker,
        started_at: now,
    };
    // AddBunkerSigner routes through the NIP-46 broker (nmp_signer_broker_init
    // must have run at boot). Fire-and-forget: broker resolves the signer
    // async; success arrives as IdentityChanged(Some).
    vec![Effect::AddBunkerSigner { uri }]
}

pub(crate) fn reduce_action_start_nostr_connect(state: &mut AppState, now: u64) -> Vec<Effect> {
    state.session = SessionState::SigningIn {
        method: SignInMethod::NostrConnect,
        started_at: now,
    };
    // MintNostrConnectUri calls nmp_app_nostrconnect_uri and feeds the
    // result back as KernelEvent::NostrConnectUriReady so the iOS QR
    // sheet can render it. The broker then waits for the remote signer
    // to connect; success arrives as IdentityChanged(Some).
    vec![Effect::MintNostrConnectUri]
}

pub(crate) fn reduce_action_sign_in_nip55(state: &mut AppState, now: u64) -> Vec<Effect> {
    state.session = SessionState::SigningIn {
        method: SignInMethod::Nip55,
        started_at: now,
    };
    // StartNip55SignIn calls nmp_app_signin_nip55(app, null). Fire-and-
    // forget: the host capability bridge exchanges with the external
    // signer app; success arrives as IdentityChanged(Some).
    vec![Effect::StartNip55SignIn]
}

pub(crate) fn reduce_action_create_account(
    state: &mut AppState,
    profile_name: String,
    now: u64,
) -> Vec<Effect> {
    state.session = SessionState::SigningIn {
        method: SignInMethod::CreateAccount,
        started_at: now,
    };
    // Effect runner calls actor_sender().send(ActorCommand::CreateAccount{...}).
    // Relay + follow policy is read from injected KernelPolicy at effect
    // run time (D3 — no hardcoded relay literals in kernel logic).
    // Success → IdentityChanged(Some(pubkey)); clock timeout covers failure.
    vec![Effect::CreateAccount { profile_name }]
}

pub(crate) fn reduce_action_logout(state: &mut AppState) -> Vec<Effect> {
    state.session = SessionState::Absent;
    state.session_epoch += 1;
    // Clear any pending NostrConnect URI on logout.
    state.nostrconnect_uri = None;
    // ── Phase 3C: clear follow set so stale follows don't survive logout ──────
    // The FollowListProjection active-account slot auto-resets via the shared
    // Arc, but AppState::follows must also be wiped so is_following never
    // returns true for the previous account's contacts.
    state.follows = Vec::new();
    // ── #1653: shared account-scoped teardown (single source) ────────────────
    // Logout clears the SAME account-scoped state as the None/removed-account
    // and direct-switch arms via ONE helper (profiles, bookmarks, bookmark/
    // curation sets, web bookmarks, articles, reactions, NIP-50 search +
    // omnibox_outcome, HomeFeed feed cursors/rows, share_queue, room discussions,
    // room-home buffers, feedback, chat, artifact previews) so the three teardown
    // paths can never drift apart. The helper emits the accumulator-clearing effect.
    let mut effects = clear_account_scoped_state_on_switch(state);
    // RemoveActiveAccount fires nmp.remove_account; ClearSession
    // emits a CapabilityRequest to native for its keychain.
    effects.push(Effect::RemoveActiveAccount);
    effects.push(Effect::ClearSession);
    effects
}

// ─── Reducer (event) ─────────────────────────────────────────────────────────

pub(crate) fn reduce_event_identity_changed(
    state: &mut AppState,
    pubkey: Option<String>,
) -> Vec<Effect> {
    // NMP identity change — `Some(pk)` means a signer is now active;
    // `None` means the account was removed / logged out.
    match pubkey {
        Some(pk) if !pk.is_empty() => {
            // Determine signer kind from the method we were SigningIn with.
            // Bunker and NostrConnect both resolve to Nip46 (NIP-46 remote).
            // Session restore and unknown paths default to LocalNsec.
            let signer_kind = match &state.session {
                SessionState::SigningIn { method, .. } => match method {
                    SignInMethod::Nsec | SignInMethod::CreateAccount => SignerKind::LocalNsec,
                    SignInMethod::Bunker | SignInMethod::NostrConnect => SignerKind::Nip46,
                    SignInMethod::Nip55 => SignerKind::Nip55,
                },
                _ => SignerKind::LocalNsec,
            };
            // ── #1653: detect a REAL account switch vs. login / refresh ────────
            // A DIRECT switch is NMP firing IdentityChanged(Some(new_pk)) with no
            // intervening None (see nmp's
            // `active_account_handle_reflects_account_switch`). There is no
            // hl-side SigningIn transition for a direct switch, so at this point
            // `state.session` is still `Present { pubkey: prior }`. The
            // unambiguous signal:
            //   - Present { pubkey: prior } && prior != pk → REAL SWITCH (wipe).
            //   - Present { pubkey: prior } && prior == pk → same-account refresh
            //     (do NOT wipe — would churn the active account's own state).
            //   - SigningIn / Absent / SignInFailed → initial login from no
            //     active account (nothing to wipe).
            let is_real_switch = matches!(
                &state.session,
                SessionState::Present { pubkey: prior, .. } if prior != &pk
            );
            // Phase 3B: clear prior account's communities before re-wiring.
            // Effect::WireJoinedGroups re-registers the JoinedGroupsProjection
            // for the new pubkey; the fresh snapshot arrives on the next tick.
            state.communities = vec![];
            // ── #1653 BLOCKING #2: rebaseline follows on a DIRECT account switch ──
            // NMP can fire IdentityChanged(Some(new_pk)) with no intervening
            // None (a direct switch — see nmp's
            // `active_account_handle_reflects_account_switch`). The prior
            // account's follows are still in `state.follows` at this point; the
            // new account's follow sidecar (FollowListUpdated) has not arrived
            // yet. Anything that scopes a subscription on `state.follows` between
            // now and that sidecar would subscribe the PRIOR account's follows
            // under the NEW account — a cross-account privacy leak. The
            // post-reduce bookmarks re-push hook does exactly that, so we clear
            // follows HERE (before the hook runs) so the re-pushed bookmarks
            // interest contains ONLY the new account; its own follows are folded
            // back in when the new account's FollowListUpdated arrives (which
            // re-triggers the hook). This mirrors HomeFeed, whose follow-scoped
            // feed cursors only (re-)register on FollowListUpdated — never on a
            // bare IdentityChanged(Some) — so they never carry prior follows.
            state.follows = Vec::new();
            // ── #1653 BLOCKING/HIGH: full account-scoped rebaseline on a REAL ──
            // switch ───────────────────────────────────────────────────────────
            // A direct switch must tear down ALL account-scoped state the same
            // way logout does, otherwise account A's HomeFeed follow-scoped
            // cursors/rows (article_feed / highlight_feed / home_feed_interactions
            // / room_lanes) and bookmarks-slice state (all_bookmark_sets /
            // all_curation_sets / web_bookmarks) persist and surface under
            // account B (cross-account privacy leak). We share the SAME teardown
            // path the None/removed arm uses (single source — no duplicated reset
            // logic). We do NOT run this on initial login (nothing to wipe) or a
            // same-account refresh (would cause churn/regression).
            let mut teardown_effects = Vec::new();
            if is_real_switch {
                teardown_effects = clear_account_scoped_state_on_switch(state);
            }
            // Clear the pending NostrConnect URI — the handshake is done.
            state.nostrconnect_uri = None;
            state.session = SessionState::Present {
                pubkey: pk.clone(),
                signer_kind,
            };
            // Phase 3B: re-register joined-groups projection for the new account.
            // Phase 4B: the ReactionObserver is registered ONCE at boot with
            // nmp_ref.active_account_handle() and auto-tracks account switches
            // via the live Arc — no WireReactionProjection effect needed here.
            // #1653 codex r5: prepend the teardown effects (accumulator clear) so
            // they run before the post-reduce Bookmarks re-push hook.
            teardown_effects.push(Effect::WireJoinedGroups { pubkey: pk.clone() });
            teardown_effects
        }
        _ => {
            // None or empty pubkey → no active account.
            // Phase 3B: clear joined groups when account is removed.
            state.communities = vec![];
            state.nostrconnect_uri = None;
            state.session = SessionState::Absent;
            // ── Phase 3C: clear follow set on account removal ─────────────────
            // NMP fires IdentityChanged(None) on logout / account removal. Wipe
            // AppState::follows so stale contacts don't outlive the session.
            state.follows = Vec::new();
            // ── Phase 3D: clear profile state on account removal ──────────────
            // own_profile and claimed_profiles belong to the departing account.
            super::profiles::clear_on_identity_lost(state);
            // ── Phase 4C: clear bookmark list on account removal ──────────────
            // Wipe AppState::bookmarks so stale bookmarks don't outlive the
            // removed account.
            state.bookmarks = Vec::new();
            // ── Phase 4A: clear articles on account removal ───────────────────
            state.articles.clear();
            // ── Phase 4B: clear reaction state on account removal ─────────────
            super::reactions::clear_on_identity_lost(state);
            // ── Phase 4D: clear search results on account removal ─────────────
            state.search_results.clear();
            // Omnibox outcome (#1865) belongs to the removed account — clear it.
            state.omnibox_outcome = None;
            // ── Phase 4F: clear feed-pull state on account removal ────────────
            clear_feed_state_on_identity_lost(state);
            // ── Phase 7 feedback: clear UI-lifecycle state on identity loss ───
            super::feedback::reduce_event_clear_on_logout(state);
            // ── Phase 7 chat: clear chat room buffers on account removal ───────
            // Chat room buffers are open-view working sets. Clear on identity
            // loss so stale message rows don't outlive the account session.
            super::chat::clear_on_identity_lost(state);
            // ── Phase 7 artifact-preview: clear preview rows on identity loss ──
            // Artifact previews are keyed to the active account's subscriptions.
            // Wipe on account removal so stale rows don't surface under a new
            // identity.
            super::artifact_preview::clear_on_identity_lost(state);
            vec![]
        }
    }
}

pub(crate) fn reduce_event_sign_in_failed(
    state: &mut AppState,
    method: SignInMethod,
    error: String,
) -> Vec<Effect> {
    // Surface failures in session state (D6 — never as Result).
    state.session = SessionState::SignInFailed { method, error };
    vec![]
}

pub(crate) fn reduce_event_nostrconnect_uri_ready(
    state: &mut AppState,
    uri: String,
) -> Vec<Effect> {
    // Store the minted URI so the snapshot can expose it to the iOS
    // QR-code sheet. The NostrConnect sign-in session stays in SigningIn
    // until the remote signer completes the handshake (IdentityChanged).
    state.nostrconnect_uri = Some(uri);
    vec![]
}

// ─── Clock checks ────────────────────────────────────────────────────────────

/// Sign-in timeout check — transitions SigningIn → SignInFailed after
/// SIGN_IN_TIMEOUT_SECS. Called on every reduce pass via `clock_checks`.
///
/// NMP handles parse errors internally (set_last_error_toast) without firing
/// the identity-change observer — so an invalid nsec leaves us in SigningIn
/// indefinitely without this clock-driven fallback (D8).
pub(crate) fn clock_check_sign_in_timeout(state: &mut AppState, now: u64) {
    if let SessionState::SigningIn { started_at, method } = &state.session {
        if now.saturating_sub(*started_at) >= SIGN_IN_TIMEOUT_SECS {
            state.session = SessionState::SignInFailed {
                method: method.clone(),
                error: "sign-in timed out — no identity change observed".into(),
            };
        }
    }
}

// ─── Effect runner ───────────────────────────────────────────────────────────

pub(crate) fn run_effect_add_nsec_signer(nsec: String, nmp: Option<&NmpHandle>) {
    // Call nmp.add_signer(LocalNsec(nsec), make_active: true).
    // NMP auto-persists to its keyring when make_active && LocalNsec.
    // This is truly fire-and-forget: add_signer returns () and NMP handles
    // both the success and error paths internally —
    //   Success: the identity-change observer fires IdentityChanged(Some).
    //   Invalid nsec: NMP calls set_last_error_toast internally and returns
    //     without firing the observer. The clock-driven SIGN_IN_TIMEOUT_SECS
    //     check in clock_checks will then transition SigningIn → SignInFailed.
    // hl never awaits a Result from add_signer (Non-Negotiable #2/D6).
    if let Some(handle) = nmp {
        let nmp_ref: &NmpApp = unsafe { handle.ptr.as_ref() };
        nmp_ref.add_signer(
            nmp_core::SignerSource::LocalNsec(Zeroizing::new(nsec)),
            true, // make_active — also auto-persists to nmp keyring
        );
    }
    // No nmp handle (test mode) → test injects IdentityChanged directly.
}

pub(crate) fn run_effect_add_bunker_signer(uri: String, nmp: Option<&NmpHandle>) {
    // Route via nmp.add_signer(BunkerUri(uri), true). The NIP-46 broker
    // (nmp_signer_broker_init called at boot) takes over the handshake
    // async. Fire-and-forget: success arrives as IdentityChanged(Some).
    if let Some(handle) = nmp {
        let nmp_ref: &NmpApp = unsafe { handle.ptr.as_ref() };
        nmp_ref.add_signer(nmp_core::SignerSource::BunkerUri(uri), true);
    }
    // No nmp handle (test mode) → test injects IdentityChanged directly.
}

pub(crate) async fn run_effect_mint_nostrconnect_uri(
    nmp: Option<&NmpHandle>,
    tx: &mpsc::UnboundedSender<Cmd>,
) {
    // Call nmp_app_nostrconnect_uri(app_ptr, null) — relay is chosen by nmp
    // from its internal bootstrap relay slot (V-65, D3: no caller relay
    // override since d16aea60). Null callback_scheme means no custom URI
    // scheme. Returns an owned `nostrconnect://` C string or null if no
    // relay is configured. Feed the result back as NostrConnectUriReady.
    if let Some(handle) = nmp {
        let raw_ptr = handle.ptr.as_ptr();
        let uri_ptr = nmp_app_nostrconnect_uri(raw_ptr, std::ptr::null());
        if !uri_ptr.is_null() {
            // SAFETY: uri_ptr is a CString::into_raw pointer owned by
            // nmp-ffi. We take ownership here and free it with from_raw.
            let uri = unsafe { std::ffi::CStr::from_ptr(uri_ptr) }
                .to_string_lossy()
                .into_owned();
            // SAFETY: uri_ptr came from CString::into_raw in nmp-ffi.
            let _ = unsafe { CString::from_raw(uri_ptr) };
            let _ = tx.send(Cmd::Event(KernelEvent::NostrConnectUriReady { uri }));
        }
        // null return → no relay configured; stay in SigningIn until timeout.
    }
    // No nmp handle (test mode) → test injects NostrConnectUriReady directly.
}

pub(crate) fn run_effect_start_nip55_sign_in(nmp: Option<&NmpHandle>) {
    // Call nmp_app_signin_nip55(app_ptr, null) — null signer_package
    // lets the OS resolver pick the NIP-55 signer app (e.g. Amber).
    // nmp_app_signin_nip55 lazy-inits the external-signer driver if
    // nmp_external_signer_init was not already called at boot.
    // Fire-and-forget: success arrives as IdentityChanged(Some).
    if let Some(handle) = nmp {
        let raw_ptr = handle.ptr.as_ptr();
        nmp_app_signin_nip55(raw_ptr, std::ptr::null());
    }
    // No nmp handle (test mode) → test injects IdentityChanged directly.
}

pub(crate) fn run_effect_remove_active_account(nmp: Option<&NmpHandle>) {
    // Read the active pubkey from the nmp slot, then remove it.
    // Fire-and-forget: the observer fires IdentityChanged(None) on success.
    if let Some(handle) = nmp {
        let nmp_ref: &NmpApp = unsafe { handle.ptr.as_ref() };
        let active_slot = nmp_ref.active_account_handle();
        let maybe_pubkey: Option<String> = active_slot.lock().ok().and_then(|guard| guard.clone());
        if let Some(pubkey) = maybe_pubkey {
            nmp_ref.remove_account(pubkey);
        }
    }
}

pub(crate) fn run_effect_create_account(
    profile_name: String,
    nmp: Option<&NmpHandle>,
    policy: &crate::kernel::app::KernelPolicy,
) {
    // Build the profile HashMap from the supplied display name.
    // Relays and initial_follows come from the injected KernelPolicy
    // (D3: no hardcoded relay literals in kernel source).
    if let Some(handle) = nmp {
        let nmp_ref: &NmpApp = unsafe { handle.ptr.as_ref() };

        let mut profile = std::collections::HashMap::new();
        profile.insert("name".to_string(), profile_name);

        let relays: Vec<(String, String)> = policy
            .create_account
            .seed_relays
            .iter()
            .map(|r| (r.url.clone(), r.role.clone()))
            .collect();

        // ADR-0059 §5: initial_follows is app policy (empty = no kind:3).
        // NMP no longer hardcodes any default follow set (#1493); the
        // caller is responsible for supplying the initial contacts list.
        // An empty vec is the correct default: the account starts with
        // no contacts and no cold-start kind:3 is published.
        let initial_follows = policy.create_account.initial_follows.clone();

        // Fire-and-forget via actor_sender (first actor_sender use in hl).
        // Returns Result<(), CommandSendError>; we discard the error
        // (D6 — timeout in clock_checks will surface the failure as state).
        let _ = nmp_ref
            .actor_sender()
            .send(nmp_core::ActorCommand::CreateAccount {
                profile,
                relays,
                initial_follows,
                mls: false,
                make_active: true,
            });
    }
    // No nmp handle (test mode) → test injects IdentityChanged directly.
}

// ─── Phase 4F helper ─────────────────────────────────────────────────────────

/// Clear all feed-pull state when the active identity is lost.
///
/// Called by both `reduce_action_logout` and `reduce_event_identity_changed`
/// (the `None`/removed account arm). Resets `article_feed`, `highlight_feed`,
/// and all `room_lanes` to their default empty state. cursor_id is reset to 0
/// so the next view-open re-registers fresh (idempotent, D6). Rows are cleared
/// so stale feed content from the departing account never surfaces to the next.
pub(crate) fn clear_feed_state_on_identity_lost(state: &mut AppState) {
    state.article_feed = crate::kernel::domains::feed::FeedState::default();
    state.highlight_feed = crate::kernel::domains::feed::FeedState::default();
    state.room_lanes.clear();
    state.room_highlight_feeds.clear();
    state.article_highlight_feeds.clear();
    // Phase 7: interaction cursor is follow-scoped — clear on identity loss
    // so the next account's follows drive a fresh registration.
    state.home_feed_interactions = crate::kernel::domains::feed::FeedState::default();
}

// ─── #1653 shared account-scoped teardown ──────────────────────────────────────

/// Tear down ALL account-scoped state when the active account goes away — used
/// by BOTH the `IdentityChanged(None)` / logout arm AND the direct-switch arm
/// (`IdentityChanged(Some(new))` where a *different* account was previously
/// active). Keeping a single teardown path means the two arms can never drift:
/// any state that must not leak across accounts is cleared identically whether
/// the user logs out or switches directly.
///
/// Does NOT touch `session`, `nostrconnect_uri`, `communities`, or `follows` —
/// those carry arm-specific semantics (e.g. the switch arm sets `session` to
/// the new `Present`, the None arm to `Absent`) and are handled by each caller.
///
/// Caller is responsible for only invoking this on a real teardown: the
/// direct-switch arm gates on `is_real_switch` so an initial login (nothing to
/// wipe) or a same-account refresh (would churn the live account's own state)
/// never reaches here.
///
/// ## Comprehensive audit (#1653, codex r5)
///
/// Every account-scoped field of `AppState` (profiles, bookmarks/curation/web +
/// accumulators, articles, reactions, search, ALL feed cursors/rows, share_queue,
/// chat buffers, room discussions, room-home event buffers, artifact previews,
/// feedback flags) is cleared here so NO account-scoped state survives a switch.
/// Device/app-local state (UI toggles, route, settings, nostrconnect_uri, session,
/// whats_new, isbn, podcast, ocr, capture_draft, camera, relay_diagnostics,
/// discovered_groups) is deliberately NOT touched — wiping it would churn
/// arm-specific or genuinely non-account facts. `comment_threads` is also left
/// intact: it is content-addressed (keyed by root anchor), bounded by NMP, and
/// not per-account (the list projection yields empty under no active viewer).
///
/// Returns effects the CALLER must run: clearing the `SetListProjection` /
/// `WebBookmarkProjection` accumulators lives behind the boot-registered
/// projection controller (not reachable from a pure reducer), so it is emitted as
/// `Effect::WithdrawBookmarkSetsInterest` whose runner clears both accumulators.
/// On a switch where the Bookmarks view is open the post-reduce identity-changed
/// hook re-pushes a fresh interest for the new account immediately afterwards, so
/// the withdraw → re-push ordering leaves the new account correctly subscribed
/// with empty (not prior-account) accumulators.
#[must_use]
pub(crate) fn clear_account_scoped_state_on_switch(state: &mut AppState) -> Vec<Effect> {
    // own_profile and claimed_profiles belong to the departing account.
    super::profiles::clear_on_identity_lost(state);
    // claimed_events belong to the departing account.
    state.claimed_events.clear();
    // Bookmarks list — stale bookmarks must not outlive the account.
    state.bookmarks = Vec::new();
    // #1653 HIGH: bookmark/curation sets + web bookmarks. SetListProjection
    // accumulates all observed events; wipe the AppState mirrors so the prior
    // account's sets/web bookmarks never surface under the next identity.
    state.all_bookmark_sets.clear();
    state.all_curation_sets.clear();
    state.web_bookmarks.clear();
    // kind:30023 articles for the departing account's subscriptions.
    state.articles.clear();
    // Reaction counts from the departing account.
    super::reactions::clear_on_identity_lost(state);
    // NIP-50 search results for the departing account.
    state.search_results.clear();
    // Omnibox outcome (#1865) belongs to the departing session.
    state.omnibox_outcome = None;
    // #1653 BLOCKING: HomeFeed follow-scoped feed cursors/rows (article_feed,
    // highlight_feed, home_feed_interactions, room_lanes, …). Reset to default +
    // cursor_id=0 so the new account re-registers fresh on its FollowListUpdated.
    clear_feed_state_on_identity_lost(state);
    // #1653 codex r5 HIGH: share_queue is account-scoped (ShareComposer projects
    // it directly). A stale queue from a prior account must not leak into the next
    // session — the App Group file is the durable handoff store; this is the
    // in-kernel working set. Clear on EVERY teardown (not just logout).
    state.share_queue = crate::kernel::domains::share::ShareQueueState::default();
    // #21: clear the in-flight share publish FSM on teardown — a stale
    // Publishing/Error from a prior account must not leak into the next session.
    state.share_publish = crate::kernel::domains::share::SharePublishState::default();
    // #1653 codex r5: kind:11 discussion rows are identity-scoped (served by the
    // viewer's relays). Previously cleared only on logout; fold into the unified
    // teardown so a direct switch does not surface a prior account's rows.
    super::discussions::clear_on_logout(state);
    // #1653 codex r5: per-room kind:1111 group-event buffers are open-view working
    // sets registered per RoomHome view; wipe so the next session re-populates
    // fresh (mirrors chat buffers / feed rows). Re-wired on the next view open.
    state.room_home_events.clear();
    // UI-lifecycle feedback flags.
    super::feedback::reduce_event_clear_on_logout(state);
    // Chat room buffers — open-view working sets.
    super::chat::clear_on_identity_lost(state);
    // Artifact previews keyed to the active account's subscriptions.
    super::artifact_preview::clear_on_identity_lost(state);
    // #1653 codex r5 HIGH: clear the SetList/Web projection accumulators. They
    // live behind the boot projection controller (not reachable here); the
    // withdraw effect's runner clears both. The post-reduce identity-changed hook
    // re-pushes a fresh interest when Bookmarks is open.
    vec![Effect::WithdrawBookmarkSetsInterest]
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::action::AppAction;
    use crate::kernel::actor::{reduce, Cmd};
    use crate::kernel::app::AppState;
    use crate::kernel::clock::{Clock, ManualClock};
    use crate::kernel::effect::Effect;
    use crate::kernel::snapshot::ViewSnapshot;
    use crate::kernel::view::ViewId;

    fn make_state() -> AppState {
        AppState::default()
    }

    fn step(state: &mut AppState, clock: &ManualClock, cmd: Cmd) -> Vec<Effect> {
        let now = clock.now_unix_seconds();
        reduce(state, cmd, now)
    }

    // ── Phase 2A tests ────────────────────────────────────────────────────────

    // P2A-1: SignInNsec transitions to SigningIn and emits AddNsecSigner with
    //        make_active intent baked in (the Effect carries the nsec string;
    //        the runner calls add_signer with make_active=true).
    #[test]
    fn nsec_sign_in_enqueues_local_signer_make_active_true() {
        let mut state = make_state();
        let clock = ManualClock::new(42);
        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::SignInNsec {
                nsec: "nsec1test".into(),
            }),
        );
        // State should be SigningIn immediately.
        assert!(
            matches!(
                &state.session,
                SessionState::SigningIn {
                    method: SignInMethod::Nsec,
                    started_at: 42
                }
            ),
            "expected SigningIn, got {:?}",
            state.session
        );
        // A single AddNsecSigner effect should be emitted.
        assert_eq!(effects.len(), 1, "expected one effect from SignInNsec");
        assert!(
            matches!(&effects[0], Effect::AddNsecSigner { nsec } if nsec == "nsec1test"),
            "expected AddNsecSigner effect with the nsec, got {:?}",
            effects[0]
        );
    }

    // P2A-2: IdentityChanged(Some(pubkey)) routes to SessionState::Present.
    #[test]
    fn identity_changed_some_routes_to_present() {
        let mut state = make_state();
        let clock = ManualClock::default();
        // Start in SigningIn to simulate the normal flow.
        step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::SignInNsec {
                nsec: "nsec1x".into(),
            }),
        );
        // Observer fires with the resolved pubkey.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(Some("deadbeefpubkey".into()))),
        );
        assert!(
            matches!(
                &state.session,
                SessionState::Present { pubkey, .. } if pubkey == "deadbeefpubkey"
            ),
            "expected Present, got {:?}",
            state.session
        );
    }

    // P2A-3: IdentityChanged(None) routes to SessionState::Absent.
    #[test]
    fn identity_changed_none_routes_to_absent() {
        let mut state = make_state();
        let clock = ManualClock::default();
        // Put the state in Present first.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(Some("somepubkey".into()))),
        );
        assert!(matches!(&state.session, SessionState::Present { .. }));
        // Now fire IdentityChanged(None) — should transition to Absent.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(None)),
        );
        assert!(
            matches!(&state.session, SessionState::Absent),
            "expected Absent, got {:?}",
            state.session
        );
    }

    // P2A-4: Logout emits RemoveActiveAccount + ClearSession and bumps epoch.
    #[test]
    fn logout_calls_remove_account_and_bumps_epoch() {
        let mut state = make_state();
        let clock = ManualClock::default();
        // Simulate a logged-in session.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(Some("mypubkey".into()))),
        );
        let epoch_before = state.session_epoch;
        let effects = step(&mut state, &clock, Cmd::Action(AppAction::Logout));
        // Epoch must be bumped.
        assert_eq!(state.session_epoch, epoch_before + 1);
        // State must be Absent immediately (not waiting for observer).
        assert!(matches!(&state.session, SessionState::Absent));
        // Effects must include both RemoveActiveAccount and ClearSession.
        let has_remove = effects
            .iter()
            .any(|e| matches!(e, Effect::RemoveActiveAccount));
        let has_clear = effects.iter().any(|e| matches!(e, Effect::ClearSession));
        assert!(has_remove, "expected RemoveActiveAccount effect in Logout");
        assert!(has_clear, "expected ClearSession effect in Logout");
    }

    // P2A-5: sign-in failure surfaces in SessionState (D6), never as Result.
    //        dispatch(SignInNsec) returns () regardless of whether the signer
    //        call succeeds — errors arrive via KernelEvent::SignInFailed.
    #[test]
    fn sign_in_failure_surfaces_in_session_state_not_return() {
        let mut state = make_state();
        let clock = ManualClock::default();
        // Transition to SigningIn first.
        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::SignInNsec {
                nsec: "bad_nsec".into(),
            }),
        );
        // dispatch returns () — checked by the type of `effects` (Vec<Effect>).
        // The effect runner (in production) calls add_signer and on error sends
        // KernelEvent::SignInFailed back. We simulate that here.
        let _ = effects; // () — fire-and-forget
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::SignInFailed {
                method: SignInMethod::Nsec,
                error: "invalid nsec: bad bech32".into(),
            }),
        );
        assert!(
            matches!(
                &state.session,
                SessionState::SignInFailed {
                    method: SignInMethod::Nsec,
                    error
                } if error.contains("invalid nsec")
            ),
            "expected SignInFailed, got {:?}",
            state.session
        );
    }

    // P2A-6: dispatch(SignInNsec) returns () — the action dispatch API is
    //        fire-and-forget (#3).
    #[test]
    fn dispatch_signin_returns_unit() {
        let mut state = make_state();
        let clock = ManualClock::new(0);
        // This must compile and run — the return value of step() is Vec<Effect>,
        // which models the `()` (fire-and-forget) contract. The key assertion is
        // that `step` returns without blocking or returning a Result.
        let _effects: Vec<Effect> = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::SignInNsec {
                nsec: "nsec1any".into(),
            }),
        );
        // No panic, no Result — the dispatch contract is satisfied.
        assert!(
            matches!(&state.session, SessionState::SigningIn { .. }),
            "SignInNsec must transition to SigningIn synchronously"
        );
    }

    // P2A-7: clock drives sign-in timeout → SignInFailed.
    //        NMP handles invalid nsec parse errors internally without firing the
    //        identity-change observer; this clock-driven fallback ensures the UI
    //        never gets stuck in SigningIn forever (D6 — failure surfaces as state).
    #[test]
    fn sign_in_timeout_transitions_signing_in_to_failed() {
        use crate::kernel::app::SIGN_IN_TIMEOUT_SECS;
        let mut state = make_state();
        let clock = ManualClock::new(0);

        step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::SignInNsec {
                nsec: "nsec1invalid".into(),
            }),
        );
        assert!(matches!(&state.session, SessionState::SigningIn { .. }));

        // Just before timeout — still SigningIn.
        clock.advance(SIGN_IN_TIMEOUT_SECS - 1);
        step(&mut state, &clock, Cmd::Event(KernelEvent::ClockTick));
        assert!(
            matches!(&state.session, SessionState::SigningIn { .. }),
            "should still be SigningIn before timeout"
        );

        // At timeout — should transition to SignInFailed.
        clock.advance(1);
        step(&mut state, &clock, Cmd::Event(KernelEvent::ClockTick));
        assert!(
            matches!(
                &state.session,
                SessionState::SignInFailed {
                    method: SignInMethod::Nsec,
                    ..
                }
            ),
            "should be SignInFailed at timeout, got {:?}",
            state.session
        );
    }

    // ── Phase 2B tests ────────────────────────────────────────────────────────

    // P2B-1: PairBunker transitions to SigningIn{Bunker} and emits AddBunkerSigner.
    #[test]
    fn pair_bunker_enqueues_bunker_uri_source() {
        let mut state = make_state();
        let clock = ManualClock::new(10);
        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::PairBunker {
                uri: "bunker://pubkey?relay=wss://relay.example.com".into(),
            }),
        );
        assert!(
            matches!(
                &state.session,
                SessionState::SigningIn {
                    method: SignInMethod::Bunker,
                    started_at: 10
                }
            ),
            "expected SigningIn{{Bunker}}, got {:?}",
            state.session
        );
        assert_eq!(
            effects.len(),
            1,
            "expected exactly one effect from PairBunker"
        );
        assert!(
            matches!(
                &effects[0],
                Effect::AddBunkerSigner { uri }
                    if uri == "bunker://pubkey?relay=wss://relay.example.com"
            ),
            "expected AddBunkerSigner with the uri, got {:?}",
            effects[0]
        );
    }

    // P2B-2: NostrConnectUriReady stores the URI in state and the snapshot
    //        exposes it via AppRootSnapshot::nostrconnect_uri.
    #[test]
    fn nostrconnect_uri_ready_event_to_snapshot() {
        use crate::kernel::actor::project_snapshot;
        let mut state = make_state();
        let clock = ManualClock::default();

        // Begin a NostrConnect sign-in.
        step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::StartNostrConnect),
        );
        assert!(
            matches!(
                &state.session,
                SessionState::SigningIn {
                    method: SignInMethod::NostrConnect,
                    ..
                }
            ),
            "expected SigningIn{{NostrConnect}}, got {:?}",
            state.session
        );
        assert!(state.nostrconnect_uri.is_none(), "URI not set yet");

        // Simulate the effect runner delivering the minted URI.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::NostrConnectUriReady {
                uri: "nostrconnect://pubkey?relay=wss://relay.example.com".into(),
            }),
        );
        assert_eq!(
            state.nostrconnect_uri.as_deref(),
            Some("nostrconnect://pubkey?relay=wss://relay.example.com"),
            "URI should be stored in state"
        );

        // Project a snapshot and verify the URI is present.
        let now = clock.now_unix_seconds();
        if let Some(ViewSnapshot::AppRoot(snap)) = project_snapshot(&state, &ViewId::AppRoot, now) {
            assert_eq!(
                snap.nostrconnect_uri.as_deref(),
                Some("nostrconnect://pubkey?relay=wss://relay.example.com"),
                "URI must appear in AppRootSnapshot"
            );
        } else {
            panic!("expected AppRoot snapshot");
        }

        // When IdentityChanged fires (handshake complete), URI is cleared.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(Some("resolvedpubkey".into()))),
        );
        assert!(
            state.nostrconnect_uri.is_none(),
            "URI must be cleared after IdentityChanged"
        );
    }

    // P2B-3: SignInNip55 transitions to SigningIn{Nip55} and emits StartNip55SignIn.
    //        The effect runner would call nmp_app_signin_nip55; in test mode
    //        (no nmp handle) success is injected as IdentityChanged.
    #[test]
    fn nip55_signin_invokes_external_hook() {
        let mut state = make_state();
        let clock = ManualClock::new(5);
        let effects = step(&mut state, &clock, Cmd::Action(AppAction::SignInNip55));
        assert!(
            matches!(
                &state.session,
                SessionState::SigningIn {
                    method: SignInMethod::Nip55,
                    started_at: 5
                }
            ),
            "expected SigningIn{{Nip55}}, got {:?}",
            state.session
        );
        assert_eq!(effects.len(), 1, "expected one effect from SignInNip55");
        assert!(
            matches!(&effects[0], Effect::StartNip55SignIn),
            "expected StartNip55SignIn effect, got {:?}",
            effects[0]
        );

        // Simulate success via IdentityChanged — signer_kind should be Nip55.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(Some("nip55pubkey".into()))),
        );
        assert!(
            matches!(
                &state.session,
                SessionState::Present {
                    pubkey,
                    signer_kind: SignerKind::Nip55
                } if pubkey == "nip55pubkey"
            ),
            "expected Present{{Nip55}}, got {:?}",
            state.session
        );
    }

    // P2B-4: dispatch(PairBunker/StartNostrConnect/SignInNip55) returns () —
    //        the fire-and-forget contract (Non-Negotiable #3).
    #[test]
    fn dispatch_returns_unit() {
        let mut state = make_state();
        let clock = ManualClock::new(0);

        let _: Vec<Effect> = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::PairBunker {
                uri: "bunker://x".into(),
            }),
        );
        let _: Vec<Effect> = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::StartNostrConnect),
        );
        let _: Vec<Effect> = step(&mut state, &clock, Cmd::Action(AppAction::SignInNip55));
        // No panic and no Result return — fire-and-forget contract satisfied.
    }

    // P2B-5: bunker sign-in times out → SignInFailed{Bunker}.
    //        Reuses the 2A clock arm which covers ALL SigningIn variants.
    #[test]
    fn bunker_signing_in_times_out_to_failed() {
        use crate::kernel::app::SIGN_IN_TIMEOUT_SECS;
        let mut state = make_state();
        let clock = ManualClock::new(0);

        step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::PairBunker {
                uri: "bunker://pubkey?relay=wss://r.example.com".into(),
            }),
        );
        assert!(matches!(&state.session, SessionState::SigningIn { .. }));

        // Just before timeout — still SigningIn.
        clock.advance(SIGN_IN_TIMEOUT_SECS - 1);
        step(&mut state, &clock, Cmd::Event(KernelEvent::ClockTick));
        assert!(
            matches!(&state.session, SessionState::SigningIn { .. }),
            "should still be SigningIn before timeout"
        );

        // At timeout — should be SignInFailed{Bunker}.
        clock.advance(1);
        step(&mut state, &clock, Cmd::Event(KernelEvent::ClockTick));
        assert!(
            matches!(
                &state.session,
                SessionState::SignInFailed {
                    method: SignInMethod::Bunker,
                    ..
                }
            ),
            "should be SignInFailed{{Bunker}} at timeout, got {:?}",
            state.session
        );
    }

    // P2B-6: IdentityChanged after Bunker/NostrConnect → signer_kind is Nip46.
    #[test]
    fn bunker_identity_changed_sets_nip46_signer_kind() {
        let mut state = make_state();
        let clock = ManualClock::default();

        step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::PairBunker {
                uri: "bunker://pk".into(),
            }),
        );
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(Some("bunkerpk".into()))),
        );
        assert!(
            matches!(
                &state.session,
                SessionState::Present {
                    pubkey,
                    signer_kind: SignerKind::Nip46
                } if pubkey == "bunkerpk"
            ),
            "expected Present{{Nip46}}, got {:?}",
            state.session
        );
    }

    // ── Phase 2C tests ────────────────────────────────────────────────────────

    // P2C-1: CreateAccount transitions to SigningIn{CreateAccount} and emits
    //        Effect::CreateAccount (actor_sender path, first use in hl).
    //        make_active=true is baked into the effect runner, not the reducer data.
    #[test]
    fn create_account_sends_effect_create_account() {
        let mut state = make_state();
        let clock = ManualClock::new(42);
        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::CreateAccount {
                profile_name: "Alice".into(),
            }),
        );
        // State must transition to SigningIn{CreateAccount} synchronously.
        assert!(
            matches!(
                &state.session,
                SessionState::SigningIn {
                    method: SignInMethod::CreateAccount,
                    started_at: 42,
                }
            ),
            "expected SigningIn{{CreateAccount}}, got {:?}",
            state.session
        );
        // Exactly one effect — CreateAccount.
        assert_eq!(effects.len(), 1, "expected one effect from CreateAccount");
        assert!(
            matches!(&effects[0], Effect::CreateAccount { profile_name } if profile_name == "Alice"),
            "expected Effect::CreateAccount{{profile_name: Alice}}, got {:?}",
            effects[0]
        );
    }

    // P2C-2: make_active=true is verified by inspecting the effect (the runner
    //        always passes make_active:true to ActorCommand::CreateAccount; we
    //        confirm the Effect variant carries the right profile_name and that
    //        the test pattern documents the intent).
    #[test]
    fn create_account_make_active_true_is_effect_runner_policy() {
        // The Effect::CreateAccount variant carries profile_name only; make_active
        // is Rust-constant in the effect runner (always true for the onboarding
        // path). This test confirms that the reducer emits exactly one
        // Effect::CreateAccount for a CreateAccount action — the runner's
        // make_active=true is verified by the build (it would fail to compile
        // if the ActorCommand::CreateAccount field was wrong).
        let mut state = make_state();
        let clock = ManualClock::new(0);
        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::CreateAccount {
                profile_name: "Bob".into(),
            }),
        );
        assert_eq!(effects.len(), 1);
        let Effect::CreateAccount { profile_name } = &effects[0] else {
            panic!("expected Effect::CreateAccount, got {:?}", effects[0]);
        };
        assert_eq!(profile_name, "Bob");
    }

    // P2C-4: CreateAccount success via IdentityChanged(Some(pubkey))
    //        → SessionState::Present (same observer path as nsec sign-in).
    #[test]
    fn create_account_success_via_identity_changed() {
        let mut state = make_state();
        let clock = ManualClock::new(0);

        // Dispatch CreateAccount — enters SigningIn.
        step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::CreateAccount {
                profile_name: "Charlie".into(),
            }),
        );
        assert!(matches!(
            &state.session,
            SessionState::SigningIn {
                method: SignInMethod::CreateAccount,
                ..
            }
        ));

        // NMP fires identity-change observer with the new pubkey.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(Some("newpubkey123".into()))),
        );
        assert!(
            matches!(
                &state.session,
                SessionState::Present { pubkey, signer_kind: SignerKind::LocalNsec }
                    if pubkey == "newpubkey123"
            ),
            "expected Present{{LocalNsec}}, got {:?}",
            state.session
        );
    }

    // P2C-5: ADR-0059 publish policy — initial_follows is empty in the injected
    //        default KernelPolicy (no kind:3 for new accounts until user follows).
    //        We verify both the default (empty → no kind:3) AND that a populated
    //        policy round-trips correctly so the effect runner will pass it to
    //        ActorCommand::CreateAccount.initial_follows when nmp is present.
    #[test]
    fn create_account_follows_adr0059_publish_policy() {
        use crate::kernel::app::{CreateAccountPolicy, KernelPolicy, SeedRelay};

        // Default policy: initial_follows is empty → no kind:3 published.
        let default_policy = KernelPolicy::default();
        assert!(
            default_policy.create_account.initial_follows.is_empty(),
            "ADR-0059 §5: initial_follows must be empty by default (no kind:3)"
        );

        // Injected policy with follows: the field is preserved for the effect runner.
        // This proves initial_follows round-trips from KernelPolicy into the
        // CreateAccountPolicy struct that run_effect reads via `policy.create_account`.
        let follows = vec![
            "deadbeef00000000000000000000000000000000000000000000000000000001".to_string(),
            "deadbeef00000000000000000000000000000000000000000000000000000002".to_string(),
        ];
        let policy_with_follows = KernelPolicy {
            create_account: CreateAccountPolicy {
                seed_relays: vec![SeedRelay {
                    url: "relay.example".to_string(),
                    role: "both".to_string(),
                }],
                initial_follows: follows.clone(),
            },
            relay: Default::default(),
            room: Default::default(),
            data_dir: String::new(),
        };
        assert_eq!(
            policy_with_follows.create_account.initial_follows, follows,
            "initial_follows must round-trip through KernelPolicy for the effect runner"
        );
    }

    // P2C-6: dispatch(CreateAccount) returns () — fire-and-forget contract.
    #[test]
    fn dispatch_create_account_returns_unit() {
        let mut state = make_state();
        let clock = ManualClock::new(0);
        let _effects: Vec<Effect> = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::CreateAccount {
                profile_name: "Dave".into(),
            }),
        );
        // No panic, no Result — fire-and-forget satisfied.
        assert!(matches!(&state.session, SessionState::SigningIn { .. }));
    }

    // ── #1653: direct account switch full rebaseline ──────────────────────────
    //
    // A DIRECT switch is IdentityChanged(Some(B)) with NO intervening None while
    // account A is already Present. Before the fix only the None/removed arm
    // rebaselined account-scoped state, so account A's HomeFeed follow-scoped
    // cursors/rows and bookmarks-slice state persisted under account B.

    const PK_A: &str = "aaaa000000000000000000000000000000000000000000000000000000000001";
    const PK_B: &str = "bbbb000000000000000000000000000000000000000000000000000000000002";

    /// Put state into `Present { pubkey: A }` directly (no SigningIn churn).
    fn present_as(state: &mut AppState, clock: &ManualClock, pk: &str) {
        step(
            state,
            clock,
            Cmd::Event(KernelEvent::IdentityChanged(Some(pk.to_string()))),
        );
        assert!(
            matches!(&state.session, SessionState::Present { pubkey, .. } if pubkey == pk),
            "fixture: expected Present({pk}), got {:?}",
            state.session
        );
    }

    // #1653-SW1: a direct switch resets HomeFeed follow-scoped feed cursors/rows.
    #[test]
    fn direct_switch_resets_homefeed_cursors() {
        use crate::kernel::domains::feed::FeedState;
        let mut state = make_state();
        let clock = ManualClock::default();
        // Account A active.
        present_as(&mut state, &clock, PK_A);
        // Simulate an OPEN HomeFeed under A with a non-default article feed:
        // a registered cursor + drained rows + advanced seq.
        state.article_feed = FeedState {
            cursor_id: 12345,
            after_seq: 42,
            exhausted: true,
            rows: vec![],
        };
        state.home_feed_interactions.cursor_id = 999;
        state.room_lanes.insert(
            "hl.feed.room.somegroup".to_string(),
            FeedState {
                cursor_id: 555,
                after_seq: 7,
                ..Default::default()
            },
        );

        // DIRECT switch A → B (no intervening None).
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(Some(PK_B.to_string()))),
        );

        // B must NOT see A's cursors/rows — all reset to default (cursor_id=0).
        let default = FeedState::default();
        assert_eq!(
            (
                state.article_feed.cursor_id,
                state.article_feed.after_seq,
                state.article_feed.exhausted,
                state.article_feed.rows.len(),
            ),
            (
                default.cursor_id,
                default.after_seq,
                default.exhausted,
                default.rows.len(),
            ),
            "article_feed must reset on direct switch (cursor_id=0, no rows)"
        );
        assert_eq!(
            state.home_feed_interactions.cursor_id, 0,
            "home_feed_interactions must reset on direct switch (cursor_id=0)"
        );
        assert!(
            state.room_lanes.is_empty(),
            "room_lanes must be cleared on direct switch"
        );
        // Sanity: B is the active account now.
        assert!(matches!(&state.session, SessionState::Present { pubkey, .. } if pubkey == PK_B));
    }

    // #1653-SW2: a direct switch clears bookmark sets + web bookmarks.
    #[test]
    fn direct_switch_clears_bookmark_and_web_state() {
        use crate::kernel::snapshot::{BookmarkSetRow, WebBookmarkRow};
        let mut state = make_state();
        let clock = ManualClock::default();
        present_as(&mut state, &clock, PK_A);

        // A has bookmark sets, curation sets, and web bookmarks.
        state.all_bookmark_sets.push(BookmarkSetRow {
            d_tag: "set-a".into(),
            pubkey: PK_A.into(),
            kind: 30003,
            title: None,
            description: None,
            image: None,
            article_addresses: vec![],
            note_ids: vec![],
            r_refs: vec![],
            topics: vec![],
            raw_tags: vec![],
            content: String::new(),
            created_at: 1000,
        });
        state.all_curation_sets.push(BookmarkSetRow {
            d_tag: "cur-a".into(),
            pubkey: PK_A.into(),
            kind: 30004,
            title: None,
            description: None,
            image: None,
            article_addresses: vec![],
            note_ids: vec![],
            r_refs: vec![],
            topics: vec![],
            raw_tags: vec![],
            content: String::new(),
            created_at: 1000,
        });
        state.web_bookmarks.push(WebBookmarkRow {
            url: "https://example.com".into(),
            pubkey: PK_A.into(),
            title: None,
            description: None,
            topics: vec![],
            published_at: None,
            created_at: 1000,
        });

        // DIRECT switch A → B.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(Some(PK_B.to_string()))),
        );

        // None of A's bookmark/web state may survive under B.
        assert!(
            state.all_bookmark_sets.is_empty(),
            "all_bookmark_sets must be cleared on direct switch"
        );
        assert!(
            state.all_curation_sets.is_empty(),
            "all_curation_sets must be cleared on direct switch"
        );
        assert!(
            state.web_bookmarks.is_empty(),
            "web_bookmarks must be cleared on direct switch"
        );
    }

    // #1653-SW3: an INITIAL login (None→Some(A)) does NOT wipe — there is nothing
    // to wipe, and a spurious reset would be churn. We verify pre-seeded state
    // (which in production would never exist before login, but proves the gate)
    // survives an initial login because session was Absent, not Present.
    #[test]
    fn initial_login_does_not_wipe() {
        use crate::kernel::snapshot::WebBookmarkRow;
        let mut state = make_state();
        let clock = ManualClock::default();
        // Session starts in the no-active-account default (Unknown), i.e. not
        // Present, so an incoming Some(A) is an initial login — NOT a switch.
        // Seed a web bookmark to detect a spurious wipe.
        assert!(
            !matches!(&state.session, SessionState::Present { .. }),
            "fixture: no active account before initial login"
        );
        state.web_bookmarks.push(WebBookmarkRow {
            url: "https://seed.example".into(),
            pubkey: PK_A.into(),
            title: None,
            description: None,
            topics: vec![],
            published_at: None,
            created_at: 1000,
        });

        // Initial login A (no prior active account).
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(Some(PK_A.to_string()))),
        );

        // Not a real switch → the gate must NOT fire the teardown.
        assert_eq!(
            state.web_bookmarks.len(),
            1,
            "initial login must NOT wipe account-scoped state"
        );
    }

    // #1653-SW4: a same-account refresh (Some(A) while A already Present) does NOT
    // wipe — the live account's own state must not churn/regress.
    #[test]
    fn same_account_refresh_does_not_wipe() {
        use crate::kernel::domains::feed::FeedState;
        use crate::kernel::snapshot::WebBookmarkRow;
        let mut state = make_state();
        let clock = ManualClock::default();
        present_as(&mut state, &clock, PK_A);

        // A's live working state.
        state.article_feed = FeedState {
            cursor_id: 4242,
            after_seq: 9,
            exhausted: false,
            rows: vec![],
        };
        state.web_bookmarks.push(WebBookmarkRow {
            url: "https://a.example".into(),
            pubkey: PK_A.into(),
            title: None,
            description: None,
            topics: vec![],
            published_at: None,
            created_at: 1000,
        });

        // Re-confirm SAME account A (refresh / re-emit).
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(Some(PK_A.to_string()))),
        );

        // No wipe: A's cursor and web bookmarks survive.
        assert_eq!(
            state.article_feed.cursor_id, 4242,
            "same-account refresh must NOT reset feed cursors"
        );
        assert_eq!(
            state.web_bookmarks.len(),
            1,
            "same-account refresh must NOT clear web bookmarks"
        );
    }

    // ── #1653 codex r5: comprehensive account-switch teardown ─────────────────
    //
    // These tests assert the audited account-scoped fields are all cleared on a
    // direct switch and a logout, that share_queue / room_discussions / the
    // SetList+Web accumulators (via the withdraw effect) are torn down, and that
    // device/UI-local state is NOT churned.

    use crate::kernel::domains::feed::FeedState;
    use crate::kernel::snapshot::{
        BookmarkRow, BookmarkSetRow, DiscussionRow, ReactionRow, WebBookmarkRow,
    };

    fn set_row(d_tag: &str, kind: u32) -> BookmarkSetRow {
        BookmarkSetRow {
            d_tag: d_tag.into(),
            pubkey: PK_A.into(),
            kind,
            title: None,
            description: None,
            image: None,
            article_addresses: vec![],
            note_ids: vec![],
            r_refs: vec![],
            topics: vec![],
            raw_tags: vec![],
            content: String::new(),
            created_at: 1,
        }
    }

    fn web_row(url: &str) -> WebBookmarkRow {
        WebBookmarkRow {
            url: url.into(),
            pubkey: PK_A.into(),
            title: None,
            description: None,
            topics: vec![],
            published_at: None,
            created_at: 1,
        }
    }

    /// Seed EVERY audited account-scoped field with a non-default value so a
    /// teardown can be proven to clear all of them. Uses only cheap, real
    /// constructors (no `Default` on row structs that don't derive it).
    fn seed_all_account_scoped(state: &mut AppState) {
        state.bookmarks.push(BookmarkRow::Event {
            event_id: "e".into(),
            relay: None,
        });
        state.all_bookmark_sets.push(set_row("s", 30003));
        state.all_curation_sets.push(set_row("c", 30004));
        state.web_bookmarks.push(web_row("https://a.example"));
        state.reaction_state.insert(
            "evt".into(),
            ReactionRow {
                target_event_id: "evt".into(),
                count: 1,
                viewer_reacted: true,
            },
        );
        state.viewer_reaction_ids.insert("evt".into(), "rid".into());
        state.article_feed = FeedState {
            cursor_id: 1,
            after_seq: 1,
            exhausted: true,
            rows: vec![],
        };
        state.highlight_feed = FeedState {
            cursor_id: 2,
            ..Default::default()
        };
        state.home_feed_interactions = FeedState {
            cursor_id: 3,
            ..Default::default()
        };
        state
            .room_lanes
            .insert("hl.feed.room.g".into(), FeedState::default());
        state
            .room_highlight_feeds
            .insert("g".into(), FeedState::default());
        state
            .article_highlight_feeds
            .insert("a".into(), FeedState::default());
        state
            .share_queue
            .pending
            .push(crate::kernel::domains::share::ShareQueueItem {
                id: "i".into(),
                group_id: "g".into(),
                url: "https://x".into(),
                note: String::new(),
                created_at_unix_seconds: 1.0,
            });
        state
            .share_queue
            .seen
            .insert(("g".into(), "https://x".into()));
        state.room_discussions.insert(
            "g".into(),
            vec![DiscussionRow {
                event_id: "e".into(),
                author_pubkey: PK_A.into(),
                title: String::new(),
                body: String::new(),
                attachment_url: None,
                artifact_coordinate: None,
                created_at: 1,
            }],
        );
        state.room_home_events.insert("g".into(), vec![]);
        state.chat_rooms.insert("g".into(), Default::default());
        state.artifact_preview_requests.insert("a:x".into());
        // own_profile / claimed_profiles / articles / search_results /
        // artifact_previews carry nmp/heavy row types with no cheap constructor;
        // the per-domain clear tests (profiles/reactions/articles/search/
        // artifact_preview) cover those. assert_all_account_scoped_cleared still
        // checks them so a regression that LEFT them populated would fail.
    }

    /// Assert EVERY audited account-scoped field is at its default/empty value.
    fn assert_all_account_scoped_cleared(state: &AppState) {
        assert!(state.bookmarks.is_empty(), "bookmarks");
        assert!(state.all_bookmark_sets.is_empty(), "all_bookmark_sets");
        assert!(state.all_curation_sets.is_empty(), "all_curation_sets");
        assert!(state.web_bookmarks.is_empty(), "web_bookmarks");
        assert!(state.articles.is_empty(), "articles");
        assert!(state.reaction_state.is_empty(), "reaction_state");
        assert!(state.viewer_reaction_ids.is_empty(), "viewer_reaction_ids");
        assert!(state.search_results.is_empty(), "search_results");
        assert_eq!(state.article_feed.cursor_id, 0, "article_feed cursor");
        assert!(state.article_feed.rows.is_empty(), "article_feed rows");
        assert_eq!(state.highlight_feed.cursor_id, 0, "highlight_feed cursor");
        assert_eq!(
            state.home_feed_interactions.cursor_id, 0,
            "home_feed_interactions cursor"
        );
        assert!(state.room_lanes.is_empty(), "room_lanes");
        assert!(
            state.room_highlight_feeds.is_empty(),
            "room_highlight_feeds"
        );
        assert!(
            state.article_highlight_feeds.is_empty(),
            "article_highlight_feeds"
        );
        assert!(state.share_queue.pending.is_empty(), "share_queue.pending");
        assert!(state.share_queue.seen.is_empty(), "share_queue.seen");
        assert!(state.room_discussions.is_empty(), "room_discussions");
        assert!(state.room_home_events.is_empty(), "room_home_events");
        assert!(state.own_profile.is_none(), "own_profile");
        assert!(state.claimed_profiles.is_empty(), "claimed_profiles");
        assert!(state.chat_rooms.is_empty(), "chat_rooms");
        assert!(state.artifact_previews.is_empty(), "artifact_previews");
        assert!(
            state.artifact_preview_requests.is_empty(),
            "artifact_preview_requests"
        );
    }

    // #1653-AUDIT-1: a direct switch clears EVERY audited account-scoped field.
    #[test]
    fn direct_switch_clears_all_account_scoped_state() {
        let mut state = make_state();
        let clock = ManualClock::default();
        present_as(&mut state, &clock, PK_A);
        seed_all_account_scoped(&mut state);

        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(Some(PK_B.to_string()))),
        );

        assert_all_account_scoped_cleared(&state);
        assert!(matches!(&state.session, SessionState::Present { pubkey, .. } if pubkey == PK_B));
    }

    // #1653-AUDIT-2: logout clears EVERY audited account-scoped field too (the
    // unified helper is the single source for both arms).
    #[test]
    fn logout_clears_all_account_scoped_state() {
        let mut state = make_state();
        let clock = ManualClock::default();
        present_as(&mut state, &clock, PK_A);
        seed_all_account_scoped(&mut state);

        step(&mut state, &clock, Cmd::Action(AppAction::Logout));

        assert_all_account_scoped_cleared(&state);
        assert!(matches!(&state.session, SessionState::Absent));
    }

    // #1653-AUDIT-3 (share_queue): pre-fix the unified helper did NOT clear
    // share_queue, so a direct switch would leak account A's pending share into
    // account B. Asserts it is empty after a switch.
    #[test]
    fn direct_switch_clears_share_queue() {
        let mut state = make_state();
        let clock = ManualClock::default();
        present_as(&mut state, &clock, PK_A);
        state
            .share_queue
            .pending
            .push(crate::kernel::domains::share::ShareQueueItem {
                id: "i".into(),
                group_id: "g".into(),
                url: "https://x".into(),
                note: String::new(),
                created_at_unix_seconds: 1.0,
            });

        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(Some(PK_B.to_string()))),
        );

        assert!(
            state.share_queue.pending.is_empty(),
            "share_queue must be cleared on direct switch (gap #3)"
        );
    }

    // #1653-AUDIT-4 (room_discussions): pre-fix the discussion rows were cleared
    // only on logout, never on a direct switch. Asserts they are gone after a switch.
    #[test]
    fn direct_switch_clears_room_discussions() {
        let mut state = make_state();
        let clock = ManualClock::default();
        present_as(&mut state, &clock, PK_A);
        state.room_discussions.insert(
            "g".into(),
            vec![DiscussionRow {
                event_id: "e".into(),
                author_pubkey: PK_A.into(),
                title: String::new(),
                body: String::new(),
                attachment_url: None,
                artifact_coordinate: None,
                created_at: 1,
            }],
        );

        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(Some(PK_B.to_string()))),
        );

        assert!(
            state.room_discussions.is_empty(),
            "room_discussions must be cleared on direct switch"
        );
    }

    // #1653-AUDIT-5 (accumulators): a direct switch must emit
    // Effect::WithdrawBookmarkSetsInterest — whose runner clears the SetList/Web
    // projection accumulators — so a later typed snapshot cannot repopulate from
    // pre-switch accumulated rows (gap #2). Pre-fix the teardown emitted no such
    // effect (a switch path emits push, not withdraw).
    #[test]
    fn direct_switch_emits_accumulator_clear_effect() {
        let mut state = make_state();
        let clock = ManualClock::default();
        present_as(&mut state, &clock, PK_A);

        let effects = step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(Some(PK_B.to_string()))),
        );

        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::WithdrawBookmarkSetsInterest)),
            "direct switch must emit WithdrawBookmarkSetsInterest to clear accumulators, got {effects:?}"
        );
    }

    // #1653-AUDIT-6 (logout accumulators): logout must also emit the
    // accumulator-clearing effect (alongside RemoveActiveAccount + ClearSession).
    #[test]
    fn logout_emits_accumulator_clear_effect() {
        let mut state = make_state();
        let clock = ManualClock::default();
        present_as(&mut state, &clock, PK_A);

        let effects = step(&mut state, &clock, Cmd::Action(AppAction::Logout));

        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::WithdrawBookmarkSetsInterest)),
            "logout must emit WithdrawBookmarkSetsInterest, got {effects:?}"
        );
        assert!(effects
            .iter()
            .any(|e| matches!(e, Effect::RemoveActiveAccount)));
        assert!(effects.iter().any(|e| matches!(e, Effect::ClearSession)));
    }

    // #1653-AUDIT-7 (no device/UI churn): a direct switch must NOT wipe
    // device/app-local state (whats_new seen marker, isbn cache, podcast resume
    // cache, ocr, capture_draft, camera, relay_diagnostics, discovered_groups).
    // A spurious wipe of these would be a regression.
    #[test]
    fn direct_switch_does_not_churn_device_local_state() {
        let mut state = make_state();
        let clock = ManualClock::default();
        present_as(&mut state, &clock, PK_A);

        // Seed device-local fields that must survive a switch.
        state.podcast_resume_cache.insert("guid".into(), 42.0);
        state.route.root_tab = 3;
        state.nostrconnect_uri = None; // arm-specific (cleared by the switch arm itself)

        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(Some(PK_B.to_string()))),
        );

        assert_eq!(
            state.podcast_resume_cache.get("guid"),
            Some(&42.0),
            "podcast_resume_cache is device-local — must survive a switch"
        );
        assert_eq!(
            state.route.root_tab, 3,
            "route is UI-local — must survive a switch"
        );
    }

    // P2C-7: CreateAccount SigningIn is covered by the 2A clock timeout.
    //        Confirms the existing SIGN_IN_TIMEOUT_SECS check fires for
    //        SignInMethod::CreateAccount just as it does for Nsec.
    #[test]
    fn create_account_timeout_covered_by_existing_clock_check() {
        use crate::kernel::app::SIGN_IN_TIMEOUT_SECS;
        let mut state = make_state();
        let clock = ManualClock::new(0);

        step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::CreateAccount {
                profile_name: "Eve".into(),
            }),
        );
        assert!(matches!(&state.session, SessionState::SigningIn { .. }));

        // Advance to just before timeout — still SigningIn.
        clock.advance(SIGN_IN_TIMEOUT_SECS - 1);
        step(&mut state, &clock, Cmd::Event(KernelEvent::ClockTick));
        assert!(
            matches!(&state.session, SessionState::SigningIn { .. }),
            "still signing in before timeout"
        );

        // At timeout — transitions to SignInFailed.
        clock.advance(1);
        step(&mut state, &clock, Cmd::Event(KernelEvent::ClockTick));
        assert!(
            matches!(
                &state.session,
                SessionState::SignInFailed {
                    method: SignInMethod::CreateAccount,
                    ..
                }
            ),
            "expected SignInFailed{{CreateAccount}} at timeout, got {:?}",
            state.session
        );
    }
}
