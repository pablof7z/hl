//! Relay write-config domain — Phase 2D.
//!
//! Owns the reducer arms, effect-runner arms, and tests for:
//!   - `AppAction::AddRelay` → `Effect::AddRelay`
//!   - `AppAction::RemoveRelay` → `Effect::RemoveRelay`
//!   - `AppAction::SetRelayRole` → `Effect::SetRelayRole`
//!
//! ## Architectural invariants
//!
//! - **D3**: no wss-scheme literals anywhere in this module. All relay URLs come
//!   from the caller (`AppAction` payload) or from the injected `RelayPolicy`.
//! - **D6**: all actions are fire-and-forget; errors surface as nmp logs, not
//!   as `Result`s crossing the dispatch boundary.
//! - **Live lane untouched**: `HighlighterCore` / `nostr_runtime.rs` /
//!   `relays.rs` are not modified by this slice.
//!
//! ## Extension contract
//!
//! Relay diagnostics / read-views (slice 2E) will add their own arms; they
//! do NOT belong here. Append write-side additions to the bottom of each
//! match in actor.rs.

use nmp_ffi::NmpApp;

use crate::kernel::action::RelayRole;
use crate::kernel::actor::NmpHandle;
use crate::kernel::app::AppState;
use crate::kernel::effect::Effect;

// ─── Reducer arms ───────────────────────────────────────────────────────────

/// Handle `AppAction::AddRelay`.
///
/// No state mutation — relay-list state lives entirely in nmp (the kernel does
/// not cache the relay list in `AppState` in Phase 2D). Returns a single
/// `Effect::AddRelay` with the normalized role string.
pub(crate) fn reduce_action_add_relay(
    _state: &mut AppState,
    url: String,
    role: RelayRole,
) -> Vec<Effect> {
    vec![Effect::AddRelay {
        url,
        role: role.normalize().to_owned(),
    }]
}

/// Handle `AppAction::RemoveRelay`.
pub(crate) fn reduce_action_remove_relay(_state: &mut AppState, url: String) -> Vec<Effect> {
    vec![Effect::RemoveRelay { url }]
}

/// Handle `AppAction::SetRelayRole`.
///
/// nmp's T66a relay-edit model treats `AddRelay` on an existing URL as an
/// upsert (overwriting the role). We follow the same semantics here.
pub(crate) fn reduce_action_set_relay_role(
    _state: &mut AppState,
    url: String,
    role: RelayRole,
) -> Vec<Effect> {
    vec![Effect::SetRelayRole {
        url,
        role: role.normalize().to_owned(),
    }]
}

/// NIP-65 (kind:10002) role string for a relay's read/write flags, or `None` when
/// the relay must be OMITTED from kind:10002 (neither read nor write — a
/// rooms/indexer-only relay, which lives only in the kind:30078 app-data).
///
/// SINGLE SOURCE OF TRUTH for the marker decision: Swift's `dispatchNip65` calls
/// this (via the binding) so the kind:10002 routing can't drift from the protocol
/// rule. Mirrors bespoke `relays.rs::nip65_tags`: (t,t)→"both" (unmarked `r` tag),
/// (t,f)→"read", (f,t)→"write", (f,f)→None (skip). Guarded by the parity test
/// `nip65_relay_role_matches_bespoke_nip65_tags`.
#[uniffi::export]
pub fn nip65_relay_role(read: bool, write: bool) -> Option<String> {
    match (read, write) {
        (true, true) => Some("both".to_string()),
        (true, false) => Some("read".to_string()),
        (false, true) => Some("write".to_string()),
        (false, false) => None,
    }
}

// ─── Effect runners ──────────────────────────────────────────────────────────

/// Execute `Effect::AddRelay`.
///
/// Calls `actor_sender().send(ActorCommand::AddRelay { url, role })`.
/// Fire-and-forget: the `CommandSendError` is discarded (D6). `nmp` is `None`
/// only in unit tests; the call is a no-op in that case (tests inject
/// `KernelEvent`s directly).
pub(crate) fn run_effect_add_relay(url: String, role: String, nmp: Option<&NmpHandle>) {
    let Some(handle) = nmp else { return };
    let nmp_ref: &NmpApp = unsafe { handle.ptr.as_ref() };
    let _ = nmp_ref
        .actor_sender()
        .send(nmp_core::ActorCommand::AddRelay { url, role });
}

/// Execute `Effect::RemoveRelay`.
pub(crate) fn run_effect_remove_relay(url: String, nmp: Option<&NmpHandle>) {
    let Some(handle) = nmp else { return };
    let nmp_ref: &NmpApp = unsafe { handle.ptr.as_ref() };
    let _ = nmp_ref
        .actor_sender()
        .send(nmp_core::ActorCommand::RemoveRelay { url });
}

/// Execute `Effect::SetRelayRole`.
///
/// nmp treats `AddRelay` on an existing URL as an upsert, so we send
/// `ActorCommand::AddRelay` with the new role (T66a upsert semantics).
pub(crate) fn run_effect_set_relay_role(url: String, role: String, nmp: Option<&NmpHandle>) {
    let Some(handle) = nmp else { return };
    let nmp_ref: &NmpApp = unsafe { handle.ptr.as_ref() };
    // T66a: AddRelay on an existing URL is an upsert (overrides the role).
    let _ = nmp_ref
        .actor_sender()
        .send(nmp_core::ActorCommand::AddRelay { url, role });
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::action::{AppAction, RelayRole};
    use crate::kernel::actor::{reduce, Cmd};
    use crate::kernel::app::AppState;
    use crate::kernel::clock::{Clock, ManualClock};
    use crate::kernel::effect::Effect;

    fn make_state() -> AppState {
        AppState::default()
    }

    fn step(state: &mut AppState, clock: &ManualClock, action: AppAction) -> Vec<Effect> {
        let now = clock.now_unix_seconds();
        reduce(state, Cmd::Action(action), now)
    }

    // ── 2D-T1: add_relay_sends_addrelay_command_with_normalized_role ─────────
    //
    // `AppAction::AddRelay` must produce exactly one `Effect::AddRelay` whose
    // `role` is the canonical nmp wire string (e.g. `"both,indexer"`). No
    // state mutation is expected (relay list state lives in nmp, not AppState).
    #[test]
    fn add_relay_sends_addrelay_command_with_normalized_role() {
        let mut state = make_state();
        let clock = ManualClock::new(0);

        let effects = step(
            &mut state,
            &clock,
            AppAction::AddRelay {
                url: "wss://example.relay".to_owned(),
                role: RelayRole::BothIndexer,
            },
        );

        assert_eq!(effects.len(), 1, "AddRelay must produce exactly one effect");
        match &effects[0] {
            Effect::AddRelay { url, role } => {
                assert_eq!(url, "wss://example.relay");
                assert_eq!(
                    role, "both,indexer",
                    "role must be normalized to nmp wire string"
                );
            }
            other => panic!("expected Effect::AddRelay, got {:?}", other),
        }
    }

    // ── 2D-T2: remove_relay_sends_removerelay ────────────────────────────────
    #[test]
    fn remove_relay_sends_removerelay() {
        let mut state = make_state();
        let clock = ManualClock::new(0);

        let effects = step(
            &mut state,
            &clock,
            AppAction::RemoveRelay {
                url: "wss://example.relay".to_owned(),
            },
        );

        assert_eq!(effects.len(), 1);
        assert!(
            matches!(&effects[0], Effect::RemoveRelay { url } if url == "wss://example.relay"),
            "expected Effect::RemoveRelay with correct URL, got {:?}",
            effects[0]
        );
    }

    // ── 2D-T3: set_relay_role_idempotent ─────────────────────────────────────
    //
    // Calling SetRelayRole on the same URL twice must produce the same effect
    // both times (the reducer is a pure function; idempotent in the reducer
    // pass — nmp's upsert semantics handle the dedup at the actor level).
    #[test]
    fn set_relay_role_idempotent() {
        let mut state = make_state();
        let clock = ManualClock::new(0);

        let effects1 = step(
            &mut state,
            &clock,
            AppAction::SetRelayRole {
                url: "wss://relay.example".to_owned(),
                role: RelayRole::Read,
            },
        );
        let effects2 = step(
            &mut state,
            &clock,
            AppAction::SetRelayRole {
                url: "wss://relay.example".to_owned(),
                role: RelayRole::Read,
            },
        );

        // Both passes produce Effect::SetRelayRole with role "read".
        assert_eq!(effects1.len(), 1);
        assert_eq!(effects2.len(), 1);
        match (&effects1[0], &effects2[0]) {
            (
                Effect::SetRelayRole { url: u1, role: r1 },
                Effect::SetRelayRole { url: u2, role: r2 },
            ) => {
                assert_eq!(u1, "wss://relay.example");
                assert_eq!(r1, "read");
                assert_eq!(u1, u2, "url must be identical across both calls");
                assert_eq!(
                    r1, r2,
                    "role must be identical across both calls (idempotent)"
                );
            }
            _ => panic!("expected two Effect::SetRelayRole effects"),
        }
    }

    // Phase 7 parity (gotcha #7): the kernel's kind:10002 (NIP-65) role decision
    // must match bespoke relays.rs::nip65_tags for all 4 read/write cases —
    // INCLUDING (f,f)→omitted. Guards the rooms-only-relay regression (a relay
    // with neither read nor write must NOT appear in kind:10002). No hardcoded
    // expectation: drive both the kernel `nip65_relay_role` and bespoke
    // `nip65_tags` from the same flags and assert the markers agree.
    #[test]
    fn nip65_relay_role_matches_bespoke_nip65_tags() {
        use crate::relays::{nip65_tags, RelayConfig};
        for (read, write) in [(true, true), (true, false), (false, true), (false, false)] {
            let cfg = RelayConfig {
                url: "wss://x".into(),
                read,
                write,
                rooms: false,
                indexer: false,
            };
            let tags = nip65_tags(&[cfg]).expect("nip65_tags");
            match nip65_relay_role(read, write) {
                None => assert!(
                    tags.is_empty(),
                    "({read},{write}): kernel omits from kind:10002 → bespoke must too"
                ),
                Some(role) => {
                    assert_eq!(tags.len(), 1, "({read},{write}): one r-tag");
                    let slice = tags[0].as_slice(); // ["r", url, marker?]
                    let marker = slice.get(2).map(String::as_str);
                    let expected = match role.as_str() {
                        "both" => None, // unmarked r-tag = read+write
                        "read" => Some("read"),
                        "write" => Some("write"),
                        other => panic!("unexpected role {other}"),
                    };
                    assert_eq!(
                        marker, expected,
                        "({read},{write}): kernel role {role} must match bespoke marker"
                    );
                }
            }
        }
    }

    // ── 2D-T5: no_hardcoded_relay_literals_in_kernel ─────────────────────────
    //
    // Grep test: verify no wss-scheme relay URL literals appear in the PRODUCTION
    // section of the new kernel files introduced by slice 2D. This is the D3 gate.
    // We strip the #[cfg(test)] block from the self-scan (relays.rs) because test
    // data intentionally contains relay URL strings; the D3 constraint applies only
    // to production logic.
    #[test]
    fn no_hardcoded_relay_literals_in_kernel() {
        // Build the banned pattern from parts so this test's own source does
        // not trip on its own assertion string (same technique as actor.rs P2C-3).
        let banned = ["wss", "://"].concat();

        // For relays.rs: strip the cfg(test) block before scanning.
        // The marker `#[cfg(test)]` begins the test module; everything after is
        // test-only and is allowed to contain relay URL strings in test data.
        let relays_src = include_str!("relays.rs");
        let relays_prod = relays_src
            .split("#[cfg(test)]")
            .next()
            .unwrap_or(relays_src);

        let action_src = include_str!("../action.rs");
        let effect_src = include_str!("../effect.rs");

        for (name, src) in &[
            ("relays.rs (prod)", relays_prod),
            ("action.rs", action_src),
            ("effect.rs", effect_src),
        ] {
            assert!(
                !src.contains(banned.as_str()),
                "D3 violation: wss-scheme literal found in kernel file `{name}`"
            );
        }
    }

    // ── 2D-T6: dispatch_returns_unit ─────────────────────────────────────────
    //
    // All relay actions must return `Vec<Effect>` (not `Result`); calling
    // `reduce` with any relay action must not panic.
    #[test]
    fn dispatch_returns_unit() {
        let mut state = make_state();
        let clock = ManualClock::new(0);

        // Each relay action must reduce without panic and return a Vec.
        let _: Vec<Effect> = step(
            &mut state,
            &clock,
            AppAction::AddRelay {
                url: String::new(),
                role: RelayRole::Both,
            },
        );
        let _: Vec<Effect> = step(
            &mut state,
            &clock,
            AppAction::RemoveRelay { url: String::new() },
        );
        let _: Vec<Effect> = step(
            &mut state,
            &clock,
            AppAction::SetRelayRole {
                url: String::new(),
                role: RelayRole::Write,
            },
        );
    }

    // ── 2D-T7: relay_role_normalize_covers_all_variants ──────────────────────
    //
    // Every `RelayRole` variant must normalize to one of the valid nmp role
    // strings documented in `nmp-core/src/actor/relay_roles.rs`.
    #[test]
    fn relay_role_normalize_covers_all_variants() {
        let valid_tokens: &[&str] = &[
            "read",
            "write",
            "both",
            "indexer",
            "read,indexer",
            "write,indexer",
            "both,indexer",
        ];
        let roles = [
            RelayRole::Read,
            RelayRole::Write,
            RelayRole::Both,
            RelayRole::Indexer,
            RelayRole::ReadIndexer,
            RelayRole::WriteIndexer,
            RelayRole::BothIndexer,
        ];
        for role in &roles {
            let normalized = role.normalize();
            assert!(
                valid_tokens.contains(&normalized),
                "RelayRole::{:?} normalized to {:?} which is not a valid nmp role string",
                role,
                normalized
            );
        }
    }
}
