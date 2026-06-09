//! Relay config: the set of relays the app connects to, each tagged with the
//! four roles that drive its routing — `read`, `write`, `rooms`, `indexer`.
//!
//! Persistence is split by what each role actually is:
//!
//! - `read` / `write` → **NIP-65 (kind:10002)**. Nostr identity; interops with
//!   any other nostr client. Re-published on every edit.
//! - `rooms` / `indexer` → **NIP-78 app-data (kind:30078)** with
//!   `d = "com.highlighter.relays"`. Highlighter-specific routing, not nostr
//!   identity — doesn't belong in kind:10002.
//!
//! `query_relays` merges both sources on URL. When neither exists yet (first
//! login), `seed_defaults()` fills in a sane starting set.

use std::collections::BTreeSet;
use std::sync::OnceLock;

use nostr_sdk::prelude::*;
use nostrdb::{Filter as NdbFilter, Ndb, Transaction};
use serde::{Deserialize, Serialize};

use crate::errors::CoreError;
use crate::models::{Nip11Document, RelayDiagnostic, RelayStatus};
use crate::nostr_runtime::NostrRuntime;

// -- Relay policy ------------------------------------------------------------

/// Canonical indexer relay. Pinned into the pool — `indexer_urls()`
/// always includes it whether or not the user's NIP-78 lists it. Used as
/// the fallback target for outbox-model lookups (kind:0/3/10002 for
/// arbitrary pubkeys, kind:10009/10012 for follows). The user can add
/// other indexer relays in their NIP-78, but they can't remove this one
/// — losing it would silently break profile/follow-list resolution for
/// the rest of the app.
/// Relays we run NIP-77 negentropy sync against for the cold-start
/// backfill of follows' kind:0/3/10002 (the "social trio"). The premise
/// for using purplepag.es here was wrong — it specialises in those kinds
/// but doesn't currently advertise or implement NIP-77 (its NIP-11
/// supported_nips list omits 77, and `examples/purple_sync_bench.rs`
/// confirms negentropy times out against it). relay.damus.io (strfry)
/// works and, crucially, isn't bound by purple's `max_limit=500` cap on
/// REQ — negentropy returned 1794 events vs REQ's 500 for a 1052-follow
/// query. Keep this list short; sync runs in parallel against each.
///
/// Policy data is packaged under `assets/relay_policy.json` so production
/// source owns routing behavior without embedding relay URLs inline.
static RELAY_POLICY: OnceLock<RelayPolicy> = OnceLock::new();

#[derive(Debug, Deserialize)]
struct RelayPolicy {
    canonical_rooms: String,
    canonical_indexer: String,
    negentropy_sync: Vec<String>,
    nostr_connect: String,
    feedback: String,
    room_explorer_curator: String,
    seed_defaults: Vec<RelayPolicyDefault>,
}

#[derive(Debug, Deserialize)]
struct RelayPolicyDefault {
    relay: Option<RelayPolicyKey>,
    url: Option<String>,
    read: bool,
    write: bool,
    rooms: bool,
    indexer: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RelayPolicyKey {
    CanonicalRooms,
    CanonicalIndexer,
    NostrConnect,
    Feedback,
    RoomExplorerCurator,
}

impl RelayPolicy {
    fn url_for(&self, relay: &RelayPolicyKey) -> &str {
        match relay {
            RelayPolicyKey::CanonicalRooms => &self.canonical_rooms,
            RelayPolicyKey::CanonicalIndexer => &self.canonical_indexer,
            RelayPolicyKey::NostrConnect => &self.nostr_connect,
            RelayPolicyKey::Feedback => &self.feedback,
            RelayPolicyKey::RoomExplorerCurator => &self.room_explorer_curator,
        }
    }
}

impl RelayPolicyDefault {
    fn url<'a>(&'a self, policy: &'a RelayPolicy) -> &'a str {
        if let Some(url) = self.url.as_deref() {
            return url;
        }
        let relay = self
            .relay
            .as_ref()
            .expect("relay_policy seed_defaults entries require relay or url");
        policy.url_for(relay)
    }
}

fn relay_policy() -> &'static RelayPolicy {
    RELAY_POLICY.get_or_init(|| {
        serde_json::from_str(include_str!("../assets/relay_policy.json"))
            .expect("relay_policy.json must be valid")
    })
}

pub fn highlighter_relay() -> &'static str {
    &relay_policy().canonical_rooms
}

pub fn purple_pages_relay() -> &'static str {
    &relay_policy().canonical_indexer
}

pub fn negentropy_sync_relays() -> &'static [String] {
    &relay_policy().negentropy_sync
}

pub fn nostr_connect_relay() -> &'static str {
    &relay_policy().nostr_connect
}

pub fn feedback_relay() -> &'static str {
    &relay_policy().feedback
}

pub fn room_explorer_curator_relay() -> &'static str {
    &relay_policy().room_explorer_curator
}

/// Perms string included in our `nostrconnect://` URI. We request only the
/// kinds Highlighter actually publishes plus encryption for NIP-46 transport.
pub const DEFAULT_NOSTR_CONNECT_PERMS: &str =
    "sign_event:11,sign_event:1111,sign_event:9802,sign_event:16,nip04_encrypt,nip04_decrypt,nip44_encrypt,nip44_decrypt";

// -- Types -------------------------------------------------------------------

/// A single row in the user's relay list, carrying all four roles.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RelayConfig {
    pub url: String,
    pub read: bool,
    pub write: bool,
    pub rooms: bool,
    pub indexer: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RelaySettingsProjection {
    pub auto_connected_urls: Vec<String>,
    pub auto_connected_configs: Vec<RelayConfig>,
    pub auto_connected_diagnostics: Vec<RelayDiagnostic>,
    pub total_visible_relays: u64,
    pub connected_count: u64,
    pub aggregate_state_label: String,
    pub has_outbox: bool,
    pub all_connected_for_header: bool,
    pub any_connected_for_header: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum RelayStatusTone {
    Connected,
    Connecting,
    Error,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct RelayAvatarProjection {
    pub icon_url: Option<String>,
    pub initial: String,
    pub hue: f64,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct RelayRowProjection {
    pub avatar: RelayAvatarProjection,
    pub primary_label: String,
    pub display_url: String,
    pub status_tone: RelayStatusTone,
    pub rtt_label: Option<String>,
    pub read: bool,
    pub write: bool,
    pub rooms: bool,
    pub indexer: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct RelayRowProjectionInput {
    pub config: RelayConfig,
    pub diagnostic: Option<RelayDiagnostic>,
    pub nip11: Option<Nip11Document>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RelayRemoveProjection {
    pub title: String,
    pub message: String,
    pub orphan_summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RelayHostedRoomsSnapshot {
    pub room_names: Vec<String>,
    pub error_message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct NetworkSettingsMutationSnapshot {
    pub applied: bool,
    pub should_reload: bool,
    pub error_message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct NetworkSettingsSnapshot {
    pub relays: Vec<RelayConfig>,
    pub diagnostics: Vec<RelayDiagnostic>,
    pub projection: RelaySettingsProjection,
    pub wifi_only_enabled: bool,
    pub error_message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct NetworkDiagnosticsSnapshot {
    pub diagnostics: Vec<RelayDiagnostic>,
    pub projection: RelaySettingsProjection,
    pub error_message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct NetworkCacheStatsSnapshot {
    pub stats: Option<crate::models::CacheStats>,
    pub error_message: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct RelayRemoveProjectionInput {
    pub url: String,
    pub orphaned_room_names: Vec<String>,
    pub empty_message_uses_url: bool,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct RelayDetailProjection {
    pub avatar: RelayAvatarProjection,
    pub name: Option<String>,
    pub description: Option<String>,
    pub state_label: String,
    pub status_tone: RelayStatusTone,
    pub rtt_label: Option<String>,
    pub remove: RelayRemoveProjection,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct RelayDetailProjectionInput {
    pub url: String,
    pub diagnostic: Option<RelayDiagnostic>,
    pub nip11: Option<Nip11Document>,
    pub orphaned_room_names: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum AddRelayProbeStatus {
    Idle,
    Checking,
    Reachable,
    Unreachable,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AddRelaySheetProjection {
    pub normalized_url: String,
    pub clipboard_url: Option<String>,
    pub is_valid: bool,
    pub is_unencrypted: bool,
    pub can_add: bool,
    pub add_config: RelayConfig,
    pub probe_status: AddRelayProbeStatus,
    pub probe_text: String,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct AddRelaySheetProjectionInput {
    pub url_text: String,
    pub clipboard_text: Option<String>,
    pub read: bool,
    pub write: bool,
    pub rooms: bool,
    pub indexer: bool,
    pub probe_in_flight: bool,
    pub probe_result: Option<Nip11Document>,
    pub probe_failed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RelayNip11ProbePlan {
    pub urls_to_probe: Vec<String>,
    pub in_flight_urls: Vec<String>,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct RelayNip11ProbePlanInput {
    pub relays: Vec<RelayConfig>,
    pub cached_urls: Vec<String>,
    pub in_flight_urls: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ImportRelayRow {
    pub config: RelayConfig,
    pub display_url: String,
    pub role_label: String,
    pub is_selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ImportRelaysProjection {
    pub rows: Vec<ImportRelayRow>,
    pub selected_count: u64,
    pub found_title: String,
    pub can_apply: bool,
    pub selected_configs: Vec<RelayConfig>,
}

/// Native import-relays source field projection. Rust owns the canonical
/// source input and whether fetching can start.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ImportRelaysSourceProjection {
    pub submit_npub: String,
    pub can_fetch: bool,
}

/// Native import-relays source field input.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ImportRelaysSourceProjectionInput {
    pub npub: String,
    pub is_fetching: bool,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct ImportRelaysProjectionInput {
    pub fetched: Vec<RelayConfig>,
    pub selected_urls: Vec<String>,
}

impl RelayConfig {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            read: false,
            write: false,
            rooms: false,
            indexer: false,
        }
    }

    pub fn read_write(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            read: true,
            write: true,
            rooms: false,
            indexer: false,
        }
    }
}

pub fn default_add_relay_config() -> RelayConfig {
    RelayConfig::read_write("")
}

/// Starting relay set for a brand-new user with no published kind:10002 and
/// no cached NIP-78 app-data yet. Called by `query_relays` as the fallback.
pub fn seed_defaults() -> Vec<RelayConfig> {
    let policy = relay_policy();
    policy
        .seed_defaults
        .iter()
        .map(|row| RelayConfig {
            url: row.url(policy).to_string(),
            read: row.read,
            write: row.write,
            rooms: row.rooms,
            indexer: row.indexer,
        })
        .collect()
}

/// Display-only row for a relay the runtime auto-pinned into the pool rather
/// than a relay the user configured in NIP-65/NIP-78. Native settings screens
/// ask Rust for this projection so they do not duplicate policy such as
/// which pinned relay is the canonical indexer.
pub fn auto_connected_display_config(url: String) -> RelayConfig {
    let normalized = url.trim().trim_end_matches('/');
    let rooms = normalized == highlighter_relay();
    let indexer = normalized == purple_pages_relay();
    RelayConfig {
        url,
        read: true,
        write: false,
        rooms,
        indexer,
    }
}

pub fn settings_projection(
    configured_relays: &[RelayConfig],
    diagnostics: &[RelayDiagnostic],
) -> RelaySettingsProjection {
    let configured_urls: BTreeSet<&str> = configured_relays
        .iter()
        .map(|relay| relay.url.as_str())
        .collect();
    let auto_url_set: BTreeSet<String> = diagnostics
        .iter()
        .filter(|row| !configured_urls.contains(row.url.as_str()))
        .map(|row| row.url.clone())
        .collect();
    let auto_connected_urls: Vec<String> = auto_url_set.iter().cloned().collect();
    let auto_connected_configs = auto_connected_urls
        .iter()
        .map(|url| auto_connected_display_config(url.clone()))
        .collect::<Vec<_>>();
    let auto_connected_diagnostics = auto_connected_urls
        .iter()
        .filter_map(|url| diagnostics.iter().find(|row| row.url == *url).cloned())
        .collect::<Vec<_>>();

    let total_visible_relays = configured_relays.len() as u64 + auto_connected_urls.len() as u64;
    let connected_count = diagnostics
        .iter()
        .filter(|row| row.state == RelayStatus::Connected)
        .count() as u64;

    RelaySettingsProjection {
        auto_connected_urls,
        auto_connected_configs,
        auto_connected_diagnostics,
        total_visible_relays,
        connected_count,
        aggregate_state_label: aggregate_state_label(total_visible_relays, connected_count),
        has_outbox: configured_relays.iter().any(|relay| relay.write),
        all_connected_for_header: connected_count == configured_relays.len() as u64
            && !configured_relays.is_empty(),
        any_connected_for_header: connected_count > 0,
    }
}

pub fn relay_row_projection(input: RelayRowProjectionInput) -> RelayRowProjection {
    let RelayRowProjectionInput {
        config,
        diagnostic,
        nip11,
    } = input;
    let display_url = display_relay_url(&config.url);
    let primary_label = nip11
        .as_ref()
        .and_then(|doc| trimmed_non_empty(doc.name.as_deref()))
        .unwrap_or_else(|| display_url.clone());

    RelayRowProjection {
        avatar: relay_avatar_projection(&config.url, nip11.as_ref()),
        primary_label,
        display_url,
        status_tone: relay_status_tone(diagnostic.as_ref().map(|row| row.state)),
        rtt_label: diagnostic.as_ref().and_then(relay_rtt_label),
        read: config.read,
        write: config.write,
        rooms: config.rooms,
        indexer: config.indexer,
    }
}

pub fn relay_detail_projection(input: RelayDetailProjectionInput) -> RelayDetailProjection {
    let RelayDetailProjectionInput {
        url,
        diagnostic,
        nip11,
        orphaned_room_names,
    } = input;

    RelayDetailProjection {
        avatar: relay_avatar_projection(&url, nip11.as_ref()),
        name: nip11
            .as_ref()
            .and_then(|doc| trimmed_non_empty(doc.name.as_deref())),
        description: nip11
            .as_ref()
            .and_then(|doc| trimmed_non_empty(doc.description.as_deref())),
        state_label: relay_status_label(diagnostic.as_ref().map(|row| row.state)),
        status_tone: relay_status_tone(diagnostic.as_ref().map(|row| row.state)),
        rtt_label: diagnostic.as_ref().and_then(relay_rtt_label),
        remove: relay_remove_projection(RelayRemoveProjectionInput {
            url,
            orphaned_room_names,
            empty_message_uses_url: false,
        }),
    }
}

pub fn relay_remove_projection(input: RelayRemoveProjectionInput) -> RelayRemoveProjection {
    let orphan_count = input.orphaned_room_names.len();
    if orphan_count == 0 {
        let message = if input.empty_message_uses_url {
            format!(
                "Highlighter will stop sending and receiving events through {}.",
                input.url
            )
        } else {
            "Highlighter will stop sending and receiving events through this relay.".into()
        };
        return RelayRemoveProjection {
            title: "Remove this relay?".into(),
            message,
            orphan_summary: None,
        };
    }

    RelayRemoveProjection {
        title: "Remove — you're a member of rooms here".into(),
        message: format!(
            "This relay hosts {orphan_count} of your rooms ({}). Removing it will cut you off from them until you re-add it.",
            joined_limited_names(&input.orphaned_room_names, 3)
        ),
        orphan_summary: Some(joined_limited_names(&input.orphaned_room_names, 5)),
    }
}

pub fn relay_hosted_rooms_snapshot(
    result: Result<Vec<String>, CoreError>,
) -> RelayHostedRoomsSnapshot {
    match result {
        Ok(room_names) => RelayHostedRoomsSnapshot {
            room_names,
            error_message: String::new(),
        },
        Err(error) => RelayHostedRoomsSnapshot {
            room_names: Vec::new(),
            error_message: error.to_string(),
        },
    }
}

pub fn network_settings_mutation_snapshot(
    result: Result<(), CoreError>,
    should_reload_on_success: bool,
    error_prefix: &str,
) -> NetworkSettingsMutationSnapshot {
    match result {
        Ok(()) => NetworkSettingsMutationSnapshot {
            applied: true,
            should_reload: should_reload_on_success,
            error_message: String::new(),
        },
        Err(error) => NetworkSettingsMutationSnapshot {
            applied: false,
            should_reload: false,
            error_message: format!("{error_prefix} — {error}"),
        },
    }
}

pub fn network_settings_snapshot(
    relays_result: Result<Vec<RelayConfig>, CoreError>,
    previous_relays: Vec<RelayConfig>,
    diagnostics: Vec<RelayDiagnostic>,
    wifi_only_enabled: bool,
) -> NetworkSettingsSnapshot {
    let (relays, error_message) = match relays_result {
        Ok(relays) => (relays, String::new()),
        Err(error) => (previous_relays, error.to_string()),
    };
    let projection = settings_projection(&relays, &diagnostics);
    NetworkSettingsSnapshot {
        relays,
        diagnostics,
        projection,
        wifi_only_enabled,
        error_message,
    }
}

pub fn network_diagnostics_snapshot(
    configured_relays: Vec<RelayConfig>,
    diagnostics: Vec<RelayDiagnostic>,
) -> NetworkDiagnosticsSnapshot {
    let projection = settings_projection(&configured_relays, &diagnostics);
    NetworkDiagnosticsSnapshot {
        diagnostics,
        projection,
        error_message: String::new(),
    }
}

pub fn network_cache_stats_snapshot(
    result: Result<crate::models::CacheStats, CoreError>,
) -> NetworkCacheStatsSnapshot {
    match result {
        Ok(stats) => NetworkCacheStatsSnapshot {
            stats: Some(stats),
            error_message: String::new(),
        },
        Err(error) => NetworkCacheStatsSnapshot {
            stats: None,
            error_message: error.to_string(),
        },
    }
}

pub fn add_relay_sheet_projection(input: AddRelaySheetProjectionInput) -> AddRelaySheetProjection {
    let normalized_url = normalize_relay_url_input(&input.url_text);
    let clipboard_url = input
        .clipboard_text
        .map(|text| normalize_relay_url_input(&text))
        .filter(|text| relay_url_is_valid(text) && text != &normalized_url);
    let is_valid = relay_url_is_valid(&normalized_url);
    let is_unencrypted = relay_url_is_unencrypted(&normalized_url);
    let (probe_status, probe_text) = add_relay_probe_status(
        input.probe_in_flight,
        input.probe_result.as_ref(),
        input.probe_failed,
    );

    AddRelaySheetProjection {
        add_config: RelayConfig {
            url: normalized_url.clone(),
            read: input.read,
            write: input.write,
            rooms: input.rooms,
            indexer: input.indexer,
        },
        normalized_url,
        clipboard_url,
        is_valid,
        is_unencrypted,
        can_add: is_valid,
        probe_status,
        probe_text,
    }
}

pub fn plan_relay_nip11_probes(input: RelayNip11ProbePlanInput) -> RelayNip11ProbePlan {
    let cached_urls = input
        .cached_urls
        .into_iter()
        .filter_map(|url| canonical_probe_url(&url))
        .collect::<BTreeSet<_>>();
    let mut in_flight_urls = input
        .in_flight_urls
        .into_iter()
        .filter_map(|url| canonical_probe_url(&url))
        .collect::<BTreeSet<_>>();
    let mut urls_to_probe = Vec::new();
    for relay in input.relays {
        let Some(url) = canonical_probe_url(&relay.url) else {
            continue;
        };
        if cached_urls.contains(&url) {
            continue;
        }
        if in_flight_urls.insert(url.clone()) {
            urls_to_probe.push(url);
        }
    }
    RelayNip11ProbePlan {
        urls_to_probe,
        in_flight_urls: in_flight_urls.into_iter().collect(),
    }
}

pub fn finish_relay_nip11_probe(in_flight_urls: Vec<String>, url: String) -> Vec<String> {
    let mut in_flight_urls = in_flight_urls
        .into_iter()
        .filter_map(|url| canonical_probe_url(&url))
        .collect::<BTreeSet<_>>();
    if let Some(url) = canonical_probe_url(&url) {
        in_flight_urls.remove(&url);
    }
    in_flight_urls.into_iter().collect()
}

pub fn default_import_relay_selection(relays: Vec<RelayConfig>) -> Vec<String> {
    relays.into_iter().map(|relay| relay.url).collect()
}

pub fn toggle_import_relay_selection(
    fetched: Vec<RelayConfig>,
    selected_urls: Vec<String>,
    url: String,
) -> Vec<String> {
    let mut selected = selected_urls.into_iter().collect::<BTreeSet<_>>();
    let known = fetched.iter().any(|relay| relay.url == url);
    if known {
        if selected.contains(&url) {
            selected.remove(&url);
        } else {
            selected.insert(url);
        }
    }
    selected_import_urls_for_fetched(&fetched, &selected)
}

/// Project the source field for importing another user's relay list. Native
/// shells render `can_fetch` and pass `submit_npub` to the fetch action.
pub fn import_relays_source_projection(
    input: ImportRelaysSourceProjectionInput,
) -> ImportRelaysSourceProjection {
    let submit_npub = input.npub.trim().to_string();
    ImportRelaysSourceProjection {
        can_fetch: !submit_npub.is_empty() && !input.is_fetching,
        submit_npub,
    }
}

pub fn import_relays_projection(input: ImportRelaysProjectionInput) -> ImportRelaysProjection {
    let selected_urls = input.selected_urls.into_iter().collect::<BTreeSet<_>>();
    let mut selected_configs = Vec::new();
    let rows = input
        .fetched
        .into_iter()
        .map(|config| {
            let is_selected = selected_urls.contains(&config.url);
            if is_selected {
                selected_configs.push(config.clone());
            }
            ImportRelayRow {
                display_url: display_relay_url(&config.url),
                role_label: relay_role_label(&config),
                config,
                is_selected,
            }
        })
        .collect::<Vec<_>>();
    let selected_count = selected_configs.len() as u64;
    let found_count = rows.len();
    ImportRelaysProjection {
        rows,
        selected_count,
        found_title: format!(
            "Found {found_count} relay{}",
            if found_count == 1 { "" } else { "s" }
        ),
        can_apply: selected_count > 0,
        selected_configs,
    }
}

fn selected_import_urls_for_fetched(
    fetched: &[RelayConfig],
    selected: &BTreeSet<String>,
) -> Vec<String> {
    fetched
        .iter()
        .filter_map(|relay| selected.contains(&relay.url).then_some(relay.url.clone()))
        .collect()
}

fn display_relay_url(raw: &str) -> String {
    raw.strip_prefix("wss://").unwrap_or(raw).to_string()
}

fn relay_role_label(row: &RelayConfig) -> String {
    match (row.read, row.write) {
        (true, true) => "Read + Write".into(),
        (true, false) => "Read".into(),
        (false, true) => "Write".into(),
        (false, false) => "No roles".into(),
    }
}

fn relay_avatar_projection(url: &str, nip11: Option<&Nip11Document>) -> RelayAvatarProjection {
    RelayAvatarProjection {
        icon_url: nip11.and_then(|doc| trimmed_non_empty(doc.icon.as_deref())),
        initial: relay_avatar_initial(url),
        hue: relay_avatar_hue(url),
    }
}

fn relay_avatar_initial(url: &str) -> String {
    let host = url
        .strip_prefix("wss://")
        .or_else(|| url.strip_prefix("ws://"))
        .unwrap_or(url);
    host.chars()
        .next()
        .map(|ch| ch.to_uppercase().collect::<String>())
        .unwrap_or_else(|| "?".into())
}

fn relay_avatar_hue(url: &str) -> f64 {
    let seed = url.chars().map(|ch| ch as u32 as f64).sum::<f64>();
    (seed % 360.0) / 360.0
}

fn relay_rtt_label(diagnostic: &RelayDiagnostic) -> Option<String> {
    diagnostic.rtt_ms.map(|rtt| format!("{rtt} ms"))
}

fn relay_status_label(status: Option<RelayStatus>) -> String {
    match status {
        Some(RelayStatus::Connected) => "Connected",
        Some(RelayStatus::Connecting) => "Connecting…",
        Some(RelayStatus::Disconnected) => "Disconnected",
        Some(RelayStatus::Terminated) => "Terminated",
        Some(RelayStatus::Banned) => "Banned",
        None => "Unknown",
    }
    .into()
}

fn relay_status_tone(status: Option<RelayStatus>) -> RelayStatusTone {
    match status {
        Some(RelayStatus::Connected) => RelayStatusTone::Connected,
        Some(RelayStatus::Connecting) => RelayStatusTone::Connecting,
        Some(RelayStatus::Disconnected | RelayStatus::Terminated | RelayStatus::Banned) => {
            RelayStatusTone::Error
        }
        None => RelayStatusTone::Unknown,
    }
}

fn joined_limited_names(names: &[String], limit: usize) -> String {
    let mut summary = names
        .iter()
        .take(limit)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(", ");
    if names.len() > limit {
        summary.push_str(", …");
    }
    summary
}

fn trimmed_non_empty(value: Option<&str>) -> Option<String> {
    value.and_then(|raw| {
        let trimmed = raw.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn canonical_probe_url(raw: &str) -> Option<String> {
    let url = normalize_relay_url_input(raw);
    (!url.is_empty()).then_some(url)
}

fn normalize_relay_url_input(input: &str) -> String {
    input.trim().to_string()
}

fn relay_url_is_valid(url: &str) -> bool {
    url.starts_with("wss://") || url.starts_with("ws://")
}

fn relay_url_is_unencrypted(url: &str) -> bool {
    url.starts_with("ws://")
}

fn add_relay_probe_status(
    probe_in_flight: bool,
    probe_result: Option<&Nip11Document>,
    probe_failed: bool,
) -> (AddRelayProbeStatus, String) {
    if probe_in_flight {
        return (AddRelayProbeStatus::Checking, "Checking relay…".into());
    }
    if let Some(doc) = probe_result {
        return (AddRelayProbeStatus::Reachable, nip11_summary(doc));
    }
    if probe_failed {
        return (
            AddRelayProbeStatus::Unreachable,
            "Couldn't reach the relay — you can still add it.".into(),
        );
    }
    (AddRelayProbeStatus::Idle, String::new())
}

fn nip11_summary(doc: &Nip11Document) -> String {
    let software_label = doc.software.as_ref().map(|name| {
        if let Some(version) = doc.version.as_ref() {
            format!("{name} {version}")
        } else {
            name.clone()
        }
    });
    let nip_count =
        (!doc.supported_nips.is_empty()).then(|| format!("{} NIPs", doc.supported_nips.len()));
    let parts = [doc.name.clone(), software_label, nip_count]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    if parts.is_empty() {
        "Reachable (no NIP-11 metadata)".into()
    } else {
        parts.join(" • ")
    }
}

fn aggregate_state_label(total_visible_relays: u64, connected_count: u64) -> String {
    if total_visible_relays == 0 {
        return "No relays".into();
    }
    if connected_count == 0 {
        return "Offline".into();
    }
    if connected_count == total_visible_relays {
        return format!("Online — {connected_count} of {total_visible_relays}");
    }
    format!("{connected_count} of {total_visible_relays} online")
}

// -- NIP-65 (kind:10002) -----------------------------------------------------

const KIND_RELAY_LIST: u16 = 10002;

/// Build the `["r", url, marker?]` tags for the provided rows. Rows with
/// neither `read` nor `write` are skipped — NIP-65 has no concept of a
/// "disabled" relay entry, only "inbox/outbox/both".
fn nip65_tags(rows: &[RelayConfig]) -> Result<Vec<Tag>, CoreError> {
    let mut tags: Vec<Tag> = Vec::new();
    for row in rows {
        let marker = match (row.read, row.write) {
            (true, true) => None,
            (true, false) => Some("read"),
            (false, true) => Some("write"),
            (false, false) => continue,
        };
        let parts: Vec<String> = match marker {
            Some(m) => vec!["r".into(), row.url.clone(), m.into()],
            None => vec!["r".into(), row.url.clone()],
        };
        tags.push(
            Tag::parse(parts).map_err(|e| CoreError::Other(format!("build relay tag: {e}")))?,
        );
    }
    Ok(tags)
}

/// Parse a kind:10002 event into `(url, read, write)` rows.
fn parse_nip65_event(event: &Event) -> Vec<(String, bool, bool)> {
    let mut out: Vec<(String, bool, bool)> = Vec::new();
    for tag in event.tags.iter() {
        let slice = tag.as_slice();
        if slice.first().map(String::as_str) != Some("r") {
            continue;
        }
        let Some(url) = slice.get(1) else { continue };
        let url = url.trim().to_string();
        if url.is_empty() {
            continue;
        }
        let (read, write) = match slice.get(2).map(String::as_str) {
            Some("read") => (true, false),
            Some("write") => (false, true),
            _ => (true, true),
        };
        out.push((url, read, write));
    }
    out
}

/// Newest kind:10002 for `user_hex` cached in nostrdb, or `None`.
fn latest_nip65(ndb: &Ndb, user_hex: &str) -> Result<Option<Event>, CoreError> {
    if user_hex.is_empty() {
        return Ok(None);
    }
    let author = PublicKey::from_hex(user_hex)
        .map_err(|e| CoreError::InvalidInput(format!("invalid user pubkey: {e}")))?;
    let txn = Transaction::new(ndb).map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;
    let pk_bytes: [u8; 32] = author.to_bytes();
    let filter = NdbFilter::new()
        .kinds([KIND_RELAY_LIST as u64])
        .authors([&pk_bytes])
        .build();
    let results = ndb
        .query(&txn, &[filter], 8)
        .map_err(|e| CoreError::Cache(format!("query relay list: {e}")))?;
    let mut newest: Option<Event> = None;
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        newest = Some(match newest {
            Some(prev) if prev.created_at >= event.created_at => prev,
            _ => event,
        });
    }
    Ok(newest)
}

// -- NIP-78 app-data (kind:30078) for rooms/indexer flags --------------------

const KIND_APP_DATA: u16 = 30078;
const APP_DATA_D_TAG: &str = "com.highlighter.relays";

/// Per-row payload stored in the NIP-78 event's JSON content. Flat shape so
/// it round-trips losslessly.
#[derive(Debug, Serialize, Deserialize)]
struct AppDataEntry {
    url: String,
    #[serde(default)]
    rooms: bool,
    #[serde(default)]
    indexer: bool,
}

/// JSON content for the kind:30078 event. Skips rows with neither flag — no
/// point persisting empty entries.
fn app_data_content(rows: &[RelayConfig]) -> String {
    let entries: Vec<AppDataEntry> = rows
        .iter()
        .filter(|r| r.rooms || r.indexer)
        .map(|r| AppDataEntry {
            url: r.url.clone(),
            rooms: r.rooms,
            indexer: r.indexer,
        })
        .collect();
    serde_json::to_string(&entries).unwrap_or_else(|_| "[]".into())
}

fn parse_app_data_event(event: &Event) -> Vec<AppDataEntry> {
    serde_json::from_str::<Vec<AppDataEntry>>(&event.content).unwrap_or_default()
}

fn latest_app_data(ndb: &Ndb, user_hex: &str) -> Result<Option<Event>, CoreError> {
    if user_hex.is_empty() {
        return Ok(None);
    }
    let author = PublicKey::from_hex(user_hex)
        .map_err(|e| CoreError::InvalidInput(format!("invalid user pubkey: {e}")))?;
    let txn = Transaction::new(ndb).map_err(|e| CoreError::Cache(format!("open ndb txn: {e}")))?;
    let pk_bytes: [u8; 32] = author.to_bytes();
    let filter = NdbFilter::new()
        .kinds([KIND_APP_DATA as u64])
        .authors([&pk_bytes])
        .tags([APP_DATA_D_TAG], 'd')
        .build();
    let results = ndb
        .query(&txn, &[filter], 8)
        .map_err(|e| CoreError::Cache(format!("query relay app-data: {e}")))?;
    let mut newest: Option<Event> = None;
    for r in &results {
        let Ok(note) = ndb.get_note_by_key(&txn, r.note_key) else {
            continue;
        };
        let Ok(json) = note.json() else { continue };
        let Ok(event) = Event::from_json(&json) else {
            continue;
        };
        newest = Some(match newest {
            Some(prev) if prev.created_at >= event.created_at => prev,
            _ => event,
        });
    }
    Ok(newest)
}

// -- Merge + public API ------------------------------------------------------

/// Merge kind:10002 and kind:30078 into the user's effective relay list,
/// deduped by URL. Falls back to `seed_defaults()` when neither event is
/// cached.
///
/// **Defaulting rule for Rooms:** if the merged result has no row flagged
/// `rooms`, append `highlighter_relay()` with `rooms = true` (and read/write
/// off so it doesn't pollute the user's NIP-65 outbox). Highlighter is
/// the canonical rooms host for the app — without it the rooms surfaces
/// can't load anything. The user can remove it via the UI by toggling
/// Rooms off and adding another relay with Rooms on; once any Rooms-
/// flagged row exists in NIP-78 this fallback stops firing.
pub fn query_relays(ndb: &Ndb, user_hex: &str) -> Result<Vec<RelayConfig>, CoreError> {
    let nip65 = latest_nip65(ndb, user_hex)?
        .as_ref()
        .map(parse_nip65_event)
        .unwrap_or_default();
    let app_data = latest_app_data(ndb, user_hex)?
        .as_ref()
        .map(parse_app_data_event)
        .unwrap_or_default();

    if nip65.is_empty() && app_data.is_empty() {
        return Ok(seed_defaults());
    }

    let mut rows: Vec<RelayConfig> = Vec::new();
    for (url, read, write) in nip65 {
        rows.push(RelayConfig {
            url,
            read,
            write,
            rooms: false,
            indexer: false,
        });
    }
    for entry in app_data {
        if let Some(row) = rows.iter_mut().find(|r| r.url == entry.url) {
            row.rooms = entry.rooms;
            row.indexer = entry.indexer;
        } else {
            rows.push(RelayConfig {
                url: entry.url,
                read: false,
                write: false,
                rooms: entry.rooms,
                indexer: entry.indexer,
            });
        }
    }

    // Rooms invariant: relay.highlighter.com is always present with rooms=true.
    let rooms_relay = highlighter_relay();
    if let Some(row) = rows.iter_mut().find(|r| r.url == rooms_relay) {
        row.rooms = true;
    } else {
        rows.push(RelayConfig {
            url: rooms_relay.to_string(),
            read: false,
            write: false,
            rooms: true,
            indexer: false,
        });
    }

    // Indexer invariant: purplepag.es is always present with indexer=true.
    let indexer_relay = purple_pages_relay();
    if let Some(row) = rows.iter_mut().find(|r| r.url == indexer_relay) {
        row.indexer = true;
    } else {
        rows.push(RelayConfig {
            url: indexer_relay.to_string(),
            read: false,
            write: false,
            rooms: false,
            indexer: true,
        });
    }

    Ok(rows)
}

/// Publish kind:10002 (NIP-65) with the current rows' read/write flags.
pub async fn publish_nip65(
    runtime: &NostrRuntime,
    rows: &[RelayConfig],
) -> Result<String, CoreError> {
    let tags = nip65_tags(rows)?;
    let builder = EventBuilder::new(Kind::Custom(KIND_RELAY_LIST), "").tags(tags);
    let client = runtime.client();
    let event = client
        .sign_event_builder(builder)
        .await
        .map_err(|e| CoreError::Signer(format!("sign relay list: {e}")))?;
    client
        .send_event(&event)
        .await
        .map_err(|e| CoreError::Relay(format!("publish relay list: {e}")))?;
    crate::nostr_runtime::mirror_social_trio_to_purple(client, &event).await;
    Ok(event.id.to_hex())
}

/// Publish kind:30078 app-data with the current rows' rooms/indexer flags.
pub async fn publish_app_data(
    runtime: &NostrRuntime,
    rows: &[RelayConfig],
) -> Result<String, CoreError> {
    let content = app_data_content(rows);
    let d_tag = Tag::parse(vec!["d".to_string(), APP_DATA_D_TAG.to_string()])
        .map_err(|e| CoreError::Other(format!("build d tag: {e}")))?;
    let builder = EventBuilder::new(Kind::Custom(KIND_APP_DATA), content).tags([d_tag]);
    let client = runtime.client();
    let event = client
        .sign_event_builder(builder)
        .await
        .map_err(|e| CoreError::Signer(format!("sign relay app-data: {e}")))?;
    client
        .send_event(&event)
        .await
        .map_err(|e| CoreError::Relay(format!("publish relay app-data: {e}")))?;
    Ok(event.id.to_hex())
}

/// Replace the user's relay list with `rows`. Re-publishes both NIP-65 and
/// NIP-78 so every flag is durable. Validates that every row's URL is a
/// non-empty `ws://` or `wss://` URL with no duplicates.
pub async fn set_relays(runtime: &NostrRuntime, rows: Vec<RelayConfig>) -> Result<(), CoreError> {
    if rows.is_empty() {
        return Err(CoreError::InvalidInput(
            "relay list must not be empty".into(),
        ));
    }
    let mut seen = std::collections::HashSet::new();
    for row in &rows {
        let url = row.url.trim();
        if !relay_url_is_valid(url) {
            return Err(CoreError::InvalidInput(format!(
                "relay URL must use a websocket scheme: {url}"
            )));
        }
        if !seen.insert(url.to_string()) {
            return Err(CoreError::InvalidInput(format!(
                "duplicate relay URL in list: {url}"
            )));
        }
    }
    publish_nip65(runtime, &rows).await?;
    publish_app_data(runtime, &rows).await?;
    Ok(())
}

/// Insert-or-update a single relay. Reads the current list, replaces the row
/// with matching URL (or appends), and re-publishes.
pub async fn upsert_relay(
    runtime: &NostrRuntime,
    user_hex: &str,
    cfg: RelayConfig,
) -> Result<(), CoreError> {
    let mut rows = query_relays(runtime.ndb(), user_hex)?;
    if let Some(existing) = rows.iter_mut().find(|r| r.url == cfg.url) {
        *existing = cfg;
    } else {
        rows.push(cfg);
    }
    set_relays(runtime, rows).await
}

/// Remove a relay by URL. Errors if the URL isn't in the list.
pub async fn remove_relay(
    runtime: &NostrRuntime,
    user_hex: &str,
    url: String,
) -> Result<(), CoreError> {
    let mut rows = query_relays(runtime.ndb(), user_hex)?;
    let before = rows.len();
    rows.retain(|r| r.url != url);
    if rows.len() == before {
        return Err(CoreError::NotFound);
    }
    set_relays(runtime, rows).await
}

/// Atomically update a single relay's role flags without touching its URL.
pub async fn set_relay_roles(
    runtime: &NostrRuntime,
    user_hex: &str,
    url: String,
    read: bool,
    write: bool,
    rooms: bool,
    indexer: bool,
) -> Result<(), CoreError> {
    let mut rows = query_relays(runtime.ndb(), user_hex)?;
    let Some(row) = rows.iter_mut().find(|r| r.url == url) else {
        return Err(CoreError::NotFound);
    };
    row.read = read;
    row.write = write;
    row.rooms = rooms;
    row.indexer = indexer;
    set_relays(runtime, rows).await
}

// -- Tests -------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_rows() -> Vec<RelayConfig> {
        vec![
            RelayConfig {
                url: "wss://relay.highlighter.com".into(),
                read: true,
                write: true,
                rooms: true,
                indexer: false,
            },
            RelayConfig {
                url: "wss://relay.damus.io".into(),
                read: true,
                write: false,
                rooms: false,
                indexer: false,
            },
            RelayConfig {
                url: "wss://purplepag.es".into(),
                read: false,
                write: false,
                rooms: false,
                indexer: true,
            },
        ]
    }

    fn add_relay_input(url_text: &str) -> AddRelaySheetProjectionInput {
        AddRelaySheetProjectionInput {
            url_text: url_text.into(),
            clipboard_text: None,
            read: true,
            write: true,
            rooms: false,
            indexer: false,
            probe_in_flight: false,
            probe_result: None,
            probe_failed: false,
        }
    }

    fn nip11_doc(
        name: Option<&str>,
        description: Option<&str>,
        icon: Option<&str>,
    ) -> Nip11Document {
        Nip11Document {
            url: "wss://relay.example.com".into(),
            name: name.map(str::to_string),
            description: description.map(str::to_string),
            pubkey: None,
            contact: None,
            software: None,
            version: None,
            supported_nips: Vec::new(),
            icon: icon.map(str::to_string),
        }
    }

    #[test]
    fn seed_defaults_has_four_rows_with_expected_roles() {
        let seed = seed_defaults();
        assert_eq!(seed.len(), 4);

        let hl = seed
            .iter()
            .find(|r| r.url.contains("highlighter"))
            .expect("hl");
        assert!(hl.read && hl.write && hl.rooms && !hl.indexer);

        let damus = seed
            .iter()
            .find(|r| r.url.contains("damus"))
            .expect("damus");
        assert!(damus.read && damus.write && !damus.rooms && !damus.indexer);

        let purple = seed
            .iter()
            .find(|r| r.url.contains("purplepag"))
            .expect("purple");
        assert!(!purple.read && !purple.write && !purple.rooms && purple.indexer);

        let primal = seed
            .iter()
            .find(|r| r.url.contains("primal"))
            .expect("primal");
        assert!(!primal.read && !primal.write && !primal.rooms && primal.indexer);
    }

    #[test]
    fn auto_connected_display_config_marks_policy_pins() {
        let rooms = auto_connected_display_config(highlighter_relay().to_string());
        assert!(rooms.read);
        assert!(rooms.rooms);
        assert!(!rooms.write);
        assert!(!rooms.indexer);

        let indexer = auto_connected_display_config(purple_pages_relay().to_string());
        assert!(indexer.read);
        assert!(indexer.indexer);
        assert!(!indexer.write);
        assert!(!indexer.rooms);

        let outbox = auto_connected_display_config("wss://outbox.example".to_string());
        assert!(outbox.read);
        assert!(!outbox.write);
        assert!(!outbox.rooms);
        assert!(!outbox.indexer);
    }

    #[test]
    fn settings_projection_builds_auto_rows_and_header_state() {
        let configured = vec![
            RelayConfig {
                url: "wss://a.example".into(),
                read: true,
                write: true,
                rooms: false,
                indexer: false,
            },
            RelayConfig {
                url: "wss://b.example".into(),
                read: true,
                write: false,
                rooms: false,
                indexer: false,
            },
        ];
        let diagnostics = vec![
            diagnostic("wss://b.example", RelayStatus::Disconnected),
            diagnostic("wss://c.example", RelayStatus::Connected),
            diagnostic("wss://a.example", RelayStatus::Connected),
        ];

        let projection = settings_projection(&configured, &diagnostics);

        assert_eq!(projection.auto_connected_urls, vec!["wss://c.example"]);
        assert_eq!(projection.auto_connected_configs.len(), 1);
        assert_eq!(projection.auto_connected_configs[0].url, "wss://c.example");
        assert!(projection.auto_connected_configs[0].read);
        assert!(!projection.auto_connected_configs[0].write);
        assert_eq!(projection.auto_connected_diagnostics.len(), 1);
        assert_eq!(
            projection.auto_connected_diagnostics[0].url,
            "wss://c.example"
        );
        assert_eq!(projection.total_visible_relays, 3);
        assert_eq!(projection.connected_count, 2);
        assert_eq!(projection.aggregate_state_label, "2 of 3 online");
        assert!(projection.has_outbox);
        assert!(projection.all_connected_for_header);
        assert!(projection.any_connected_for_header);
    }

    #[test]
    fn settings_projection_handles_offline_and_empty_states() {
        let configured = vec![RelayConfig {
            url: "wss://a.example".into(),
            read: true,
            write: false,
            rooms: false,
            indexer: false,
        }];
        let offline = settings_projection(
            &configured,
            &[diagnostic("wss://a.example", RelayStatus::Disconnected)],
        );

        assert_eq!(offline.total_visible_relays, 1);
        assert_eq!(offline.connected_count, 0);
        assert_eq!(offline.aggregate_state_label, "Offline");
        assert!(!offline.has_outbox);
        assert!(!offline.all_connected_for_header);
        assert!(!offline.any_connected_for_header);

        let empty = settings_projection(&[], &[]);
        assert_eq!(empty.aggregate_state_label, "No relays");
        assert_eq!(empty.total_visible_relays, 0);
    }

    #[test]
    fn relay_row_projection_uses_nip11_name_and_status_facts() {
        let mut diag = diagnostic("wss://relay.example.com", RelayStatus::Connected);
        diag.rtt_ms = Some(42);

        let projection = relay_row_projection(RelayRowProjectionInput {
            config: RelayConfig {
                url: "wss://relay.example.com".into(),
                read: true,
                write: false,
                rooms: true,
                indexer: false,
            },
            diagnostic: Some(diag),
            nip11: Some(nip11_doc(
                Some("  Example Relay  "),
                None,
                Some("  https://cdn.example/icon.png  "),
            )),
        });

        assert_eq!(projection.primary_label, "Example Relay");
        assert_eq!(projection.display_url, "relay.example.com");
        assert_eq!(projection.status_tone, RelayStatusTone::Connected);
        assert_eq!(projection.rtt_label.as_deref(), Some("42 ms"));
        assert_eq!(
            projection.avatar.icon_url.as_deref(),
            Some("https://cdn.example/icon.png")
        );
        assert_eq!(projection.avatar.initial, "R");
        assert!(projection.read);
        assert!(!projection.write);
        assert!(projection.rooms);
        assert!(!projection.indexer);
    }

    #[test]
    fn relay_row_projection_falls_back_to_url_and_unknown_status() {
        let projection = relay_row_projection(RelayRowProjectionInput {
            config: RelayConfig::read_write("ws://alpha.example"),
            diagnostic: None,
            nip11: Some(nip11_doc(Some("  "), None, Some("  "))),
        });

        assert_eq!(projection.primary_label, "ws://alpha.example");
        assert_eq!(projection.display_url, "ws://alpha.example");
        assert_eq!(projection.status_tone, RelayStatusTone::Unknown);
        assert!(projection.rtt_label.is_none());
        assert!(projection.avatar.icon_url.is_none());
        assert_eq!(projection.avatar.initial, "A");
    }

    #[test]
    fn relay_detail_projection_matches_previous_labels_and_remove_copy() {
        let projection = relay_detail_projection(RelayDetailProjectionInput {
            url: "wss://relay.example.com".into(),
            diagnostic: Some(diagnostic(
                "wss://relay.example.com",
                RelayStatus::Connecting,
            )),
            nip11: Some(nip11_doc(
                Some(" Relay Name "),
                Some(" Relay description "),
                None,
            )),
            orphaned_room_names: vec![
                "Books".into(),
                "Podcasts".into(),
                "Articles".into(),
                "Video".into(),
                "Research".into(),
                "Design".into(),
            ],
        });

        assert_eq!(projection.name.as_deref(), Some("Relay Name"));
        assert_eq!(projection.description.as_deref(), Some("Relay description"));
        assert_eq!(projection.state_label, "Connecting…");
        assert_eq!(projection.status_tone, RelayStatusTone::Connecting);
        assert_eq!(
            projection.remove.title,
            "Remove — you're a member of rooms here"
        );
        assert_eq!(
            projection.remove.message,
            "This relay hosts 6 of your rooms (Books, Podcasts, Articles, …). Removing it will cut you off from them until you re-add it."
        );
        assert_eq!(
            projection.remove.orphan_summary.as_deref(),
            Some("Books, Podcasts, Articles, Video, Research, …")
        );
    }

    #[test]
    fn relay_remove_projection_handles_no_orphan_rooms() {
        let projection = relay_remove_projection(RelayRemoveProjectionInput {
            url: "wss://relay.example.com".into(),
            orphaned_room_names: Vec::new(),
            empty_message_uses_url: true,
        });

        assert_eq!(projection.title, "Remove this relay?");
        assert_eq!(
            projection.message,
            "Highlighter will stop sending and receiving events through wss://relay.example.com."
        );
        assert!(projection.orphan_summary.is_none());

        let detail_projection = relay_detail_projection(RelayDetailProjectionInput {
            url: "wss://relay.example.com".into(),
            diagnostic: None,
            nip11: None,
            orphaned_room_names: Vec::new(),
        });
        assert_eq!(
            detail_projection.remove.message,
            "Highlighter will stop sending and receiving events through this relay."
        );
    }

    #[test]
    fn relay_hosted_rooms_snapshot_surfaces_room_names_and_errors() {
        let success = relay_hosted_rooms_snapshot(Ok(vec!["Books".into(), "Podcasts".into()]));
        assert_eq!(success.room_names, vec!["Books", "Podcasts"]);
        assert!(success.error_message.is_empty());

        let failure = relay_hosted_rooms_snapshot(Err(CoreError::NotAuthenticated));
        assert!(failure.room_names.is_empty());
        assert_eq!(failure.error_message, "not authenticated");
    }

    #[test]
    fn network_settings_mutation_snapshot_projects_reload_and_error_copy() {
        let success = network_settings_mutation_snapshot(Ok(()), true, "Couldn't add relay");
        assert!(success.applied);
        assert!(success.should_reload);
        assert!(success.error_message.is_empty());

        let no_reload = network_settings_mutation_snapshot(Ok(()), false, "Couldn't reconnect");
        assert!(no_reload.applied);
        assert!(!no_reload.should_reload);

        let failure = network_settings_mutation_snapshot(
            Err(CoreError::Relay("offline".into())),
            true,
            "Couldn't remove relay",
        );
        assert!(!failure.applied);
        assert!(!failure.should_reload);
        assert_eq!(
            failure.error_message,
            "Couldn't remove relay — relay error: offline"
        );
    }

    #[test]
    fn network_settings_snapshot_projects_load_and_preserves_previous_on_error() {
        let relays = vec![RelayConfig::read_write("wss://relay.example.com")];
        let diagnostics = vec![diagnostic(
            "wss://relay.example.com",
            RelayStatus::Connected,
        )];
        let snapshot =
            network_settings_snapshot(Ok(relays.clone()), Vec::new(), diagnostics.clone(), true);
        assert_eq!(snapshot.relays, relays);
        assert_eq!(snapshot.diagnostics, diagnostics);
        assert!(snapshot.wifi_only_enabled);
        assert!(snapshot.error_message.is_empty());
        assert_eq!(snapshot.projection.total_visible_relays, 1);
        assert_eq!(snapshot.projection.connected_count, 1);

        let previous = vec![RelayConfig::read_write("wss://previous.example.com")];
        let failure = network_settings_snapshot(
            Err(CoreError::NotAuthenticated),
            previous.clone(),
            Vec::new(),
            false,
        );
        assert_eq!(failure.relays, previous);
        assert_eq!(failure.error_message, "not authenticated");
        assert_eq!(failure.projection.total_visible_relays, 1);
    }

    #[test]
    fn network_diagnostics_snapshot_projects_live_rows() {
        let configured = vec![RelayConfig::read_write("wss://relay.example.com")];
        let diagnostics = vec![diagnostic(
            "wss://relay.example.com",
            RelayStatus::Connected,
        )];
        let snapshot = network_diagnostics_snapshot(configured, diagnostics.clone());
        assert_eq!(snapshot.diagnostics, diagnostics);
        assert!(snapshot.error_message.is_empty());
        assert_eq!(snapshot.projection.connected_count, 1);
        assert_eq!(snapshot.projection.aggregate_state_label, "Online — 1 of 1");
    }

    #[test]
    fn network_cache_stats_snapshot_surfaces_stats_and_errors() {
        let stats = crate::models::CacheStats {
            event_count_estimate: 42,
            disk_bytes: 2048,
        };
        let success = network_cache_stats_snapshot(Ok(stats.clone()));
        assert_eq!(success.stats, Some(stats));
        assert!(success.error_message.is_empty());

        let failure = network_cache_stats_snapshot(Err(CoreError::Cache("missing".into())));
        assert!(failure.stats.is_none());
        assert_eq!(failure.error_message, "cache error: missing");
    }

    #[test]
    fn add_relay_projection_validates_url_and_builds_add_config() {
        let mut input = add_relay_input("  ws://relay.example.com  ");
        input.clipboard_text = Some(" wss://paste.example.com ".into());
        input.write = false;
        input.rooms = true;
        let projection = add_relay_sheet_projection(input);

        assert_eq!(projection.normalized_url, "ws://relay.example.com");
        assert_eq!(
            projection.clipboard_url.as_deref(),
            Some("wss://paste.example.com")
        );
        assert!(projection.is_valid);
        assert!(projection.is_unencrypted);
        assert!(projection.can_add);
        assert_eq!(
            projection.add_config,
            RelayConfig {
                url: "ws://relay.example.com".into(),
                read: true,
                write: false,
                rooms: true,
                indexer: false,
            }
        );
    }

    #[test]
    fn add_relay_projection_rejects_invalid_and_duplicate_clipboard_urls() {
        let mut invalid_input = add_relay_input("https://relay.example.com");
        invalid_input.clipboard_text = Some("https://paste.example.com".into());
        let invalid = add_relay_sheet_projection(invalid_input);
        assert!(!invalid.is_valid);
        assert!(!invalid.can_add);
        assert!(invalid.clipboard_url.is_none());

        let mut duplicate_input = add_relay_input("wss://relay.example.com");
        duplicate_input.clipboard_text = Some(" wss://relay.example.com ".into());
        let duplicate = add_relay_sheet_projection(duplicate_input);
        assert!(duplicate.clipboard_url.is_none());
    }

    #[test]
    fn add_relay_projection_projects_probe_status_and_summary() {
        let mut checking_input = add_relay_input("wss://relay.example.com");
        checking_input.probe_in_flight = true;
        let checking = add_relay_sheet_projection(checking_input);
        assert_eq!(checking.probe_status, AddRelayProbeStatus::Checking);
        assert_eq!(checking.probe_text, "Checking relay…");

        let mut reachable_input = add_relay_input("wss://relay.example.com");
        reachable_input.probe_result = Some(Nip11Document {
            url: "wss://relay.example.com".into(),
            name: Some("Example Relay".into()),
            description: None,
            pubkey: None,
            contact: None,
            software: Some("strfry".into()),
            version: Some("1.0".into()),
            supported_nips: vec![1, 11, 65],
            icon: None,
        });
        let reachable = add_relay_sheet_projection(reachable_input);
        assert_eq!(reachable.probe_status, AddRelayProbeStatus::Reachable);
        assert_eq!(reachable.probe_text, "Example Relay • strfry 1.0 • 3 NIPs");

        let mut unreachable_input = add_relay_input("wss://relay.example.com");
        unreachable_input.probe_failed = true;
        let unreachable = add_relay_sheet_projection(unreachable_input);
        assert_eq!(unreachable.probe_status, AddRelayProbeStatus::Unreachable);
        assert_eq!(
            unreachable.probe_text,
            "Couldn't reach the relay — you can still add it."
        );
    }

    #[test]
    fn plan_relay_nip11_probes_skips_cached_and_in_flight_urls() {
        let plan = plan_relay_nip11_probes(RelayNip11ProbePlanInput {
            relays: vec![
                RelayConfig::read_write(" wss://one.example "),
                RelayConfig::read_write("wss://two.example"),
                RelayConfig::read_write("wss://three.example"),
                RelayConfig::read_write("wss://two.example"),
            ],
            cached_urls: vec!["wss://one.example".into()],
            in_flight_urls: vec!["wss://three.example".into(), " ".into()],
        });

        assert_eq!(plan.urls_to_probe, vec!["wss://two.example"]);
        assert_eq!(
            plan.in_flight_urls,
            vec!["wss://three.example", "wss://two.example"]
        );
    }

    #[test]
    fn finish_relay_nip11_probe_canonicalizes_and_removes_url() {
        let remaining = finish_relay_nip11_probe(
            vec![
                "wss://one.example".into(),
                " wss://two.example ".into(),
                " ".into(),
            ],
            "wss://two.example".into(),
        );

        assert_eq!(remaining, vec!["wss://one.example"]);
    }

    #[test]
    fn default_add_relay_config_matches_new_sheet_defaults() {
        assert_eq!(
            default_add_relay_config(),
            RelayConfig {
                url: String::new(),
                read: true,
                write: true,
                rooms: false,
                indexer: false,
            }
        );
    }

    #[test]
    fn default_import_relay_selection_selects_every_fetched_url() {
        let rows = vec![
            RelayConfig::read_write("wss://one.example"),
            RelayConfig {
                url: "wss://two.example".into(),
                read: true,
                write: false,
                rooms: false,
                indexer: false,
            },
        ];

        assert_eq!(
            default_import_relay_selection(rows),
            vec!["wss://one.example", "wss://two.example"]
        );
    }

    #[test]
    fn toggle_import_relay_selection_canonicalizes_to_fetched_order() {
        let fetched = vec![
            RelayConfig::read_write("wss://one.example"),
            RelayConfig::read_write("wss://two.example"),
            RelayConfig::read_write("wss://three.example"),
        ];
        let selected = toggle_import_relay_selection(
            fetched.clone(),
            vec!["wss://two.example".into(), "wss://missing.example".into()],
            "wss://one.example".into(),
        );
        assert_eq!(selected, vec!["wss://one.example", "wss://two.example"]);

        let selected =
            toggle_import_relay_selection(fetched.clone(), selected, "wss://two.example".into());
        assert_eq!(selected, vec!["wss://one.example"]);

        let selected =
            toggle_import_relay_selection(fetched, selected, "wss://missing.example".into());
        assert_eq!(selected, vec!["wss://one.example"]);
    }

    #[test]
    fn import_relays_projection_formats_rows_and_selected_configs() {
        let projection = import_relays_projection(ImportRelaysProjectionInput {
            fetched: vec![
                RelayConfig::read_write("wss://one.example"),
                RelayConfig {
                    url: "wss://two.example".into(),
                    read: true,
                    write: false,
                    rooms: false,
                    indexer: false,
                },
                RelayConfig {
                    url: "ws://three.example".into(),
                    read: false,
                    write: true,
                    rooms: false,
                    indexer: false,
                },
                RelayConfig::new("wss://four.example"),
            ],
            selected_urls: vec!["wss://two.example".into(), "ws://three.example".into()],
        });

        assert_eq!(projection.found_title, "Found 4 relays");
        assert_eq!(projection.selected_count, 2);
        assert!(projection.can_apply);
        assert_eq!(projection.rows[0].display_url, "one.example");
        assert_eq!(projection.rows[0].role_label, "Read + Write");
        assert!(!projection.rows[0].is_selected);
        assert_eq!(projection.rows[1].role_label, "Read");
        assert!(projection.rows[1].is_selected);
        assert_eq!(projection.rows[2].display_url, "ws://three.example");
        assert_eq!(projection.rows[2].role_label, "Write");
        assert!(projection.rows[2].is_selected);
        assert_eq!(projection.rows[3].role_label, "No roles");
        assert_eq!(
            projection
                .selected_configs
                .iter()
                .map(|row| row.url.as_str())
                .collect::<Vec<_>>(),
            vec!["wss://two.example", "ws://three.example"]
        );
    }

    #[test]
    fn import_relays_projection_handles_empty_and_singular_titles() {
        let empty = import_relays_projection(ImportRelaysProjectionInput {
            fetched: Vec::new(),
            selected_urls: vec!["wss://missing.example".into()],
        });
        assert_eq!(empty.found_title, "Found 0 relays");
        assert_eq!(empty.selected_count, 0);
        assert!(!empty.can_apply);
        assert!(empty.selected_configs.is_empty());

        let single = import_relays_projection(ImportRelaysProjectionInput {
            fetched: vec![RelayConfig::read_write("wss://one.example")],
            selected_urls: vec!["wss://one.example".into()],
        });
        assert_eq!(single.found_title, "Found 1 relay");
        assert_eq!(single.selected_count, 1);
        assert!(single.can_apply);
    }

    #[test]
    fn import_relays_source_projection_trims_and_blocks_empty_or_fetching() {
        let ready = import_relays_source_projection(ImportRelaysSourceProjectionInput {
            npub: "  npub1example  ".into(),
            is_fetching: false,
        });
        let blank = import_relays_source_projection(ImportRelaysSourceProjectionInput {
            npub: " \n ".into(),
            is_fetching: false,
        });
        let fetching = import_relays_source_projection(ImportRelaysSourceProjectionInput {
            npub: "npub1example".into(),
            is_fetching: true,
        });

        assert_eq!(ready.submit_npub, "npub1example");
        assert!(ready.can_fetch);
        assert_eq!(blank.submit_npub, "");
        assert!(!blank.can_fetch);
        assert!(!fetching.can_fetch);
    }

    #[test]
    fn nip65_tags_use_marker_for_asymmetric_rows_and_none_for_both() {
        let tags = nip65_tags(&sample_rows()).expect("build tags");
        let hl = tags
            .iter()
            .find(|t| {
                t.as_slice().get(1).map(String::as_str) == Some("wss://relay.highlighter.com")
            })
            .expect("hl tag");
        assert_eq!(hl.as_slice().len(), 2);

        let damus = tags
            .iter()
            .find(|t| t.as_slice().get(1).map(String::as_str) == Some("wss://relay.damus.io"))
            .expect("damus tag");
        assert_eq!(damus.as_slice().get(2).map(String::as_str), Some("read"));
    }

    #[test]
    fn nip65_tags_skip_rows_with_neither_read_nor_write() {
        let tags = nip65_tags(&sample_rows()).expect("build tags");
        assert!(tags
            .iter()
            .all(|t| t.as_slice().get(1).map(String::as_str) != Some("wss://purplepag.es")));
    }

    #[test]
    fn nip65_roundtrip_preserves_read_write_flags() {
        let keys = Keys::generate();
        let rows = sample_rows();
        let tags = nip65_tags(&rows).expect("build tags");
        let event = EventBuilder::new(Kind::Custom(KIND_RELAY_LIST), "")
            .tags(tags)
            .sign_with_keys(&keys)
            .expect("sign");

        let parsed = parse_nip65_event(&event);
        assert_eq!(parsed.len(), 2);

        let hl = parsed
            .iter()
            .find(|(u, _, _)| u == "wss://relay.highlighter.com")
            .expect("hl");
        assert!(hl.1 && hl.2);

        let damus = parsed
            .iter()
            .find(|(u, _, _)| u == "wss://relay.damus.io")
            .expect("damus");
        assert!(damus.1 && !damus.2);
    }

    #[test]
    fn app_data_content_round_trip_preserves_rooms_and_indexer() {
        let keys = Keys::generate();
        let rows = sample_rows();
        let content = app_data_content(&rows);
        let d_tag = Tag::parse(vec!["d".to_string(), APP_DATA_D_TAG.to_string()]).expect("d tag");
        let event = EventBuilder::new(Kind::Custom(KIND_APP_DATA), content)
            .tags([d_tag])
            .sign_with_keys(&keys)
            .expect("sign");

        let entries = parse_app_data_event(&event);
        assert_eq!(entries.len(), 2);

        let hl = entries
            .iter()
            .find(|e| e.url == "wss://relay.highlighter.com")
            .expect("hl entry");
        assert!(hl.rooms && !hl.indexer);

        let purple = entries
            .iter()
            .find(|e| e.url == "wss://purplepag.es")
            .expect("purple entry");
        assert!(!purple.rooms && purple.indexer);
    }

    #[test]
    fn parse_nip65_event_handles_missing_marker_as_both() {
        let keys = Keys::generate();
        let tag = Tag::parse(vec!["r".to_string(), "wss://one.example".to_string()]).expect("tag");
        let event = EventBuilder::new(Kind::Custom(KIND_RELAY_LIST), "")
            .tags([tag])
            .sign_with_keys(&keys)
            .expect("sign");

        let parsed = parse_nip65_event(&event);
        assert_eq!(parsed.len(), 1);
        assert!(parsed[0].1 && parsed[0].2);
    }

    #[test]
    fn app_data_content_empty_array_when_no_rooms_or_indexer_rows() {
        let rows = vec![RelayConfig::read_write("wss://a.example")];
        assert_eq!(app_data_content(&rows), "[]");
    }

    fn diagnostic(url: &str, state: RelayStatus) -> RelayDiagnostic {
        RelayDiagnostic {
            url: url.into(),
            state,
            rtt_ms: None,
            bytes_sent: 0,
            bytes_received: 0,
            connected_since_ts: None,
        }
    }
}
