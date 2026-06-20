//! Per-domain handler modules for the kernel actor.
//!
//! Each module owns the reducer arms, event arms, clock-check arms, effect-runner
//! arms, and snapshot helpers for its slice of `AppState`. The top-level
//! dispatchers in `actor.rs` route to these handlers so each future migration
//! slice adds its own file + a single dispatch line rather than editing the
//! shared monolith match statements.

pub(crate) mod auth;
pub(crate) mod bookmarks;
pub(crate) mod communities;
pub(crate) mod discovery;
pub(crate) mod follows;
pub(crate) mod profiles;
pub(crate) mod projections;
pub(crate) mod reactions;
pub(crate) mod relay_diagnostics;
pub(crate) mod relays;
pub(crate) mod room_home;
pub(crate) mod route;
pub(crate) mod session;

// ── Phase 4A additions (append-only) ─────────────────────────────────────────
pub(crate) mod articles;
