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

// ── Phase 4D additions (append-only) ─────────────────────────────────────────
pub(crate) mod search;

// ── Phase 4F additions (append-only) ─────────────────────────────────────────
/// Feed-pull core — ADR-0058 shared engine (Phase 4F).
///
/// `pub` so `AppState` in `app.rs` can name `feed::FeedState` without an alias.
pub mod feed;

// ── Phase 4G additions (append-only) ─────────────────────────────────────────
pub(crate) mod articles_feed;

// ── Phase 4H additions (append-only) ─────────────────────────────────────────
pub(crate) mod highlight_feed;

// ── Phase 4J additions (append-only) ─────────────────────────────────────────
pub(crate) mod home_feed;

// ── Phase 5A additions (append-only) ─────────────────────────────────────────
pub(crate) mod whats_new;

// ── Phase 5C additions (append-only) ─────────────────────────────────────────
pub(crate) mod isbn;

// ── Phase 5K additions (append-only) ─────────────────────────────────────────
pub(crate) mod share;

// ── Phase 5H additions (append-only) ─────────────────────────────────────────
pub(crate) mod podcast;

// ── Phase 5D additions (append-only) ─────────────────────────────────────────
pub(crate) mod ocr;

// ── Phase 5F additions (append-only) ─────────────────────────────────────────
pub(crate) mod capture_draft;

// ── Phase 5G additions (append-only) ─────────────────────────────────────────
pub(crate) mod blossom;

// ── Phase 5I additions (append-only) ─────────────────────────────────────────
pub(crate) mod podcast_transcript;

// ── Phase 5E additions (append-only) ─────────────────────────────────────────
pub(crate) mod camera;

// ── Phase 7 additions (append-only) ─────────────────────────────────────────
pub(crate) mod comments;

// ── Phase 7 feedback additions (append-only) ─────────────────────────────────
pub(crate) mod feedback;
// ── Phase 7 chat additions (append-only) ─────────────────────────────────────
pub(crate) mod chat;
// ── Phase 7 discussions additions (append-only) ──────────────────────────────
pub(crate) mod discussions;

// ── Phase 7 artifact-preview additions (append-only) ─────────────────────────
pub(crate) mod artifact_preview;
