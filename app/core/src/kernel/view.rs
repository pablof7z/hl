//! View registry — maps open `ViewId`s to their `ViewRoute` and the last
//! emitted snapshot. The kernel only recomputes and emits snapshots for
//! currently-open views (Non-Negotiable #7 / D5).

use std::collections::HashMap;

use crate::kernel::snapshot::ViewSnapshot;

/// Stable identifier for an open view instance.
///
/// In Phase 1 there are two: the app-root decision surface and the root shell.
/// Later phases add per-screen identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, uniffi::Enum)]
pub enum ViewId {
    /// The root entry point that decides which top-level route to show
    /// (`Onboarding`, `Login`, or `RootShell`).
    AppRoot,
    /// The main tab shell (visible only when session is present).
    RootShell,

    // ── Phase 2E additions ────────────────────────────────────────────────────
    /// Network settings overview screen (relay list + role configuration).
    NetworkSettings,
    /// Relay-diagnostics detail screen (connection stats, counters, sub list).
    RelayDiagnostics,

    // ── Phase 3B additions (append-only) ─────────────────────────────────────
    /// Joined-groups / communities list for the active account.
    Communities,

    // ── Phase 3E additions (append-only) ─────────────────────────────────────
    /// Room explorer / discovery screen.
    RoomExplorer,

    // ── Phase 3D additions (append-only) ─────────────────────────────────────
    /// Profile detail view for a specific pubkey.
    ///
    /// `pubkey` is a raw 64-char lowercase hex pubkey. The view is opened by
    /// `AppAction::ClaimProfile{pubkey}` (which also sends `Effect::ClaimProfile`
    /// to register the NMP interest) and closed by `AppAction::ReleaseProfile`.
    Profile {
        /// Raw 64-char hex pubkey of the profile being viewed.
        pubkey: String,
    },

    // ── Phase 3F additions (append-only) ─────────────────────────────────────
    /// Per-room home shell view for a specific NIP-29 group.
    ///
    /// `group_id` is the NIP-29 local group id (the `["d", _]` tag value).
    /// Opened by the native UI when the user taps on a room; the actor wires
    /// the `GroupEventsProjection` via `Effect::WireGroupEvents` on view open
    /// and releases the buffer via `Effect::ReleaseGroupEvents` on close.
    RoomHome {
        /// NIP-29 local group id.
        group_id: String,
    },

    // ── Phase 4C additions (append-only) ─────────────────────────────────────
    /// Bookmark library view — the active account's NIP-51 kind:10003 list.
    /// Kind:10003 article-bookmark toggle only (sets/web/curation stay bespoke).
    Bookmarks,
}

/// Which projection to compute for a registered view.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum ViewRoute {
    AppRoot,
    RootShell,

    // ── Phase 2E additions ────────────────────────────────────────────────────
    /// Network settings projection — relay list with raw role/status data.
    NetworkSettings,
    /// Relay-diagnostics projection — raw counters and connection state per relay.
    RelayDiagnostics,

    // ── Phase 3B additions (append-only) ─────────────────────────────────────
    Communities,

    // ── Phase 3E additions (append-only) ─────────────────────────────────────
    /// Room explorer / discovery screen.
    RoomExplorer,

    // ── Phase 3D additions (append-only) ─────────────────────────────────────
    /// Profile detail projection — `ProfileSnapshot` (identity + relationship +
    /// communities). Articles/highlights deferred to Phase 4.
    Profile {
        /// Raw 64-char hex pubkey of the profile to project.
        pubkey: String,
    },

    // ── Phase 3F additions (append-only) ─────────────────────────────────────
    /// Room-home projection — `RoomHomeSnapshot` (header + metadata + membership
    /// + empty lanes). Lane bodies deferred to Phase 4.
    RoomHome {
        /// NIP-29 local group id.
        group_id: String,
    },

    // ── Phase 4C additions (append-only) ─────────────────────────────────────
    /// Bookmarks projection — `BookmarksSnapshot` (raw kind:10003 rows).
    /// Kind:10003 article-bookmark toggle only.
    Bookmarks,
}

/// Tracks open views and their last-emitted snapshots.
#[derive(Debug, Default)]
pub struct ViewRegistry {
    views: HashMap<ViewId, (ViewRoute, Option<ViewSnapshot>)>,
}

impl ViewRegistry {
    /// Register a view. If it was already open, the existing snapshot is kept.
    pub fn open(&mut self, id: ViewId, route: ViewRoute) {
        self.views.entry(id).or_insert((route, None));
    }

    /// Deregister a view and discard its last snapshot.
    pub fn close(&mut self, id: &ViewId) {
        self.views.remove(id);
    }

    pub fn is_open(&self, id: &ViewId) -> bool {
        self.views.contains_key(id)
    }

    pub fn open_ids(&self) -> impl Iterator<Item = &ViewId> {
        self.views.keys()
    }

    /// The last snapshot that was stored for this view, if any.
    pub fn last_snapshot(&self, id: &ViewId) -> Option<&ViewSnapshot> {
        self.views.get(id)?.1.as_ref()
    }

    /// Overwrite the stored snapshot for an open view. No-op for closed views.
    pub fn update_snapshot(&mut self, id: &ViewId, snapshot: ViewSnapshot) {
        if let Some(entry) = self.views.get_mut(id) {
            entry.1 = Some(snapshot);
        }
    }

    /// Current snapshot for pull access (`current_snapshot` FFI method).
    pub fn current_snapshot(&self, id: &ViewId) -> Option<ViewSnapshot> {
        self.views.get(id)?.1.clone()
    }

    /// The route registered for this view.
    pub fn route(&self, id: &ViewId) -> Option<&ViewRoute> {
        Some(&self.views.get(id)?.0)
    }

    /// Number of open views.
    pub fn open_count(&self) -> usize {
        self.views.len()
    }
}
