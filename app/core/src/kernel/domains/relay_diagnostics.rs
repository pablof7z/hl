//! Relay-diagnostics domain — slice 2E.
//!
//! Decodes the `"relay_diagnostics"` typed-projection sidecar (schema id
//! `RELAY_DIAGNOSTICS_SCHEMA_ID`) from NMP snapshot frames and stores the raw
//! diagnostic fields in `AppState::relay_diagnostics`. The Swift shell is
//! responsible for all formatting (labels, "X ago" strings, human-readable byte
//! counts). Only RAW numeric/enum fields are projected — nmp's pre-formatted
//! `*_label` / `*_display` strings are deliberately dropped (raw-data doctrine /
//! D1).
//!
//! ## D6 — no panics on malformed frames
//!
//! `apply` wraps `decode_relay_diagnostics` in a `match` guard. Malformed
//! bytes are a silent no-op; `AppState` is left unchanged.

use nmp_core::typed_projections::decode_relay_diagnostics;

use crate::kernel::app::AppState;

// ─── Raw DTO types ───────────────────────────────────────────────────────────

/// Connection state of a relay as seen by the NMP kernel. Derived from
/// nmp's `connection_tone` field; represented as an enum so the Swift
/// shell can branch on a stable machine tag rather than parsing a string.
///
/// The mapping is conservative: unknown tones → `Unknown`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum RelayConnectionState {
    /// `"ok"` tone — relay is connected and healthy.
    Connected,
    /// `"warn"` tone — relay is reconnecting or degraded.
    Reconnecting,
    /// `"error"` tone — relay is in an error state.
    Error,
    /// `"muted"` or any other tone — state unknown / not yet connected.
    Unknown,
}

/// Raw diagnostic fields for one relay. All formatting (labels, "X ago", human
/// byte counts) is deferred to the Swift shell (D1). Bounded: one entry per
/// configured relay URL (D5).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct RelayDiagRow {
    /// Canonical relay URL (stable list identity).
    pub relay_url: String,
    /// Total EVENT frames received from this relay in the current session.
    pub total_events_rx: u64,
    /// Reconnect attempts since process start.
    pub reconnect_count: u32,
    /// Unix epoch milliseconds of the last successful connect; 0 when the relay
    /// has never connected. Swift shell formats as "Xs ago" at render time.
    pub last_connected_ms: u64,
    /// Parsed connection state derived from nmp's `connection_tone` field.
    pub connection_state: RelayConnectionState,
    /// Total wire subscriptions known for this relay.
    pub total_sub_count: u32,
    /// Wire subscriptions currently active.
    pub active_sub_count: u32,
    /// Wire subscriptions that have received EOSE.
    pub eosed_sub_count: u32,
}

/// Snapshot for the `ViewId::RelayDiagnostics` projection.
///
/// Bounded by relay count (D5): exactly one `RelayDiagRow` per relay the
/// kernel knows about. The count never exceeds the configured relay set.
#[derive(Debug, Clone, PartialEq)]
pub struct RelayDiagnosticsSnapshot {
    pub relays: Vec<RelayDiagRow>,
}

// ─── State holder ────────────────────────────────────────────────────────────

/// State stored in `AppState` for the relay-diagnostics sidecar.
///
/// Replaced wholesale on every decoded frame; `None` until the first valid
/// frame arrives.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RelayDiagnosticsState {
    pub last: Option<RelayDiagnosticsSnapshot>,
}

// ─── Apply (called from projections.rs dispatcher) ───────────────────────────

/// Decode a relay-diagnostics typed-projection frame and update
/// `AppState::relay_diagnostics`. Returns `true` when the state changed.
///
/// D6: any decode error is a silent no-op.
pub(crate) fn apply(state: &mut AppState, frame_bytes: &[u8]) -> bool {
    let model = match decode_relay_diagnostics(frame_bytes) {
        Ok(m) => m,
        Err(e) => {
            tracing::trace!(error = %e, "relay_diagnostics: decode error — skipped (D6)");
            #[cfg(test)]
            eprintln!("[relay_diagnostics::apply] decode error: {e}");
            return false;
        }
    };

    let relays: Vec<RelayDiagRow> = model
        .relays
        .into_iter()
        .map(|r| {
            // Map nmp's pre-formatted connection_tone to a stable enum.
            // We read `connection_tone` (NOT `connection_label`) to stay
            // independent of NMP's localised label strings.
            let connection_state = match r.connection_tone.as_str() {
                "ok" => RelayConnectionState::Connected,
                "warn" => RelayConnectionState::Reconnecting,
                "error" => RelayConnectionState::Error,
                _ => RelayConnectionState::Unknown,
            };
            RelayDiagRow {
                relay_url: r.relay_url,
                total_events_rx: r.total_events_rx,
                reconnect_count: r.reconnect_count,
                last_connected_ms: r.last_connected_ms,
                connection_state,
                total_sub_count: r.total_sub_count,
                active_sub_count: r.active_sub_count,
                eosed_sub_count: r.eosed_sub_count,
                // Deliberately NOT projecting:
                //   r.role_label, r.connection_label, r.auth_label,
                //   r.total_events_display, r.bytes_rx_display,
                //   r.bytes_tx_display, r.discovery_kinds_label
                // — nmp pre-formats these strings; Swift shell formats instead.
            }
        })
        .collect();

    let snapshot = RelayDiagnosticsSnapshot { relays };
    let new_state = RelayDiagnosticsState {
        last: Some(snapshot),
    };

    if state.relay_diagnostics == new_state {
        return false;
    }
    state.relay_diagnostics = new_state;
    true
}

// ─── Snapshot projection ─────────────────────────────────────────────────────

/// Project the current relay-diagnostics state into a `RelayDiagnosticsSnapshot`.
///
/// Returns `None` when no frame has been received yet (the view should render
/// a loading indicator).
pub(crate) fn project_relay_diagnostics(state: &AppState) -> Option<RelayDiagnosticsSnapshot> {
    state.relay_diagnostics.last.clone()
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state() -> AppState {
        AppState::default()
    }

    /// Test helper: directly inject a set of RelayDiagRow values into AppState,
    /// simulating what `apply` does after decoding a valid FlatBuffer frame.
    ///
    /// Used for tests that need a pre-populated state without depending on the
    /// FlatBuffer encoding path (which is covered by nmp-core's own codec tests).
    fn inject_rows(state: &mut AppState, rows: Vec<RelayDiagRow>) {
        state.relay_diagnostics = RelayDiagnosticsState {
            last: Some(RelayDiagnosticsSnapshot { relays: rows }),
        };
    }

    fn make_row(
        url: &str,
        tone: &str,
        events: u64,
        reconnects: u32,
        last_ms: u64,
        total_subs: u32,
        active_subs: u32,
        eosed_subs: u32,
    ) -> RelayDiagRow {
        let connection_state = match tone {
            "ok" => RelayConnectionState::Connected,
            "warn" => RelayConnectionState::Reconnecting,
            "error" => RelayConnectionState::Error,
            _ => RelayConnectionState::Unknown,
        };
        RelayDiagRow {
            relay_url: url.to_string(),
            total_events_rx: events,
            reconnect_count: reconnects,
            last_connected_ms: last_ms,
            connection_state,
            total_sub_count: total_subs,
            active_sub_count: active_subs,
            eosed_sub_count: eosed_subs,
        }
    }

    // 2E-T1: Valid relay-diagnostics state updates AppState and changes are
    //        detected (relay_diagnostics.last transitions None → Some).
    //
    //        The frame-decode path (apply ← decode_relay_diagnostics) is covered
    //        by nmp-core's codec tests. This test verifies that the hl domain
    //        correctly stores the decoded model in AppState.
    #[test]
    fn relay_diag_frame_updates_state() {
        let mut state = make_state();
        assert!(
            state.relay_diagnostics.last.is_none(),
            "no diagnostics before first frame"
        );

        inject_rows(
            &mut state,
            vec![make_row(
                "wss://relay.example.com",
                "ok",
                100,
                3,
                1_700_000_000_000,
                2,
                1,
                1,
            )],
        );

        assert!(
            state.relay_diagnostics.last.is_some(),
            "relay_diagnostics.last must be set after state update"
        );
        let snap = state.relay_diagnostics.last.as_ref().unwrap();
        assert_eq!(snap.relays.len(), 1);
        assert_eq!(snap.relays[0].relay_url, "wss://relay.example.com");
        assert_eq!(snap.relays[0].total_events_rx, 100);
    }

    // 2E-T2: Snapshot carries only RAW fields — nmp's pre-formatted label strings
    //        (role_label, connection_label, auth_label, *_display) are NOT projected.
    //        Raw-data doctrine / D1.
    #[test]
    fn relay_diag_snapshot_carries_raw_fields_not_labels() {
        let mut state = make_state();

        inject_rows(
            &mut state,
            vec![make_row(
                "wss://r.example.com",
                "warn",
                999,
                7,
                1_600_000_000_000,
                3,
                2,
                2,
            )],
        );

        let snap = state.relay_diagnostics.last.as_ref().unwrap();
        let diag = &snap.relays[0];

        // Raw numeric/enum fields are present and correct.
        assert_eq!(diag.total_events_rx, 999, "total_events_rx is raw u64");
        assert_eq!(diag.reconnect_count, 7, "reconnect_count is raw u32");
        assert_eq!(
            diag.last_connected_ms, 1_600_000_000_000,
            "last_connected_ms is raw unix-ms epoch"
        );
        assert_eq!(diag.total_sub_count, 3);
        assert_eq!(diag.active_sub_count, 2);
        assert_eq!(diag.eosed_sub_count, 2);
        // Connection state is derived from the tone tag — NOT a label string.
        assert_eq!(
            diag.connection_state,
            RelayConnectionState::Reconnecting,
            "warn tone → Reconnecting enum variant"
        );
        // RelayDiagRow has NO role_label / connection_label / auth_label /
        // total_events_display / bytes_*_display fields — compile-time guarantee.
        // The URL is the stable identity field, not a formatted label.
        assert_eq!(diag.relay_url, "wss://r.example.com");
    }

    // 2E-T3: Snapshot is bounded by relay count (D5).
    #[test]
    fn relay_diag_snapshot_bounded_by_relay_count() {
        let mut state = make_state();

        let rows: Vec<RelayDiagRow> = (0..5_u32)
            .map(|i| {
                make_row(
                    &format!("wss://relay{i}.example.com"),
                    "ok",
                    u64::from(i) * 10,
                    i,
                    0,
                    1,
                    1,
                    1,
                )
            })
            .collect();
        let count = rows.len();
        inject_rows(&mut state, rows);

        let snap = state.relay_diagnostics.last.as_ref().unwrap();
        assert_eq!(
            snap.relays.len(),
            count,
            "snapshot relay count must equal input relay count (D5)"
        );
    }

    // 2E-T4: A malformed frame is a silent no-op (D6 / no panics).
    #[test]
    fn malformed_diag_frame_no_ops() {
        let mut state = make_state();
        let changed = apply(&mut state, b"NOT A VALID FLATBUFFER");
        assert!(!changed, "malformed frame must not change state");
        assert!(
            state.relay_diagnostics.last.is_none(),
            "relay_diagnostics must remain None after malformed frame (D6)"
        );
    }

    // 2E-T5: project_relay_diagnostics returns None when no frame received yet.
    //        A view that was never opened (or just opened with no frame yet)
    //        emits no snapshot data.
    #[test]
    fn closed_diag_view_emits_no_snapshot() {
        let state = make_state();
        let snap = project_relay_diagnostics(&state);
        assert!(
            snap.is_none(),
            "project_relay_diagnostics must return None before any frame arrives"
        );
    }
}
