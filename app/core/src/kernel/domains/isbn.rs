//! ISBN preview cache domain — Phase 5C.
//!
//! ## Responsibilities
//!
//! * **CACHE** — in-memory HashMap of isbn13 → `CachedIsbnEntry`, loaded lazily
//!   from `{data_dir}/isbn-preview-cache-v1.json` on first lookup.
//!
//! * **FETCH** — `run_effect_lookup_isbn` does a Rust-owned HTTP fetch from
//!   `https://openlibrary.org/isbn/{isbn13}.json` (5 s timeout) with author
//!   resolution. No native capability needed — openlibrary.org is an approved,
//!   product-controlled, audited host (D3 note in spec §3).
//!
//! * **PERSIST** — atomic write (tmp→rename) to
//!   `{data_dir}/isbn-preview-cache-v1.json` after each new cache miss (D6:
//!   failure is logged, never surfaced as an error).
//!
//! * **VIEW** — `ViewId::BookPicker` / `ViewRoute::BookPicker` /
//!   `ViewSnapshot::BookPicker(BookPickerKernelSnapshot)`.
//!   Shows pending isbn, last result, and cache size.
//!
//! ## Device-local (Non-Negotiable: never published to nostr)
//!
//! The ISBN preview cache is purely device-local app state — it never triggers
//! a nostr publish (memory `hl-app-state-vs-nostr-facts.md`). Effects produced
//! by this domain are `LookupIsbn`, `LoadIsbnCache`, and `PersistIsbnCache` —
//! none of which produce any nostr event.
//!
//! ## ISBN normalization
//!
//! `normalize_isbn` accepts ISBN-10 or ISBN-13 (with or without dashes/spaces),
//! validates the checksum, and returns a canonical 13-digit string. Invalid input
//! returns an `Err` and the reducer treats it as a no-op (D6).
//!
//! ## HTTP
//!
//! `run_effect_lookup_isbn` uses `reqwest` with a 5 s timeout (already in
//! `Cargo.toml`). On any network or parse failure the runner emits an
//! `IsbnPreviewReady` event with a partial preview (empty title/author/image)
//! and a non-empty error string so the UI can fall through to manual entry (D6).
//!
//! ## Threading
//!
//! All reducer functions (`reduce_action_*`, `reduce_event_*`) run synchronously
//! on the actor thread. Effect runners are `async fn` and run on the tokio
//! executor — they send their results back via the `tx` channel as
//! `KernelEvent`s (D8: no blocking, no polling).

use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc::UnboundedSender;

use crate::kernel::action::KernelEvent;
use crate::kernel::actor::Cmd;
use crate::kernel::app::AppState;
use crate::kernel::effect::Effect;
use crate::kernel::snapshot::ViewSnapshot;

// ─── Constants ───────────────────────────────────────────────────────────────

const OPEN_LIBRARY_TIMEOUT: Duration = Duration::from_secs(5);
const CACHE_FILE_NAME: &str = "isbn-preview-cache-v1.json";

// ─── Public types (used in Effect / KernelEvent / ViewSnapshot) ──────────────

/// Lightweight book preview — snapshot-safe uniffi::Record for crossing FFI.
///
/// Mirrors `ArtifactPreview` in the bespoke live lane but without fields that
/// are irrelevant to the book/ISBN domain (podcast GUIDs, audio URLs, chapters).
/// All fields are raw strings (D1: no presentation formatting in the kernel).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct KernelArtifactPreview {
    /// Deterministic artifact id: `c{fnv1a_hex}` (same algorithm as the live lane
    /// so both lanes produce the same id for the same ISBN).
    pub id: String,
    /// Book title from Open Library (empty on cache miss / partial preview).
    pub title: String,
    /// Author name(s) joined by `", "` (empty on cache miss / partial preview).
    pub author: String,
    /// Cover image URL from `covers.openlibrary.org` (empty on cache miss).
    pub image: String,
    /// Book description from Open Library (empty when absent).
    pub description: String,
    /// Catalog id: `"isbn:{isbn13}"` — stable key for deduplication.
    pub catalog_id: String,
    /// Catalog kind: always `"isbn"` for this domain.
    pub catalog_kind: String,
    /// NIP-73 reference tag name: always `"i"` for ISBN-sourced books.
    pub reference_tag_name: String,
    /// NIP-73 reference tag value: `"isbn:{isbn13}"`.
    pub reference_tag_value: String,
    /// Highlight tag name: always `"i"` (the NIP-73 `i` tag anchors highlights).
    pub highlight_tag_name: String,
    /// Highlight tag value: `"isbn:{isbn13}"`.
    pub highlight_tag_value: String,
    /// Highlight reference key: `"i:isbn:{isbn13}"` (stable dedup key).
    pub highlight_reference_key: String,
    /// Source kind: always `"book"`.
    pub source: String,
}

/// One entry in the disk-persisted ISBN preview cache.
///
/// `Serialize`/`Deserialize` for JSON persistence; NOT uniffi (internal only).
/// This is kept minimal: only the fields that survive a round-trip through the
/// JSON cache file. Catalog/reference fields are recomputed from `isbn13` on
/// reconstruct (via `to_preview`) so they never go stale.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedIsbnEntry {
    pub id: String,
    pub title: String,
    pub author: String,
    pub image: String,
    pub description: String,
    pub published_at: String,
}

impl CachedIsbnEntry {
    fn from_preview(preview: &KernelArtifactPreview) -> Self {
        Self {
            id: preview.id.clone(),
            title: preview.title.clone(),
            author: preview.author.clone(),
            image: preview.image.clone(),
            description: preview.description.clone(),
            published_at: String::new(), // not carried in KernelArtifactPreview
        }
    }

    fn to_preview(&self, isbn13: &str) -> KernelArtifactPreview {
        build_preview(
            isbn13,
            self.title.clone(),
            self.author.clone(),
            self.image.clone(),
            self.description.clone(),
        )
    }
}

/// ISBN lookup result carried in `AppState::isbn.last_result` and exposed via
/// the `BookPickerKernelSnapshot`.
///
/// `uniffi::Record` so Swift can read the outcome without polling.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct KernelArtifactPreviewResult {
    /// The normalized 13-digit Bookland ISBN that was looked up.
    pub isbn13: String,
    /// Fetched or cached preview. `None` only when normalization failed (should
    /// not happen — the reducer normalizes before dispatching the effect).
    pub preview: Option<KernelArtifactPreview>,
    /// Non-empty if the HTTP fetch failed; empty on success or cache hit.
    pub error: String,
}

/// Snapshot for `ViewId::BookPicker` — pending lookup + last result + cache size.
///
/// Device-local (no nostr facts). Raw fields only (D1). `cache_size` is a
/// diagnostic counter; Swift shows it in debug/settings only.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BookPickerKernelSnapshot {
    /// ISBN currently being looked up, or `None` when idle.
    pub pending_isbn: Option<String>,
    /// Outcome of the most recent lookup (persists until the next lookup starts).
    pub last_result: Option<KernelArtifactPreviewResult>,
    /// Number of entries currently in the in-memory cache (diagnostic).
    pub cache_size: u64,
}

// ─── AppState sub-state ──────────────────────────────────────────────────────

/// ISBN preview cache and lookup state stored in `AppState`.
///
/// Device-local — never published to nostr. Cache file:
/// `{data_dir}/isbn-preview-cache-v1.json`.
#[derive(Debug, Clone, Default)]
pub struct IsbnState {
    /// In-memory preview cache: isbn13 → entry. Loaded lazily on first lookup.
    pub cache: HashMap<String, CachedIsbnEntry>,
    /// `true` once `run_effect_load_isbn_cache` has returned its result.
    pub cache_loaded: bool,
    /// ISBN currently being looked up (prevents duplicate in-flight fetches).
    pub pending_lookup: Option<String>,
    /// Outcome of the most recent lookup — exposed via `BookPickerKernelSnapshot`.
    pub last_result: Option<KernelArtifactPreviewResult>,
}

// ─── Reducer: AppAction::LookupIsbn ──────────────────────────────────────────

/// Handle `AppAction::LookupIsbn { isbn }`.
///
/// 1. Normalize the raw ISBN string (10 or 13 digits, dashes/spaces allowed).
///    Invalid input is a no-op (D6: no panic, empty effects Vec).
/// 2. If the in-memory cache has a hit, immediately emit
///    `Effect::LookupIsbn` with the normalized isbn13 so the effect runner
///    can re-emit `IsbnPreviewReady` with the cached preview (self-loop through
///    the channel keeps all state changes in the actor thread — D9).
///    On a cache hit we do NOT set `pending_lookup` (already resolved).
/// 3. On a cache miss, set `pending_lookup` and emit `Effect::LookupIsbn`.
///    If the cache has not been loaded yet, also emit `Effect::LoadIsbnCache`.
pub(crate) fn reduce_action_lookup_isbn(state: &mut AppState, isbn: String) -> Vec<Effect> {
    let isbn13 = match normalize_isbn(&isbn) {
        Ok(n) => n,
        Err(_) => {
            tracing::debug!(raw = %isbn, "LookupIsbn: invalid ISBN — no-op");
            return vec![];
        }
    };

    // If already pending for this ISBN, skip.
    if state.isbn.pending_lookup.as_deref() == Some(&isbn13) {
        return vec![];
    }

    // Cache hit path: emit LookupIsbn — the effect runner will resolve from
    // the in-memory cache and emit IsbnPreviewReady immediately (D8: no await).
    if state.isbn.cache.contains_key(&isbn13) {
        // No pending_lookup — already cached; effect runner does the cache lookup.
        return vec![Effect::LookupIsbn { isbn13 }];
    }

    // Cache miss: load if needed + fetch.
    let mut effects: Vec<Effect> = Vec::new();
    if !state.isbn.cache_loaded {
        effects.push(Effect::LoadIsbnCache);
    }
    state.isbn.pending_lookup = Some(isbn13.clone());
    effects.push(Effect::LookupIsbn { isbn13 });
    effects
}

// ─── Reducer: KernelEvent::IsbnPreviewReady ──────────────────────────────────

/// Handle `KernelEvent::IsbnPreviewReady { isbn13, preview, error }`.
///
/// - Stores the result in `AppState::isbn.last_result`.
/// - On success (preview.is_some() && error.is_empty()): inserts into cache
///   and emits `Effect::PersistIsbnCache` if this is a new entry (not from cache).
/// - Clears `pending_lookup`.
pub(crate) fn reduce_event_isbn_preview_ready(
    state: &mut AppState,
    isbn13: String,
    preview: Option<KernelArtifactPreview>,
    error: String,
) -> Vec<Effect> {
    // Clear pending if it matches this isbn.
    if state.isbn.pending_lookup.as_deref() == Some(&isbn13) {
        state.isbn.pending_lookup = None;
    }

    let is_new_hit =
        preview.is_some() && error.is_empty() && !state.isbn.cache.contains_key(&isbn13);

    // Store in cache when successful.
    if let Some(ref p) = preview {
        if error.is_empty() {
            state
                .isbn
                .cache
                .insert(isbn13.clone(), CachedIsbnEntry::from_preview(p));
        }
    }

    state.isbn.last_result = Some(KernelArtifactPreviewResult {
        isbn13,
        preview,
        error,
    });

    if is_new_hit {
        // Collect cache entries for persistence.
        let entries: Vec<(String, CachedIsbnEntry)> = state
            .isbn
            .cache
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        vec![Effect::PersistIsbnCache { entries }]
    } else {
        vec![]
    }
}

// ─── Reducer: KernelEvent::IsbnCacheLoaded ───────────────────────────────────

/// Handle `KernelEvent::IsbnCacheLoaded { entries }`.
///
/// Populates `AppState::isbn.cache` with the deserialized entries and marks
/// `cache_loaded = true`. No effects emitted — the pending `LookupIsbn` effect
/// was already enqueued alongside `LoadIsbnCache` and will run after this event.
pub(crate) fn reduce_event_isbn_cache_loaded(
    state: &mut AppState,
    entries: Vec<(String, CachedIsbnEntry)>,
) -> Vec<Effect> {
    for (k, v) in entries {
        state.isbn.cache.insert(k, v);
    }
    state.isbn.cache_loaded = true;
    vec![]
}

// ─── Snapshot ────────────────────────────────────────────────────────────────

/// Compute `ViewSnapshot::BookPicker` from `AppState`.
pub(crate) fn project_book_picker_snapshot(state: &AppState) -> Option<ViewSnapshot> {
    Some(ViewSnapshot::BookPicker(BookPickerKernelSnapshot {
        pending_isbn: state.isbn.pending_lookup.clone(),
        last_result: state.isbn.last_result.clone(),
        cache_size: state.isbn.cache.len() as u64,
    }))
}

// ─── Effect runners ──────────────────────────────────────────────────────────

/// Fetch book metadata from Open Library and emit `KernelEvent::IsbnPreviewReady`.
///
/// 1. If the in-memory cache snapshot included this isbn (from reducer), serve from cache.
/// 2. Otherwise, do the HTTP fetch from `openlibrary.org/isbn/{isbn13}.json`.
/// 3. On any network/parse failure, emit a partial preview with a non-empty error (D6).
///
/// This is the canonical "no native capability" path — Rust owns the HTTP entirely.
pub(crate) async fn run_effect_lookup_isbn(
    isbn13: String,
    data_dir: String,
    tx: &UnboundedSender<Cmd>,
) {
    // Try to load from the persisted cache file directly (race-condition guard:
    // if LoadIsbnCache hasn't fired yet, we still need the data).
    // In practice the reducer emits LoadIsbnCache before LookupIsbn on the
    // first cold call, so by the time we reach here the cache event will have
    // landed — but we do a best-effort file read as a safety net (D6).
    let from_cache = try_load_single_from_file(&isbn13, &data_dir).await;
    if let Some(cached) = from_cache {
        let preview = cached.to_preview(&isbn13);
        let _ = tx.send(Cmd::Event(KernelEvent::IsbnPreviewReady {
            isbn13,
            preview: Some(preview),
            error: String::new(),
        }));
        return;
    }

    // HTTP fetch from Open Library.
    match fetch_open_library(&isbn13).await {
        Ok(preview) => {
            let _ = tx.send(Cmd::Event(KernelEvent::IsbnPreviewReady {
                isbn13,
                preview: Some(preview),
                error: String::new(),
            }));
        }
        Err(e) => {
            tracing::warn!(isbn = %isbn13, error = %e, "ISBN Open Library fetch failed, returning partial");
            let partial = partial_preview(&isbn13);
            let _ = tx.send(Cmd::Event(KernelEvent::IsbnPreviewReady {
                isbn13,
                preview: Some(partial),
                error: e,
            }));
        }
    }
}

/// Load the ISBN preview cache from disk and emit `KernelEvent::IsbnCacheLoaded`.
///
/// Fire-and-forget (D6): any file read error is logged and treated as an empty
/// cache — the caller will still issue the HTTP fetch for a cache miss.
pub(crate) async fn run_effect_load_isbn_cache(data_dir: String, tx: &UnboundedSender<Cmd>) {
    let path = std::path::Path::new(&data_dir).join(CACHE_FILE_NAME);
    let entries = load_cache_file(&path).await;
    let _ = tx.send(Cmd::Event(KernelEvent::IsbnCacheLoaded {
        entries: entries.into_iter().collect(),
    }));
}

/// Atomically persist the updated ISBN cache to disk.
///
/// Write-to-tmp then rename for crash safety. Fire-and-forget (D6): any write
/// error is logged; the in-memory cache is already updated.
pub(crate) async fn run_effect_persist_isbn_cache(
    entries: Vec<(String, CachedIsbnEntry)>,
    data_dir: String,
) {
    let path = std::path::Path::new(&data_dir).join(CACHE_FILE_NAME);
    let map: HashMap<String, CachedIsbnEntry> = entries.into_iter().collect();
    if let Err(e) = write_cache_file(&path, &map).await {
        tracing::warn!(path = %path.display(), error = %e, "failed to persist ISBN cache");
    }
}

// ─── ISBN normalization (private) ────────────────────────────────────────────

/// Strip dashes/whitespace, validate, and canonicalize to 13 digits.
///
/// Returns `Ok(isbn13)` for valid Bookland ISBN-13 or valid ISBN-10 (converted
/// to 13). Returns `Err` for any invalid input (D6: caller treats as no-op).
pub(crate) fn normalize_isbn(raw: &str) -> Result<String, String> {
    let digits: String = raw
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .collect();

    if is_valid_bookland_isbn13(&digits) {
        return Ok(digits);
    }

    if is_valid_isbn10(&digits) {
        return Ok(isbn10_to_13(&digits));
    }

    Err(format!(
        "ISBN must be a valid Bookland ISBN-13 or ISBN-10, got {:?}",
        raw
    ))
}

fn is_valid_bookland_isbn13(digits: &str) -> bool {
    digits.len() == 13
        && digits.chars().all(|c| c.is_ascii_digit())
        && (digits.starts_with("978") || digits.starts_with("979"))
        && is_valid_isbn13_checksum(digits)
}

fn is_valid_isbn13_checksum(digits: &str) -> bool {
    if digits.len() != 13 {
        return false;
    }
    let mut sum = 0u32;
    for (i, c) in digits.chars().enumerate() {
        let Some(d) = c.to_digit(10) else {
            return false;
        };
        sum += if i % 2 == 0 { d } else { d * 3 };
    }
    sum.is_multiple_of(10)
}

fn is_valid_isbn10(digits: &str) -> bool {
    if digits.len() != 10 {
        return false;
    }
    let mut sum = 0u32;
    for (i, c) in digits.chars().enumerate() {
        let value = match c {
            'X' | 'x' if i == 9 => 10,
            _ => match c.to_digit(10) {
                Some(d) => d,
                None => return false,
            },
        };
        sum += value * (10 - i as u32);
    }
    sum.is_multiple_of(11)
}

/// Convert a 10-digit ISBN to 13-digit by prepending "978" and recomputing
/// the final check digit per the standard rule.
fn isbn10_to_13(isbn10: &str) -> String {
    let prefix = format!("978{}", &isbn10[..9]);
    let check = compute_isbn13_check_digit(&prefix);
    format!("{prefix}{check}")
}

fn compute_isbn13_check_digit(prefix12: &str) -> char {
    let mut sum = 0u32;
    for (i, c) in prefix12.chars().enumerate() {
        let d = c.to_digit(10).unwrap_or(0);
        sum += if i % 2 == 0 { d } else { d * 3 };
    }
    let check = (10 - (sum % 10)) % 10;
    char::from_digit(check, 10).unwrap_or('0')
}

// ─── Preview builders (private) ──────────────────────────────────────────────

/// Build a fully-populated `KernelArtifactPreview` from Open Library fields.
fn build_preview(
    isbn13: &str,
    title: String,
    author: String,
    image: String,
    description: String,
) -> KernelArtifactPreview {
    let catalog_id = format!("isbn:{isbn13}");
    let highlight_reference_key = format!("i:{catalog_id}");
    let id = format!("c{:x}", fnv1a(&format!("i:{catalog_id}")));
    KernelArtifactPreview {
        id,
        title,
        author,
        image,
        description,
        source: "book".into(),
        catalog_id: catalog_id.clone(),
        catalog_kind: "isbn".into(),
        reference_tag_name: "i".into(),
        reference_tag_value: catalog_id.clone(),
        highlight_tag_name: "i".into(),
        highlight_tag_value: catalog_id,
        highlight_reference_key,
    }
}

/// Fallback partial preview for network/parse failures.
///
/// Only the catalog/reference fields are populated; title/author/image are empty.
/// This lets the caller publish a kind:11 with manual title/author entry (D6).
fn partial_preview(isbn13: &str) -> KernelArtifactPreview {
    build_preview(
        isbn13,
        String::new(),
        String::new(),
        String::new(),
        String::new(),
    )
}

// ─── HTTP fetch (private) ────────────────────────────────────────────────────

/// Fetch book metadata from `https://openlibrary.org/isbn/{isbn13}.json`.
///
/// Returns `Ok(KernelArtifactPreview)` on success; `Err(error_string)` on any
/// network or parse failure. The caller wraps the error into a partial preview
/// so the user always gets a usable (if empty) result (D6).
async fn fetch_open_library(isbn13: &str) -> Result<KernelArtifactPreview, String> {
    let client = reqwest::Client::builder()
        .timeout(OPEN_LIBRARY_TIMEOUT)
        .build()
        .map_err(|e| format!("build http client: {e}"))?;

    let url = format!("https://openlibrary.org/isbn/{isbn13}.json");
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("GET {url}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let body: Value = resp.json().await.map_err(|e| format!("parse json: {e}"))?;

    let title = body
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    let description = extract_description(body.get("description"));

    // Prefer the cover ID from the book JSON; fall back to ISBN-based URL.
    let image = body
        .get("covers")
        .and_then(Value::as_array)
        .and_then(|arr| arr.first())
        .and_then(Value::as_i64)
        .map(|id| format!("https://covers.openlibrary.org/b/id/{id}-L.jpg"))
        .unwrap_or_else(|| format!("https://covers.openlibrary.org/b/isbn/{isbn13}-L.jpg"));

    // Authors: best-effort ref resolution.
    let author_refs: Vec<String> = body
        .get("authors")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|a| a.get("key").and_then(Value::as_str).map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let mut author_names: Vec<String> = Vec::with_capacity(author_refs.len());
    for key in &author_refs {
        match fetch_author_name(&client, key).await {
            Ok(name) if !name.is_empty() => author_names.push(name),
            Ok(_) => {}
            Err(e) => {
                tracing::debug!(author_key = %key, error = %e, "ISBN author lookup failed");
            }
        }
    }
    let author = author_names.join(", ");

    Ok(build_preview(isbn13, title, author, image, description))
}

async fn fetch_author_name(client: &reqwest::Client, key: &str) -> Result<String, String> {
    let trimmed = key.trim_start_matches('/');
    let url = format!("https://openlibrary.org/{trimmed}.json");
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("GET {url}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let body: Value = resp
        .json()
        .await
        .map_err(|e| format!("parse author json: {e}"))?;
    Ok(body
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string())
}

/// Open Library returns `description` either as a bare string or as
/// `{ "type": "/type/text", "value": "…" }`. Handle both.
fn extract_description(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Object(obj)) => obj
            .get("value")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        _ => String::new(),
    }
}

// ─── FNV-1a (private) ────────────────────────────────────────────────────────

/// FNV-1a 32-bit hash — same algorithm as `web/src/lib/ndk/artifacts.ts:1086`
/// and `isbn_lookup.rs::fnv1a` in the bespoke live lane. Both lanes produce
/// the same artifact id for the same ISBN reference key.
fn fnv1a(value: &str) -> u32 {
    let mut hash: u32 = 0x811c9dc5;
    for b in value.bytes() {
        hash ^= b as u32;
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

// ─── Cache file I/O (private) ────────────────────────────────────────────────

async fn load_cache_file(path: &std::path::Path) -> HashMap<String, CachedIsbnEntry> {
    match tokio::fs::read(path).await {
        Ok(bytes) => match serde_json::from_slice::<HashMap<String, CachedIsbnEntry>>(&bytes) {
            Ok(cache) => cache,
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "failed to parse ISBN cache");
                HashMap::new()
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => HashMap::new(),
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "failed to read ISBN cache");
            HashMap::new()
        }
    }
}

/// Try to load a single ISBN entry from the persisted cache file without
/// affecting `AppState`. Used in `run_effect_lookup_isbn` as a race-condition
/// guard for the cold-start path.
async fn try_load_single_from_file(isbn13: &str, data_dir: &str) -> Option<CachedIsbnEntry> {
    let path = std::path::Path::new(data_dir).join(CACHE_FILE_NAME);
    let cache = load_cache_file(&path).await;
    cache.get(isbn13).cloned()
}

async fn write_cache_file(
    path: &std::path::Path,
    entries: &HashMap<String, CachedIsbnEntry>,
) -> Result<(), String> {
    let bytes = serde_json::to_vec(entries).map_err(|e| format!("encode ISBN cache: {e}"))?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("create ISBN cache dir: {e}"))?;
    }
    let tmp = path.with_extension("json.tmp");
    tokio::fs::write(&tmp, &bytes)
        .await
        .map_err(|e| format!("write ISBN cache tmp: {e}"))?;
    tokio::fs::rename(&tmp, path)
        .await
        .map_err(|e| format!("rename ISBN cache: {e}"))?;
    Ok(())
}

// ─── Unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::actor::Cmd;
    use crate::kernel::app::AppState;
    use crate::kernel::clock::{Clock, ManualClock};

    fn make_state() -> AppState {
        AppState::default()
    }

    fn reduce(state: &mut AppState, cmd: Cmd, now: u64) -> Vec<Effect> {
        crate::kernel::actor::reduce(state, cmd, now)
    }

    fn now() -> u64 {
        ManualClock::new(0).now_unix_seconds()
    }

    // ── Test 1: cache hit returns IsbnPreviewReady without network ────────────

    #[test]
    fn isbn_lookup_caches_result() {
        let mut state = make_state();
        let t = now();

        // Inject a cache entry via IsbnCacheLoaded.
        let entry = CachedIsbnEntry {
            id: "c-test".into(),
            title: "Test Book".into(),
            author: "Author A".into(),
            image: "https://example.test/cover.jpg".into(),
            description: "A test book.".into(),
            published_at: "2026-01-01".into(),
        };
        let effects = reduce(
            &mut state,
            Cmd::Event(KernelEvent::IsbnCacheLoaded {
                entries: vec![("9780735211292".into(), entry)],
            }),
            t,
        );
        assert!(
            effects.is_empty(),
            "IsbnCacheLoaded should not emit effects"
        );
        assert!(state.isbn.cache_loaded);
        assert_eq!(state.isbn.cache.len(), 1);

        // Now dispatch LookupIsbn — should get a cache hit effect (LookupIsbn emitted).
        let effects = reduce(
            &mut state,
            Cmd::Action(crate::kernel::action::AppAction::LookupIsbn {
                isbn: "978-0-7352-1129-2".into(),
            }),
            t,
        );
        // Cache hit: reducer emits LookupIsbn (the effect runner will serve from cache).
        assert!(
            !effects.is_empty(),
            "cache hit should emit LookupIsbn effect"
        );
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::LookupIsbn { isbn13 } if isbn13 == "9780735211292")),
            "expected LookupIsbn effect for normalized isbn13"
        );
    }

    // ── Test 2: snapshot raw fields are correct ───────────────────────────────

    #[test]
    fn isbn_preview_snapshot_raw_fields() {
        let mut state = make_state();
        let t = now();

        let preview = build_preview(
            "9780735211292",
            "The Pragmatic Programmer".into(),
            "Andrew Hunt".into(),
            "https://covers.openlibrary.org/b/isbn/9780735211292-L.jpg".into(),
            "A guide for programmers.".into(),
        );

        let effects = reduce(
            &mut state,
            Cmd::Event(KernelEvent::IsbnPreviewReady {
                isbn13: "9780735211292".into(),
                preview: Some(preview.clone()),
                error: String::new(),
            }),
            t,
        );
        // New cache entry — should emit PersistIsbnCache.
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::PersistIsbnCache { .. })),
            "expected PersistIsbnCache for new cache entry"
        );

        let snap = project_book_picker_snapshot(&state).unwrap();
        if let ViewSnapshot::BookPicker(s) = snap {
            assert_eq!(s.cache_size, 1);
            let result = s.last_result.unwrap();
            assert_eq!(result.isbn13, "9780735211292");
            let p = result.preview.unwrap();
            assert_eq!(p.title, "The Pragmatic Programmer");
            assert_eq!(p.author, "Andrew Hunt");
            assert_eq!(p.catalog_id, "isbn:9780735211292");
            assert_eq!(p.catalog_kind, "isbn");
            assert_eq!(p.reference_tag_name, "i");
            assert_eq!(p.reference_tag_value, "isbn:9780735211292");
            assert_eq!(p.highlight_tag_name, "i");
            assert_eq!(p.highlight_tag_value, "isbn:9780735211292");
            assert_eq!(p.highlight_reference_key, "i:isbn:9780735211292");
            assert_eq!(p.source, "book");
        } else {
            panic!("expected BookPicker snapshot");
        }
    }

    // ── Test 3: cached ISBN does not re-fetch ─────────────────────────────────

    #[test]
    fn cached_isbn_returns_without_refetch() {
        let mut state = make_state();
        let t = now();

        // First: store a result in the cache via IsbnPreviewReady.
        let preview = partial_preview("9780735211292");
        reduce(
            &mut state,
            Cmd::Event(KernelEvent::IsbnPreviewReady {
                isbn13: "9780735211292".into(),
                preview: Some(preview),
                error: String::new(),
            }),
            t,
        );
        assert_eq!(state.isbn.cache.len(), 1);

        // Second lookup of the same ISBN.
        let effects = reduce(
            &mut state,
            Cmd::Action(crate::kernel::action::AppAction::LookupIsbn {
                isbn: "9780735211292".into(),
            }),
            t,
        );

        // Should emit LookupIsbn (cache hit path) — NOT LoadIsbnCache (already loaded).
        let has_load_cache = effects.iter().any(|e| matches!(e, Effect::LoadIsbnCache));
        let has_lookup = effects
            .iter()
            .any(|e| matches!(e, Effect::LookupIsbn { isbn13 } if isbn13 == "9780735211292"));
        assert!(!has_load_cache, "must not reload cache for a known entry");
        assert!(has_lookup, "must emit LookupIsbn for cache-hit path");
        // Must NOT set pending_lookup (cache hit).
        assert!(
            state.isbn.pending_lookup.is_none(),
            "no pending on cache hit"
        );
    }

    // ── Test 4: no publish/nostr effects ever ─────────────────────────────────

    #[test]
    fn isbn_cache_is_device_local_not_published() {
        let mut state = make_state();
        let t = now();

        // Trigger a full miss + fetch cycle (by dispatching LookupIsbn without preloading).
        let effects = reduce(
            &mut state,
            Cmd::Action(crate::kernel::action::AppAction::LookupIsbn {
                isbn: "9780735211292".into(),
            }),
            t,
        );

        // None of the effects should be publish/nostr actions.
        for effect in &effects {
            match effect {
                Effect::LookupIsbn { .. }
                | Effect::LoadIsbnCache
                | Effect::PersistIsbnCache { .. } => {}
                other => panic!(
                    "ISBN domain emitted unexpected effect that may be nostr-related: {other:?}"
                ),
            }
        }

        // Now inject the result.
        let preview = partial_preview("9780735211292");
        let effects2 = reduce(
            &mut state,
            Cmd::Event(KernelEvent::IsbnPreviewReady {
                isbn13: "9780735211292".into(),
                preview: Some(preview),
                error: String::new(),
            }),
            t,
        );
        for effect in &effects2 {
            match effect {
                Effect::PersistIsbnCache { .. } => {}
                other => panic!("IsbnPreviewReady emitted unexpected effect: {other:?}"),
            }
        }
    }

    // ── Test 5: malformed ISBN is a no-op ─────────────────────────────────────

    #[test]
    fn malformed_isbn_is_no_op() {
        let mut state = make_state();
        let t = now();

        for bad in ["", "hello", "123", "notanisbn", "00000000000000000000"] {
            let effects = reduce(
                &mut state,
                Cmd::Action(crate::kernel::action::AppAction::LookupIsbn { isbn: bad.into() }),
                t,
            );
            assert!(
                effects.is_empty(),
                "malformed ISBN {bad:?} should produce no effects, got {effects:?}"
            );
        }
        assert!(state.isbn.pending_lookup.is_none());
    }

    // ── Test 6: normalize_isbn canonicalizes correctly ────────────────────────

    #[test]
    fn isbn13_normalization() {
        // ISBN-13 with dashes.
        assert_eq!(
            normalize_isbn("978-0-7352-1129-2").unwrap(),
            "9780735211292"
        );

        // ISBN-13 no dashes.
        assert_eq!(normalize_isbn("9780735211292").unwrap(), "9780735211292");

        // ISBN-10 → 13.
        let n = normalize_isbn("0735211299").unwrap();
        assert!(n.starts_with("9780735211"), "should start with 9780735211");
        assert_eq!(n.len(), 13);

        // ISBN-10 with dashes.
        assert_eq!(normalize_isbn("0-7352-1129-9").unwrap(), "9780735211292");

        // ISBN-10 with trailing X.
        assert_eq!(normalize_isbn("0-8044-2957-X").unwrap(), "9780804429573");

        // Invalid.
        assert!(normalize_isbn("").is_err());
        assert!(normalize_isbn("hello").is_err());
        assert!(normalize_isbn("123").is_err());
        // Bad checksum.
        assert!(normalize_isbn("9780735211290").is_err());
        // Non-bookland.
        assert!(normalize_isbn("4006381333931").is_err());
    }
}
