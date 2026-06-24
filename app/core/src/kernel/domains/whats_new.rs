//! What's New seen-state domain — Phase 5A (device-local, no nostr publish).
//!
//! ## Responsibilities
//!
//! * **READ** — parse the bundled `resources/whats-new.json` changelog and the
//!   persisted `{data_dir}/whats-new-state-v1.json` seen marker. Compute
//!   `should_present` (true when unseen entries exist) and expose the filtered
//!   entry list via `ViewSnapshot::WhatsNew(WhatsNewSnapshot)`.
//!
//! * **WRITE** — `AppAction::MarkWhatsNewSeen { shipped_at_unix }` advances the
//!   monotonic seen marker (never moves backward) and persists it to disk as a
//!   fire-and-forget `Effect::PersistWhatsNewSeen`. The marker is NEVER published
//!   to Nostr — it is device-local app state per `hl-app-state-vs-nostr-facts`.
//!
//! ## File layout
//!
//! State file: `{data_dir}/whats-new-state-v1.json`
//! JSON shape: `{ "last_seen_at_unix_seconds": <u64> }`
//! Monotonic: `MarkWhatsNewSeen` never moves the marker backward (`.max()`).
//!
//! ## No nostr publish
//!
//! `MarkWhatsNewSeen` emits only `Effect::PersistWhatsNewSeen` (disk write).
//! Tests verify that no `Effect::Publish*` variants appear in the output.

use crate::kernel::app::AppState;
use crate::kernel::effect::Effect;
use crate::kernel::snapshot::{ViewSnapshot, WhatsNewEntryRow, WhatsNewSnapshot};

// ─── Bundled JSON ─────────────────────────────────────────────────────────────

/// The bundled What's New JSON embedded at compile time.
/// Path is relative to the crate root (`app/core/`).
const BUNDLED_WHATS_NEW_JSON: &str = include_str!("../../../resources/whats-new.json");

/// State file name within `data_dir`.
pub(crate) const STATE_FILE_NAME: &str = "whats-new-state-v1.json";

// ─── Internal state ───────────────────────────────────────────────────────────

/// Domain-internal entry type (not the FFI snapshot type).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WhatsNewEntry {
    pub shipped_at_iso: String,
    pub shipped_at_unix: u64,
    pub lines: Vec<String>,
}

/// `AppState::whats_new` — mutable seen-state for What's New.
///
/// Device-local: never serialised into a Nostr event.
/// NOT cleared on logout — the marker is per-device, not per-account.
#[derive(Debug, Clone, Default)]
pub struct WhatsNewState {
    /// Unseen entries (filter: `shipped_at_unix > last_seen_marker`).
    pub(crate) entries: Vec<WhatsNewEntry>,
    /// `true` when `entries` is non-empty and should trigger the sheet.
    pub(crate) should_present: bool,
    /// The latest persisted seen marker (last_seen_at_unix_seconds from disk).
    pub(crate) last_seen_unix: Option<u64>,
}

// ─── Reducer ──────────────────────────────────────────────────────────────────

/// Reduce a `AppAction::PrepareWhatsNew` — emit `Effect::LoadWhatsNewState`.
pub(crate) fn reduce_action_prepare_whats_new() -> Vec<Effect> {
    vec![Effect::LoadWhatsNewState]
}

/// Reduce a `AppAction::MarkWhatsNewSeen { shipped_at_unix }`:
///   1. Advance the local state's `last_seen_unix` monotonically.
///   2. Re-filter `entries` to remove newly-seen items.
///   3. Update `should_present`.
///   4. Emit `Effect::PersistWhatsNewSeen` (fire-and-forget disk write).
///
/// NEVER emits any publish / nostr effect (`hl-app-state-vs-nostr-facts`).
pub(crate) fn reduce_action_mark_whats_new_seen(
    state: &mut AppState,
    shipped_at_unix: u64,
) -> Vec<Effect> {
    let current = state.whats_new.last_seen_unix.unwrap_or(0);
    let next = current.max(shipped_at_unix);
    state.whats_new.last_seen_unix = Some(next);

    // Re-filter entries: drop anything at or before the new marker.
    state.whats_new.entries.retain(|e| e.shipped_at_unix > next);
    state.whats_new.should_present = !state.whats_new.entries.is_empty();

    vec![Effect::PersistWhatsNewSeen {
        shipped_at_unix: next,
    }]
}

/// Apply `KernelEvent::WhatsNewLoaded { entries, should_present }`.
///
/// Called by `reduce_event` in `actor.rs` after the effect runner resolves
/// the bundled JSON + seen marker from disk.
pub(crate) fn reduce_event_whats_new_loaded(
    state: &mut AppState,
    entries: Vec<WhatsNewEntryRow>,
    should_present: bool,
) -> Vec<Effect> {
    state.whats_new.entries = entries
        .into_iter()
        .map(|r| WhatsNewEntry {
            shipped_at_iso: r.shipped_at_iso,
            shipped_at_unix: r.shipped_at_unix,
            lines: r.lines,
        })
        .collect();
    state.whats_new.should_present = should_present;
    vec![]
}

// ─── Snapshot projection ─────────────────────────────────────────────────────

/// Project `ViewId::WhatsNew` from `AppState::whats_new`.
///
/// D1: raw fields only — no "N new features" label, no formatted strings.
pub(crate) fn project_whats_new_snapshot(state: &AppState) -> Option<ViewSnapshot> {
    let entries: Vec<WhatsNewEntryRow> = state
        .whats_new
        .entries
        .iter()
        .map(|e| WhatsNewEntryRow {
            shipped_at_iso: e.shipped_at_iso.clone(),
            shipped_at_unix: e.shipped_at_unix,
            lines: e.lines.clone(),
        })
        .collect();

    Some(ViewSnapshot::WhatsNew(WhatsNewSnapshot {
        entries,
        should_present: state.whats_new.should_present,
    }))
}

// ─── Effect runner helpers ────────────────────────────────────────────────────

/// Parse the bundled JSON and return decoded, sorted entries.
/// Newest-first by `shipped_at_unix`. Empty lines are skipped.
///
/// Returns `None` on any decode or validation error (D6: silent no-op; the
/// effect runner sends `WhatsNewLoaded { entries: [], should_present: false }`).
pub(crate) fn decode_bundled_entries() -> Option<Vec<WhatsNewEntryRow>> {
    decode_entries_from_json(BUNDLED_WHATS_NEW_JSON)
}

/// Decode and sort entries from a JSON string (also used in tests).
fn decode_entries_from_json(json: &str) -> Option<Vec<WhatsNewEntryRow>> {
    #[derive(serde::Deserialize)]
    struct Payload {
        schema_version: u32,
        entries: Vec<EntryPayload>,
    }
    #[derive(serde::Deserialize)]
    struct EntryPayload {
        shipped_at: String,
        lines: Vec<String>,
    }

    let payload: Payload = serde_json::from_str(json).ok()?;
    if payload.schema_version != 1 {
        tracing::warn!(
            schema_version = payload.schema_version,
            "whats_new: unsupported schema version — no-op (D6)"
        );
        return None;
    }

    let mut entries = Vec::new();
    for e in payload.entries {
        if e.lines.is_empty() {
            continue;
        }
        let unix = parse_iso8601_utc(&e.shipped_at)?;
        entries.push(WhatsNewEntryRow {
            shipped_at_iso: e.shipped_at,
            shipped_at_unix: unix,
            lines: e.lines,
        });
    }
    // Newest-first.
    entries.sort_by(|a, b| b.shipped_at_unix.cmp(&a.shipped_at_unix));
    Some(entries)
}

/// Parse `YYYY-MM-DDTHH:MM:SSZ` to UNIX seconds. Returns `None` on any error.
///
/// Identical algorithm to `whats_new.rs::parse_iso8601_utc` — kept here so
/// the kernel domain has no dependency on the live `whats_new` module.
pub(crate) fn parse_iso8601_utc(value: &str) -> Option<u64> {
    if value.len() != 20
        || &value[4..5] != "-"
        || &value[7..8] != "-"
        || &value[10..11] != "T"
        || &value[13..14] != ":"
        || &value[16..17] != ":"
        || &value[19..20] != "Z"
    {
        return None;
    }
    let year: i32 = value[0..4].parse().ok()?;
    let month: u32 = value[5..7].parse().ok()?;
    let day: u32 = value[8..10].parse().ok()?;
    let hour: u32 = value[11..13].parse().ok()?;
    let minute: u32 = value[14..16].parse().ok()?;
    let second: u32 = value[17..19].parse().ok()?;

    if month == 0
        || month > 12
        || day == 0
        || day > days_in_month(year, month)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return None;
    }

    let days = days_from_civil(year, month, day);
    if days < 0 {
        return None;
    }
    Some(days as u64 * 86_400 + hour as u64 * 3_600 + minute as u64 * 60 + second as u64)
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let year = year - i32::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let month = month as i32;
    let day = day as i32;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    (era * 146_097 + doe - 719_468) as i64
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::action::{AppAction, KernelEvent};
    use crate::kernel::actor::{reduce, Cmd};
    use crate::kernel::clock::{Clock, ManualClock};
    use crate::kernel::effect::Effect;
    use crate::kernel::snapshot::{ViewSnapshot, WhatsNewEntryRow};

    fn make_state() -> AppState {
        AppState::default()
    }

    fn step(state: &mut AppState, clock: &ManualClock, cmd: Cmd) -> Vec<Effect> {
        let now = clock.now_unix_seconds();
        reduce(state, cmd, now)
    }

    fn sample_entries() -> Vec<WhatsNewEntryRow> {
        vec![
            WhatsNewEntryRow {
                shipped_at_iso: "2026-05-14T21:45:00Z".to_string(),
                shipped_at_unix: 1_778_795_100,
                lines: vec!["Newest feature".to_string()],
            },
            WhatsNewEntryRow {
                shipped_at_iso: "2026-05-14T12:00:00Z".to_string(),
                shipped_at_unix: 1_778_760_000,
                lines: vec!["Older feature".to_string()],
            },
        ]
    }

    // 5A-T1: whats_new_shows_unseen_items
    //
    // Injecting KernelEvent::WhatsNewLoaded with entries + should_present: true
    // must update AppState::whats_new and surface entries in the snapshot.
    #[test]
    fn whats_new_shows_unseen_items() {
        let mut state = make_state();
        let clock = ManualClock::default();

        assert!(!state.whats_new.should_present);
        assert!(state.whats_new.entries.is_empty());

        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::WhatsNewLoaded {
                entries: sample_entries(),
                should_present: true,
            }),
        );

        assert!(
            state.whats_new.should_present,
            "should_present must be true"
        );
        assert_eq!(state.whats_new.entries.len(), 2);
        assert_eq!(
            state.whats_new.entries[0].shipped_at_iso,
            "2026-05-14T21:45:00Z"
        );

        let snap = project_whats_new_snapshot(&state);
        assert!(snap.is_some(), "snapshot must be Some");
        if let Some(ViewSnapshot::WhatsNew(s)) = snap {
            assert!(s.should_present);
            assert_eq!(s.entries.len(), 2);
            assert_eq!(s.entries[0].shipped_at_iso, "2026-05-14T21:45:00Z");
            assert_eq!(s.entries[0].shipped_at_unix, 1_778_795_100);
            assert_eq!(s.entries[0].lines, vec!["Newest feature"]);
        } else {
            panic!("expected ViewSnapshot::WhatsNew");
        }
    }

    // 5A-T2: mark_seen_persists_device_local
    //
    // AppAction::MarkWhatsNewSeen must emit Effect::PersistWhatsNewSeen and
    // update AppState::whats_new.should_present.
    #[test]
    fn mark_seen_persists_device_local() {
        let mut state = make_state();
        let clock = ManualClock::default();

        // Load entries first.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::WhatsNewLoaded {
                entries: sample_entries(),
                should_present: true,
            }),
        );
        assert!(state.whats_new.should_present);

        // Mark the newer entry as seen (should collapse should_present to false
        // because all entries are now <= the new marker).
        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::MarkWhatsNewSeen {
                shipped_at_unix: 1_778_795_100,
            }),
        );

        // Must emit PersistWhatsNewSeen.
        let persist_effects: Vec<_> = effects
            .iter()
            .filter(|e| matches!(e, Effect::PersistWhatsNewSeen { .. }))
            .collect();
        assert_eq!(
            persist_effects.len(),
            1,
            "must emit exactly one PersistWhatsNewSeen"
        );
        if let Effect::PersistWhatsNewSeen { shipped_at_unix } = &persist_effects[0] {
            assert_eq!(*shipped_at_unix, 1_778_795_100);
        }

        // AppState must reflect seen state.
        assert!(
            !state.whats_new.should_present,
            "should_present must be false after marking all as seen"
        );
        assert!(
            state.whats_new.entries.is_empty(),
            "entries must be empty after marking all as seen"
        );
    }

    // 5A-T3: seen_state_not_published_to_nostr
    //
    // AppAction::MarkWhatsNewSeen must NOT emit any publish/nostr effects.
    // Device-local only (hl-app-state-vs-nostr-facts).
    #[test]
    fn seen_state_not_published_to_nostr() {
        let mut state = make_state();
        let clock = ManualClock::default();

        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::WhatsNewLoaded {
                entries: sample_entries(),
                should_present: true,
            }),
        );

        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::MarkWhatsNewSeen {
                shipped_at_unix: 1_778_795_100,
            }),
        );

        // No publish effects — device-local only.
        let nostr_publish: Vec<_> = effects
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    Effect::PublishHighlightEvent { .. }
                        | Effect::DispatchFollowAction { .. }
                        | Effect::DispatchNip29Action { .. }
                        | Effect::DispatchShareToRoom { .. }
                        | Effect::DispatchBookmarkAction { .. }
                        | Effect::DispatchReactAction { .. }
                )
            })
            .collect();

        assert!(
            nostr_publish.is_empty(),
            "MarkWhatsNewSeen must not emit any nostr publish effects (hl-app-state-vs-nostr-facts); got: {nostr_publish:?}"
        );
    }

    // 5A-T4: snapshot_raw_fields
    //
    // ViewSnapshot::WhatsNew must contain raw shipped_at_iso, shipped_at_unix, lines.
    // No labels, no formatted strings (D1).
    #[test]
    fn snapshot_raw_fields() {
        let mut state = make_state();
        let clock = ManualClock::default();

        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::WhatsNewLoaded {
                entries: vec![WhatsNewEntryRow {
                    shipped_at_iso: "2026-05-14T21:45:00Z".to_string(),
                    shipped_at_unix: 1_778_795_100,
                    lines: vec!["Line A".to_string(), "Line B".to_string()],
                }],
                should_present: true,
            }),
        );

        let snap = project_whats_new_snapshot(&state);
        if let Some(ViewSnapshot::WhatsNew(s)) = snap {
            assert_eq!(s.entries.len(), 1);
            let row = &s.entries[0];
            // Raw ISO string — no formatting.
            assert_eq!(row.shipped_at_iso, "2026-05-14T21:45:00Z");
            // Raw unix timestamp — no "X ago" label.
            assert_eq!(row.shipped_at_unix, 1_778_795_100);
            // Raw lines — no bullets, no labels.
            assert_eq!(row.lines, vec!["Line A", "Line B"]);
        } else {
            panic!("expected ViewSnapshot::WhatsNew");
        }
    }

    // 5A-T5: prepare_whats_new_emits_load_effect
    //
    // AppAction::PrepareWhatsNew must emit exactly one Effect::LoadWhatsNewState.
    #[test]
    fn prepare_whats_new_emits_load_effect() {
        let mut state = make_state();
        let clock = ManualClock::default();

        let effects = step(&mut state, &clock, Cmd::Action(AppAction::PrepareWhatsNew));

        assert_eq!(
            effects.len(),
            1,
            "PrepareWhatsNew must emit exactly one effect"
        );
        assert!(
            matches!(effects[0], Effect::LoadWhatsNewState),
            "expected Effect::LoadWhatsNewState, got {:?}",
            effects[0]
        );
    }

    // 5A-T6: mark_seen_monotonic
    //
    // MarkWhatsNewSeen never moves the marker backward.
    #[test]
    fn mark_seen_monotonic() {
        let mut state = make_state();
        let clock = ManualClock::default();

        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::WhatsNewLoaded {
                entries: sample_entries(),
                should_present: true,
            }),
        );

        // Mark the newer one first.
        step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::MarkWhatsNewSeen {
                shipped_at_unix: 1_778_795_100,
            }),
        );
        assert_eq!(state.whats_new.last_seen_unix, Some(1_778_795_100));

        // Attempt to move backward — must be a no-op on the marker value.
        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::MarkWhatsNewSeen {
                shipped_at_unix: 1_778_760_000,
            }),
        );

        // Marker must not have moved backward.
        assert_eq!(
            state.whats_new.last_seen_unix,
            Some(1_778_795_100),
            "marker must not move backward"
        );

        // Effect still emitted (fire-and-forget; the effect runner handles dedup).
        let has_persist = effects
            .iter()
            .any(|e| matches!(e, Effect::PersistWhatsNewSeen { shipped_at_unix } if *shipped_at_unix == 1_778_795_100));
        assert!(has_persist, "PersistWhatsNewSeen must carry the max value");
    }

    // 5A-T7: bundled_json_decodes
    //
    // The compile-time include_str! must decode at least 1 entry.
    #[test]
    fn bundled_json_decodes() {
        let entries = decode_bundled_entries();
        assert!(entries.is_some(), "bundled JSON must decode without error");
        let entries = entries.unwrap();
        assert!(
            !entries.is_empty(),
            "bundled JSON must have at least one entry"
        );
        // Newest-first invariant.
        if entries.len() >= 2 {
            assert!(
                entries[0].shipped_at_unix >= entries[1].shipped_at_unix,
                "entries must be newest-first"
            );
        }
    }
}
