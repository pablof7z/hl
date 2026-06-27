//! Chat domain — NIP-29 kind:9 group chat projection + write actions (Phase 7).
//!
//! ## Responsibilities
//!
//! * **READ** — register a `ChatObserver` wrapper around `GroupChatProjection`
//!   (nmp-nip29) per open room as a `KernelEventObserver`. On each kind:9 event
//!   (kind:11 is filtered out here — see §D6 note) the observer:
//!   (a) delegates ingest to the underlying `GroupChatProjection`,
//!   (b) snapshots the group's message list,
//!   (c) recovers `reply_to_event_id` from raw event tags (prefer `["e",id,"","reply"]`),
//!   (d) sends `KernelEvent::ChatRoomUpdated { group_id, messages }` into the actor channel.
//!
//!   This is the Family-B integration pattern (observer→actor-channel path), the
//!   same as `comments.rs` and `reactions.rs`. The underlying `GroupChatProjection`
//!   accepts both kind:9 and kind:11 with matching `h` tag; the `ChatObserver`
//!   adds a kind:9-only gate before forwarding updates.
//!
//! * **WRITE** — `hl.chat.post` envelope → `reduce_action_post_chat` →
//!   `Effect::DispatchChatPost { json }` → `run_effect_dispatch_chat_post` calls
//!   `nmp_app_dispatch_action("nmp.nip29.post_chat_message", json)` fire-and-forget.
//!   Kernel is the sole kind:9 writer.
//!
//! ## Wire registration
//!
//! `register_chat_projection(nmp_ref, group_id, host_relay_url, tx)` is called
//! when `hl.chat.open` is dispatched. Rust derives `host_relay_url` from the
//! joined-community projection, creates a `GroupChatProjection` scoped to
//! `GroupId { host_relay_url, local_id: group_id }`, and wraps it in a
//! `ChatObserver`. Per-room wiring; cleared on close and logout.
//!
//! ## threading
//!
//! `ChatObserver::on_kernel_event` is called from the NMP event-dispatch thread.
//! It:
//! 1. Checks `event.kind == KIND_CHAT_MESSAGE` (kind:9 only gate).
//! 2. Delegates ingest to `self.projection.on_kernel_event(event)`.
//! 3. Calls `self.projection.snapshot()` — one brief Mutex acquire.
//! 4. Recovers `reply_to_event_id` from the raw event tags.
//! 5. Sends `Cmd::Event(KernelEvent::ChatRoomUpdated{…})` — non-blocking.
//!
//! D8 compliant: no blocking awaits. D6: kind != 9 → silent no-op.
//!
//! ## D-rules satisfied
//!
//! * D1 — `ChatMessageRawRow` carries raw protocol fields only (no formatted
//!   timestamps, no byline strings, no `show_header` logic). Swift owns all
//!   display formatting; `show_header` and reply preview are computed in the
//!   snapshot projection from raw data.
//! * D6 — kind:11 events (which `GroupChatProjection` would accept) are silently
//!   rejected at the observer level. Malformed post payloads (empty content,
//!   empty group) → no effects.
//! * Non-Negotiable #3 — `reduce_action_post_chat` returns `Vec<Effect>` (never
//!   `Result`); fire-and-forget.

use std::sync::{Arc, Mutex};

use nmp_core::dispatch_envelope::{encode_dispatch_envelope, DISPATCH_ENVELOPE_SCHEMA_VERSION};
use nmp_core::substrate::{ActionPayload, ObservedProjection, ObservedProjectionRegistrar};
use nmp_core::ObservedProjectionSink;
use nmp_ffi::{nmp_app_dispatch_action_bytes, nmp_free_string};
use nmp_nip29::{action::PublishGroupEventInput, GroupId};
use nmp_planner::InterestShape;
use tokio::sync::mpsc;

use crate::kernel::action::KernelEvent;
use crate::kernel::actor::Cmd;
use crate::kernel::app::AppState;
use crate::kernel::effect::Effect;
use crate::kernel::snapshot::{
    ChatMessageRawRow, ChatMessageRow, ChatReplyPreview, RoomChatSnapshot,
};

// Kind constants matching nmp-nip29's kinds module
const KIND_CHAT_MESSAGE: u32 = 9;
const CHAT_PAGE_SIZE: usize = 50;
const CHAT_MAX_PAGES: u32 = 20;
/// Maximum messages exposed in a bounded snapshot (page_count * 50, hard cap 1000).
pub(crate) const CHAT_MAX_MESSAGES: usize = CHAT_MAX_PAGES as usize * CHAT_PAGE_SIZE;
/// Gap threshold for `show_header` grouping (300 seconds).
const SHOW_HEADER_GAP_SECS: u64 = 300;

// ─── Per-room state ──────────────────────────────────────────────────────────

/// Per-room authoritative message buffer (newest-first, bounded).
///
/// Keyed by NIP-29 local `group_id` in `AppState::chat_rooms`.
/// Cleared on `hl.chat.close`, `Logout`, and `IdentityChanged(None)`.
#[derive(Debug, Clone, Default)]
pub struct ChatRoomState {
    /// Newest-first authoritative bounded message set (from `ChatObserver`).
    pub messages: Vec<ChatMessageRawRow>,
    /// Current page count (incremented by `hl.chat.load_more`; capped at 20).
    pub page_count: u32,
    /// Monotonic revision bumped on every `ChatRoomUpdated` for activity detection.
    pub activity_revision: u64,
}

// ─── READ side: ObservedProjectionSink wrapper ───────────────────────────────

/// Observer wrapper that ingests NMP `KernelEvent`s (raw Nostr events) into
/// the `GroupChatProjection` and sends `KernelEvent::ChatRoomUpdated` back into
/// the actor channel for kind:9 events in this group.
///
/// This is the Family-B integration pattern (same as `CommentObserver` in
/// comments.rs and `ReactionObserver` in reactions.rs). On each kind:9 event
/// the observer:
/// 1. Checks `event.kind == KIND_CHAT_MESSAGE` (kind:9 filter — GroupChatProjection
///    also accepts kind:11 but the HL chat domain is kind:9 only per design §2).
/// 2. Delegates ingest to the `GroupChatProjection` (which also gates on group h-tag).
/// 3. Snapshots the updated message list from `projection.snapshot()`.
/// 4. Recovers `reply_to_event_id` from the raw event tags.
/// 5. Sends `KernelEvent::ChatRoomUpdated` into the actor channel.
///
/// D6: kind:11 events are silently dropped at step 1.
struct ChatObserver {
    group_id: String,
    tx: mpsc::UnboundedSender<Cmd>,
    messages: Mutex<Vec<ChatMessageRawRow>>,
    /// Accumulated `event_id → reply_to_event_id` recovered from raw tags.
    /// `GroupChatProjection.snapshot()` does not carry `reply_to`, and a single
    /// `on_kernel_event` only has the raw tags for the *triggering* event — so we
    /// remember each event's recovered reply target here and look every message's
    /// reply_to up by id when rebuilding rows. Without this, every row except the
    /// newest loses its reply preview on each resnapshot.
    reply_index: std::sync::Mutex<std::collections::HashMap<String, String>>,
}

impl ObservedProjectionSink for ChatObserver {
    fn on_kernel_event(&self, event: &nmp_core::substrate::KernelEvent) {
        // D6: only kind:9 chat messages enter the HL chat read model.
        // GroupChatProjection accepts kind:9 and kind:11; we gate here.
        if event.kind != KIND_CHAT_MESSAGE {
            return;
        }

        if h_tag_value(&event.tags) != Some(self.group_id.as_str()) {
            return;
        }

        // Recover reply_to_event_id from the raw event tags and remember it,
        // keyed by this event's id. Per design: prefer ["e", id, "", "reply"]
        // marker; fallback to first "e" tag.
        if let Some(reply_to) = recover_reply_to(&event.tags) {
            if let Ok(mut idx) = self.reply_index.lock() {
                idx.insert(event.id.clone(), reply_to);
            }
        }

        let messages: Vec<ChatMessageRawRow> = {
            let idx = self.reply_index.lock().ok();
            let reply = idx.as_ref().and_then(|i| i.get(&event.id).cloned());
            let Ok(mut messages) = self.messages.lock() else {
                return;
            };
            messages.push(ChatMessageRawRow {
                event_id: event.id.clone(),
                author_pubkey: event.author.clone(),
                content: event.content.clone(),
                created_at: event.created_at,
                reply_to_event_id: reply,
            });
            messages.sort_by(|a, b| {
                b.created_at
                    .cmp(&a.created_at)
                    .then_with(|| b.event_id.cmp(&a.event_id))
            });
            messages.dedup_by(|a, b| a.event_id == b.event_id);
            messages.truncate(CHAT_MAX_MESSAGES);
            messages.clone()
        };

        let _ = self.tx.send(Cmd::Event(KernelEvent::ChatRoomUpdated {
            group_id: self.group_id.clone(),
            messages,
        }));
    }
}

fn h_tag_value(tags: &[Vec<String>]) -> Option<&str> {
    tags.iter()
        .find(|t| t.len() >= 2 && t[0] == "h" && !t[1].is_empty())
        .map(|t| t[1].as_str())
}

/// Recover `reply_to_event_id` from raw event tags.
///
/// Prefer the canonical NIP-29 marker form: `["e", id, "", "reply"]`.
/// Fallback: the first `["e", id, ...]` tag (any length ≥ 2).
///
/// Returns `None` when no `e` tag is present (not a reply).
pub(crate) fn recover_reply_to(tags: &[Vec<String>]) -> Option<String> {
    // First pass: look for the ["e", id, "", "reply"] marker form.
    for tag in tags {
        if tag.len() >= 4 && tag[0] == "e" && tag[3] == "reply" && !tag[1].is_empty() {
            return Some(tag[1].clone());
        }
    }
    // Fallback: first "e" tag with a non-empty value.
    for tag in tags {
        if tag.len() >= 2 && tag[0] == "e" && !tag[1].is_empty() {
            return Some(tag[1].clone());
        }
    }
    None
}

// ─── State event handler (called from reduce_event in actor.rs) ──────────────

/// Apply a `KernelEvent::ChatRoomUpdated` to `AppState::chat_rooms`.
///
/// Upserts the message buffer for `group_id`. Increments `activity_revision`
/// so the snapshot can signal new activity (e.g. for the unread pill).
/// Preserves `page_count` from the existing state.
///
/// D1: stores raw `ChatMessageRawRow` fields only — no formatted strings.
pub(crate) fn reduce_event_chat_room_updated(
    state: &mut AppState,
    group_id: String,
    messages: Vec<ChatMessageRawRow>,
) -> Vec<Effect> {
    // D6: only update a room that is currently OPEN. After `hl.chat.close` or
    // logout the room entry is removed; a stray event from an observer that has
    // not yet been released must NOT recreate the room (cross-session leak).
    let Some(entry) = state.chat_rooms.get_mut(&group_id) else {
        return vec![];
    };
    entry.activity_revision = entry.activity_revision.saturating_add(1);
    entry.messages = messages;
    vec![]
}

// ─── Action reducers ─────────────────────────────────────────────────────────

fn host_relay_for_group(state: &AppState, group_id: &str) -> Option<String> {
    let group_id = group_id.trim();
    if group_id.is_empty() {
        return None;
    }
    state
        .communities
        .iter()
        .find(|c| c.group_id == group_id)
        .map(|c| c.host_relay_url.trim().to_string())
        .filter(|relay| !relay.is_empty())
}

/// Handle `hl.chat.open` — emit `Effect::WireGroupChat { group_id, host_relay_url }`.
///
/// Inserts an empty `ChatRoomState` for the group if not already present.
/// Wiring is deferred to the effect runner (which creates the `ChatObserver`).
pub(crate) fn reduce_action_open_chat(state: &mut AppState, group_id: String) -> Vec<Effect> {
    let group_id = group_id.trim().to_string();
    let Some(host_relay_url) = host_relay_for_group(state, &group_id) else {
        return vec![];
    };
    state.chat_rooms.entry(group_id.clone()).or_default();
    vec![Effect::WireGroupChat {
        group_id,
        host_relay_url,
    }]
}

/// Handle `hl.chat.close` — clear the room buffer from `AppState::chat_rooms`
/// and emit `Effect::ReleaseChatRoom { group_id }` for any future async cleanup
/// (e.g. observer slot release when nmp supports it).
///
/// State mutation happens here (in the reducer) because `run_effect` does not
/// have access to `AppState`. `ReleaseChatRoom` is reserved for future async
/// observer de-registration.
pub(crate) fn reduce_action_close_chat(state: &mut AppState, group_id: String) -> Vec<Effect> {
    if group_id.trim().is_empty() {
        return vec![];
    }
    // Clear the hl-side buffer immediately in the reducer.
    state.chat_rooms.remove(&group_id);
    vec![Effect::ReleaseChatRoom { group_id }]
}

/// Handle `hl.chat.load_more` — increment `page_count` if `has_more` and below cap.
///
/// D6: no-op if the group is not open or is already at max pages.
pub(crate) fn reduce_action_load_more_chat(state: &mut AppState, group_id: String) -> Vec<Effect> {
    let entry = state.chat_rooms.entry(group_id).or_default();
    let has_more = entry.messages.len() > (entry.page_count as usize * CHAT_PAGE_SIZE);
    if has_more && entry.page_count < CHAT_MAX_PAGES {
        entry.page_count += 1;
    }
    vec![]
}

/// Handle `hl.chat.post` — trim content; no-op if empty or no active session;
/// emit `Effect::DispatchChatPost { json }` via `nmp.nip29.post_chat_message`.
///
/// D6: empty content → no effects. No active session → no effects.
/// Payload shape matches `PostChatMessageInput { group, content,
/// previous_event_id_prefixes, reply_to_event_id }`.
pub(crate) fn reduce_action_post_chat(state: &AppState, payload: PostChatPayload) -> Vec<Effect> {
    let content = payload.content.trim().to_string();
    let group_id = payload.group_id.trim().to_string();
    let Some(host_relay_url) = host_relay_for_group(state, &group_id) else {
        return vec![];
    };
    if content.is_empty() {
        return vec![];
    }

    // Build the generic NIP-29 publish_group_event wire shape.
    // Use serde_json::json! (never format!) for safe serialisation.
    let mut json_map = serde_json::json!({
        "group": {
            "host_relay_url": host_relay_url,
            "local_id": group_id,
        },
        "kind": KIND_CHAT_MESSAGE,
        "content": content,
        "tags": [],
    });

    if let Some(reply) = payload.reply_to_event_id {
        if !reply.trim().is_empty() {
            json_map["tags"] = serde_json::Value::Array(vec![serde_json::Value::Array(vec![
                serde_json::Value::String("e".to_string()),
                serde_json::Value::String(reply),
                serde_json::Value::String(String::new()),
                serde_json::Value::String("reply".to_string()),
            ])]);
        }
    }

    let json = match serde_json::to_string(&json_map) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "chat::reduce_action_post_chat: serde error — no effect emitted");
            return vec![];
        }
    };

    vec![Effect::DispatchChatPost { json }]
}

/// Handle `hl.chat.mark_seen` — optional; native may also keep pill state locally.
///
/// Currently a no-op at the kernel level (pill state is Swift-local, D1).
/// Included for completeness and future extension.
pub(crate) fn reduce_action_mark_seen(
    _state: &mut AppState,
    _group_id: String,
    _visible_event_ids: Vec<String>,
) -> Vec<Effect> {
    vec![]
}

// ─── Effect runners ──────────────────────────────────────────────────────────

/// Execute `Effect::WireGroupChat` — register a `ChatObserver` wrapping a fresh
/// `GroupChatProjection` scoped to `group_id` as a `KernelEventObserver` against
/// `nmp_ref`.
///
/// Creates a NEW `GroupChatProjection` per group per session. Fire-and-forget (D6):
/// if observer registration fails (slot full), logged as a warning; the room buffer
/// will not update.
///
/// No-op if `nmp` is `None` (test mode — tests inject `ChatRoomUpdated` directly).
pub(crate) fn run_effect_wire_group_chat(
    group_id: String,
    host_relay_url: String,
    nmp: Option<&crate::kernel::actor::NmpHandle>,
    tx: mpsc::UnboundedSender<Cmd>,
) {
    let Some(handle) = nmp else { return };

    let _gid = GroupId::new(host_relay_url, group_id.clone());
    let consumer_id = format!("hl.chat.{group_id}");
    let mut shape = InterestShape::default();
    shape.kinds.insert(KIND_CHAT_MESSAGE);
    shape
        .tags
        .entry("h".to_string())
        .or_default()
        .insert(group_id.clone());
    let observer = Arc::new(ChatObserver {
        group_id,
        tx,
        messages: Mutex::new(Vec::new()),
        reply_index: std::sync::Mutex::new(std::collections::HashMap::new()),
    });

    // SAFETY: ptr is a valid non-null NmpApp pointer kept alive by NmpHandle
    // for the full actor lifetime (outlives this call).
    let nmp_ref: &nmp_ffi::NmpApp = unsafe { handle.ptr.as_ref() };
    let observer_id = nmp_ref.open_observed_projection(ObservedProjection::from_shape(
        observer as Arc<dyn ObservedProjectionSink>,
        consumer_id,
        1,
        shape,
        CHAT_MAX_MESSAGES,
    ));
    if observer_id.0 == 0 {
        tracing::warn!("chat::run_effect_wire_group_chat: event-observer registration failed (D6)");
    }
}

/// Execute `Effect::DispatchChatPost` — calls `nmp_app_dispatch_action`
/// with namespace `"nmp.nip29.post_chat_message"` and the serialised JSON payload.
///
/// Fire-and-forget (D6, Non-Negotiable #3): the returned correlation_id JSON
/// string is freed and discarded.
///
/// No-op if `nmp` is `None` (test mode — tests drive the authoritative read
/// path via injected `KernelEvent::ChatRoomUpdated`).
pub(crate) fn run_effect_dispatch_chat_post(
    json: String,
    nmp: Option<&crate::kernel::actor::NmpHandle>,
) {
    let Some(handle) = nmp else { return };

    let action = match serde_json::from_str::<PublishGroupEventInput>(&json) {
        Ok(a) => a,
        Err(e) => {
            tracing::warn!(error = %e, "chat: failed to deserialise PublishGroupEventInput");
            return;
        }
    };
    let payload_bytes = action.encode();
    let correlation_id = uuid::Uuid::new_v4().to_string();
    let envelope = encode_dispatch_envelope(
        &correlation_id,
        "nmp.nip29.publish_group_event",
        DISPATCH_ENVELOPE_SCHEMA_VERSION,
        &payload_bytes,
    );

    let result_ptr =
        nmp_app_dispatch_action_bytes(handle.ptr.as_ptr(), envelope.as_ptr(), envelope.len());

    if !result_ptr.is_null() {
        nmp_free_string(result_ptr);
    }
}

// ─── Clear on identity loss ──────────────────────────────────────────────────

/// Clear all chat room buffers from `AppState` on logout or identity loss.
///
/// Chat content is not per-account, but the view buffer is an open-view working
/// set. Clear on `Logout` and `IdentityChanged(None)` to avoid stale cross-session
/// UI. Re-wiring happens when `hl.chat.open` is dispatched for the new session.
pub(crate) fn clear_on_identity_lost(state: &mut AppState) {
    state.chat_rooms.clear();
}

// ─── Snapshot computation ────────────────────────────────────────────────────

/// Compute a `RoomChatSnapshot` for the given `group_id`.
///
/// The bounded visible window is `page_count * 50`, capped at `CHAT_MAX_MESSAGES`.
/// Raw rows are newest-first in the authoritative buffer; the snapshot rows are
/// projected oldest-first for the visible window (chat scrolls downward).
///
/// `show_header` is `true` for the first row, author changes, or a gap greater
/// than 300 seconds.
///
/// `reply_to` is resolved only when the replied-to event is inside the bounded
/// visible window.
///
/// D1: no formatted strings, no `is_from_me`, no display names.
/// D6: returns an empty snapshot if the group is not in `chat_rooms` — never panics.
pub(crate) fn compute_room_chat_snapshot(state: &AppState, group_id: &str) -> RoomChatSnapshot {
    let room = match state.chat_rooms.get(group_id) {
        Some(r) => r,
        None => {
            return RoomChatSnapshot {
                group_id: group_id.to_string(),
                rows: vec![],
                has_more: false,
                page_count: 0,
                has_activity: false,
                activity_revision: 0,
            };
        }
    };

    let page_count = room.page_count.max(1); // start at page 1 when first opened
    let window = (page_count as usize * CHAT_PAGE_SIZE).min(CHAT_MAX_MESSAGES);
    let total = room.messages.len();

    // `room.messages` is newest-first (authoritative buffer).
    // Oldest-first slice for display: take the newest `window` items, reverse.
    let has_more = total > window;
    let visible_newest_first: Vec<&ChatMessageRawRow> = room.messages.iter().take(window).collect();
    // Now oldest-first for display
    let mut visible: Vec<&ChatMessageRawRow> = visible_newest_first;
    visible.reverse();

    // Build an index for reply preview lookup (event_id → index in visible).
    // Only events inside the bounded window qualify for reply previews.
    let preview_index: std::collections::HashMap<&str, &ChatMessageRawRow> =
        visible.iter().map(|r| (r.event_id.as_str(), *r)).collect();

    let mut rows: Vec<ChatMessageRow> = Vec::with_capacity(visible.len());
    let mut prev_author: Option<&str> = None;
    let mut prev_created_at: Option<u64> = None;

    for row in &visible {
        // show_header: first row, author change, or gap > 300s
        let show_header = match (prev_author, prev_created_at) {
            (None, _) => true, // first row
            (Some(pa), Some(pt)) => {
                pa != row.author_pubkey.as_str()
                    || row.created_at.saturating_sub(pt) > SHOW_HEADER_GAP_SECS
            }
            (Some(pa), None) => pa != row.author_pubkey.as_str(),
        };

        // Resolve reply preview only if the parent is in the bounded window.
        let reply_to = row.reply_to_event_id.as_deref().and_then(|parent_id| {
            preview_index.get(parent_id).map(|parent| ChatReplyPreview {
                event_id: parent.event_id.clone(),
                author_pubkey: parent.author_pubkey.clone(),
                content: parent.content.clone(),
                created_at: parent.created_at,
            })
        });

        rows.push(ChatMessageRow {
            event_id: row.event_id.clone(),
            author_pubkey: row.author_pubkey.clone(),
            content: row.content.clone(),
            created_at: row.created_at,
            reply_to_event_id: row.reply_to_event_id.clone(),
            reply_to,
            show_header,
        });

        prev_author = Some(row.author_pubkey.as_str());
        prev_created_at = Some(row.created_at);
    }

    RoomChatSnapshot {
        group_id: group_id.to_string(),
        rows,
        has_more,
        page_count,
        has_activity: room.activity_revision > 0,
        activity_revision: room.activity_revision,
    }
}

// ─── Payload structs ─────────────────────────────────────────────────────────

/// `hl.chat.open` envelope payload.
#[derive(Debug, serde::Deserialize)]
pub(crate) struct OpenChatPayload {
    pub group_id: String,
}

/// `hl.chat.close` envelope payload.
#[derive(Debug, serde::Deserialize)]
pub(crate) struct CloseChatPayload {
    pub group_id: String,
}

/// `hl.chat.load_more` envelope payload.
#[derive(Debug, serde::Deserialize)]
pub(crate) struct LoadMoreChatPayload {
    pub group_id: String,
}

/// `hl.chat.post` envelope payload.
#[derive(Debug, serde::Deserialize)]
pub(crate) struct PostChatPayload {
    pub group_id: String,
    pub content: String,
    pub reply_to_event_id: Option<String>,
}

/// `hl.chat.mark_seen` envelope payload.
#[derive(Debug, serde::Deserialize)]
pub(crate) struct MarkSeenChatPayload {
    pub group_id: String,
    #[serde(default)]
    pub visible_event_ids: Vec<String>,
}

// ─── Unit tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kernel::action::KernelEvent;
    use crate::kernel::actor::{reduce, Cmd};
    use crate::kernel::clock::{Clock, ManualClock};
    use crate::kernel::effect::Effect;

    fn make_state() -> AppState {
        AppState::default()
    }

    fn seed_community(state: &mut AppState, group_id: &str, host_relay_url: &str) {
        state
            .communities
            .push(crate::kernel::snapshot::CommunityRow {
                group_id: group_id.to_string(),
                host_relay_url: host_relay_url.to_string(),
                name: None,
                picture: None,
                about: None,
                member_count: 0,
                public: true,
                open: true,
                is_admin: false,
            });
    }

    fn step(state: &mut AppState, clock: &ManualClock, cmd: Cmd) -> Vec<Effect> {
        let now = clock.now_unix_seconds();
        reduce(state, cmd, now)
    }

    fn inject_chat_updated(
        state: &mut AppState,
        clock: &ManualClock,
        group_id: &str,
        messages: Vec<ChatMessageRawRow>,
    ) -> Vec<Effect> {
        // Realistic flow: the room must be OPEN before observer events land
        // (`reduce_event_chat_room_updated` no-ops on a closed/absent room — D6).
        state.chat_rooms.entry(group_id.to_string()).or_default();
        step(
            state,
            clock,
            Cmd::Event(KernelEvent::ChatRoomUpdated {
                group_id: group_id.to_string(),
                messages,
            }),
        )
    }

    fn make_raw_row(
        event_id: &str,
        author: &str,
        content: &str,
        created_at: u64,
    ) -> ChatMessageRawRow {
        ChatMessageRawRow {
            event_id: event_id.to_string(),
            author_pubkey: author.to_string(),
            content: content.to_string(),
            created_at,
            reply_to_event_id: None,
        }
    }

    // 7-C1: chat_consumes_group_chat_projection_kind9_only
    //
    // The ChatObserver must NOT forward kind:11 events; only kind:9 enters the
    // RoomChatSnapshot. This tests the observer-level kind gate.
    #[test]
    fn chat_consumes_group_chat_projection_kind9_only() {
        use tokio::sync::mpsc;

        let (tx, mut rx) = mpsc::unbounded_channel::<Cmd>();
        let group_id = "test-room".to_string();
        let observer = ChatObserver {
            group_id: group_id.clone(),
            tx,
            messages: Mutex::new(Vec::new()),
            reply_index: std::sync::Mutex::new(std::collections::HashMap::new()),
        };

        // kind:11 event with matching h tag — must be rejected by ChatObserver
        let kind11_event = nmp_core::substrate::KernelEvent {
            id: "aaaa000000000000000000000000000000000000000000000000000000000001".to_string(),
            author: "bbbb000000000000000000000000000000000000000000000000000000000002".to_string(),
            kind: 11, // kind:11 — must NOT enter chat snapshot
            created_at: 1_000_000,
            tags: vec![vec!["h".to_string(), group_id.clone()]],
            content: "discussion post".to_string(),
            relay_provenance: vec![],
        };
        observer.on_kernel_event(&kind11_event);
        assert!(
            rx.try_recv().is_err(),
            "kind:11 event must not send ChatRoomUpdated (kind:9 filter)"
        );

        // kind:9 event with matching h tag — must be accepted
        let kind9_event = nmp_core::substrate::KernelEvent {
            id: "cccc000000000000000000000000000000000000000000000000000000000003".to_string(),
            author: "bbbb000000000000000000000000000000000000000000000000000000000002".to_string(),
            kind: KIND_CHAT_MESSAGE,
            created_at: 1_000_001,
            tags: vec![vec!["h".to_string(), group_id]],
            content: "hello chat".to_string(),
            relay_provenance: vec![],
        };
        observer.on_kernel_event(&kind9_event);
        let cmd = rx
            .try_recv()
            .expect("kind:9 event must send ChatRoomUpdated");
        match cmd {
            Cmd::Event(KernelEvent::ChatRoomUpdated {
                group_id: gid,
                messages,
            }) => {
                assert_eq!(gid, "test-room", "group_id must match");
                assert_eq!(messages.len(), 1, "one kind:9 message");
                assert_eq!(messages[0].content, "hello chat");
            }
            _ => panic!("expected ChatRoomUpdated"),
        }
    }

    // 7-C2: chat_recovers_reply_to_from_raw
    //
    // `recover_reply_to` must prefer the ["e", id, "", "reply"] marker form and
    // fall back to the first "e" tag when no marker is present.
    #[test]
    fn chat_recovers_reply_to_from_raw() {
        // Marker form — preferred
        let tags_with_marker = vec![
            vec!["h".to_string(), "room".to_string()],
            vec![
                "e".to_string(),
                "reply_id_000000000000000000000000000000000000000000000000000000001".to_string(),
                "".to_string(),
                "reply".to_string(),
            ],
        ];
        let result = recover_reply_to(&tags_with_marker);
        assert_eq!(
            result,
            Some("reply_id_000000000000000000000000000000000000000000000000000000001".to_string()),
            "must extract reply id from marker form"
        );

        // Fallback form — first e tag
        let tags_fallback = vec![
            vec!["h".to_string(), "room".to_string()],
            vec![
                "e".to_string(),
                "fallback_id_00000000000000000000000000000000000000000000000000000001".to_string(),
            ],
        ];
        let result2 = recover_reply_to(&tags_fallback);
        assert_eq!(
            result2,
            Some(
                "fallback_id_00000000000000000000000000000000000000000000000000000001".to_string()
            ),
            "must extract reply id from fallback form"
        );

        // No e tag — no reply
        let tags_no_reply = vec![vec!["h".to_string(), "room".to_string()]];
        let result3 = recover_reply_to(&tags_no_reply);
        assert!(result3.is_none(), "no e tag → no reply_to");
    }

    // 7-C3: post_chat_dispatches_publish_group_event
    //
    // hl.chat.post must produce exactly one Effect::DispatchChatPost with a
    // serde-valid JSON payload containing group/kind/content/tags.
    #[test]
    fn post_chat_dispatches_publish_group_event() {
        let mut state = make_state();
        let clock = ManualClock::default();
        seed_community(&mut state, "test-room", "wss://relay.example.com");

        let payload = serde_json::json!({
            "group_id": "test-room",
            "content": "hello world",
        });
        let envelope = crate::kernel::action::AppActionEnvelope {
            namespace: "hl.chat.post".to_string(),
            json: serde_json::to_string(&payload).unwrap(),
        };

        let effects = step(&mut state, &clock, Cmd::ActionEnvelope(envelope));

        assert_eq!(effects.len(), 1, "must emit exactly one effect");
        match &effects[0] {
            Effect::DispatchChatPost { json } => {
                let parsed: serde_json::Value =
                    serde_json::from_str(json).expect("must be valid JSON");
                assert_eq!(parsed["group"]["local_id"].as_str().unwrap(), "test-room");
                assert_eq!(
                    parsed["group"]["host_relay_url"].as_str().unwrap(),
                    "wss://relay.example.com"
                );
                assert_eq!(parsed["kind"].as_u64().unwrap(), KIND_CHAT_MESSAGE as u64);
                assert_eq!(parsed["content"].as_str().unwrap(), "hello world");
                assert!(
                    parsed["tags"].as_array().unwrap().is_empty(),
                    "tags must be empty array"
                );
            }
            _ => panic!("expected DispatchChatPost"),
        }
    }

    #[test]
    fn open_chat_derives_host_relay_from_joined_community() {
        let mut state = make_state();
        seed_community(&mut state, "test-room", "wss://relay.example.com");

        let effects = reduce_action_open_chat(&mut state, "test-room".to_string());

        assert_eq!(effects.len(), 1, "known room must wire chat");
        match &effects[0] {
            Effect::WireGroupChat {
                group_id,
                host_relay_url,
            } => {
                assert_eq!(group_id, "test-room");
                assert_eq!(host_relay_url, "wss://relay.example.com");
            }
            _ => panic!("expected WireGroupChat"),
        }
        assert!(
            state.chat_rooms.contains_key("test-room"),
            "known room should create a chat buffer"
        );

        let effects = reduce_action_open_chat(&mut state, "unknown-room".to_string());
        assert!(
            effects.is_empty(),
            "unknown room must fail closed without wiring chat"
        );
        assert!(
            !state.chat_rooms.contains_key("unknown-room"),
            "unknown room must not create a chat buffer"
        );
    }

    // 7-C4: chat_snapshot_bounded_1000
    //
    // The snapshot must cap at 1000 messages (CHAT_MAX_MESSAGES) regardless of
    // how many raw rows are in the buffer.
    #[test]
    fn chat_snapshot_bounded_1000() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let group_id = "big-room";

        // Inject 1100 messages (newest-first, different created_at)
        let messages: Vec<ChatMessageRawRow> = (0u64..1100)
            .rev()
            .map(|i| ChatMessageRawRow {
                event_id: format!("{:0>64}", format!("{:x}", i)),
                author_pubkey: "aaaa000000000000000000000000000000000000000000000000000000000001"
                    .to_string(),
                content: format!("msg {i}"),
                created_at: 1_000_000 + i,
                reply_to_event_id: None,
            })
            .collect();

        inject_chat_updated(&mut state, &clock, group_id, messages);

        // Set page_count to 20 (max pages) so the window is 1000
        if let Some(r) = state.chat_rooms.get_mut(group_id) {
            r.page_count = 20;
        }

        let snapshot = compute_room_chat_snapshot(&state, group_id);
        assert_eq!(
            snapshot.rows.len(),
            CHAT_MAX_MESSAGES,
            "snapshot must be bounded at 1000"
        );
        assert!(
            snapshot.has_more,
            "has_more must be true with 1100 messages at max window"
        );
    }

    // 7-C5: chat_cleared_on_logout
    //
    // Logout and IdentityChanged(None) must clear AppState::chat_rooms.
    #[test]
    fn chat_cleared_on_logout() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let group_id = "room1";

        // Seed a chat room
        state.chat_rooms.insert(
            group_id.to_string(),
            ChatRoomState {
                messages: vec![make_raw_row("evt1", "author1", "hello", 1_000_000)],
                page_count: 1,
                activity_revision: 1,
            },
        );
        // Seed a session so logout has something to clear
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(Some("somepubkey".into()))),
        );

        assert!(
            !state.chat_rooms.is_empty(),
            "chat_rooms must be populated before logout"
        );

        // Logout
        step(
            &mut state,
            &clock,
            Cmd::Action(crate::kernel::action::AppAction::Logout),
        );
        assert!(
            state.chat_rooms.is_empty(),
            "chat_rooms must be empty after Logout"
        );

        // Re-populate and test IdentityChanged(None)
        state
            .chat_rooms
            .insert(group_id.to_string(), ChatRoomState::default());
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(None)),
        );
        assert!(
            state.chat_rooms.is_empty(),
            "chat_rooms must be empty after IdentityChanged(None)"
        );
    }

    // 7-C5b: chat_event_after_close_or_logout_is_dropped
    //
    // A stray observer event (ChatRoomUpdated) that arrives AFTER the room was
    // closed (or after logout) must NOT recreate the room — otherwise a not-yet-
    // released observer leaks stale messages into the next session.
    #[test]
    fn chat_event_after_close_or_logout_is_dropped() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let group_id = "ghost-room";

        // Room was never opened (closed / logged out): injecting an event is a no-op.
        let msgs = vec![make_raw_row("evt1", "author1", "stale", 1_000_000)];
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::ChatRoomUpdated {
                group_id: group_id.to_string(),
                messages: msgs,
            }),
        );
        assert!(
            !state.chat_rooms.contains_key(group_id),
            "ChatRoomUpdated for a closed/absent room must NOT recreate it (D6)"
        );

        // Open then close, then a late observer event must not resurrect it.
        seed_community(&mut state, group_id, "wss://r.example");
        reduce_action_open_chat(&mut state, group_id.to_string());
        reduce_action_close_chat(&mut state, group_id.to_string());
        assert!(
            !state.chat_rooms.contains_key(group_id),
            "closed room removed"
        );
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::ChatRoomUpdated {
                group_id: group_id.to_string(),
                messages: vec![make_raw_row("evt2", "author1", "late", 1_000_001)],
            }),
        );
        assert!(
            !state.chat_rooms.contains_key(group_id),
            "late event after close must not resurrect the room"
        );
    }

    // 7-C6: malformed_no_op
    //
    // hl.chat.post with empty content must produce no effects (D6).
    #[test]
    fn malformed_no_op() {
        let mut state = make_state();
        let clock = ManualClock::default();
        seed_community(&mut state, "test-room", "wss://relay.example.com");

        // Empty content
        let payload = serde_json::json!({
            "group_id": "test-room",
            "content": "",
        });
        let envelope = crate::kernel::action::AppActionEnvelope {
            namespace: "hl.chat.post".to_string(),
            json: serde_json::to_string(&payload).unwrap(),
        };
        let effects = step(&mut state, &clock, Cmd::ActionEnvelope(envelope));
        assert!(
            effects.is_empty(),
            "empty content must produce no effects (D6)"
        );

        // Empty group_id
        let payload2 = serde_json::json!({
            "group_id": "",
            "content": "hello",
        });
        let envelope2 = crate::kernel::action::AppActionEnvelope {
            namespace: "hl.chat.post".to_string(),
            json: serde_json::to_string(&payload2).unwrap(),
        };
        let effects2 = step(&mut state, &clock, Cmd::ActionEnvelope(envelope2));
        assert!(
            effects2.is_empty(),
            "empty group_id must produce no effects (D6)"
        );

        // Unknown group_id: fail closed instead of widening or trusting the UI.
        let payload3 = serde_json::json!({
            "group_id": "unknown-room",
            "content": "hello",
        });
        let envelope3 = crate::kernel::action::AppActionEnvelope {
            namespace: "hl.chat.post".to_string(),
            json: serde_json::to_string(&payload3).unwrap(),
        };
        let effects3 = step(&mut state, &clock, Cmd::ActionEnvelope(envelope3));
        assert!(
            effects3.is_empty(),
            "unknown group_id must produce no effects (D3 fail-closed)"
        );
    }

    // 7-C7: chat_rows_oldest_first_with_header_gaps
    //
    // The snapshot rows must be oldest-first. show_header must be true for the
    // first row, on author change, and on a gap > 300 seconds.
    #[test]
    fn chat_rows_oldest_first_with_header_gaps() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let group_id = "header-room";
        let author_a = "aaaa000000000000000000000000000000000000000000000000000000000001";
        let author_b = "bbbb000000000000000000000000000000000000000000000000000000000002";

        // Newest-first in the buffer:
        // msg4 (t=1001, author_b), msg3 (t=1001, author_a), msg2 (t=1000 + 301=1301, author_a), msg1 (t=1000, author_a)
        // Oldest-first for display: msg1, msg2 (301s gap), msg3 (author change back to a? No — same author, small gap)
        // Let's use clear timestamps:
        // msg1 t=1000 author_a -> show_header=true (first)
        // msg2 t=1001 author_a -> show_header=false (same author, 1s gap)
        // msg3 t=1500 author_a -> show_header=true (gap > 300s from t=1001)
        // msg4 t=1501 author_b -> show_header=true (author change)
        let messages_newest_first = vec![
            ChatMessageRawRow {
                event_id: "msg4".to_string(),
                author_pubkey: author_b.to_string(),
                content: "b1".to_string(),
                created_at: 1501,
                reply_to_event_id: None,
            },
            ChatMessageRawRow {
                event_id: "msg3".to_string(),
                author_pubkey: author_a.to_string(),
                content: "a3".to_string(),
                created_at: 1500,
                reply_to_event_id: None,
            },
            ChatMessageRawRow {
                event_id: "msg2".to_string(),
                author_pubkey: author_a.to_string(),
                content: "a2".to_string(),
                created_at: 1001,
                reply_to_event_id: None,
            },
            ChatMessageRawRow {
                event_id: "msg1".to_string(),
                author_pubkey: author_a.to_string(),
                content: "a1".to_string(),
                created_at: 1000,
                reply_to_event_id: None,
            },
        ];
        inject_chat_updated(&mut state, &clock, group_id, messages_newest_first);

        let snapshot = compute_room_chat_snapshot(&state, group_id);
        assert_eq!(snapshot.rows.len(), 4, "four rows");

        // Oldest-first order
        assert_eq!(snapshot.rows[0].event_id, "msg1");
        assert_eq!(snapshot.rows[1].event_id, "msg2");
        assert_eq!(snapshot.rows[2].event_id, "msg3");
        assert_eq!(snapshot.rows[3].event_id, "msg4");

        // show_header logic
        assert!(
            snapshot.rows[0].show_header,
            "first row must have show_header=true"
        );
        assert!(
            !snapshot.rows[1].show_header,
            "msg2: same author, 1s gap → false"
        );
        assert!(
            snapshot.rows[2].show_header,
            "msg3: same author but 499s gap > 300s → true"
        );
        assert!(snapshot.rows[3].show_header, "msg4: author change → true");
    }

    // 7-C8: chat_load_more_caps_at_20_pages
    //
    // Repeated hl.chat.load_more must never exceed CHAT_MAX_PAGES=20.
    #[test]
    fn chat_load_more_caps_at_20_pages() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let group_id = "paginate-room";

        // Populate enough messages to always have_more
        let messages: Vec<ChatMessageRawRow> = (0u64..1100)
            .rev()
            .map(|i| {
                make_raw_row(
                    &format!("{:0>64}", format!("{:x}", i)),
                    "aaaa000000000000000000000000000000000000000000000000000000000001",
                    "msg",
                    1_000_000 + i,
                )
            })
            .collect();
        inject_chat_updated(&mut state, &clock, group_id, messages);

        // Dispatch load_more 25 times — must cap at 20
        for _ in 0..25 {
            let payload = serde_json::json!({ "group_id": group_id });
            let envelope = crate::kernel::action::AppActionEnvelope {
                namespace: "hl.chat.load_more".to_string(),
                json: serde_json::to_string(&payload).unwrap(),
            };
            step(&mut state, &clock, Cmd::ActionEnvelope(envelope));
        }

        let page_count = state
            .chat_rooms
            .get(group_id)
            .map(|r| r.page_count)
            .unwrap_or(0);
        assert_eq!(
            page_count, CHAT_MAX_PAGES,
            "page_count must cap at CHAT_MAX_PAGES={CHAT_MAX_PAGES}"
        );
    }

    // 7-C9: chat_reply_preview_only_for_visible_window
    //
    // reply_to in the snapshot must only be Some when the parent message is
    // within the bounded visible window.
    #[test]
    fn chat_reply_preview_only_for_visible_window() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let group_id = "reply-room";
        let parent_id = "parent_00000000000000000000000000000000000000000000000000000000001";
        let child_id = "child_000000000000000000000000000000000000000000000000000000000002";

        // Parent message at t=1000, child message at t=1001 referring to parent
        let messages = vec![
            ChatMessageRawRow {
                event_id: child_id.to_string(),
                author_pubkey: "aaaa000000000000000000000000000000000000000000000000000000000001"
                    .to_string(),
                content: "reply".to_string(),
                created_at: 1001,
                reply_to_event_id: Some(parent_id.to_string()),
            },
            ChatMessageRawRow {
                event_id: parent_id.to_string(),
                author_pubkey: "bbbb000000000000000000000000000000000000000000000000000000000002"
                    .to_string(),
                content: "original".to_string(),
                created_at: 1000,
                reply_to_event_id: None,
            },
        ];
        inject_chat_updated(&mut state, &clock, group_id, messages);

        let snapshot = compute_room_chat_snapshot(&state, group_id);

        // Find child row — oldest-first so parent is index 0, child is index 1
        let child_row = snapshot
            .rows
            .iter()
            .find(|r| r.event_id == child_id)
            .expect("child row must be present");
        assert!(
            child_row.reply_to.is_some(),
            "reply_to must be Some when parent is in visible window"
        );
        let preview = child_row.reply_to.as_ref().unwrap();
        assert_eq!(preview.event_id, parent_id);
        assert_eq!(preview.content, "original");
    }
}
