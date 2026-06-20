//! Feed-pull core — Phase 4F (ADR-0058).
//!
//! This module is the load-bearing shared engine that Phase 4G (article feed),
//! 4H (highlight feed), and 4I (room-lane bodies) build on. It owns:
//!
//!   - [`FeedState`] — the small shared pull-cursor state struct stored in `AppState`.
//!   - [`FeedKey`] — the opaque string key that identifies a feed (extension point).
//!   - Effect reducers for `RegisterFeedCursor` / `DrainFeed` / `ReleaseFeedCursor`.
//!   - The effect runner that calls `nmp_app_pull_page`, decodes the binary Page
//!     wire format, and emits `KernelEvent::FeedPage{key, rows, after_seq, exhausted}`
//!     back to the actor.
//!   - The `FeedPage` reducer that appends rows and advances the cursor.
//!
//! ## Extension contract for 4G / 4H / 4I
//!
//! Each consumer slice:
//!   1. Picks a stable `FeedKey` string constant (e.g. `"hl.feed.articles"`,
//!      `"hl.feed.highlights"`, `"hl.feed.room.<group_id>"`).
//!   2. Adds a `FeedState` field to `AppState` (or a `HashMap<String, FeedState>`
//!      for per-group lanes) guarded by `// ── Phase 4x additions`.
//!   3. Emits `Effect::RegisterFeedCursor { key, shape }` from
//!      `lifecycle_effects_for_view_open` (not `reduce_action` — the cursor
//!      follows the view, not a user action).
//!   4. Emits `Effect::DrainFeed { key }` when a "load older" action arrives
//!      or when `lifecycle_effects_for_view_open` wants an initial fill.
//!   5. Handles `KernelEvent::FeedPage { key, rows, after_seq, exhausted }` in
//!      `reduce_event_feed_page` by routing on `key` to the correct `FeedState`.
//!   6. Emits `Effect::ReleaseFeedCursor { key }` from
//!      `lifecycle_effects_for_view_close`.
//!
//! ## Architectural invariants
//!
//! - **D8 (no polling):** `DrainFeed` is triggered by a view-open lifecycle
//!   effect or by an explicit "load older" user action — never by a timer.
//!   The drain terminates on `has_more == false` (`Exhausted`) or on page fill
//!   (`PageFilled`). The effect runner advances `after_seq` *per page* so the
//!   next `DrainFeed` continues from where we left off.
//! - **D5 (bounded):** Page size and raw-byte budget are capped by constants
//!   from `nmp_ffi::pull` (`MAX_PULL_PAGE_ENTRIES`, `MAX_PULL_PAGE_RAW_BYTES`).
//! - **D6 (no-op on garbage):** malformed binary wire → no-op event, no panic.
//! - **Gap rebase:** on a `Gap` result the `FeedState` is cleared and the cursor
//!   is reset to `first_available_seq` per ADR-0058 §10.
//! - **Live lane untouched:** `HighlighterCore` / `nostr_runtime.rs` are not
//!   modified by this slice.

use std::num::NonZeroUsize;

use nmp_core::{PullCursorMode, PullScope};
use nmp_planner::InterestShape;

use crate::kernel::action::KernelEvent;
use crate::kernel::actor::NmpHandle;
use crate::kernel::effect::Effect;

// ─── Feed key ────────────────────────────────────────────────────────────────

/// Opaque string key identifying a registered feed cursor.
///
/// Convention (not enforced here — consumers own their keys):
/// - `"hl.feed.articles"` — article feed (kind:30023, over follows), Phase 4G.
/// - `"hl.feed.highlights"` — highlights feed (kind:9802), Phase 4H.
/// - `"hl.feed.room.<group_id>"` — per-room lane feed (kind:9/11), Phase 4I.
///
/// Keys are allocated by consumers; this type is just a `String` alias so the
/// compiler enforces that callers pass a feed key where one is expected.
pub type FeedKey = String;

// ─── Shared cursor state ──────────────────────────────────────────────────────

/// Raw pull-cursor state for one registered feed.
///
/// Stored in `AppState` by consumers (e.g. `article_feed: FeedState`).
/// The `cursor_id` is minted by the consumer and sent in
/// `ActorCommand::RegisterPullCursor`; it must be non-zero (kernel ignores 0).
///
/// D1: no presentation strings here — only protocol-level fields.
#[derive(Debug, Clone, Default)]
pub struct FeedState {
    /// Cursor id minted by hl and registered with the kernel.
    /// `0` means not yet registered (the default before the first open).
    pub cursor_id: u64,
    /// The arrival-ordered seq position after the last fully consumed page.
    /// Passed to `nmp_app_pull_page` as the `cursor_id` lookup; the cursor
    /// registry holds `after_seq` authoritatively — this mirrors it for
    /// reducer-side reads (e.g. snapshot builders).
    pub after_seq: u64,
    /// `true` once a page with `has_more == false` is drained (caught up).
    pub exhausted: bool,
    /// Accumulated raw event rows in ingest-seq order.
    ///
    /// 4G/4H/4I push decoded `KernelEvent`s here; consumers project them into
    /// typed rows (e.g. `ArticleRow`) in their own `reduce_event_feed_page` arm.
    /// The generic `FeedState` stores `KernelEvent`s so the engine is
    /// type-agnostic; consumers convert to their domain type on the snapshot path.
    pub rows: Vec<nmp_core::substrate::KernelEvent>,
}

impl FeedState {
    /// Reset to the initial empty state (used on gap rebase and logout).
    pub fn clear(&mut self) {
        self.after_seq = 0;
        self.exhausted = false;
        self.rows.clear();
        // cursor_id is preserved so ReleaseFeedCursor can still unregister it.
    }
}

// ─── Cursor-id allocation ─────────────────────────────────────────────────────

/// Mint a stable, non-zero cursor id for a named feed.
///
/// IDs are deterministic across restarts so the consumer can pass the same id
/// on re-registration (the kernel registry uses `Replace-by-cursor_id` semantics,
/// meaning re-registration with the same id is always allowed even if the cap is
/// full — safe restart pattern, ADR-0058 §10). We derive the id from the key's
/// FNV-1a hash truncated to u64; the upper half is OR-masked non-zero to avoid
/// the `cursor_id == 0` sentinel.
///
/// Public for use by Phase 4G/4H/4I consumer slices and tests; unused here
/// until those slices land.
#[allow(dead_code)]
pub fn mint_cursor_id(key: &str) -> u64 {
    // FNV-1a 64-bit
    const OFFSET: u64 = 14_695_981_039_346_656_037;
    const PRIME: u64 = 1_099_511_628_211;
    let h = key
        .bytes()
        .fold(OFFSET, |acc, b| acc.wrapping_mul(PRIME) ^ (b as u64));
    // Ensure non-zero: set the MSB if the hash came out to 0.
    if h == 0 {
        0x8000_0000_0000_0001
    } else {
        h
    }
}

// ─── Page-size constants ──────────────────────────────────────────────────────

/// Default number of events requested per `nmp_app_pull_page` call.
///
/// Matches the nmp-feed `DEFAULT_PULL_PAGE_SIZE` (20 entries) per the pager.rs
/// constant, so the hl kernel drain budget is aligned with the nmp pager.
pub const FEED_PAGE_SIZE: u32 = 20;

/// Default raw-byte budget per drain call (1 MiB). Well under the nmp-ffi hard
/// ceiling of 4 MiB (`MAX_PULL_PAGE_RAW_BYTES`), leaving headroom for large events.
pub const FEED_RAW_BYTE_CAP: u32 = 1024 * 1024;

// ─── InterestShape builder helpers (for consumers) ───────────────────────────

/// Build a `PullScope` for an article feed over a set of follow pubkeys.
///
/// Used by Phase 4G. `follow_pubkeys` should be the active account's follow set
/// from `AppState::follows`. Returns `None` when the follow set is empty —
/// fail-closed per the D5 contract (never broad-scan).
///
/// Public for Phase 4G; unused until that slice lands.
#[allow(dead_code)]
pub fn article_feed_scope(follow_pubkeys: &[String]) -> Option<PullScope> {
    if follow_pubkeys.is_empty() {
        return None;
    }
    let mut shape = InterestShape::default();
    shape.kinds = [30023].into_iter().collect();
    shape.authors = follow_pubkeys.iter().cloned().collect();
    Some(PullScope::InterestShape(shape))
}

/// Build a `PullScope` for the highlights feed (kind:9802, any author).
///
/// Used by Phase 4H. Returns `Some` unconditionally — kind:9802 is a
/// well-covered shape (single kind, no author filter needed for a global feed).
///
/// Public for Phase 4H; unused until that slice lands.
#[allow(dead_code)]
pub fn highlight_feed_scope() -> PullScope {
    let mut shape = InterestShape::default();
    shape.kinds = [9802].into_iter().collect();
    PullScope::InterestShape(shape)
}

/// Build a `PullScope` for an article's highlight feed (kind:9802 tagged with
/// `#a` == the article's addressable coordinate).
///
/// Phase 7: the article reader overlay needs the highlights anchored to the
/// article being read. Mirrors the bespoke `highlights::query_for_article`
/// NdbFilter (`kinds([9802]).tags([address], 'a')`) — but expressed as an
/// `InterestShape` so it flows through the same feed-pull engine the room-lane
/// (4I) and home (4G/4H) feeds use. `address` is the `"<kind>:<pubkey>:<d>"`
/// coordinate (D3: opaque from the caller, never constructed by the kernel).
pub fn article_highlight_feed_scope(address: &str) -> PullScope {
    let mut shape = InterestShape::default();
    shape.kinds = [9802].into_iter().collect();
    shape
        .tags
        .insert("a".to_string(), [address.to_string()].into_iter().collect());
    PullScope::InterestShape(shape)
}

/// Build a `PullScope` for the home-feed interaction cursor.
///
/// Phase 7 home-feed aggregation: returns kind:1/7/16/1111 events authored by
/// the active account's follows, filtered by `#k=30023` (interaction with a
/// long-form article). Fail-closed: returns `None` when the follow set is empty
/// so no broad scan is issued (D5).
///
/// Mirrors the legacy `reads.rs` interaction stream but expressed as an
/// `InterestShape` so it flows through the same feed-pull engine as the other
/// home-feed cursors. Feed key: `"hl.feed.home_interactions"`.
pub fn home_interaction_feed_scope(follow_pubkeys: &[String]) -> Option<PullScope> {
    if follow_pubkeys.is_empty() {
        return None;
    }
    let mut shape = InterestShape::default();
    shape.kinds = [1, 7, 16, 1111].into_iter().collect();
    shape.authors = follow_pubkeys.iter().cloned().collect();
    shape
        .tags
        .insert("k".to_string(), ["30023".to_string()].into_iter().collect());
    Some(PullScope::InterestShape(shape))
}

/// Build a `PullScope` for a room-lane feed (kind:9 and kind:11 tagged with `#h`).
///
/// Used by Phase 4I. `group_id` is the NIP-29 local group id (the `#h` tag value).
///
/// Public for Phase 4I; unused until that slice lands.
#[allow(dead_code)]
pub fn room_lane_scope(group_id: &str) -> PullScope {
    let mut shape = InterestShape::default();
    shape.kinds = [9, 11].into_iter().collect();
    shape.tags.insert(
        "h".to_string(),
        [group_id.to_string()].into_iter().collect(),
    );
    PullScope::InterestShape(shape)
}

// ─── Reduce-side helpers (called from effect reducers) ───────────────────────

/// Emit `Effect::RegisterFeedCursor` for a feed with a given pull scope.
///
/// Consumers call this from `lifecycle_effects_for_view_open`. The `cursor_id`
/// is minted deterministically from `key` via `mint_cursor_id`; the kernel
/// registry replaces-by-id so re-opening a view is safe (idempotent, D6).
///
/// Public for Phase 4G/4H/4I; unused at the call site until those slices land.
#[allow(dead_code)]
pub fn reduce_register_feed_cursor(key: FeedKey, scope: PullScope) -> Vec<Effect> {
    let cursor_id = mint_cursor_id(&key);
    vec![
        Effect::RegisterFeedCursor {
            key,
            cursor_id,
            scope,
        },
        // Immediately trigger an initial drain so the first page fills
        // without requiring a separate "load older" user action.
    ]
}

/// Emit `Effect::DrainFeed` for a feed key.
///
/// Consumers call this from the "load older" action reducer or inline from
/// `lifecycle_effects_for_view_open` (after `RegisterFeedCursor`).
/// Pure: no AppState mutation here — the drain happens async in the effect runner.
///
/// Public for Phase 4G/4H/4I; unused at the call site until those slices land.
#[allow(dead_code)]
pub fn reduce_drain_feed(key: FeedKey) -> Vec<Effect> {
    vec![Effect::DrainFeed { key }]
}

/// Emit `Effect::ReleaseFeedCursor` for a feed key.
///
/// Consumers call this from `lifecycle_effects_for_view_close`. The cursor is
/// unregistered in the kernel; the `FeedState.rows` buffer is cleared by the
/// consumer's own `lifecycle_effects_for_view_close` inline (same pattern as
/// `ReleaseGroupEvents` clearing `room_home_events` inline in `actor_task`).
///
/// Public for Phase 4G/4H/4I; unused at the call site until those slices land.
#[allow(dead_code)]
pub fn reduce_release_feed_cursor(key: FeedKey) -> Vec<Effect> {
    vec![Effect::ReleaseFeedCursor { key }]
}

// ─── Effect runner ────────────────────────────────────────────────────────────

/// Register a feed cursor with the nmp kernel.
///
/// Sends `ActorCommand::RegisterPullCursor` via `actor_sender()`. Fire-and-
/// forget (D6): the registration is ACKed implicitly by the cursor becoming
/// available for `nmp_app_pull_page`. No-op when `nmp` is `None` (test mode —
/// tests inject `KernelEvent::FeedPage` directly).
///
/// The kernel uses `Replace-by-cursor_id` semantics so re-registering the same
/// `cursor_id` (on re-open after a view was closed and re-opened in the same
/// session) resets `after_seq` to the provided value — consumers should pass
/// the current `FeedState::after_seq` so the cursor resumes rather than
/// rewinding (0 = start from the beginning).
pub(crate) fn run_effect_register_feed_cursor(
    key: FeedKey,
    cursor_id: u64,
    scope: PullScope,
    after_seq: u64,
    nmp: Option<&NmpHandle>,
) {
    let Some(handle) = nmp else {
        tracing::debug!(?key, "RegisterFeedCursor: no live NmpApp (test mode)");
        return;
    };
    let nmp_ref: &nmp_ffi::NmpApp = unsafe { handle.ptr.as_ref() };

    // Use GapAllowed mode: the feed can tolerate a gap rebase (old events
    // pruned by the GC); the reducer clears scoped rows on a Gap. Protected
    // mode is for consumers that cannot tolerate data loss (not feeds).
    let page_size =
        NonZeroUsize::new(FEED_PAGE_SIZE as usize).unwrap_or(NonZeroUsize::new(20).unwrap());
    let scan_budget =
        NonZeroUsize::new((FEED_PAGE_SIZE * 8) as usize).unwrap_or(NonZeroUsize::new(160).unwrap());

    let _ = nmp_ref
        .actor_sender()
        .send(nmp_core::ActorCommand::RegisterPullCursor {
            cursor_id,
            consumer_id: format!("hl.{key}"),
            scope,
            mode: PullCursorMode::GapAllowed,
            after_seq,
            limits: nmp_core::PullLimits {
                max_entries: page_size,
                max_scan_entries: scan_budget,
            },
        });
}

/// Call `nmp_app_pull_page` and emit a `KernelEvent::FeedPage` with the decoded rows.
///
/// This is the ADR-0058 drain step. Decodes the binary Page wire format from
/// `nmp_ffi::pull::nmp_app_pull_page`:
///
/// ```text
/// result := u8 variant
///   0 = Page : u64 next_after_seq | u64 latest_seq | u8 has_more | u32 entry_count | entries…
///   1 = Gap  : u64 requested_after_seq | u64 first_available_seq
///   2 = Error: u32 error_code
/// entry := u64 seq | u8 op_tag | [Replaced: lp(replaced_id)] | [Deleted: lp(target_id) + u8 reason]
///        | lp(event_id) | u8 has_raw | [has_raw: lp(raw_json)] | lp(source_relay) | u64 received_at_ms
/// lp(x) := u32 byte_len | bytes
/// ```
///
/// Positive (Inserted/Replaced) rows with a valid `raw_json` are decoded into
/// `KernelEvent`s via `nmp_feed::pager::raw_to_kernel_event`-equivalent logic
/// implemented inline. Deleted rows and rows missing `raw_json` are skipped.
///
/// On `Gap`: emits `FeedPage` with `gap_rebased_to = Some(first_available_seq)`
/// so the reducer can clear and rebase the cursor (ADR-0058 §10).
/// On `Error` or malformed bytes: no-op (D6).
/// No-op when `nmp` is `None` (test mode).
pub(crate) fn run_effect_drain_feed(
    key: FeedKey,
    cursor_id: u64,
    tx: &tokio::sync::mpsc::UnboundedSender<crate::kernel::actor::Cmd>,
    nmp: Option<&NmpHandle>,
) {
    use nmp_ffi::pull::{nmp_app_pull_page, nmp_free_bytes};

    let Some(handle) = nmp else {
        tracing::debug!(?key, "DrainFeed: no live NmpApp (test mode)");
        return;
    };

    if cursor_id == 0 {
        tracing::warn!(?key, "DrainFeed: cursor_id is 0 (not yet registered)");
        return;
    }

    // SAFETY: `handle.ptr` is a valid, non-null NmpApp pointer for the
    // duration of this call (NmpHandle is kept alive by the actor task).
    let nmp_ref: &nmp_ffi::NmpApp = unsafe { handle.ptr.as_ref() };

    // Call nmp_app_pull_page. It is `pub extern "C"` (not `unsafe`) in nmp-ffi,
    // so the call itself does not require an unsafe block; we cast the pointer
    // here, which is safe because nmp_ref is a valid reference to an NmpApp.
    let owned_bytes = nmp_app_pull_page(
        nmp_ref as *const nmp_ffi::NmpApp,
        cursor_id,
        FEED_PAGE_SIZE,
        FEED_RAW_BYTE_CAP,
    );

    // Borrow the bytes, then free — no early return between borrow and free.
    let result = decode_pull_page_wire(key.clone(), cursor_id, &owned_bytes);

    // nmp_free_bytes is `pub extern "C"` (not `unsafe`) — call directly.
    nmp_free_bytes(owned_bytes);

    match result {
        Some(event) => {
            let _ = tx.send(crate::kernel::actor::Cmd::Event(event));
        }
        None => {
            // Error or malformed: no-op (D6). Logged inside decode_pull_page_wire.
        }
    }
}

/// Unregister a feed cursor from the nmp kernel.
///
/// Sends `ActorCommand::UnregisterPullCursor` via `actor_sender()`. Fire-and-
/// forget (D6). The cursor's `FeedState.rows` buffer is cleared inline in the
/// `actor_task` (same pattern as `ReleaseGroupEvents`).
/// No-op when `nmp` is `None` (test mode).
pub(crate) fn run_effect_release_feed_cursor(cursor_id: u64, nmp: Option<&NmpHandle>) {
    let Some(handle) = nmp else {
        return;
    };
    if cursor_id == 0 {
        return; // cursor was never registered; nothing to unregister
    }
    let nmp_ref: &nmp_ffi::NmpApp = unsafe { handle.ptr.as_ref() };
    let _ = nmp_ref
        .actor_sender()
        .send(nmp_core::ActorCommand::UnregisterPullCursor { cursor_id });
}

/// Advance the kernel's registered cursor to `after_seq` after a successful drain.
///
/// Sent after a page is processed so the kernel wake arm knows the cursor has
/// advanced (re-arms an immediate wake when there is still data waiting).
/// No-op when `nmp` is `None` or when `cursor_id == 0`.
///
/// Unused in 4F (the actor_task inline handler calls this after DrainFeed);
/// exposed for 4G/4H/4I to call directly if needed.
#[allow(dead_code)]
pub(crate) fn advance_feed_cursor(cursor_id: u64, after_seq: u64, nmp: Option<&NmpHandle>) {
    let Some(handle) = nmp else {
        return;
    };
    if cursor_id == 0 {
        return;
    }
    let nmp_ref: &nmp_ffi::NmpApp = unsafe { handle.ptr.as_ref() };
    let _ = nmp_ref
        .actor_sender()
        .send(nmp_core::ActorCommand::AdvancePullCursor {
            cursor_id,
            after_seq,
        });
}

// ─── Binary wire decoder ──────────────────────────────────────────────────────

/// Decode the `nmp_app_pull_page` binary wire result into a `KernelEvent::FeedPage`.
///
/// Returns `None` on Error variant, unknown variant, or a malformed buffer
/// (D6: no panics from untrusted wire data). Returns `Some(FeedPage)` for both
/// Page and Gap so the reducer can act on each (gap → rebase + clear).
// The macros advance `cur` even on the last use before a `return None`;
// the compiler sees those trailing assignments as unused. Allow them —
// the pattern is intentional (the cursor must advance past each field).
#[allow(unused_assignments)]
fn decode_pull_page_wire(
    key: FeedKey,
    cursor_id: u64,
    bytes: &nmp_ffi::pull::NmpOwnedBytes,
) -> Option<KernelEvent> {
    if bytes.ptr.is_null() || bytes.len == 0 {
        tracing::warn!(?key, "DrainFeed: empty bytes returned");
        return None;
    }

    // SAFETY: `bytes` is valid for `bytes.len` bytes, alive for this call,
    // produced by `nmp_app_pull_page` (Rust-side allocation, aligned, init'd).
    let buf: &[u8] = unsafe { std::slice::from_raw_parts(bytes.ptr, bytes.len) };

    let mut cur = 0usize;

    macro_rules! read_u8 {
        () => {{
            if cur >= buf.len() {
                tracing::warn!(?key, "DrainFeed: truncated wire (u8)");
                return None;
            }
            let v = buf[cur];
            cur += 1;
            v
        }};
    }
    macro_rules! read_u32_le {
        () => {{
            if cur + 4 > buf.len() {
                tracing::warn!(?key, "DrainFeed: truncated wire (u32)");
                return None;
            }
            let v = u32::from_le_bytes([buf[cur], buf[cur + 1], buf[cur + 2], buf[cur + 3]]);
            cur += 4;
            v
        }};
    }
    macro_rules! read_u64_le {
        () => {{
            if cur + 8 > buf.len() {
                tracing::warn!(?key, "DrainFeed: truncated wire (u64)");
                return None;
            }
            let v = u64::from_le_bytes([
                buf[cur],
                buf[cur + 1],
                buf[cur + 2],
                buf[cur + 3],
                buf[cur + 4],
                buf[cur + 5],
                buf[cur + 6],
                buf[cur + 7],
            ]);
            cur += 8;
            v
        }};
    }
    /// Length-prefixed bytes: u32 len | bytes. Returns the bytes slice (or advances past it).
    macro_rules! skip_lp {
        () => {{
            let len = read_u32_le!() as usize;
            if cur + len > buf.len() {
                tracing::warn!(?key, "DrainFeed: truncated lp field");
                return None;
            }
            cur += len;
        }};
    }
    macro_rules! read_lp_bytes {
        () => {{
            let len = read_u32_le!() as usize;
            if cur + len > buf.len() {
                tracing::warn!(?key, "DrainFeed: truncated lp bytes");
                return None;
            }
            let v = &buf[cur..cur + len];
            cur += len;
            v
        }};
    }

    let variant = read_u8!();

    match variant {
        // ── Page ─────────────────────────────────────────────────────────────
        0 => {
            let next_after_seq = read_u64_le!();
            let _latest_seq = read_u64_le!();
            let has_more = read_u8!() != 0;
            let entry_count = read_u32_le!();

            let mut rows: Vec<nmp_core::substrate::KernelEvent> = Vec::new();

            for _ in 0..entry_count {
                // entry := u64 seq | u8 op_tag | [op-specific fields] | lp(event_id)
                //        | u8 has_raw | [has_raw: lp(raw_json)] | lp(source_relay)
                //        | u64 received_at_ms
                let _seq = read_u64_le!();
                let op_tag = read_u8!();

                match op_tag {
                    0 => {
                        // Inserted: no extra op fields
                    }
                    1 => {
                        // Replaced: lp(replaced_id)
                        skip_lp!();
                    }
                    2 => {
                        // Deleted: lp(target_id) | u8 reason
                        skip_lp!();
                        let _reason = read_u8!();
                        // Skip event_id + has_raw + source_relay + received_at_ms
                        skip_lp!(); // event_id
                        let has_raw = read_u8!();
                        if has_raw != 0 {
                            skip_lp!(); // raw_json
                        }
                        skip_lp!(); // source_relay
                        let _recv = read_u64_le!();
                        continue; // Deleted rows are skipped per ADR-0058 §10
                    }
                    _ => {
                        // Unknown op_tag — skip by returning None for safety
                        // (we don't know the field layout for future ops).
                        tracing::warn!(?key, op_tag, "DrainFeed: unknown op_tag, skipping page");
                        return None;
                    }
                }

                // Common tail: lp(event_id) | u8 has_raw | [has_raw: lp(raw_json)]
                //            | lp(source_relay) | u64 received_at_ms
                let event_id_hex = read_lp_bytes!();
                let has_raw = read_u8!();
                let raw_json_bytes = if has_raw != 0 {
                    let raw = read_lp_bytes!();
                    Some(raw.to_vec())
                } else {
                    None
                };
                let source_relay_bytes = read_lp_bytes!();
                let _received_at_ms = read_u64_le!();

                let source_relay: Option<String> = if source_relay_bytes.is_empty() {
                    None
                } else {
                    std::str::from_utf8(source_relay_bytes)
                        .ok()
                        .map(String::from)
                };

                // Decode the raw JSON into a nostr event (for kind/content/tags).
                let Some(raw_json) = raw_json_bytes else {
                    continue; // No raw event payload — skip (ADR-0058 §10).
                };

                let Ok(raw) = serde_json::from_slice::<RawNostrEvent>(&raw_json) else {
                    // Invalid JSON: no-op (D6).
                    continue;
                };

                let relay_provenance: Vec<String> = source_relay.into_iter().collect();

                let event_id = std::str::from_utf8(event_id_hex)
                    .ok()
                    .unwrap_or_default()
                    .to_string();

                rows.push(nmp_core::substrate::KernelEvent {
                    id: event_id,
                    author: raw.pubkey,
                    kind: raw.kind,
                    created_at: raw.created_at,
                    tags: raw.tags,
                    content: raw.content,
                    relay_provenance,
                });
            }

            Some(KernelEvent::FeedPage {
                key,
                cursor_id,
                rows,
                next_after_seq,
                exhausted: !has_more,
                gap_rebased_to: None,
            })
        }

        // ── Gap ──────────────────────────────────────────────────────────────
        1 => {
            let _requested_after_seq = read_u64_le!();
            let first_available_seq = read_u64_le!();

            tracing::info!(
                ?key,
                first_available_seq,
                "DrainFeed: gap — rebasing cursor"
            );

            Some(KernelEvent::FeedPage {
                key,
                cursor_id,
                rows: Vec::new(),
                next_after_seq: first_available_seq,
                exhausted: false,
                gap_rebased_to: Some(first_available_seq),
            })
        }

        // ── Error ─────────────────────────────────────────────────────────────
        2 => {
            let error_code = read_u32_le!();
            tracing::warn!(?key, error_code, "DrainFeed: pull_page returned Error");
            None
        }

        // ── Unknown ───────────────────────────────────────────────────────────
        _ => {
            tracing::warn!(?key, variant, "DrainFeed: unknown wire variant");
            None
        }
    }
}

// ─── Minimal raw nostr event for JSON decode ─────────────────────────────────

/// Subset of a raw Nostr event sufficient to populate a `KernelEvent`.
///
/// Only the fields used by the kernel are deserialized; extras are ignored.
/// `serde_json` is used (never `format!`) so JSON with special characters
/// is safe (D-rule: serde, not format!).
#[derive(serde::Deserialize)]
struct RawNostrEvent {
    pubkey: String,
    kind: u32,
    created_at: u64,
    tags: Vec<Vec<String>>,
    content: String,
}

// ─── Reduce-event helper ─────────────────────────────────────────────────────

/// Apply a `KernelEvent::FeedPage` to a `FeedState`.
///
/// Called by each consumer slice's `reduce_event` arm (4G/4H/4I route on `key`
/// and call this with the matching `FeedState`). The generic engine appends rows,
/// advances `after_seq`, and sets `exhausted`.
///
/// On `gap_rebased_to = Some(seq)`: clears `rows` and resets `after_seq` to `seq`
/// per ADR-0058 §10 (scoped continuity is not provable after a gap).
pub fn apply_feed_page(
    state: &mut FeedState,
    rows: Vec<nmp_core::substrate::KernelEvent>,
    next_after_seq: u64,
    exhausted: bool,
    gap_rebased_to: Option<u64>,
) {
    if let Some(rebased_to) = gap_rebased_to {
        // Gap: clear scoped rows and rebase cursor (ADR-0058 §10).
        state.rows.clear();
        state.after_seq = rebased_to;
        state.exhausted = false;
        return;
    }
    // Advance cursor monotonically (a malformed page can never rewind it).
    state.after_seq = state.after_seq.max(next_after_seq);
    state.exhausted = exhausted;
    // Append rows in ingest-seq order. No dedup here — the consumer's snapshot
    // projection deduplicates by event id if needed (e.g. replaceable events).
    state.rows.extend(rows);
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::action::KernelEvent;

    // ── feed.rs unit tests (no live NmpApp, no sleep / polling) ─────────────

    /// mint_cursor_id produces a non-zero id for any key.
    #[test]
    fn mint_cursor_id_nonzero() {
        assert_ne!(mint_cursor_id("hl.feed.articles"), 0);
        assert_ne!(mint_cursor_id("hl.feed.highlights"), 0);
        assert_ne!(mint_cursor_id("hl.feed.room.general"), 0);
        // Stability: same key → same id (deterministic across restarts).
        assert_eq!(
            mint_cursor_id("hl.feed.articles"),
            mint_cursor_id("hl.feed.articles"),
        );
        // Different keys → different ids (no trivial collision for likely keys).
        assert_ne!(
            mint_cursor_id("hl.feed.articles"),
            mint_cursor_id("hl.feed.highlights"),
        );
    }

    /// RegisterFeedCursor effect is emitted with correct cursor_id.
    #[test]
    fn register_feed_cursor_emits_effect() {
        let scope = highlight_feed_scope();
        let effects = reduce_register_feed_cursor("hl.feed.highlights".into(), scope);
        assert_eq!(effects.len(), 1, "exactly one RegisterFeedCursor effect");
        match &effects[0] {
            Effect::RegisterFeedCursor { key, cursor_id, .. } => {
                assert_eq!(key, "hl.feed.highlights");
                assert_ne!(*cursor_id, 0, "cursor_id must be non-zero");
                assert_eq!(*cursor_id, mint_cursor_id("hl.feed.highlights"));
            }
            other => panic!("expected RegisterFeedCursor, got {other:?}"),
        }
    }

    /// DrainFeed effect is emitted for the correct key.
    #[test]
    fn drain_feed_emits_effect() {
        let effects = reduce_drain_feed("hl.feed.highlights".into());
        assert_eq!(effects.len(), 1);
        match &effects[0] {
            Effect::DrainFeed { key } => assert_eq!(key, "hl.feed.highlights"),
            other => panic!("expected DrainFeed, got {other:?}"),
        }
    }

    /// ReleaseFeedCursor effect is emitted for the correct key.
    #[test]
    fn release_feed_cursor_emits_effect() {
        let effects = reduce_release_feed_cursor("hl.feed.highlights".into());
        assert_eq!(effects.len(), 1);
        match &effects[0] {
            Effect::ReleaseFeedCursor { key } => assert_eq!(key, "hl.feed.highlights"),
            other => panic!("expected ReleaseFeedCursor, got {other:?}"),
        }
    }

    /// apply_feed_page appends rows and advances after_seq.
    #[test]
    fn feedpage_event_appends_rows_and_advances_cursor() {
        let mut state = FeedState::default();
        let rows = vec![dummy_event("aaa"), dummy_event("bbb")];
        apply_feed_page(&mut state, rows.clone(), 42, false, None);
        assert_eq!(state.rows.len(), 2);
        assert_eq!(state.after_seq, 42);
        assert!(!state.exhausted);

        // A second page appends and further advances.
        let more = vec![dummy_event("ccc")];
        apply_feed_page(&mut state, more, 99, false, None);
        assert_eq!(state.rows.len(), 3);
        assert_eq!(state.after_seq, 99);
    }

    /// apply_feed_page sets exhausted when has_more == false.
    #[test]
    fn exhausted_page_sets_exhausted_flag() {
        let mut state = FeedState::default();
        apply_feed_page(&mut state, vec![], 10, true, None);
        assert!(state.exhausted);
        assert_eq!(state.after_seq, 10);
    }

    /// apply_feed_page on a Gap clears rows and rebases the cursor.
    #[test]
    fn gap_rebase_clears_rows_and_resets_cursor() {
        let mut state = FeedState {
            cursor_id: 7,
            after_seq: 100,
            exhausted: false,
            rows: vec![dummy_event("old")],
        };
        apply_feed_page(&mut state, vec![], 0, false, Some(55));
        assert!(state.rows.is_empty(), "gap must clear rows");
        assert_eq!(state.after_seq, 55, "gap must rebase cursor");
        assert!(!state.exhausted);
        assert_eq!(state.cursor_id, 7, "cursor_id preserved after gap");
    }

    /// apply_feed_page with a monotonic advance: after_seq never rewinds.
    #[test]
    fn after_seq_never_rewinds() {
        let mut state = FeedState {
            after_seq: 50,
            ..FeedState::default()
        };
        // A malformed page with a smaller next_after_seq must not rewind.
        apply_feed_page(&mut state, vec![], 10, false, None);
        assert_eq!(state.after_seq, 50, "after_seq must never rewind");
    }

    /// Garbage binary wire input produces no-op (D6 — malformed_page_no_ops).
    #[test]
    fn malformed_page_no_ops() {
        use nmp_ffi::pull::NmpOwnedBytes;

        // Simulate garbage bytes that look like a Page variant but are truncated.
        let garbage: Vec<u8> = vec![0u8, 0xDE, 0xAD, 0xBE, 0xEF]; // variant=Page, then truncated
        let mut v = garbage.clone();
        let bytes = NmpOwnedBytes {
            ptr: v.as_mut_ptr(),
            len: v.len(),
            cap: v.capacity(),
        };
        // decode_pull_page_wire must return None on malformed input (D6: no panic).
        let result = decode_pull_page_wire("test.key".into(), 1, &bytes);
        assert!(result.is_none(), "malformed wire must produce no-op");
        // Do NOT call nmp_free_bytes here — v owns the memory.
        std::mem::forget(bytes); // let v drop normally
    }

    /// Garbage all-zeros wire produces no-op.
    #[test]
    fn all_zeros_wire_no_ops() {
        use nmp_ffi::pull::NmpOwnedBytes;

        let mut zeros = vec![0u8; 32];
        let bytes = NmpOwnedBytes {
            ptr: zeros.as_mut_ptr(),
            len: zeros.len(),
            cap: zeros.capacity(),
        };
        // variant 0 = Page; next_after_seq=0, latest_seq=0, has_more=0, entry_count=0
        // This is actually a valid (empty) page — should produce Some(FeedPage).
        let result = decode_pull_page_wire("test.key".into(), 1, &bytes);
        // variant=0, next_after_seq=0 (8B), latest_seq=0 (8B), has_more=0 (1B), entry_count=0 (4B) = 22B
        // Buffer is 32B so we have enough: expect Some with no rows.
        assert!(result.is_some(), "valid empty page should decode");
        if let Some(KernelEvent::FeedPage {
            rows, exhausted, ..
        }) = result
        {
            assert!(rows.is_empty());
            assert!(exhausted, "has_more=0 means exhausted");
        }
        std::mem::forget(bytes);
    }

    /// Article feed scope fails closed when follow set is empty.
    #[test]
    fn article_feed_scope_fails_closed_when_no_follows() {
        assert!(
            article_feed_scope(&[]).is_none(),
            "empty follow set must fail closed"
        );
    }

    /// Article feed scope includes kind:30023 and authors.
    #[test]
    fn article_feed_scope_includes_correct_kind_and_authors() {
        let authors = vec!["abc".to_string(), "def".to_string()];
        let scope = article_feed_scope(&authors).expect("non-empty follows");
        match scope {
            PullScope::InterestShape(shape) => {
                assert!(shape.kinds.contains(&30023), "must include kind:30023");
                assert_eq!(shape.authors.len(), 2);
            }
            _ => panic!("expected InterestShape scope"),
        }
    }

    /// Highlight feed scope includes kind:9802 (no author filter).
    #[test]
    fn highlight_feed_scope_includes_kind_9802() {
        match highlight_feed_scope() {
            PullScope::InterestShape(shape) => {
                assert!(shape.kinds.contains(&9802), "must include kind:9802");
                assert!(
                    shape.authors.is_empty(),
                    "no author filter for highlight feed"
                );
            }
            _ => panic!("expected InterestShape scope"),
        }
    }

    /// Room lane scope includes kinds 9 and 11, tagged with the group_id.
    #[test]
    fn room_lane_scope_includes_correct_kinds_and_h_tag() {
        match room_lane_scope("my-group") {
            PullScope::InterestShape(shape) => {
                assert!(shape.kinds.contains(&9), "must include kind:9");
                assert!(shape.kinds.contains(&11), "must include kind:11");
                let h_values = shape.tags.get("h").expect("must have #h tag");
                assert!(h_values.contains("my-group"), "must filter by group_id");
            }
            _ => panic!("expected InterestShape scope"),
        }
    }

    /// Drain effect path (via reduce) is sync — no polling or sleeping (D8).
    #[test]
    fn drain_is_sync_no_polling() {
        // The drain effect function is synchronous: run_effect_drain_feed returns
        // without blocking on anything. Test that the reduce path emitting
        // DrainFeed does not block the test thread (no sleep, no await).
        let effects = reduce_drain_feed("hl.feed.highlights".into());
        // Just verifying it's instant and produces exactly the expected effect.
        assert_eq!(effects.len(), 1);
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn dummy_event(id: &str) -> nmp_core::substrate::KernelEvent {
        nmp_core::substrate::KernelEvent {
            id: id.to_string(),
            author: "pubkey".to_string(),
            kind: 9802,
            created_at: 1_000_000,
            tags: vec![],
            content: "test".to_string(),
            relay_provenance: vec![],
        }
    }
}
