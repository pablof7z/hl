//! Profiles domain — claim/release API + `ProfileSnapshot` projection (slice 3D).
//!
//! ## Responsibilities
//!
//! * **READ (own profile)** — decode the `"profile"` typed-sidecar from the NMP
//!   update callback into `AppState::own_profile`. Schema id `"profile"`, file id
//!   `b"KPRF"`. Called from `projections::dispatch_typed_frame` when the arm
//!   matches. Decode fn: `nmp_core::typed_projections::decode_profile`.
//!
//! * **READ (visited profiles)** — populated into `AppState::claimed_profiles`
//!   via `KernelEvent::ProfileCardUpdated` (actor.rs). The former bulk
//!   `"claimed_profiles"` typed sidecar was deleted by NMP ADR-0063 Lane H;
//!   visited-profile resolution is now served by the per-key `refs.profile`
//!   row-delta projection (`nmp_core::refs::RefProfileStore`). Model:
//!   `ProfileCardModel` (raw hex pubkey, optional display fields, nip05,
//!   about, lud16, etc.) — no bech32 or NIP-05 label formatting (D3 / raw-data
//!   doctrine). Swift formats every display string.
//!
//! * **CLAIM / RELEASE** — `AppAction::ClaimProfile { pubkey }` (emitted when a
//!   `ViewId::Profile{pubkey}` view is opened) → `Effect::ClaimProfile { pubkey }`
//!   → `nmp_app_claim_profile(raw_ptr, pubkey, consumer_id, force:0, liveness:Live)`.
//!   `AppAction::ReleaseProfile { pubkey }` (emitted on view close) →
//!   `Effect::ReleaseProfile { pubkey }` → `nmp_app_release_profile(raw_ptr, pubkey,
//!   consumer_id)`. Consumer id is `"hl.profile.<pubkey>"` — stable, one per view.
//!
//! * **SNAPSHOT** — `project_profile_snapshot(state, pubkey)` assembles a
//!   `ProfileSnapshot` from raw `ProfileCardModel` fields + `AppState::is_following`
//!   (Phase 3C) + the subset of `AppState::communities` that matches the viewed
//!   pubkey (Phase 3D communities-on-profile, optional). Phase 4 (articles /
//!   highlights) deferred.
//!
//! ## NMP C ABI
//!
//! `nmp_app_claim_profile` and `nmp_app_release_profile` are `#[no_mangle] extern "C"`
//! in `crates/nmp-ffi/src/timeline.rs:141`. Their FFI signatures are declared
//! in this module so the profiles effect runner can call them without going
//! through an intermediate Rust wrapper.
//!
//! ## Threading
//!
//! `apply_own_profile` runs on the **actor thread** inside
//! `projections::dispatch_typed_frame` (called from `reduce_event`). It is
//! synchronous and non-blocking (FlatBuffers decode only — no I/O). D6: decode
//! errors leave `AppState` fields unchanged (silent no-op).

use std::ffi::CString;
use std::os::raw::{c_char, c_int};

use nmp_core::typed_projections::decode_profile;
use nmp_ffi::NmpApp;

use crate::kernel::actor::NmpHandle;
use crate::kernel::app::AppState;
use crate::kernel::effect::Effect;
use crate::kernel::snapshot::{CommunityRow, ProfileSnapshot, ViewSnapshot};

// ─── nmp-ffi C ABI declarations ─────────────────────────────────────────────

// ADR-0063 Lane D/H: the per-kind `nmp_app_claim_profile` /
// `nmp_app_release_profile` symbols were deleted; profiles now resolve through
// the unified, origin-blind `nmp_app_resolve_ref` / `nmp_app_release_ref`
// C-ABI (both `#[no_mangle] extern "C"` in nmp-ffi/src/resolve_ref.rs). We
// declare them here to drive the profiles effect runner without a wrapper.
#[allow(improper_ctypes)] // NmpApp is opaque; the pointer is safe.
extern "C" {
    fn nmp_app_resolve_ref(
        app: *mut NmpApp,
        namespace: c_int,
        key: *const c_char,
        consumer_id: *const c_char,
        shape: c_int,
        liveness: c_int,
    );
    fn nmp_app_release_ref(
        app: *mut NmpApp,
        namespace: c_int,
        key: *const c_char,
        consumer_id: *const c_char,
    );
}

// `RefNamespace` FFI code: 0 = Profile (resolve_ref.rs `decode_namespace`).
const REF_NAMESPACE_PROFILE: c_int = 0;

// `RefShape` FFI code: 1 = profile.card — the full `ProfileCard` shape used by
// an open profile screen (resolve_ref.rs `decode_shape` `(0,1)`).
const REF_SHAPE_PROFILE_CARD: c_int = 1;

// `RefLiveness` int (D6: 0 = CacheOk, non-zero = Live; refs.rs `from_ffi`).
// We use `1` (Live/Tailing) for open profile views so profile edits arrive
// reactively while the view is open.
const LIVENESS_LIVE: c_int = 1;

/// Stable consumer-id prefix. The per-pubkey suffix makes each profile view
/// an independent refcount owner. Must not contain NUL bytes.
const CONSUMER_ID_PREFIX: &str = "hl.profile.";

// ─── READ side: own profile projection ──────────────────────────────────────

/// Apply a decoded `"profile"` FlatBuffers payload to `AppState::own_profile`.
///
/// Called from `projections::dispatch_typed_frame` when `schema_id == "profile"`.
/// The `"profile"` built-in carries the active account's own kind:0 card; it does
/// NOT require a `ClaimProfile` call (the kernel registers it at boot).
///
/// D6: any decode error leaves `AppState::own_profile` unchanged (silent no-op).
pub(crate) fn apply_own_profile(state: &mut AppState, payload: &[u8]) {
    match decode_profile(payload) {
        Ok(model) => {
            state.own_profile = Some(model);
        }
        Err(e) => {
            tracing::trace!(
                error = %e,
                "profiles::apply_own_profile: decode error — own_profile unchanged (D6)"
            );
        }
    }
}

// ─── READ side: claimed profiles projection (ADR-0063: removed) ──────────────
//
// NMP ADR-0063 Lane H deleted the bulk `"claimed_profiles"` typed sidecar
// (`decode_claimed_profiles` / `ClaimedProfilesModel` / `CLAIMED_PROFILES_*`).
// Visited-profile resolution is now served by the per-key `refs.profile`
// row-delta projection (`nmp_core::refs::RefProfileStore`, sidecar key
// `nmp_core::refs::host_store::REFS_PROFILE_KEY`), which is a STATEFUL merge
// cache (row deltas keyed by `(session_id, snapshot_epoch)`) rather than a
// whole-snapshot replace.
//
// `AppState::claimed_profiles` is retained: it still backs the
// `ViewId::Profile{pubkey}` snapshot (`project_profile_snapshot`) and is
// populated through `KernelEvent::ProfileCardUpdated` (actor.rs). The
// claimed-profiles *sidecar decode path* is the only thing removed here — the
// bulk decoder no longer exists in nmp-core.
//
// FOLLOW-UP (refs.profile adoption): wire `RefProfileStore` into `AppState`,
// add a `refs.profile` arm in `projections::dispatch_typed_frame` that threads
// the frame's `(session_id, snapshot_epoch)` into `RefProfileStore::apply_sidecar`,
// and read visited-profile cards from it. That is a stateful-cache migration
// (frame-identity plumbing) tracked separately from this drift fix.

// ─── WRITE side: view-lifecycle helpers ─────────────────────────────────────

/// Pure function called by the actor loop when a view is opened.
///
/// Returns `[Effect::ClaimProfile { pubkey }]` for `ViewId::Profile` so the
/// actor runs the NMP claim immediately upon view registration — without going
/// through `AppAction::ClaimProfile` in the reducer. This is the primary
/// lifecycle path; `reduce_action_claim_profile` below covers the (rare)
/// direct-dispatch path from native code.
///
/// Returns an empty Vec for all other `ViewId` variants.
pub(crate) fn lifecycle_effects_for_view_open(id: &crate::kernel::view::ViewId) -> Vec<Effect> {
    if let crate::kernel::view::ViewId::Profile { pubkey } = id {
        vec![Effect::ClaimProfile {
            pubkey: pubkey.clone(),
        }]
    } else {
        Vec::new()
    }
}

/// Pure function called by the actor loop when a view is closed.
///
/// Returns `[Effect::ReleaseProfile { pubkey }]` for `ViewId::Profile`.
/// Called before `registry.close()` so the subscription is released before the
/// view leaves the active registry (avoids an orphaned claim).
///
/// Returns an empty Vec for all other `ViewId` variants.
pub(crate) fn lifecycle_effects_for_view_close(id: &crate::kernel::view::ViewId) -> Vec<Effect> {
    if let crate::kernel::view::ViewId::Profile { pubkey } = id {
        vec![Effect::ReleaseProfile {
            pubkey: pubkey.clone(),
        }]
    } else {
        Vec::new()
    }
}

// ─── WRITE side: reduce_action helpers ──────────────────────────────────────

/// Handle `AppAction::ClaimProfile { pubkey }` — emit `Effect::ClaimProfile`.
///
/// Secondary path: covers the rare case where native code dispatches
/// `AppAction::ClaimProfile` directly (e.g., prefetch before opening the view).
/// The primary lifecycle path is `lifecycle_effects_for_view_open` above,
/// called by the actor loop on `Cmd::OpenView(ViewId::Profile{..})`.
pub(crate) fn reduce_action_claim_profile(pubkey: String) -> Vec<Effect> {
    vec![Effect::ClaimProfile { pubkey }]
}

/// Handle `AppAction::ReleaseProfile { pubkey }` — emit `Effect::ReleaseProfile`.
///
/// Secondary path: covers the rare case where native code dispatches
/// `AppAction::ReleaseProfile` directly. The primary lifecycle path is
/// `lifecycle_effects_for_view_close` above, called on `Cmd::CloseView`.
pub(crate) fn reduce_action_release_profile(pubkey: String) -> Vec<Effect> {
    vec![Effect::ReleaseProfile { pubkey }]
}

// ─── Effect runners ──────────────────────────────────────────────────────────

/// Execute `Effect::ClaimProfile` — calls `nmp_app_resolve_ref` for the
/// `(Profile, pubkey)` reference under the stable consumer-id
/// `"hl.profile.<pubkey>"`, `shape = profile.card`, `liveness = Live`.
///
/// Live liveness means a `Tailing` kind:0 subscription stays open while the
/// view is open so profile edits arrive reactively. CacheOk would be correct
/// for feed-row avatars, but the Profile view needs live updates.
///
/// No-op if `nmp` is `None` (test mode — tests inject `ProfileCardUpdated`
/// directly into the reducer via `Cmd::Event`).
pub(crate) fn run_effect_claim_profile(pubkey: String, nmp: Option<&NmpHandle>) {
    let Some(handle) = nmp else { return };

    let consumer_id = format!("{CONSUMER_ID_PREFIX}{pubkey}");

    let pubkey_c = match CString::new(pubkey) {
        Ok(s) => s,
        Err(_) => return,
    };
    let consumer_c = match CString::new(consumer_id) {
        Ok(s) => s,
        Err(_) => return,
    };

    // SAFETY: handle.ptr is a valid non-null NmpApp pointer kept alive by
    // NmpHandle for the full actor lifetime. pubkey_c and consumer_c are valid
    // CStrings alive for the duration of this call. nmp_app_resolve_ref is
    // FFI-clean (null/invalid key is a silent no-op — nmp D6 contract).
    unsafe {
        nmp_app_resolve_ref(
            handle.ptr.as_ptr(),
            REF_NAMESPACE_PROFILE,
            pubkey_c.as_ptr(),
            consumer_c.as_ptr(),
            REF_SHAPE_PROFILE_CARD,
            LIVENESS_LIVE, // liveness = Live (Tailing sub)
        );
    }
}

/// Execute `Effect::ReleaseProfile` — calls `nmp_app_release_ref` for the
/// `(Profile, pubkey)` reference under consumer-id `"hl.profile.<pubkey>"`.
///
/// Decrements the per-consumer refcount. When the count reaches zero NMP
/// cancels the kind:0 subscription and tears down the resolver slot. D6:
/// null/invalid key is a silent no-op in nmp-ffi.
///
/// No-op if `nmp` is `None` (test mode).
pub(crate) fn run_effect_release_profile(pubkey: String, nmp: Option<&NmpHandle>) {
    let Some(handle) = nmp else { return };

    let consumer_id = format!("{CONSUMER_ID_PREFIX}{pubkey}");

    let pubkey_c = match CString::new(pubkey) {
        Ok(s) => s,
        Err(_) => return,
    };
    let consumer_c = match CString::new(consumer_id) {
        Ok(s) => s,
        Err(_) => return,
    };

    // SAFETY: handle.ptr is a valid non-null NmpApp pointer kept alive by
    // NmpHandle for the full actor lifetime. CStrings alive for duration of call.
    unsafe {
        nmp_app_release_ref(
            handle.ptr.as_ptr(),
            REF_NAMESPACE_PROFILE,
            pubkey_c.as_ptr(),
            consumer_c.as_ptr(),
        );
    }
}

// ─── Snapshot projection ─────────────────────────────────────────────────────

/// Assemble a `ViewSnapshot::Profile` for the given `pubkey`.
///
/// Sources:
/// - Identity fields: `ProfileCardModel` from `AppState::claimed_profiles`
///   (or `AppState::own_profile` if the viewed pubkey is the active account).
/// - `is_following`: derived from `AppState::is_following(pubkey)` (3C).
/// - `communities`: the subset of `AppState::communities` that the viewed
///   pubkey is a member of. Phase 3D surfaces communities from the active
///   account's joined-groups list (the intersection is approximate — NIP-29
///   member lists are not yet projected individually per Phase 3D spec; Phase
///   4 will add per-pubkey group-membership interests). Kept as `Vec<CommunityRow>`
///   to satisfy the spec; the vec may be empty if the viewed pubkey is not in
///   any of the active account's joined rooms.
///
/// Returns `None` if neither `claimed_profiles` nor `own_profile` has a card
/// for this pubkey (data not yet arrived — the view renders a loading state).
///
/// Called from `actor::project_snapshot` on the actor thread for every open
/// `ViewId::Profile{pubkey}`. Non-blocking (HashMap lookup + Vec clone only).
pub(crate) fn project_profile_snapshot(state: &AppState, pubkey: &str) -> Option<ViewSnapshot> {
    // Try claimed_profiles first; fall back to own_profile for the active account.
    let card = state.claimed_profiles.get(pubkey).or_else(|| {
        // If the viewed pubkey is the active account, use the own_profile card.
        state.own_profile.as_ref().filter(|p| p.pubkey == pubkey)
    })?;

    let is_following = state.is_following(pubkey);

    // Collect communities that include this pubkey as a member.
    // Phase 3D: we surface the active account's joined communities as context;
    // per-pubkey membership interests (to truly show which rooms they're in)
    // are deferred to Phase 4. This gives the Profile view the communities
    // data required by the spec without a per-pubkey interest lookup.
    let communities: Vec<CommunityRow> = state.communities.clone();

    Some(ViewSnapshot::Profile(ProfileSnapshot {
        pubkey: card.pubkey.clone(),
        display_name: card.display_name.clone(),
        name: card.name.clone(),
        raw_display_name: card.raw_display_name.clone(),
        picture_url: card.picture_url.clone(),
        banner: card.banner.clone(),
        website: card.website.clone(),
        nip05: card.nip05.clone(),
        about: card.about.clone(),
        lud16: card.lud16.clone(),
        is_following,
        communities,
    }))
}

// ─── Identity-change handler ─────────────────────────────────────────────────

/// Clear profile state on `IdentityChanged(None)` or logout.
///
/// Called from `auth::reduce_event_identity_changed` when the identity is
/// removed. Own profile and claimed profiles belong to the departing account
/// and must not survive into the next session.
pub(crate) fn clear_on_identity_lost(state: &mut AppState) {
    state.own_profile = None;
    state.claimed_profiles.clear();
}

// ─── Unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use nmp_core::typed_projections::ProfileCardModel;
    use crate::kernel::action::{AppAction, KernelEvent};
    use crate::kernel::actor::{reduce, Cmd};
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

    fn make_profile_card(pubkey: &str) -> ProfileCardModel {
        ProfileCardModel {
            pubkey: pubkey.to_string(),
            npub: String::new(),
            display_name: Some("Alice".to_string()),
            name: Some("alice".to_string()),
            raw_display_name: Some("Alice".to_string()),
            display_name_camel: None,
            picture_url: Some("https://example.com/avatar.jpg".to_string()),
            banner: None,
            website: Some("https://example.com".to_string()),
            nip05: "alice@example.com".to_string(),
            about: "I am Alice.".to_string(),
            lud16: Some("alice@getalby.com".to_string()),
            lud06: None,
            lnurl: None,
        }
    }

    // 3D-T1: open_view(ViewId::Profile{pk}) → lifecycle_effects_for_view_open → Effect::ClaimProfile{pk}
    //
    // The actor loop calls `lifecycle_effects_for_view_open` when it receives
    // `Cmd::OpenView(ViewId::Profile{..})`. This is the PRIMARY lifecycle path
    // that ties ClaimProfile to view open (the finding the codex review raised).
    // We test the extracted pure function directly so the test stays synchronous.
    #[test]
    fn claim_profile_on_view_open_emits_effect() {
        let pk = "deadbeef00000000000000000000000000000000000000000000000000000001";
        let id = ViewId::Profile {
            pubkey: pk.to_string(),
        };
        let effects = lifecycle_effects_for_view_open(&id);
        assert_eq!(
            effects.len(),
            1,
            "opening a Profile view must emit exactly one lifecycle effect"
        );
        match &effects[0] {
            Effect::ClaimProfile { pubkey } => {
                assert_eq!(pubkey, pk, "pubkey threads through verbatim");
            }
            other => panic!("expected Effect::ClaimProfile, got {:?}", other),
        }

        // Non-Profile views produce no lifecycle effects.
        let effects_non_profile = lifecycle_effects_for_view_open(&ViewId::AppRoot);
        assert!(
            effects_non_profile.is_empty(),
            "non-Profile views must emit no lifecycle effects on open"
        );
    }

    // 3D-T2: close_view(ViewId::Profile{pk}) → lifecycle_effects_for_view_close → Effect::ReleaseProfile{pk}
    //
    // The actor loop calls `lifecycle_effects_for_view_close` when it receives
    // `Cmd::CloseView(ViewId::Profile{..})`. This is the PRIMARY lifecycle path
    // that ties ReleaseProfile to view close (the finding the codex review raised).
    #[test]
    fn release_profile_on_view_close_emits_effect() {
        let pk = "cafebabe00000000000000000000000000000000000000000000000000000002";
        let id = ViewId::Profile {
            pubkey: pk.to_string(),
        };
        let effects = lifecycle_effects_for_view_close(&id);
        assert_eq!(
            effects.len(),
            1,
            "closing a Profile view must emit exactly one lifecycle effect"
        );
        match &effects[0] {
            Effect::ReleaseProfile { pubkey } => {
                assert_eq!(pubkey, pk, "pubkey threads through verbatim");
            }
            other => panic!("expected Effect::ReleaseProfile, got {:?}", other),
        }

        // Non-Profile views produce no lifecycle effects.
        let effects_non_profile = lifecycle_effects_for_view_close(&ViewId::AppRoot);
        assert!(
            effects_non_profile.is_empty(),
            "non-Profile views must emit no lifecycle effects on close"
        );
    }

    // 3D-T3: profile_frame_updates_state_raw_fields
    //
    // Injecting KernelEvent::ProfileCardUpdated stores the ProfileCardModel in
    // AppState::claimed_profiles and makes project_profile_snapshot return the
    // raw fields (no label-stripping, no bech32 encoding).
    #[test]
    fn profile_frame_updates_state_raw_fields() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let pk = "aabbcc0000000000000000000000000000000000000000000000000000000001";
        let card = make_profile_card(pk);

        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::ProfileCardUpdated {
                pubkey: pk.to_string(),
                card: Box::new(card.clone()),
            }),
        );

        // The model must be stored in claimed_profiles.
        assert!(
            state.claimed_profiles.contains_key(pk),
            "ProfileCardUpdated must insert into claimed_profiles"
        );

        let stored = &state.claimed_profiles[pk];
        assert_eq!(stored.pubkey, pk, "pubkey stored verbatim");
        assert_eq!(
            stored.nip05, "alice@example.com",
            "nip05 stored raw — no label strip"
        );
        assert_eq!(stored.about, "I am Alice.", "about stored raw");
        // display_name is Option<String> — assert raw, no formatting
        assert_eq!(
            stored.display_name.as_deref(),
            Some("Alice"),
            "display_name is raw Option<String>"
        );

        // Project a snapshot and verify raw field pass-through.
        let snap = project_profile_snapshot(&state, pk).unwrap();
        if let ViewSnapshot::Profile(ps) = snap {
            assert_eq!(ps.pubkey, pk);
            assert_eq!(ps.nip05, "alice@example.com");
            assert_eq!(ps.about, "I am Alice.");
            assert_eq!(ps.display_name.as_deref(), Some("Alice"));
            assert!(!ps.nip05.starts_with("_@"), "nip05 must NOT be stripped");
        } else {
            panic!("expected Profile snapshot");
        }
    }

    // 3D-T4: profile_snapshot_includes_is_following_from_follows
    //
    // ProfileSnapshot::is_following is derived from AppState::follows (Phase 3C),
    // not from the profile card itself. Verify the derivation.
    #[test]
    fn profile_snapshot_includes_is_following_from_follows() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let pk = "1111110000000000000000000000000000000000000000000000000000000001";
        let card = make_profile_card(pk);

        // Insert profile but do NOT add to follows.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::ProfileCardUpdated {
                pubkey: pk.to_string(),
                card: Box::new(card.clone()),
            }),
        );

        let snap1 = project_profile_snapshot(&state, pk).unwrap();
        if let ViewSnapshot::Profile(ps) = snap1 {
            assert!(
                !ps.is_following,
                "is_following must be false before FollowListUpdated"
            );
        }

        // Add to follows via FollowListUpdated.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::FollowListUpdated(vec![pk.to_string()])),
        );

        let snap2 = project_profile_snapshot(&state, pk).unwrap();
        if let ViewSnapshot::Profile(ps) = snap2 {
            assert!(
                ps.is_following,
                "is_following must be true after FollowListUpdated includes pubkey"
            );
        } else {
            panic!("expected Profile snapshot after FollowListUpdated");
        }
    }

    // 3D-T5: malformed_profile_frame_no_ops
    //
    // apply_own_profile with garbage bytes must not panic or corrupt AppState (D6).
    // (The bulk `claimed_profiles` sidecar decode path was removed by NMP
    // ADR-0063 Lane H; `claimed_profiles` is now populated via
    // `KernelEvent::ProfileCardUpdated`.)
    #[test]
    fn malformed_profile_frame_no_ops() {
        let mut state = make_state();
        // Seed own_profile with a known value.
        state.own_profile = Some(make_profile_card(
            "aaaa000000000000000000000000000000000000000000000000000000000001",
        ));
        // Garbage bytes must leave own_profile unchanged.
        apply_own_profile(&mut state, b"NOT A VALID FLATBUFFER");
        assert!(
            state.own_profile.is_some(),
            "malformed payload must leave own_profile unchanged (D6)"
        );
    }

    // 3D-T6: closed_profile_view_emits_no_snapshot
    //
    // project_profile_snapshot returns None when the pubkey has no data in
    // claimed_profiles or own_profile (view renders a loading state).
    #[test]
    fn closed_profile_view_emits_no_snapshot() {
        let state = make_state();
        let pk = "nodata000000000000000000000000000000000000000000000000000000001";
        let result = project_profile_snapshot(&state, pk);
        assert!(
            result.is_none(),
            "snapshot must be None when no profile data exists"
        );
    }

    // 3D-T7: profile_cleared_on_identity_changed_none
    //
    // own_profile and claimed_profiles must be wiped when IdentityChanged(None)
    // fires so stale profile data from the previous account does not survive.
    #[test]
    fn profile_cleared_on_identity_changed_none() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let pk = "deadbeef00000000000000000000000000000000000000000000000000000001";

        // Seed state.
        state.own_profile = Some(make_profile_card(pk));
        state
            .claimed_profiles
            .insert(pk.to_string(), make_profile_card(pk));

        // IdentityChanged(None) must clear profiles.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(None)),
        );

        assert!(
            state.own_profile.is_none(),
            "own_profile must be None after IdentityChanged(None)"
        );
        assert!(
            state.claimed_profiles.is_empty(),
            "claimed_profiles must be empty after IdentityChanged(None)"
        );
    }

    // 3D-T8: profile_cleared_on_logout
    //
    // Logout must also clear profile state via the same path as IdentityChanged(None).
    #[test]
    fn profile_cleared_on_logout() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let pk = "cafebabe00000000000000000000000000000000000000000000000000000002";

        // Put the actor in a signed-in state first so Logout is reachable.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(Some(pk.to_string()))),
        );

        state.own_profile = Some(make_profile_card(pk));
        state
            .claimed_profiles
            .insert(pk.to_string(), make_profile_card(pk));

        step(&mut state, &clock, Cmd::Action(AppAction::Logout));

        assert!(
            state.own_profile.is_none(),
            "own_profile must be None after Logout"
        );
        assert!(
            state.claimed_profiles.is_empty(),
            "claimed_profiles must be empty after Logout"
        );
    }

    // 3D-T9: claim_release_fire_and_forget (Non-Negotiable #3)
    //
    // Both ClaimProfile and ReleaseProfile dispatch must return () — they are
    // fire-and-forget actions with no Result propagation.
    #[test]
    fn claim_release_fire_and_forget() {
        let mut state = make_state();
        let clock = ManualClock::new(0);
        let pk = "aaaa000000000000000000000000000000000000000000000000000000000001";

        // Both return Vec<Effect> which models the () contract (Non-Negotiable #3).
        let _claim: Vec<Effect> = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::ClaimProfile {
                pubkey: pk.to_string(),
            }),
        );
        let _release: Vec<Effect> = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::ReleaseProfile {
                pubkey: pk.to_string(),
            }),
        );
        // No panic, no Result — fire-and-forget contract satisfied.
    }

    // 3D-T10: own_profile_surfaces_for_active_account_pubkey
    //
    // If the viewed pubkey matches the active account's own_profile, the snapshot
    // must fall back to own_profile when claimed_profiles has no entry for it.
    #[test]
    fn own_profile_surfaces_for_active_account_pubkey() {
        let mut state = make_state();
        let pk = "bbbb000000000000000000000000000000000000000000000000000000000001";

        // Set own_profile — simulates the "profile" sidecar for the active account.
        state.own_profile = Some(make_profile_card(pk));

        // claimed_profiles is empty — should fall back to own_profile.
        let snap = project_profile_snapshot(&state, pk).unwrap();
        if let ViewSnapshot::Profile(ps) = snap {
            assert_eq!(ps.pubkey, pk, "own_profile pubkey must match");
            assert_eq!(
                ps.display_name.as_deref(),
                Some("Alice"),
                "own_profile display_name passed through"
            );
        } else {
            panic!("expected Profile snapshot from own_profile fallback");
        }
    }
}
