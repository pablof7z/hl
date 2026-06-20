//! Share-extension capability types — Phase 5K.
//!
//! The iOS App Group container is a native resource; draining and writing it is
//! a capability (the kernel cannot access the App Group directly). All business
//! logic — dedupe, partition, target projections, share-URL assembly — lives in
//! the Rust domain (`kernel/domains/share.rs`). Native only executes raw file
//! I/O (D7: native capabilities execute, Rust decides).

/// What the kernel is asking the native share bridge to do.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum ShareOp {
    /// Read `pending-shares-v1.json` from the App Group, return its contents
    /// as raw payloads, then delete the file. The kernel deduplicate and
    /// processes the items; native just reads and clears the handoff store.
    DrainQueue,
    /// Write the communities-picker JSON the iOS share extension reads at
    /// launch time into the App Group container
    /// (`joined-communities-v1.json`). The JSON bytes are built by the kernel
    /// from `AppState::communities` (ported `communities_snapshot_json` logic).
    /// Native atomically overwrites the file.
    WriteCommunitiesSnapshot {
        /// UTF-8 JSON bytes produced by the kernel.
        json_bytes: Vec<u8>,
    },
}

/// One raw share payload from the App Group handoff store.
///
/// All fields are raw strings (D1: no formatted strings, no decoded/validated
/// types across the capability boundary). The kernel validates and processes
/// them. `id` is the share item's stable identifier (for dedupe).
#[derive(Debug, Clone, uniffi::Record)]
pub struct RawSharePayload {
    /// Stable identifier for this share item (for dedupe by the kernel).
    pub id: String,
    /// NIP-29 local group id the share should be posted into.
    pub group_id: String,
    /// URL or text content the user shared.
    pub url: String,
    /// Optional note the user added in the share extension UI.
    pub note: String,
    /// UNIX second timestamp when the share was queued.
    pub created_at_unix_seconds: f64,
}

/// Raw result from the native share capability bridge, reported via
/// `provide_capability_result`. Errors are data (D6).
#[derive(Debug, Clone, uniffi::Enum)]
pub enum ShareResult {
    /// `DrainQueue` completed. Contains all pending share payloads (possibly
    /// empty when no shares were queued). The native side has already deleted
    /// the file.
    Pending(Vec<RawSharePayload>),
    /// `WriteCommunitiesSnapshot` completed successfully.
    CommunitiesWritten,
    /// A native OS error occurred (file-not-found counts as empty, not an error
    /// — see note in kernel domain handler). Errors are data (D6).
    Error(String),
}
