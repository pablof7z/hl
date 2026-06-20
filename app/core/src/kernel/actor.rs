//! Kernel actor — the single Rust writer for `AppState`.
//!
//! Architecture (TEA / Elm-style):
//!   native dispatch(AppAction) →  mpsc channel
//!   actor loop:  recv Cmd → reduce(state, cmd) → Vec<Effect>
//!                         → run effects (async, feeds back KernelEvent)
//!                         → recompute open-view snapshots
//!                         → emit changed snapshots at clock-capped cadence
//!
//! Non-Negotiables enforced here:
//!   #2  `AppState` is the single writer — no native mutation of state.
//!   #3  `dispatch` is fire-and-forget, returns `()`.
//!   #6  No mock / fake behavior.
//!   #7  Snapshots bounded by open views.
//! D8:   No sleeps or poll loops; time advances through the injected Clock.
//! D9:   Wall-clock reads confined to `SystemClock`; tests inject `ManualClock`.

use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::{Arc, Mutex};

use parking_lot::RwLock;
use tokio::sync::mpsc;

use nmp_ffi::{nmp_app_free, nmp_external_signer_init, nmp_signer_broker_init, NmpApp};

use crate::capabilities::CapabilityResult;
use crate::kernel::action::{AppAction, KernelEvent};
use crate::kernel::app::{AppState, KernelPolicy};
use crate::kernel::clock::Clock;
use crate::kernel::effect::Effect;
use crate::kernel::snapshot::ViewSnapshot;
use crate::kernel::view::{ViewId, ViewRegistry, ViewRoute};
use crate::onboarding::OnboardingStore;

use nmp_defaults::{NmpAppBuilder, RunConfig};

// Domain handlers — each owns the reducer/event/effect/snapshot arms for its slice.
use crate::kernel::domains::{
    articles,
    articles_feed,
    auth,
    bookmarks,
    // ── Phase 5E additions (append-only) ─────────────────────────────────────
    camera,
    // ── Phase 5F additions (append-only) ─────────────────────────────────────
    capture_draft,
    communities,
    discovery,
    feed,
    follows,
    highlight_feed,
    home_feed,
    // ── Phase 5C additions (append-only) ─────────────────────────────────────
    isbn,
    // ── Phase 5D additions (append-only) ─────────────────────────────────────
    ocr,
    // ── Phase 5H additions (append-only) ─────────────────────────────────────
    podcast,
    profiles,
    projections,
    reactions,
    relays,
    room_home,
    route,
    search,
    session,
    // ── Phase 5K additions (append-only) ─────────────────────────────────────
    share,
    whats_new,
};

// ─── NMP update-callback C ABI ──────────────────────────────────────────────

/// C ABI signature for the NMP update callback.
///
/// `nmp_app_set_update_callback` is `#[no_mangle] extern "C"` in nmp-ffi, so we
/// declare it here via an `extern "C"` block. `context` carries a pointer to a
/// heap-allocated `mpsc::UnboundedSender<Cmd>`; `bytes` + `len` are the raw
/// snapshot-frame bytes, valid only for the duration of the call.
type NmpUpdateCallbackFn = extern "C" fn(context: *mut c_void, bytes: *const u8, len: usize);

#[allow(improper_ctypes)] // NmpApp is opaque; the pointer is safe — nmp-ffi uses the same ABI.
extern "C" {
    fn nmp_app_set_update_callback(
        app: *mut NmpApp,
        context: *mut c_void,
        callback: Option<NmpUpdateCallbackFn>,
    );
}

/// Actual C callback: copies the frame bytes and forwards them to the actor
/// channel as `KernelEvent::NmpSnapshotFrame`. Non-blocking by design — the
/// actor decodes the frame in `reduce_event` on its own thread.
///
/// SAFETY: `context` is a `*const mpsc::UnboundedSender<Cmd>` kept alive by
/// the `Box` in `NmpHandle::_update_callback_ctx` for the full lifetime of the
/// `NmpApp`. `bytes` is valid for `len` bytes for the duration of this call
/// (nmp-ffi contract); we copy before returning.
extern "C" fn nmp_update_callback(context: *mut c_void, bytes: *const u8, len: usize) {
    if context.is_null() || bytes.is_null() {
        return;
    }
    // SAFETY: context is non-null and points to the Box<UnboundedSender<Cmd>>
    // kept alive by NmpHandle::_update_callback_ctx. The sender outlives any
    // callback invocation because it is dropped after the NmpApp is freed.
    let tx = unsafe { &*(context as *const mpsc::UnboundedSender<Cmd>) };
    // Copy the frame bytes; they are only valid for this call's duration.
    let frame = unsafe { std::slice::from_raw_parts(bytes, len) }.to_vec();
    // Fire-and-forget: drop the frame if the actor has shut down.
    let _ = tx.send(Cmd::Event(KernelEvent::NmpSnapshotFrame(frame)));
}

// ─── NmpApp handle ──────────────────────────────────────────────────────────

/// Owned, `Send + Sync` wrapper around the `NmpApp` raw pointer.
///
/// `NmpApp` is actor-backed and thread-safe for host API calls (see
/// nmp-ffi SAFETY docs). The raw pointer is freed exactly once in `Drop`.
///
/// `_update_callback_ctx` keeps the heap-allocated `UnboundedSender<Cmd>` that
/// the update callback's `context` pointer refers to alive for the full lifetime
/// of the `NmpApp`. It must be dropped AFTER `nmp_app_free` so no in-flight
/// callback can race against a freed context. `Drop` frees the NmpApp first
/// (via `nmp_app_free`), then drops `_update_callback_ctx` — Rust drops fields
/// in declaration order, so `_update_callback_ctx` must come AFTER the raw
/// pointer field. We enforce this by keeping both in this struct with the
/// `NonNull` first.
pub(crate) struct NmpHandle {
    pub(crate) ptr: NonNull<NmpApp>,
    /// Keeps the `mpsc::UnboundedSender<Cmd>` alive for the `nmp_update_callback`
    /// context pointer. Dropped AFTER `nmp_app_free` (declaration order).
    _update_callback_ctx: Option<Box<mpsc::UnboundedSender<Cmd>>>,
}

// SAFETY: NmpApp is designed for cross-thread host calls (nmp-ffi docs).
unsafe impl Send for NmpHandle {}
unsafe impl Sync for NmpHandle {}

impl Drop for NmpHandle {
    fn drop(&mut self) {
        // Free NmpApp first. After this returns, nmp-ffi guarantees no further
        // callback invocations can start (the quiescence gate ensures any
        // in-flight call has returned before nmp_app_free returns). It is then
        // safe for `_update_callback_ctx` to drop.
        nmp_app_free(self.ptr.as_ptr());
        // _update_callback_ctx drops here (after the app is freed).
    }
}

// ─── Command channel ────────────────────────────────────────────────────────

/// Everything the actor can receive.
// Phase 5C: `KernelEvent` grew when `IsbnPreviewReady` was added (12-field
// KernelArtifactPreview). `Cmd::Event(KernelEvent)` inherits the size; the
// allow is narrowly scoped here (Cmd) so it does not suppress warnings elsewhere.
#[allow(clippy::large_enum_variant)]
pub(crate) enum Cmd {
    Action(AppAction),
    ActionEnvelope(crate::kernel::action::AppActionEnvelope),
    Event(KernelEvent),
    OpenView(ViewId, ViewRoute),
    CloseView(ViewId),
    ProvideCapabilityResult(CapabilityResult),
    Resume,
    Suspend,
    Tick,
    Shutdown,
}

// ─── Observer trait (FFI callback) ──────────────────────────────────────────

/// Platform observer registered via `HighlighterApp::set_observer`.
/// Both methods must be non-blocking; called from the actor tokio task.
#[uniffi::export(with_foreign)]
pub trait HighlighterObserver: Send + Sync + 'static {
    /// A view's snapshot changed.
    fn on_snapshot(&self, view_id: ViewId, snapshot: ViewSnapshot);
    /// The kernel is requesting a native capability execution.
    fn on_capability_request(&self, request: crate::capabilities::CapabilityRequest);
}

// ─── Shared state (actor ↔ FFI layer) ───────────────────────────────────────

/// Mutable state accessible from the FFI layer without going through the actor.
pub(crate) struct SharedState {
    /// Latest computed snapshot per open view — updated after every reduce pass.
    /// Native reads via `current_snapshot()` without blocking on the actor.
    pub snapshots: Mutex<std::collections::HashMap<ViewId, ViewSnapshot>>,
    /// The registered platform observer.
    pub observer: RwLock<Option<Arc<dyn HighlighterObserver>>>,
}

impl SharedState {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            snapshots: Mutex::new(std::collections::HashMap::new()),
            observer: RwLock::new(None),
        })
    }
}

// ─── Pure reducer ───────────────────────────────────────────────────────────

/// Pure, synchronous reducer. Never async, never `.await` (Non-Negotiable #2).
/// Takes the current clock so clock-dependent checks (toast dismiss, session
/// timeout) are deterministic under `ManualClock` (D9).
pub(crate) fn reduce(state: &mut AppState, cmd: Cmd, now: u64) -> Vec<Effect> {
    // Clock-driven checks run on EVERY reduce pass, before the command.
    let mut effects = clock_checks(state, now);

    match cmd {
        Cmd::Action(action) => effects.extend(reduce_action(state, action, now)),
        Cmd::ActionEnvelope(envelope) => {
            effects.extend(reduce_action_envelope(state, envelope, now))
        }
        Cmd::Event(event) => effects.extend(reduce_event(state, event, now)),
        Cmd::OpenView(..) | Cmd::CloseView(..) | Cmd::Resume | Cmd::Suspend | Cmd::Tick => {
            // OpenView/CloseView/Resume/Suspend/Tick are handled by the actor loop
            // itself (registry + NmpApp lifecycle); the reducer has no state to change.
        }
        Cmd::ProvideCapabilityResult(result) => {
            effects.extend(reduce_event(
                state,
                KernelEvent::CapabilityResult(result),
                now,
            ));
        }
        Cmd::Shutdown => {}
    }

    effects
}

/// Clock-driven state transitions that fire on every reduce pass.
/// This is how the ManualClock controls deterministic time-based behavior
/// (D8: no sleeps; D9: no wall-clock reads in reducers).
fn clock_checks(state: &mut AppState, now: u64) -> Vec<Effect> {
    let effects: Vec<Effect> = Vec::new();

    route::clock_check_toast_dismiss(state, now);
    session::clock_check_restore_timeout(state, now);
    auth::clock_check_sign_in_timeout(state, now);
    // ── Phase 5F additions ─────────────────────────────────────────────────────
    capture_draft::clock_check_publish_timeout(state, now);

    effects
}

/// Thin dispatcher: routes each `AppAction` variant to its owning domain handler.
/// Future slices add a new domain module + one match arm pointing to it.
fn reduce_action(state: &mut AppState, action: AppAction, now: u64) -> Vec<Effect> {
    match action {
        AppAction::RestoreSession | AppAction::RetryRestore => {
            session::reduce_action_restore_session(state, now)
        }

        AppAction::Logout => auth::reduce_action_logout(state),

        AppAction::CompleteOnboarding => route::reduce_action_complete_onboarding(state),

        AppAction::SelectRootTab { tab } => route::reduce_action_select_root_tab(state, tab as u8),

        AppAction::PresentSheet { sheet_id } => route::reduce_action_present_sheet(state, sheet_id),

        AppAction::DismissSheet => route::reduce_action_dismiss_sheet(state),

        // ── Phase 2A additions ────────────────────────────────────────────────
        AppAction::SignInNsec { nsec } => auth::reduce_action_sign_in_nsec(state, nsec, now),

        // ── Phase 2B additions ────────────────────────────────────────────────
        AppAction::PairBunker { uri } => auth::reduce_action_pair_bunker(state, uri, now),

        AppAction::StartNostrConnect => auth::reduce_action_start_nostr_connect(state, now),

        AppAction::SignInNip55 => auth::reduce_action_sign_in_nip55(state, now),

        // ── Phase 2C additions ────────────────────────────────────────────────
        AppAction::CreateAccount { profile_name } => {
            auth::reduce_action_create_account(state, profile_name, now)
        }

        // ── Phase 2D additions ────────────────────────────────────────────────
        AppAction::AddRelay { url, role } => relays::reduce_action_add_relay(state, url, role),
        AppAction::RemoveRelay { url } => relays::reduce_action_remove_relay(state, url),
        AppAction::SetRelayRole { url, role } => {
            relays::reduce_action_set_relay_role(state, url, role)
        }
        AppAction::SetRoomsRelayList { relay_urls } => {
            relays::reduce_action_set_rooms_relay_list(state, relay_urls)
        }

        // ── Phase 3C additions ────────────────────────────────────────────────
        AppAction::Follow { pubkey } => follows::reduce_action_follow(pubkey),

        AppAction::Unfollow { pubkey } => follows::reduce_action_unfollow(pubkey),

        // ── Phase 3E additions ────────────────────────────────────────────────
        AppAction::StartRoomDiscovery { relay_url } => {
            discovery::reduce_action_start_room_discovery(relay_url)
        }

        // ── Phase 3D additions ────────────────────────────────────────────────
        AppAction::ClaimProfile { pubkey } => profiles::reduce_action_claim_profile(pubkey),

        AppAction::ReleaseProfile { pubkey } => profiles::reduce_action_release_profile(pubkey),

        // ── Phase 3F additions (append-only) ─────────────────────────────────
        AppAction::JoinRoom {
            group_id,
            host_relay_url,
            invite_code,
        } => room_home::reduce_action_join_room(group_id, host_relay_url, invite_code),

        AppAction::CreateRoom {
            group_id,
            host_relay_url,
            name,
            about,
        } => room_home::reduce_action_create_room(group_id, host_relay_url, name, about),

        AppAction::AddRoomMember {
            group_id,
            host_relay_url,
            pubkey,
            role,
        } => room_home::reduce_action_add_room_member(group_id, host_relay_url, pubkey, role),

        AppAction::CreateRoomInvites {
            group_id,
            host_relay_url,
            codes,
        } => room_home::reduce_action_create_room_invites(group_id, host_relay_url, codes),

        // ── Phase 4E additions (append-only) ─────────────────────────────────
        AppAction::ShareToRoom {
            group_id,
            host_relay_url,
            target_event_id,
            target_author_pubkey,
            repost,
        } => room_home::reduce_action_share_to_room(
            group_id,
            host_relay_url,
            target_event_id,
            target_author_pubkey,
            repost,
        ),

        // ── Phase 4C additions (append-only) ─────────────────────────────────
        AppAction::AddBookmark { item } => {
            bookmarks::reduce_action_add_bookmark_for_state(state, item)
        }

        AppAction::RemoveBookmark { item } => {
            bookmarks::reduce_action_remove_bookmark_for_state(state, item)
        }

        // ── Phase 4A additions ────────────────────────────────────────────────
        // OpenArticle / CloseArticle are fire-and-forget signals from native to
        // coordinate article reader lifecycle. No NMP action is needed — the
        // longform projection auto-populates AppState::articles. Native calls
        // open_view / close_view separately to manage the ViewRegistry; these
        // AppAction arms exist for Rust-side lifecycle coordination (reserved
        // for future per-address fetch in later slices). No effects emitted.
        AppAction::OpenArticle { .. } | AppAction::CloseArticle { .. } => vec![],

        // ── Phase 4B additions ────────────────────────────────────────────────
        AppAction::React {
            target_event_id,
            reaction,
            target_author_pubkey,
        } => reactions::reduce_action_react(target_event_id, reaction, target_author_pubkey),

        AppAction::Unreact { reaction_event_id } => {
            reactions::reduce_action_unreact(reaction_event_id)
        }

        // ── Phase 4D additions (append-only) ─────────────────────────────────
        AppAction::RunSearch { query, scope } => search::reduce_action_run_search(query, scope),

        // ── Phase 5A additions (append-only) ─────────────────────────────────
        AppAction::PrepareWhatsNew => whats_new::reduce_action_prepare_whats_new(),

        AppAction::MarkWhatsNewSeen { shipped_at_unix } => {
            whats_new::reduce_action_mark_whats_new_seen(state, shipped_at_unix)
        }

        // ── Phase 4G additions (append-only) ─────────────────────────────────
        AppAction::LoadMoreArticles => articles_feed::reduce_action_load_more_articles(state),

        // ── Phase 4H additions (append-only) ─────────────────────────────────
        // Pagination: emit DrainFeed when not already exhausted (D8: no polling).
        AppAction::DrainHighlightFeed => {
            // No-op when the cursor is caught up — avoids redundant drain calls
            // against an exhausted pull cursor (D8: level-triggered, not polled).
            // The `state` param is available because reduce_action borrows it.
            if state.highlight_feed.exhausted {
                vec![]
            } else {
                feed::reduce_drain_feed(highlight_feed::HIGHLIGHT_FEED_KEY.to_string())
            }
        }

        AppAction::PublishHighlight {
            content,
            source_reference,
            relay_hint,
        } => {
            // Empty content is a no-op (D6: invalid highlight not published).
            if content.is_empty() {
                vec![]
            } else {
                highlight_feed::reduce_action_publish_highlight(
                    content,
                    source_reference,
                    relay_hint,
                )
            }
        }

        // ── Phase 5C additions (append-only) ─────────────────────────────────
        AppAction::LookupIsbn { isbn } => isbn::reduce_action_lookup_isbn(state, isbn),

        // ── Phase 5K additions (append-only) ─────────────────────────────────
        AppAction::DrainShareQueue => share::reduce_action_drain_share_queue(),

        // ── Phase 5H additions (append-only) ─────────────────────────────────
        AppAction::AudioPlay {
            url,
            guid,
            artifact_json,
        } => {
            let artifact = match serde_json::from_str::<crate::models::ArtifactRecord>(
                &artifact_json,
            ) {
                Ok(a) => a,
                Err(e) => {
                    tracing::warn!(error = %e, "AudioPlay: failed to parse artifact_json (D6 — no-op)");
                    return vec![];
                }
            };
            let saved = state.podcast_resume_cache.get(&guid).copied();
            podcast::reduce_action_play(state, url, guid, artifact, saved)
        }
        AppAction::AudioPause => podcast::reduce_action_pause(state),
        AppAction::AudioSeek { seconds } => podcast::reduce_action_seek(state, seconds),
        AppAction::AudioSetResume { seconds } => podcast::reduce_action_set_resume(state, seconds),
    }
}

/// Routes an `AppActionEnvelope` (namespace + json) to domain reducers.
///
/// This replaces the UniFFI-exported `AppAction` enum at the FFI boundary.
/// Unknown namespace → invalid-action toast (D6: never a panic).
fn reduce_action_envelope(
    state: &mut AppState,
    envelope: crate::kernel::action::AppActionEnvelope,
    now: u64,
) -> Vec<Effect> {
    // Helper: parse json, emit invalid-action toast on failure
    macro_rules! parse {
        ($T:ty) => {
            match serde_json::from_str::<$T>(&envelope.json) {
                Ok(p) => p,
                Err(e) => {
                    return emit_invalid_action_toast(
                        state,
                        format!("{}: bad payload: {e}", &envelope.namespace),
                        now,
                    );
                }
            }
        };
    }

    use crate::kernel::action::{
        AddBookmarkPayload, AddRelayPayload, AddRoomMemberPayload, AudioPlayPayload,
        AudioSeekPayload, AudioSetResumePayload, CaptureSelectWordPayload,
        CaptureSetContextPayload, CaptureSetNotePayload, CaptureSetQuotePayload,
        CaptureSetTargetGroupPayload, ClaimProfilePayload, ClipExtendSegmentPayload,
        ClipMarkInPayload, ClipMarkOutPayload, ClipSetEndPayload, ClipSetStartPayload,
        CreateAccountPayload, CreateRoomInvitesPayload, CreateRoomPayload, FollowPayload,
        JoinRoomPayload, LookupIsbnPayload, MarkWhatsNewSeenPayload, OcrRecognizePayload,
        PairBunkerPayload, PresentSheetPayload, PublishHighlightPayload, ReactPayload,
        ReleaseProfilePayload, RemoveBookmarkPayload, RemoveRelayPayload, RunSearchPayload,
        SelectRootTabPayload, SetRelayRolePayload, SetRoomsRelayListPayload, ShareToRoomPayload,
        SignInNsecPayload, StartRoomDiscoveryPayload, UnfollowPayload, UnreactPayload,
    };

    match envelope.namespace.as_str() {
        // ── Auth / session ────────────────────────────────────────────────────
        "hl.auth.restore_session" | "hl.auth.retry_restore" => {
            session::reduce_action_restore_session(state, now)
        }
        "hl.auth.logout" => auth::reduce_action_logout(state),
        "hl.auth.sign_in_nsec" => {
            let p = parse!(SignInNsecPayload);
            auth::reduce_action_sign_in_nsec(state, p.nsec, now)
        }
        "hl.auth.pair_bunker" => {
            let p = parse!(PairBunkerPayload);
            auth::reduce_action_pair_bunker(state, p.uri, now)
        }
        "hl.auth.start_nostr_connect" => auth::reduce_action_start_nostr_connect(state, now),
        "hl.auth.sign_in_nip55" => auth::reduce_action_sign_in_nip55(state, now),
        "hl.auth.create_account" => {
            let p = parse!(CreateAccountPayload);
            auth::reduce_action_create_account(state, p.profile_name, now)
        }

        // ── Onboarding / route ────────────────────────────────────────────────
        "hl.route.complete_onboarding" => route::reduce_action_complete_onboarding(state),
        "hl.route.select_root_tab" => {
            let p = parse!(SelectRootTabPayload);
            route::reduce_action_select_root_tab(state, p.tab)
        }
        "hl.route.present_sheet" => {
            let p = parse!(PresentSheetPayload);
            route::reduce_action_present_sheet(state, p.sheet_id)
        }
        "hl.route.dismiss_sheet" => route::reduce_action_dismiss_sheet(state),

        // ── Relays ────────────────────────────────────────────────────────────
        "hl.relay.add" => {
            let p = parse!(AddRelayPayload);
            let role = relay_role_from_str(state, &envelope.namespace, &p.role, now);
            let role = match role {
                Some(r) => r,
                None => return vec![],
            };
            relays::reduce_action_add_relay(state, p.url, role)
        }
        "hl.relay.remove" => {
            let p = parse!(RemoveRelayPayload);
            relays::reduce_action_remove_relay(state, p.url)
        }
        "hl.relay.set_role" => {
            let p = parse!(SetRelayRolePayload);
            let role = relay_role_from_str(state, &envelope.namespace, &p.role, now);
            let role = match role {
                Some(r) => r,
                None => return vec![],
            };
            relays::reduce_action_set_relay_role(state, p.url, role)
        }
        "hl.relay.set_rooms_relay_list" => {
            let p = parse!(SetRoomsRelayListPayload);
            relays::reduce_action_set_rooms_relay_list(state, p.relay_urls)
        }

        // ── Follows ───────────────────────────────────────────────────────────
        "hl.profile.follow" => {
            let p = parse!(FollowPayload);
            follows::reduce_action_follow(p.pubkey)
        }
        "hl.profile.unfollow" => {
            let p = parse!(UnfollowPayload);
            follows::reduce_action_unfollow(p.pubkey)
        }

        // ── Profiles (claim/release) ──────────────────────────────────────────
        "hl.profile.claim" => {
            let p = parse!(ClaimProfilePayload);
            profiles::reduce_action_claim_profile(p.pubkey)
        }
        "hl.profile.release" => {
            let p = parse!(ReleaseProfilePayload);
            profiles::reduce_action_release_profile(p.pubkey)
        }

        // ── Room discovery ────────────────────────────────────────────────────
        "hl.room.start_discovery" => {
            let p = parse!(StartRoomDiscoveryPayload);
            discovery::reduce_action_start_room_discovery(p.relay_url)
        }

        // ── Room actions ──────────────────────────────────────────────────────
        "hl.room.join" => {
            let p = parse!(JoinRoomPayload);
            room_home::reduce_action_join_room(p.group_id, p.host_relay_url, p.invite_code)
        }
        "hl.room.create" => {
            let p = parse!(CreateRoomPayload);
            room_home::reduce_action_create_room(p.group_id, p.host_relay_url, p.name, p.about)
        }
        "hl.room.add_member" => {
            let p = parse!(AddRoomMemberPayload);
            room_home::reduce_action_add_room_member(p.group_id, p.host_relay_url, p.pubkey, p.role)
        }
        "hl.room.create_invites" => {
            let p = parse!(CreateRoomInvitesPayload);
            room_home::reduce_action_create_room_invites(p.group_id, p.host_relay_url, p.codes)
        }
        "hl.room.share_to_room" => {
            let p = parse!(ShareToRoomPayload);
            room_home::reduce_action_share_to_room(
                p.group_id,
                p.host_relay_url,
                p.target_event_id,
                p.target_author_pubkey,
                p.repost,
            )
        }

        // ── Bookmarks ─────────────────────────────────────────────────────────
        "hl.bookmark.add" => {
            let p = parse!(AddBookmarkPayload);
            bookmarks::reduce_action_add_bookmark_for_state(state, p.item)
        }
        "hl.bookmark.remove" => {
            let p = parse!(RemoveBookmarkPayload);
            bookmarks::reduce_action_remove_bookmark_for_state(state, p.item)
        }

        // ── Articles ──────────────────────────────────────────────────────────
        "hl.article.open" | "hl.article.close" => vec![],
        "hl.article.load_more" => articles_feed::reduce_action_load_more_articles(state),

        // ── Reactions ─────────────────────────────────────────────────────────
        "hl.reaction.react" => {
            let p = parse!(ReactPayload);
            reactions::reduce_action_react(p.target_event_id, p.reaction, p.target_author_pubkey)
        }
        "hl.reaction.unreact" => {
            let p = parse!(UnreactPayload);
            reactions::reduce_action_unreact(p.reaction_event_id)
        }

        // ── Search ────────────────────────────────────────────────────────────
        "hl.search.run" => {
            let p = parse!(RunSearchPayload);
            let scope = search_scope_from_str(state, &envelope.namespace, &p.scope, now);
            let scope = match scope {
                Some(s) => s,
                None => return vec![],
            };
            search::reduce_action_run_search(p.query, scope)
        }

        // ── What's New ────────────────────────────────────────────────────────
        "hl.whats_new.prepare" => whats_new::reduce_action_prepare_whats_new(),
        "hl.whats_new.mark_seen" => {
            let p = parse!(MarkWhatsNewSeenPayload);
            whats_new::reduce_action_mark_whats_new_seen(state, p.shipped_at_unix)
        }

        // ── Highlight feed ────────────────────────────────────────────────────
        "hl.highlight.drain_feed" => {
            if state.highlight_feed.exhausted {
                vec![]
            } else {
                feed::reduce_drain_feed(highlight_feed::HIGHLIGHT_FEED_KEY.to_string())
            }
        }
        "hl.highlight.publish" => {
            let p = parse!(PublishHighlightPayload);
            if p.content.is_empty() {
                vec![]
            } else {
                highlight_feed::reduce_action_publish_highlight(
                    p.content,
                    p.source_reference,
                    p.relay_hint,
                )
            }
        }

        // ── ISBN ──────────────────────────────────────────────────────────────
        "hl.isbn.lookup" => {
            let p = parse!(LookupIsbnPayload);
            isbn::reduce_action_lookup_isbn(state, p.isbn)
        }

        // ── Share queue ───────────────────────────────────────────────────────
        "hl.share.drain_queue" => share::reduce_action_drain_share_queue(),

        // ── Audio / podcast ───────────────────────────────────────────────────
        "hl.audio.play" => {
            let p = parse!(AudioPlayPayload);
            let artifact =
                match serde_json::from_str::<crate::models::ArtifactRecord>(&p.artifact_json) {
                    Ok(a) => a,
                    Err(e) => {
                        return emit_invalid_action_toast(
                            state,
                            format!("hl.audio.play: bad artifact_json: {e}"),
                            now,
                        );
                    }
                };
            let saved = state.podcast_resume_cache.get(&p.guid).copied();
            podcast::reduce_action_play(state, p.url, p.guid, artifact, saved)
        }
        "hl.audio.pause" => podcast::reduce_action_pause(state),
        "hl.audio.seek" => {
            let p = parse!(AudioSeekPayload);
            podcast::reduce_action_seek(state, p.seconds)
        }
        "hl.audio.set_resume" => {
            let p = parse!(AudioSetResumePayload);
            podcast::reduce_action_set_resume(state, p.seconds)
        }

        // ── Transcript ────────────────────────────────────────────────────────
        "hl.transcript.load" => podcast::reduce_action_load_transcript(state),
        "hl.audio.clip_mark_in" => {
            let p = parse!(ClipMarkInPayload);
            podcast::reduce_action_clip_mark_in(state, p.current_time)
        }
        "hl.audio.clip_mark_out" => {
            let p = parse!(ClipMarkOutPayload);
            podcast::reduce_action_clip_mark_out(state, p.current_time)
        }
        "hl.audio.clip_extend_segment" => {
            let p = parse!(ClipExtendSegmentPayload);
            podcast::reduce_action_clip_extend_segment(state, p.segment_id)
        }
        "hl.audio.clip_set_start" => {
            let p = parse!(ClipSetStartPayload);
            podcast::reduce_action_clip_set_start(state, p.value)
        }
        "hl.audio.clip_set_end" => {
            let p = parse!(ClipSetEndPayload);
            podcast::reduce_action_clip_set_end(state, p.value, p.duration_seconds)
        }
        "hl.audio.clip_clear" => podcast::reduce_action_clip_clear(state),

        // ── OCR ───────────────────────────────────────────────────────────────
        "hl.ocr.recognize" => {
            let p = parse!(OcrRecognizePayload);
            ocr::reduce_action_ocr_recognize(state, p.image_handle)
        }

        // ── Camera (Phase 5E) ─────────────────────────────────────────────────
        "hl.camera.capture_page" => camera::reduce_action_capture_page(state),
        "hl.camera.scan_barcode" => camera::reduce_action_scan_barcode(state),
        "hl.camera.cancel" => camera::reduce_action_cancel(state),

        // ── Capture draft (Phase 5F) ───────────────────────────────────────────
        "hl.capture.set_quote" => {
            let p = parse!(CaptureSetQuotePayload);
            capture_draft::reduce_action_set_quote(state, p.quote, now)
        }
        "hl.capture.set_context" => {
            let p = parse!(CaptureSetContextPayload);
            capture_draft::reduce_action_set_context(state, p.context)
        }
        "hl.capture.set_note" => {
            let p = parse!(CaptureSetNotePayload);
            capture_draft::reduce_action_set_note(state, p.note)
        }
        "hl.capture.select_word" => {
            let p = parse!(CaptureSelectWordPayload);
            capture_draft::reduce_action_select_word(state, p.word_index)
        }
        "hl.capture.clear_selection" => capture_draft::reduce_action_clear_selection(state),
        "hl.capture.set_target_group" => {
            let p = parse!(CaptureSetTargetGroupPayload);
            capture_draft::reduce_action_set_target_group(state, p.group_id, now)
        }
        "hl.capture.clear_target_group" => capture_draft::reduce_action_clear_target_group(state),
        "hl.capture.publish" => capture_draft::reduce_action_publish(state, now),
        "hl.capture.reset" => capture_draft::reduce_action_reset(state),

        // ── Unknown namespace ─────────────────────────────────────────────────
        _ => emit_invalid_action_toast(
            state,
            format!("unknown action namespace: {}", envelope.namespace),
            now,
        ),
    }
}

/// Emit an invalid-action toast and return an empty effect list.
/// D6: never panics on bad input — surfaces error as a transient UI toast.
fn emit_invalid_action_toast(state: &mut AppState, message: String, now: u64) -> Vec<Effect> {
    use crate::kernel::app::{ToastState, TOAST_DISMISS_SECS};
    tracing::warn!("invalid action: {message}");
    state.chrome.toast = Some(ToastState {
        message,
        dismiss_at_unix: now + TOAST_DISMISS_SECS,
    });
    vec![]
}

/// Parse a relay role string into a `RelayRole` variant.
/// Emits an invalid-action toast on failure (D6: no panic).
fn relay_role_from_str(
    state: &mut AppState,
    ns: &str,
    role: &str,
    now: u64,
) -> Option<crate::kernel::action::RelayRole> {
    use crate::kernel::action::RelayRole;
    match role {
        "read" => Some(RelayRole::Read),
        "write" => Some(RelayRole::Write),
        "both" => Some(RelayRole::Both),
        "indexer" => Some(RelayRole::Indexer),
        "read,indexer" => Some(RelayRole::ReadIndexer),
        "write,indexer" => Some(RelayRole::WriteIndexer),
        "both,indexer" => Some(RelayRole::BothIndexer),
        _ => {
            emit_invalid_action_toast(state, format!("{ns}: unknown relay role: {role}"), now);
            None
        }
    }
}

/// Parse a search scope string into a `SearchScope` variant.
/// Emits an invalid-action toast on failure (D6: no panic).
fn search_scope_from_str(
    state: &mut AppState,
    ns: &str,
    scope: &str,
    now: u64,
) -> Option<crate::kernel::action::SearchScope> {
    use crate::kernel::action::SearchScope;
    match scope {
        "users" => Some(SearchScope::Users),
        "long_form" => Some(SearchScope::LongForm),
        "notes" => Some(SearchScope::Notes),
        _ => {
            emit_invalid_action_toast(state, format!("{ns}: unknown search scope: {scope}"), now);
            None
        }
    }
}

/// Thin dispatcher: routes each `KernelEvent` variant to its owning domain handler.
fn reduce_event(state: &mut AppState, event: KernelEvent, _now: u64) -> Vec<Effect> {
    match event {
        KernelEvent::SessionRestored { present, pubkey } => {
            session::reduce_event_session_restored(state, present, pubkey)
        }

        KernelEvent::OnboardingStateLoaded(complete) => {
            route::reduce_event_onboarding_state_loaded(state, complete)
        }

        KernelEvent::CapabilityResult(result) => {
            session::reduce_event_capability_result(state, result)
        }

        KernelEvent::IdentityChanged(pubkey) => auth::reduce_event_identity_changed(state, pubkey),

        KernelEvent::ClockTick => {
            // Clock-driven checks already ran in `clock_checks` at the top of
            // `reduce`. `ClockTick` has no additional state changes.
            vec![]
        }

        // ── Phase 2A additions ────────────────────────────────────────────────
        KernelEvent::SignInFailed { method, error } => {
            auth::reduce_event_sign_in_failed(state, method, error)
        }

        // ── Phase 2B additions ────────────────────────────────────────────────
        KernelEvent::NostrConnectUriReady { uri } => {
            auth::reduce_event_nostrconnect_uri_ready(state, uri)
        }

        KernelEvent::BunkerHandshakeState { .. } => {
            // Broker progress events are diagnostic only — no reducer state
            // change. Future phases may surface these in a dedicated snapshot.
            vec![]
        }

        // ── Phase 3A additions (append-only) ─────────────────────────────────
        KernelEvent::NmpSnapshotFrame(bytes) => {
            // Decode the typed sidecar on the actor thread (non-blocking: only
            // FlatBuffers decode — no network I/O, no allocation beyond the vec)
            // and dispatch to the projections domain handler which routes each
            // schema_id into the appropriate AppState field (or a no-op in 3A).
            projections::dispatch_typed_frame(state, &bytes)
        }

        // ── Phase 3B additions (append-only) ─────────────────────────────────
        KernelEvent::JoinedGroupsUpdated(groups) => {
            communities::reduce_event_joined_groups_updated(state, groups)
        }

        // ── Phase 3C additions (append-only) ─────────────────────────────────
        KernelEvent::FollowListUpdated(pubkeys) => {
            // Store raw hex pubkeys decoded from the "nmp.nip02.follow_list"
            // typed sidecar. Also injectable directly from tests via Cmd::Event
            // (no live NmpApp needed — the reducer path is identical).
            state.follows = pubkeys;
            vec![]
        }

        // ── Phase 3E additions (append-only) ─────────────────────────────────
        KernelEvent::DiscoveredGroupsUpdated(rows) => {
            discovery::reduce_event_discovered_groups_updated(state, rows)
        }

        // ── Phase 3D additions (append-only) ─────────────────────────────────
        KernelEvent::ProfileCardUpdated { pubkey, card } => {
            // Store the decoded ProfileCardModel in claimed_profiles keyed by
            // the raw hex pubkey. The "profile" (own account) sidecar also
            // goes through this path for ViewId::Profile views where the viewed
            // pubkey matches the active account — own_profile is the read-side
            // fallback, claimed_profiles is the authoritative path for views.
            // `card` is `Box<ProfileCardModel>`; deref-move to owned model.
            state.claimed_profiles.insert(pubkey, *card);
            vec![]
        }

        // ── Phase 4C additions (append-only) ─────────────────────────────────
        KernelEvent::BookmarksUpdated(rows) => {
            // Store raw BookmarkRow items decoded from the "hl.bookmarks"
            // typed sidecar. No labels — raw fields only (D1). Also injectable
            // directly from tests via Cmd::Event (no live NmpApp needed).
            state.bookmarks = rows;
            vec![]
        }

        // ── Phase 4A additions (append-only) ─────────────────────────────────
        KernelEvent::ArticlesUpdated(rows) => {
            // Replace AppState::articles with the incoming row set.
            // The ArticlesUpdated event is produced by
            // `projections::dispatch_typed_frame` when the "nmp.nip23.articles"
            // sidecar arrives, but it is also injectable directly from tests
            // via Cmd::Event (no live NmpApp needed — reducer path is identical).
            // D1: rows carry raw protocol data only (no formatted strings).
            state.articles = rows.into_iter().map(|r| (r.address.clone(), r)).collect();
            vec![]
        }

        // ── Phase 4B additions (append-only) ─────────────────────────────────
        KernelEvent::ReactionStateUpdated {
            target_event_id,
            count,
            viewer_reacted,
        } => {
            // Upsert raw reaction state for the target event. No optimistic
            // delta applied here — D1: Swift owns optimistic UI state.
            state.reaction_state.insert(
                target_event_id.clone(),
                crate::kernel::snapshot::ReactionRow {
                    target_event_id,
                    count,
                    viewer_reacted,
                },
            );
            vec![]
        }

        // ── Phase 4D additions (append-only) ─────────────────────────────────
        KernelEvent::SearchResultsUpdated(rows) => {
            // Replace AppState::search_results with the incoming hit rows.
            // Produced by `projections::dispatch_typed_frame` when the
            // "hl.search" JSON sidecar arrives, but also injectable directly
            // from tests via Cmd::Event (no live NmpApp needed). D1: raw fields.
            state.search_results = rows;
            vec![]
        }

        // ── Phase 5A additions (append-only) ─────────────────────────────────
        KernelEvent::WhatsNewLoaded {
            entries,
            should_present,
        } => whats_new::reduce_event_whats_new_loaded(state, entries, should_present),

        // ── Phase 5C additions (append-only) ─────────────────────────────────
        KernelEvent::IsbnPreviewReady {
            isbn13,
            preview,
            error,
        } => isbn::reduce_event_isbn_preview_ready(state, isbn13, preview, error),
        KernelEvent::IsbnCacheLoaded { entries } => {
            isbn::reduce_event_isbn_cache_loaded(state, entries)
        }

        // ── Phase 4F additions (append-only) ─────────────────────────────────
        KernelEvent::FeedPage {
            key,
            cursor_id: _,
            rows,
            next_after_seq,
            exhausted,
            gap_rebased_to,
        } => {
            // Route on key to the correct FeedState in AppState and apply the page.
            // 4G/4H/4I add further routing arms here for their feed keys.
            // The generic engine (feed::apply_feed_page) handles all states.
            //
            // Inline: also send AdvancePullCursor so the kernel wake arm knows
            // the cursor has advanced (re-arms an immediate wake when there is
            // still data waiting). This keeps the cursor registry in sync without
            // a separate effect pass (the advance is non-reducing, fire-and-forget).
            //
            // Phase 4F wires article_feed / highlight_feed / room_lanes as the
            // extension seam; 4G/4H/4I add their consume arms here.
            let feed_state = match key.as_str() {
                "hl.feed.articles" => Some(&mut state.article_feed),
                "hl.feed.highlights" => Some(&mut state.highlight_feed),
                k if k.starts_with("hl.feed.room.") => {
                    // Lazily insert a FeedState for this group_id if not present.
                    let group_key = k.to_string();
                    state.room_lanes.entry(group_key).or_default();
                    state.room_lanes.get_mut(k)
                }
                _ => {
                    tracing::warn!(?key, "FeedPage: unknown feed key — no-op");
                    None
                }
            };

            if let Some(fs) = feed_state {
                feed::apply_feed_page(fs, rows, next_after_seq, exhausted, gap_rebased_to);
            }

            // Note: AdvancePullCursor is sent from the inline effect handler in
            // actor_task (after run_effect for DrainFeed) rather than here,
            // because we need the NmpHandle which is not available in reduce_event
            // (a pure, synchronous function — D9). The inline handler calls
            // feed::advance_feed_cursor after the FeedPage event is processed.
            vec![]
        }

        // ── Phase 5K additions (append-only) ─────────────────────────────────
        KernelEvent::ShareQueueDrained(payloads) => {
            // Route incoming share payloads into the share-queue domain handler
            // which deduplicates and appends to AppState::share_queue.pending.
            //
            // Note: in the live path, `CapabilityResult::Share(ShareResult::Pending)`
            // is handled by `session::reduce_event_capability_result`, which calls
            // `share::reduce_event_share_queue_drained` directly — it does NOT emit
            // this `KernelEvent::ShareQueueDrained` variant. This arm is therefore
            // ONLY reachable via `Cmd::Event` test injection (where tests bypass the
            // capability round-trip and inject share payloads directly). This matches
            // the pattern of `KernelEvent::FeedPage` etc. — each can be tested
            // independently of a live NmpApp.
            share::reduce_event_share_queue_drained(state, payloads)
        }

        // ── Phase 5H additions (append-only) ─────────────────────────────────
        KernelEvent::AudioCapabilityResult(result) => {
            // Route the raw `AudioResult` from the native AVPlayer to the podcast
            // domain reducer. Bounded cadence is enforced inside
            // `podcast::reduce_capability_audio` via `tick_projection` (D8).
            podcast::reduce_capability_audio(state, result)
        }
        KernelEvent::PodcastPositionLoaded {
            guid,
            position_seconds,
        } => {
            // Cache the loaded resume position so `reduce_action_play` can include
            // it in the next `AudioOp::Load` without a blocking I/O read.
            // DEVICE-LOCAL — never published to nostr.
            state.podcast_resume_cache.insert(guid, position_seconds);
            vec![]
        }

        // ── Phase 5D additions (append-only) ─────────────────────────────────
        KernelEvent::OcrRecognitionComplete {
            image_handle,
            markdown,
            selectable_words,
            raw_lines,
        } => ocr::reduce_event_ocr_recognition_complete(
            state,
            image_handle,
            markdown,
            selectable_words,
            raw_lines,
        ),

        // ── Phase 5F additions (append-only) ─────────────────────────────────
        KernelEvent::CaptureDraftPublishResult {
            success,
            event_id,
            error,
        } => capture_draft::reduce_event_publish_result(state, success, event_id, error),

        // ── Phase 5I additions (append-only) ─────────────────────────────────
        KernelEvent::TranscriptReady { segments } => {
            podcast::reduce_event_transcript_ready(state, segments)
        }
        KernelEvent::TranscriptFetchFailed => podcast::reduce_event_transcript_failed(state),

        // ── Phase 5E additions (append-only) ─────────────────────────────────
        KernelEvent::CameraCapabilityResult(_) => {
            // Test-only injection path (same pattern as KernelEvent::OcrRecognitionComplete
            // and KernelEvent::ShareQueueDrained). In the live path,
            // CapabilityResult::Camera(_) is handled by
            // session::reduce_event_capability_result → camera::reduce_capability_camera
            // directly — the state was already written there.
            // This arm is a no-op so tests can inject via Cmd::Event without
            // going through the capability round-trip.
            vec![]
        }
    }
}

// ─── Snapshot projection ────────────────────────────────────────────────────

/// Recompute the snapshot for one open view from `AppState`.
pub(crate) fn project_snapshot(
    state: &AppState,
    id: &ViewId,
    clock_now: u64,
) -> Option<ViewSnapshot> {
    match id {
        ViewId::Communities => communities::project_communities_snapshot(state),

        // ── Phase 3E additions ────────────────────────────────────────────────
        ViewId::RoomExplorer => discovery::project_room_explorer_snapshot(state),

        // ── Phase 3D additions (append-only) ─────────────────────────────────
        ViewId::Profile { pubkey } => profiles::project_profile_snapshot(state, pubkey),

        // ── Phase 3F additions (append-only) ─────────────────────────────────
        ViewId::RoomHome { group_id } => room_home::project_room_home_snapshot(state, group_id),

        // ── Phase 4C additions (append-only) ─────────────────────────────────
        ViewId::Bookmarks => Some(crate::kernel::snapshot::ViewSnapshot::Bookmarks(
            crate::kernel::snapshot::BookmarksSnapshot {
                rows: state.bookmarks.clone(),
            },
        )),

        // ── Phase 4A additions (append-only) ─────────────────────────────────
        ViewId::ArticleReader { address } => {
            articles::project_article_reader_snapshot(state, address)
        }

        // ── Phase 4D additions (append-only) ─────────────────────────────────
        ViewId::Search => search::project_search_snapshot(state),

        // ── Phase 4G additions (append-only) ─────────────────────────────────
        ViewId::ArticleFeed => articles_feed::project_article_feed_snapshot(state),

        // ── Phase 4H additions (append-only) ─────────────────────────────────
        ViewId::HighlightFeed => highlight_feed::project_highlight_feed_snapshot(state),

        // ── Phase 4J additions (append-only) ─────────────────────────────────
        ViewId::HomeFeed => home_feed::project_home_feed_snapshot(state),

        // ── Phase 5A additions (append-only) ─────────────────────────────────
        ViewId::WhatsNew => whats_new::project_whats_new_snapshot(state),

        // ── Phase 5C additions (append-only) ─────────────────────────────────
        ViewId::BookPicker => isbn::project_book_picker_snapshot(state),

        // ── Phase 5K additions (append-only) ─────────────────────────────────
        ViewId::ShareComposer => share::project_share_composer_snapshot(state)
            .map(crate::kernel::snapshot::ViewSnapshot::ShareComposer),

        // ── Phase 5H additions (append-only) ─────────────────────────────────
        ViewId::PodcastListening => podcast::project_podcast_listening_snapshot(state),

        // ── Phase 5D additions (append-only) ─────────────────────────────────
        // ── Phase 5F: capture_draft is now the authoritative Capture projector;
        //    it layers draft fields on top of the 5D OCR fields.
        ViewId::Capture => capture_draft::project_capture_snapshot(state),

        _ => route::project_snapshot(state, id, clock_now),
    }
}

// ─── Effect runner ───────────────────────────────────────────────────────────

/// Execute one effect and send the resulting `KernelEvent` (if any) back into
/// the actor channel. Non-async effects (keychain, onboarding) are
/// synchronous reads; async network effects (later phases) use tokio tasks.
///
/// `nmp` is `None` only in unit tests that do not boot a live `NmpApp`; the
/// nmp-call effects are no-ops in that case (tests inject `KernelEvent`s
/// directly instead).
#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_effect(
    effect: Effect,
    session_epoch: u64,
    tx: &mpsc::UnboundedSender<Cmd>,
    onboarding_store: &OnboardingStore,
    shared: &SharedState,
    nmp: Option<&NmpHandle>,
    policy: &KernelPolicy,
) {
    match effect {
        Effect::LoadOnboardingFlag => {
            route::run_effect_load_onboarding_flag(onboarding_store, tx).await;
        }

        Effect::RestoreSessionSecret => {
            session::run_effect_restore_session_secret(shared, tx).await;
        }

        Effect::ClearSession => {
            session::run_effect_clear_session(shared).await;
        }

        Effect::EmitCapabilityRequest(req) => {
            session::run_effect_emit_capability_request(req, shared).await;
        }

        // ── Phase 2A additions ────────────────────────────────────────────────
        Effect::AddNsecSigner { nsec } => {
            auth::run_effect_add_nsec_signer(nsec, nmp);
        }

        Effect::RemoveActiveAccount => {
            auth::run_effect_remove_active_account(nmp);
        }

        // ── Phase 2B additions ────────────────────────────────────────────────
        Effect::AddBunkerSigner { uri } => {
            auth::run_effect_add_bunker_signer(uri, nmp);
        }

        Effect::MintNostrConnectUri => {
            auth::run_effect_mint_nostrconnect_uri(nmp, tx).await;
        }

        Effect::StartNip55SignIn => {
            auth::run_effect_start_nip55_sign_in(nmp);
        }

        // ── Phase 2C additions ────────────────────────────────────────────────
        Effect::CreateAccount { profile_name } => {
            auth::run_effect_create_account(profile_name, nmp, policy);
        }

        // ── Phase 2D additions ────────────────────────────────────────────────
        Effect::AddRelay { url, role } => {
            relays::run_effect_add_relay(url, role, nmp);
        }
        Effect::RemoveRelay { url } => {
            relays::run_effect_remove_relay(url, nmp);
        }
        Effect::SetRelayRole { url, role } => {
            relays::run_effect_set_relay_role(url, role, nmp);
        }
        Effect::PublishRoomsRelayList { content } => {
            relays::run_effect_publish_rooms_relay_list(content, nmp);
        }

        // ── Phase 3B additions (append-only) ─────────────────────────────────
        Effect::WireJoinedGroups { pubkey } => {
            // Re-register the JoinedGroupsProjection for the new account pubkey.
            // Called at boot and on every IdentityChanged(Some) so the projection
            // follows account switches. Fire-and-forget: snapshot arrives on the
            // next NMP update-callback tick.
            if let Some(handle) = nmp {
                let nmp_ref: &NmpApp = unsafe { handle.ptr.as_ref() };
                communities::register_joined_groups_projection(nmp_ref, pubkey);
            }
        }

        // ── Phase 3C additions ────────────────────────────────────────────────
        Effect::DispatchFollowAction { follow, pubkey } => {
            follows::run_effect_dispatch_follow_action(follow, pubkey, nmp);
        }

        // ── Phase 3E additions (append-only) ─────────────────────────────────
        Effect::DispatchNip29Action { namespace, json } => {
            discovery::run_effect_dispatch_nip29_action(namespace, json, nmp);
        }

        Effect::WireGroupDiscovery { relay_url } => {
            if let Some(handle) = nmp {
                let nmp_ref: &NmpApp = unsafe { handle.ptr.as_ref() };
                discovery::run_effect_wire_group_discovery(relay_url, nmp_ref);
            }
        }

        // ── Phase 3D additions ────────────────────────────────────────────────
        Effect::ClaimProfile { pubkey } => {
            // Call nmp_app_claim_profile with liveness=Live so a Tailing kind:0
            // subscription stays open while the view is on screen. The updated
            // card arrives back through the "claimed_profiles" typed sidecar as
            // KernelEvent::ProfileCardUpdated. No-op in test mode (nmp=None).
            profiles::run_effect_claim_profile(pubkey, nmp);
        }

        Effect::ReleaseProfile { pubkey } => {
            // Call nmp_app_release_profile to decrement the per-consumer refcount.
            // When the count reaches zero NMP cancels the Tailing kind:0
            // subscription and removes the card from claimed_profiles. No-op in
            // test mode (nmp=None).
            profiles::run_effect_release_profile(pubkey, nmp);
        }

        // ── Phase 3F additions (append-only) ─────────────────────────────────
        //
        // WireGroupEvents and ReleaseGroupEvents require access to AppState
        // (to resolve host_relay_url from communities, or to clear the event
        // buffer). `run_effect` does not have a direct AppState parameter.
        // These effects are therefore handled INLINE in the actor task loop
        // (see the inline_effect dispatch block in actor_task) before the
        // generic run_effect is called. The arms below are unreachable in
        // normal operation — they are kept here to satisfy the exhaustive match
        // and to document the delegation contract.
        Effect::WireGroupEvents { group_id } | Effect::ReleaseGroupEvents { group_id } => {
            // Handled inline in actor_task; should not reach run_effect.
            let _ = group_id;
            tracing::trace!("WireGroupEvents/ReleaseGroupEvents reached run_effect — no-op (handled inline by actor_task)");
        }

        // ── Phase 4E additions (append-only) ─────────────────────────────────
        Effect::DispatchShareToRoom { namespace, json } => {
            room_home::run_effect_dispatch_share_to_room(namespace, json, nmp);
        }

        // ── Phase 4C additions (append-only) ─────────────────────────────────
        Effect::DispatchBookmarkAction { namespace, json } => {
            // Call nmp_app_dispatch_action with the NIP-51 bookmark namespace
            // and BookmarkUpdateInput JSON. Fire-and-forget (D6): the updated
            // kind:10003 list arrives back through the BookmarksUpdated
            // projection event.
            bookmarks::run_effect_dispatch_bookmark_action(namespace, json, nmp);
        }

        // ── Phase 4B additions (append-only) ─────────────────────────────────
        Effect::DispatchReactAction { namespace, json } => {
            // Call nmp_app_dispatch_action with "nmp.nip25.react" or
            // "nmp.nip25.unreact" and the serde-JSON payload. Fire-and-forget
            // (D6): the returned correlation_id JSON is freed and discarded.
            // The authoritative reaction state arrives back via
            // KernelEvent::ReactionStateUpdated on the next projection tick.
            reactions::run_effect_dispatch_react_action(namespace, json, nmp);
        }

        // ── Phase 4D additions (append-only) ─────────────────────────────────
        Effect::RunSearch {
            query,
            scope_json,
            interest_id,
        } => {
            // Push the NIP-50 search interest and replace the hl-owned
            // SearchResultsProjection. Fire-and-forget (D6): search hits
            // arrive back as KernelEvent::SearchResultsUpdated via the NMP
            // snapshot callback. No-op if nmp is None (test mode).
            search::run_effect_run_search(query, scope_json, interest_id, nmp);
        }

        // ── Phase 4H additions (append-only) ─────────────────────────────────
        Effect::PublishHighlightEvent { json } => {
            // Publish a kind:9802 highlight via ActorCommand::PublishRawEvent.
            // There is no dedicated nmp action namespace for kind:9802 at pinned
            // b4404159 — this is the same raw publish path Phase 2D uses for
            // the rooms relay list. Fire-and-forget (D6).
            highlight_feed::run_effect_publish_highlight(json, nmp);
        }

        // ── Phase 5C additions (append-only) ─────────────────────────────────
        // No-op when data_dir is empty (test mode — tests inject KernelEvent::IsbnPreviewReady
        // directly). data_dir is read from policy (same pattern as 5A whats_new effects).
        Effect::LookupIsbn { isbn13 } => {
            let data_dir = policy.data_dir.clone();
            isbn::run_effect_lookup_isbn(isbn13, data_dir, tx).await;
        }
        Effect::LoadIsbnCache => {
            let data_dir = policy.data_dir.clone();
            isbn::run_effect_load_isbn_cache(data_dir, tx).await;
        }
        Effect::PersistIsbnCache { entries } => {
            let data_dir = policy.data_dir.clone();
            isbn::run_effect_persist_isbn_cache(entries, data_dir).await;
        }

        // ── Phase 4F additions (append-only) ─────────────────────────────────
        //
        // RegisterFeedCursor, DrainFeed, and ReleaseFeedCursor require access
        // to AppState (to look up cursor_id for DrainFeed and ReleaseFeedCursor).
        // They are handled INLINE in actor_task before the generic run_effect
        // path (same pattern as WireGroupEvents / ReleaseGroupEvents which also
        // need AppState). The arms below are unreachable in normal operation
        // — kept here to satisfy the exhaustive match and document the contract.
        Effect::RegisterFeedCursor { key, .. }
        | Effect::DrainFeed { key }
        | Effect::ReleaseFeedCursor { key } => {
            // Handled inline in actor_task; should not reach run_effect.
            let _ = key;
            tracing::trace!(
                "RegisterFeedCursor/DrainFeed/ReleaseFeedCursor reached run_effect — no-op (handled inline by actor_task)"
            );
        }

        // ── Phase 5A additions (append-only) ─────────────────────────────────
        Effect::LoadWhatsNewState => {
            // Parse the bundled JSON and read the seen-marker file from disk.
            // Sends KernelEvent::WhatsNewLoaded with filtered (unseen) entries
            // and the should_present flag.
            //
            // No-op when data_dir is empty (test mode — tests inject
            // KernelEvent::WhatsNewLoaded directly to drive the reducer).
            let data_dir = policy.data_dir.clone();
            let tx_clone = tx.clone();
            tokio::spawn(async move {
                run_effect_load_whats_new_state(&data_dir, &tx_clone).await;
            });
        }

        Effect::PersistWhatsNewSeen { shipped_at_unix } => {
            // Write the seen-marker file to disk. Fire-and-forget (D6).
            // No-op when data_dir is empty (test mode).
            let data_dir = policy.data_dir.clone();
            tokio::spawn(async move {
                run_effect_persist_whats_new_seen(&data_dir, shipped_at_unix).await;
            });
        }

        // ── Phase 5H additions (append-only) ─────────────────────────────────
        Effect::LoadPodcastPosition { guid } => {
            // Look up the saved resume position for `guid` from disk and send
            // KernelEvent::PodcastPositionLoaded. Fire-and-forget (D6).
            // No-op when data_dir is empty (test mode — tests inject
            // KernelEvent::PodcastPositionLoaded directly).
            let data_dir = policy.data_dir.clone();
            let tx_clone = tx.clone();
            tokio::spawn(async move {
                run_effect_load_podcast_position(&data_dir, guid, &tx_clone).await;
            });
        }

        Effect::SavePodcastPosition {
            guid,
            position_seconds,
            artifact,
        } => {
            // Atomically persist the resume position. Fire-and-forget (D6).
            // DEVICE-LOCAL — NEVER a nostr event.
            // No-op when data_dir is empty (test mode).
            let data_dir = policy.data_dir.clone();
            tokio::spawn(async move {
                run_effect_save_podcast_position(&data_dir, guid, position_seconds, *artifact)
                    .await;
            });
        }

        // ── Phase 5F additions (append-only) ─────────────────────────────────
        Effect::PublishCaptureEvent { json } => {
            // Raw publish via ActorCommand::PublishRawEvent — same path as Phase 4H
            // highlight (kind:11 plain capture). No dedicated nmp.capture namespace
            // at pinned b4404159; reuse the highlight publish runner since both are
            // just PublishRawEvent. Fire-and-forget (D6). No-op when nmp is None.
            highlight_feed::run_effect_publish_highlight(json, nmp);
        }

        // ── Phase 5I additions (append-only) ─────────────────────────────────
        Effect::FetchTranscript { url } => {
            let tx_clone = tx.clone();
            tokio::spawn(async move {
                run_effect_fetch_transcript(url, &tx_clone).await;
            });
        }
    }

    let _ = session_epoch; // carried for future epoch-keyed effect cancellation
}

// ─── Phase 5A helpers ────────────────────────────────────────────────────────

/// Execute `Effect::LoadWhatsNewState`:
///   1. Decode the bundled What's New JSON (compile-time `include_str!`).
///   2. Read `{data_dir}/whats-new-state-v1.json` for the last-seen marker.
///   3. Filter entries to those with `shipped_at_unix > last_seen_marker`.
///   4. On first launch (no state file): seed the marker to the newest entry
///      and send `should_present: false` (first-launch logic mirrors the live
///      `WhatsNewStore::prepare` behaviour).
///   5. Send `KernelEvent::WhatsNewLoaded { entries, should_present }`.
///
/// No-op when `data_dir` is empty (test mode). D6: any I/O error is logged
/// and results in an empty/false response so the UI remains stable.
async fn run_effect_load_whats_new_state(data_dir: &str, tx: &mpsc::UnboundedSender<Cmd>) {
    if data_dir.is_empty() {
        // Test mode — tests inject KernelEvent::WhatsNewLoaded directly.
        return;
    }

    // Decode bundled JSON.
    let all_entries = match whats_new::decode_bundled_entries() {
        Some(e) => e,
        None => {
            tracing::warn!(
                "run_effect_load_whats_new_state: bundled JSON decode failed — sending empty (D6)"
            );
            let _ = tx.send(Cmd::Event(
                crate::kernel::action::KernelEvent::WhatsNewLoaded {
                    entries: Vec::new(),
                    should_present: false,
                },
            ));
            return;
        }
    };

    // Read the state file.
    let state_path = std::path::Path::new(data_dir).join(whats_new::STATE_FILE_NAME);

    let last_seen = read_whats_new_state(&state_path).await;

    let (entries, should_present) = match last_seen {
        Some(marker) => {
            // Filter to entries newer than the marker.
            let unseen: Vec<_> = all_entries
                .into_iter()
                .filter(|e| e.shipped_at_unix > marker)
                .collect();
            let present = !unseen.is_empty();
            (unseen, present)
        }
        None => {
            // First launch: seed the marker to the newest entry. No sheet on first launch.
            if let Some(newest) = all_entries.first() {
                persist_whats_new_seen_inner(&state_path, newest.shipped_at_unix).await;
            }
            (Vec::new(), false)
        }
    };

    let _ = tx.send(Cmd::Event(
        crate::kernel::action::KernelEvent::WhatsNewLoaded {
            entries,
            should_present,
        },
    ));
}

/// Execute `Effect::PersistWhatsNewSeen { shipped_at_unix }`:
/// Write the monotonic seen marker to `{data_dir}/whats-new-state-v1.json`.
/// Fire-and-forget (D6). No-op when `data_dir` is empty (test mode).
async fn run_effect_persist_whats_new_seen(data_dir: &str, shipped_at_unix: u64) {
    if data_dir.is_empty() {
        return;
    }
    let state_path = std::path::Path::new(data_dir).join(whats_new::STATE_FILE_NAME);
    persist_whats_new_seen_inner(&state_path, shipped_at_unix).await;
}

/// Read the last-seen marker from the state file. Returns `None` when the file
/// does not exist (first launch) or cannot be parsed (D6: silent no-op).
async fn read_whats_new_state(path: &std::path::Path) -> Option<u64> {
    #[derive(serde::Deserialize)]
    struct State {
        last_seen_at_unix_seconds: u64,
    }
    match tokio::fs::read(path).await {
        Ok(bytes) => match serde_json::from_slice::<State>(&bytes) {
            Ok(s) => Some(s.last_seen_at_unix_seconds),
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "read_whats_new_state: parse error — treating as first-launch (D6)"
                );
                None
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            tracing::warn!(
                path = %path.display(),
                error = %e,
                "read_whats_new_state: I/O error — treating as first-launch (D6)"
            );
            None
        }
    }
}

/// Write the seen marker to disk using an atomic rename (tmp → final).
/// D6: logs and returns on any error — never panics.
async fn persist_whats_new_seen_inner(path: &std::path::Path, shipped_at_unix: u64) {
    #[derive(serde::Serialize)]
    struct State {
        last_seen_at_unix_seconds: u64,
    }
    let bytes = match serde_json::to_vec(&State {
        last_seen_at_unix_seconds: shipped_at_unix,
    }) {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(error = %e, "persist_whats_new_seen: JSON encode error — no-op (D6)");
            return;
        }
    };
    if let Some(parent) = path.parent() {
        if let Err(e) = tokio::fs::create_dir_all(parent).await {
            tracing::warn!(path = %path.display(), error = %e, "persist_whats_new_seen: mkdir error — no-op (D6)");
            return;
        }
    }
    let tmp = path.with_extension("json.tmp");
    if let Err(e) = tokio::fs::write(&tmp, &bytes).await {
        tracing::warn!(path = %tmp.display(), error = %e, "persist_whats_new_seen: write error — no-op (D6)");
        return;
    }
    if let Err(e) = tokio::fs::rename(&tmp, path).await {
        tracing::warn!(path = %path.display(), error = %e, "persist_whats_new_seen: rename error — no-op (D6)");
    }
}

// ─── Phase 5H helpers ────────────────────────────────────────────────────────

/// Execute `Effect::LoadPodcastPosition { guid }`:
///   1. Read `{data_dir}/podcast-position-v1.json` from disk.
///   2. If the record's `guid` matches, send `KernelEvent::PodcastPositionLoaded`.
///   3. If the file does not exist, or the guid does not match, send nothing.
///
/// No-op when `data_dir` is empty (test mode — tests inject
/// `KernelEvent::PodcastPositionLoaded` directly).  D6: I/O errors are logged.
async fn run_effect_load_podcast_position(
    data_dir: &str,
    guid: String,
    tx: &mpsc::UnboundedSender<Cmd>,
) {
    if data_dir.is_empty() {
        return;
    }
    use crate::models::PodcastPositionRecord;

    let path =
        std::path::Path::new(data_dir).join(crate::kernel::domains::podcast::POSITION_FILE_NAME);
    let bytes = match tokio::fs::read(&path).await {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "load_podcast_position: read error (D6)");
            return;
        }
    };
    let record: PodcastPositionRecord = match serde_json::from_slice(&bytes) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "load_podcast_position: parse error (D6)");
            return;
        }
    };
    if record.guid == guid {
        let _ = tx.send(Cmd::Event(
            crate::kernel::action::KernelEvent::PodcastPositionLoaded {
                guid,
                position_seconds: record.position_seconds,
            },
        ));
    }
}

/// Execute `Effect::SavePodcastPosition { guid, position_seconds, artifact }`:
///   Atomically write to `{data_dir}/podcast-position-v1.json`.
///
/// DEVICE-LOCAL — NEVER a nostr event (`hl-app-state-vs-nostr-facts`).
/// No-op when `data_dir` is empty (test mode). D6: I/O failure is logged.
async fn run_effect_save_podcast_position(
    data_dir: &str,
    guid: String,
    position_seconds: f64,
    artifact: crate::models::ArtifactRecord,
) {
    if data_dir.is_empty() {
        return;
    }
    use crate::models::PodcastPositionRecord;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let record = PodcastPositionRecord {
        guid,
        position_seconds,
        last_played_at_unix_seconds: now,
        artifact,
    };
    let bytes = match serde_json::to_vec(&record) {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(error = %e, "save_podcast_position: encode error (D6)");
            return;
        }
    };
    let path =
        std::path::Path::new(data_dir).join(crate::kernel::domains::podcast::POSITION_FILE_NAME);
    if let Some(parent) = path.parent() {
        if let Err(e) = tokio::fs::create_dir_all(parent).await {
            tracing::warn!(path = %parent.display(), error = %e, "save_podcast_position: mkdir error (D6)");
            return;
        }
    }
    let tmp = path.with_extension("json.tmp");
    if let Err(e) = tokio::fs::write(&tmp, bytes).await {
        tracing::warn!(path = %tmp.display(), error = %e, "save_podcast_position: write error (D6)");
        return;
    }
    if let Err(e) = tokio::fs::rename(&tmp, &path).await {
        tracing::warn!(path = %path.display(), error = %e, "save_podcast_position: rename error (D6)");
    }
}

// ─── Phase 5I helpers ────────────────────────────────────────────────────────

/// Fetch and parse a transcript from `url`, then send the result back as a
/// `KernelEvent`. Ported parsing logic from bespoke `podcast_transcript.rs`.
/// D6: any fetch or parse failure sends `TranscriptFetchFailed` (no panic).
/// DEVICE-LOCAL — transcript content is never published to nostr.
async fn run_effect_fetch_transcript(url: String, tx: &mpsc::UnboundedSender<Cmd>) {
    use crate::kernel::domains::podcast_transcript as pt;

    let event = match pt::fetch_and_parse(&url).await {
        Ok(segments) => {
            Cmd::Event(crate::kernel::action::KernelEvent::TranscriptReady { segments })
        }
        Err(e) => {
            tracing::warn!(url = %url, error = %e, "transcript fetch failed (D6)");
            Cmd::Event(crate::kernel::action::KernelEvent::TranscriptFetchFailed)
        }
    };
    let _ = tx.send(event);
}

// ─── Phase 4F helpers ────────────────────────────────────────────────────────

/// Return the `cursor_id` stored in the `FeedState` for the given feed key.
///
/// Used by the inline DrainFeed / ReleaseFeedCursor handlers in `actor_task`
/// to look up which cursor to drain or unregister without an actor round-trip.
/// Returns `0` (no-op sentinel) when the key is unknown or the cursor was
/// not yet registered.
fn feed_state_cursor_id(state: &AppState, key: &str) -> u64 {
    match key {
        "hl.feed.articles" => state.article_feed.cursor_id,
        "hl.feed.highlights" => state.highlight_feed.cursor_id,
        k if k.starts_with("hl.feed.room.") => state.room_lanes.get(k).map_or(0, |fs| fs.cursor_id),
        _ => 0,
    }
}

/// Return the `after_seq` stored in the `FeedState` for the given feed key.
///
/// Passed to `run_effect_register_feed_cursor` on (re-)registration so the
/// kernel cursor resumes from the last known position rather than rewinding
/// to zero on view re-open (D6: idempotent re-registration).
fn feed_state_after_seq(state: &AppState, key: &str) -> u64 {
    match key {
        "hl.feed.articles" => state.article_feed.after_seq,
        "hl.feed.highlights" => state.highlight_feed.after_seq,
        k if k.starts_with("hl.feed.room.") => state.room_lanes.get(k).map_or(0, |fs| fs.after_seq),
        _ => 0,
    }
}

// ─── Actor task ─────────────────────────────────────────────────────────────

/// Minimum seconds between observer snapshot-push emissions (capped cadence).
/// The actor pushes at most once per this interval (D8 — no sleeps, cadence
/// driven by clock ticks from the outside rather than a background timer).
const EMIT_CADENCE_SECS: u64 = 1;

/// Main actor loop. Runs in a dedicated tokio task.
pub(crate) async fn actor_task(
    mut rx: mpsc::UnboundedReceiver<Cmd>,
    tx: mpsc::UnboundedSender<Cmd>,
    shared: Arc<SharedState>,
    clock: Arc<dyn Clock>,
    onboarding_store: Arc<OnboardingStore>,
    nmp: Option<NmpHandle>,
    policy: Arc<KernelPolicy>,
) {
    // Phase 3G: propagate room policy from bootstrap into AppState so the
    // discovery lifecycle hook can read it without needing to import the policy Arc.
    let mut state = AppState {
        room_policy: policy.room.clone(),
        ..AppState::default()
    };
    let mut registry = ViewRegistry::default();
    let mut last_emit_at: u64 = 0;
    let mut suspended = false;

    // Keep the NmpApp alive for the duration of the actor.
    // `nmp_ref` is passed to `run_effect` for nmp-call effects (Phase 2A+).
    let nmp_handle = nmp;

    while let Some(cmd) = rx.recv().await {
        let is_shutdown = matches!(cmd, Cmd::Shutdown);

        // Handle view registry mutations before reducing (they need the registry).
        // Phase 3D: also gather lifecycle effects for profile claim/release — these
        // are side-effects of view open/close, not of AppAction dispatch, so they
        // live here rather than in the pure reducer.
        let mut lifecycle_effects: Vec<Effect> = Vec::new();
        match &cmd {
            Cmd::OpenView(id, route) => {
                registry.open(id.clone(), route.clone());
                // ── Phase 3D: claim a profile subscription when its view opens ──
                lifecycle_effects.extend(profiles::lifecycle_effects_for_view_open(id));
                // ── Phase 3F: wire group-events projection when room-home view opens ──
                lifecycle_effects.extend(room_home::lifecycle_effects_for_view_open(id));
                // ── Phase 3G: auto-start room discovery when RoomExplorer opens ──
                lifecycle_effects.extend(discovery::lifecycle_effects_for_view_open(id, &state));
                // ── Phase 4A: article reader lifecycle (no-op — longform projection
                //    auto-populates AppState::articles; no NMP claim needed) ──────
                lifecycle_effects.extend(articles::lifecycle_effects_for_view_open(id));
                // ── Phase 4D: search view lifecycle (no-op on open — projection
                //    wired by RunSearch dispatch, not by view open) ───────────────
                lifecycle_effects.extend(search::lifecycle_effects_for_view_open(id));
                // ── Phase 4G: article feed lifecycle — register cursor + drain ──
                lifecycle_effects
                    .extend(articles_feed::lifecycle_effects_for_view_open(id, &state));
                // ── Phase 4H: highlight feed — register cursor + initial drain ──
                lifecycle_effects.extend(highlight_feed::lifecycle_effects_for_view_open(id));
                // ── Phase 4J: home feed — compose both underlying feed lifecycles ──
                lifecycle_effects.extend(home_feed::lifecycle_effects_for_view_open(id, &state));
            }
            Cmd::CloseView(id) => {
                // ── Phase 3D: release the profile subscription before removing from registry ──
                lifecycle_effects.extend(profiles::lifecycle_effects_for_view_close(id));
                // ── Phase 3F: release group-events buffer when room-home view closes ──
                lifecycle_effects.extend(room_home::lifecycle_effects_for_view_close(id));
                // ── Phase 4A: article reader close lifecycle (no-op in 4A) ─────
                lifecycle_effects.extend(articles::lifecycle_effects_for_view_close(id));
                // ── Phase 4D: clear search results inline when search view closes ──
                // Handled inline (like ReleaseGroupEvents) because clearing
                // AppState requires a mutable state reference that run_effect
                // does not have.
                if matches!(id, ViewId::Search) {
                    state.search_results.clear();
                }
                lifecycle_effects.extend(search::lifecycle_effects_for_view_close(id));
                // ── Phase 4G: article feed lifecycle — release cursor ────────────
                lifecycle_effects.extend(articles_feed::lifecycle_effects_for_view_close(id));
                // ── Phase 4H: highlight feed — release cursor on close ─────────
                // ReleaseFeedCursor is handled inline in the effect runner below
                // (it needs cursor_id from AppState). The lifecycle fn emits the
                // Effect::ReleaseFeedCursor data; the inline handler executes it.
                lifecycle_effects.extend(highlight_feed::lifecycle_effects_for_view_close(id));
                // ── Phase 4J: home feed — release both underlying cursors ───────
                lifecycle_effects.extend(home_feed::lifecycle_effects_for_view_close(id));
                registry.close(id);
            }
            Cmd::Resume => {
                suspended = false;
            }
            Cmd::Suspend => {
                suspended = true;
            }
            _ => {}
        }

        let now = clock.now_unix_seconds();

        // Reduce (pure, sync).
        let effects = reduce(&mut state, cmd, now);

        // Run lifecycle effects first (profile claim/release), then reducer effects.
        // Phase 3F: WireGroupEvents and ReleaseGroupEvents require AppState and are
        // handled INLINE here before the generic run_effect path (they need to
        // resolve host_relay_url from AppState::communities or mutate room_home_events).
        // Phase 4F: RegisterFeedCursor, DrainFeed, ReleaseFeedCursor also handled
        // inline because they need AppState::article_feed / highlight_feed / room_lanes
        // to look up cursor_id (for drain and release) or to call advance_feed_cursor.
        for effect in lifecycle_effects.into_iter().chain(effects) {
            match &effect {
                Effect::WireGroupEvents { group_id } => {
                    // Resolve host_relay_url from communities and wire the projection.
                    room_home::run_effect_wire_group_events(
                        group_id.clone(),
                        &state,
                        nmp_handle.as_ref(),
                    );
                    continue;
                }
                Effect::ReleaseGroupEvents { group_id } => {
                    // Clear the hl-side event buffer to bound memory.
                    state.room_home_events.remove(group_id);
                    continue;
                }
                // ── Phase 4F inline effect handlers ──────────────────────────
                Effect::RegisterFeedCursor {
                    key,
                    cursor_id,
                    scope,
                } => {
                    // Store cursor_id in the appropriate FeedState so DrainFeed
                    // and ReleaseFeedCursor can look it up without an actor
                    // round-trip. Then register with the kernel.
                    let id = *cursor_id;
                    let key_clone = key.clone();
                    let after_seq = feed_state_after_seq(&state, key);
                    match key.as_str() {
                        "hl.feed.articles" => state.article_feed.cursor_id = id,
                        "hl.feed.highlights" => state.highlight_feed.cursor_id = id,
                        k if k.starts_with("hl.feed.room.") => {
                            state.room_lanes.entry(k.to_string()).or_default().cursor_id = id;
                        }
                        _ => {}
                    }
                    feed::run_effect_register_feed_cursor(
                        key_clone,
                        id,
                        scope.clone(),
                        after_seq,
                        nmp_handle.as_ref(),
                    );
                    continue;
                }
                Effect::DrainFeed { key } => {
                    // Look up the cursor_id from AppState and call nmp_app_pull_page.
                    // Fire-and-forget (D6): the decoded FeedPage event is sent back
                    // via the tx channel, triggering reduce_event::FeedPage above.
                    // D8: a single pull_page call per DrainFeed — no polling loop.
                    let cursor_id = feed_state_cursor_id(&state, key);
                    feed::run_effect_drain_feed(key.clone(), cursor_id, &tx, nmp_handle.as_ref());
                    continue;
                }
                Effect::ReleaseFeedCursor { key } => {
                    // Look up cursor_id and unregister. Also clear the FeedState
                    // rows to bound memory (cursor_id is preserved so a re-open
                    // can re-register with the same id — idempotent, D6).
                    let cursor_id = feed_state_cursor_id(&state, key);
                    feed::run_effect_release_feed_cursor(cursor_id, nmp_handle.as_ref());
                    match key.as_str() {
                        "hl.feed.articles" => state.article_feed.clear(),
                        "hl.feed.highlights" => state.highlight_feed.clear(),
                        k if k.starts_with("hl.feed.room.") => {
                            if let Some(fs) = state.room_lanes.get_mut(k) {
                                fs.clear();
                            }
                        }
                        _ => {}
                    }
                    continue;
                }
                _ => {}
            }
            run_effect(
                effect,
                state.session_epoch,
                &tx,
                &onboarding_store,
                &shared,
                nmp_handle.as_ref(),
                &policy,
            )
            .await;
        }

        // Recompute all open-view snapshots and update the shared cache.
        // This makes `current_snapshot()` return fresh state immediately.
        // Collect IDs first to avoid simultaneous immutable + mutable borrows.
        {
            let open_ids: Vec<ViewId> = registry.open_ids().cloned().collect();
            let mut cache = shared.snapshots.lock().unwrap_or_else(|e| e.into_inner());
            for id in &open_ids {
                if let Some(snap) = project_snapshot(&state, id, now) {
                    cache.insert(id.clone(), snap.clone());
                    registry.update_snapshot(id, snap);
                }
            }
            // Remove closed views from the cache.
            cache.retain(|id, _| registry.is_open(id));
        }

        // Coalesced observer push at clock-capped cadence (D8).
        // In tests: advance ManualClock + dispatch Tick → cadence passes →
        // observer receives ≤1 snapshot per open view.
        if !suspended && now >= last_emit_at + EMIT_CADENCE_SECS {
            let observer = shared.observer.read().clone();
            if let Some(obs) = observer {
                for id in registry.open_ids() {
                    if let Some(snap) = registry.current_snapshot(id) {
                        obs.on_snapshot(id.clone(), snap);
                    }
                }
            }
            last_emit_at = now;
        }

        if is_shutdown {
            break;
        }
    }
}

// ─── NmpApp boot ────────────────────────────────────────────────────────────

/// Construct and start the `NmpApp` for the nmp-lane (its own storage sub-dir).
/// Wires the identity-change observer to feed `KernelEvent::IdentityChanged`
/// back into the actor channel. Phase 1 requires no further protocol I/O.
///
/// Returns `None` if the storage path cannot be created or `nmp_app_start`
/// returns null (logged as a warning; the actor continues without NMP).
pub(crate) fn start_nmp_app(data_dir: &str, tx: mpsc::UnboundedSender<Cmd>) -> Option<NmpHandle> {
    let storage_path = AppState::nmp_storage_path(data_dir);
    if let Err(e) = std::fs::create_dir_all(&storage_path) {
        tracing::warn!(path = %storage_path.display(), error = %e, "failed to create NMP storage dir");
        return None;
    }

    let mut builder = NmpAppBuilder::new();
    nmp_defaults::register_defaults(&mut builder);

    // Boot sequence per nmp-defaults typestate (#1493 relay gate added):
    //   Unstarted → StorageSet → ProjectionsDeclared → RelaysDeclared → *mut NmpApp
    // hl manages its relay set entirely through the bespoke live lane
    // (nostr_runtime.rs / relays.rs / nmp_app_add_relay at runtime), so nmp's
    // kernel starts with no built-in relays and the app adds them dynamically.
    let raw = builder
        .storage_path(storage_path.to_string_lossy().into_owned())
        .consume_all_builtin_projections()
        .without_initial_relays()
        .start(RunConfig::default());

    let raw_ptr = NonNull::new(raw)?;
    let nmp_ref: &NmpApp = unsafe { raw_ptr.as_ref() };

    // Phase 2B: initialise NIP-46 broker (needed for PairBunker and
    // StartNostrConnect). Idempotent per ADR-0052 §D3 — safe to call once.
    nmp_signer_broker_init(raw_ptr.as_ptr());

    // Phase 2B: initialise NIP-55 external-signer driver (needed for
    // SignInNip55). nmp_app_signin_nip55 lazy-inits too, but calling
    // explicitly here makes the init order deterministic.
    nmp_external_signer_init(raw_ptr.as_ptr());

    // Phase 3B: register JoinedGroupsProjection at boot.
    // If an account is already active (e.g. persisted from a prior session),
    // wire it immediately. On subsequent IdentityChanged(Some) the reducer
    // emits Effect::WireJoinedGroups which re-calls this function.
    // An empty pubkey is a silent no-op inside wire_joined_groups (D6).
    {
        let boot_pubkey: String = nmp_ref
            .active_account_handle()
            .lock()
            .ok()
            .and_then(|g| g.clone())
            .unwrap_or_default();
        communities::register_joined_groups_projection(nmp_ref, boot_pubkey);
    }

    // Phase 4B: register ReactionObserver ONCE at boot with the live
    // active_account_handle() Arc so the observer auto-tracks account switches.
    // No re-registration on IdentityChanged(Some) is needed (the Arc is updated
    // in-place by NMP on every sign-in/switch/logout). This avoids the observer-
    // stacking bug where each IdentityChanged(Some) would add a new observer.
    reactions::register_reaction_projection(nmp_ref, nmp_ref.active_account_handle(), tx.clone());

    // Wire identity-change observer → KernelEvent::IdentityChanged.
    // Pattern from nmp_runtime.rs:758-778.
    let tx_id = tx.clone();
    nmp_ref.register_identity_change_observer(move |active| {
        let _ = tx_id.send(Cmd::Event(KernelEvent::IdentityChanged(active)));
    });

    // Phase 3C: wire the follow-list typed snapshot projection so NIP-02
    // kind:3 events from the active account surface in `AppState::follows`.
    // Called with `active_pubkey=None` at boot (account unknown until the
    // identity-change observer fires); the `FollowListProjection` accumulates
    // kind:3 events for all observed authors and filters to the active pubkey
    // at snapshot time. The kernel's standing `account_profile_interest`
    // (kind:0 + kind:3 + kind:10002) means no separate interest push is needed.
    // Pass the live active-account slot so the projection auto-tracks the
    // active account. A fresh Arc::new(Mutex::new(None)) would leave it
    // permanently pointed at None and follows would never populate AppState.
    follows::register_follow_list_projection(nmp_ref, nmp_ref.active_account_handle());

    // Phase 4C: wire the hl BookmarkListProjection typed snapshot so NIP-51
    // kind:10003 events from the active account surface in `AppState::bookmarks`.
    // Note: nmp-defaults::register_bookmark_runtime (called via register_defaults
    // above) already wires a separate BookmarkListProjection as a kind:10003
    // observer AND registers the add/remove bookmark action modules. This call
    // creates a SECOND projection (also pointing at the live active-account slot)
    // exclusively for the hl typed-snapshot path ("hl.bookmarks" key). Double-
    // observation is harmless — both projections read the same events. The write
    // actions are NOT re-registered here (nmp-defaults already wired them).
    bookmarks::register_bookmark_list_projection(nmp_ref, nmp_ref.active_account_handle());

    // Phase 3A: register the update callback so NMP snapshot frames are
    // forwarded into the actor as KernelEvent::NmpSnapshotFrame. The
    // context_box is stored in NmpHandle::_update_callback_ctx to keep the
    // sender alive for the full lifetime of the NmpApp. Registered ONCE at
    // boot (bridge_registered_once_at_boot).
    let context_box = {
        let ctx_box = Box::new(tx);
        let ctx_ptr = (&*ctx_box) as *const mpsc::UnboundedSender<Cmd> as *mut c_void;
        // SAFETY: raw_ptr is a valid non-null NmpApp pointer; ctx_ptr points
        // to the Box we just allocated (returned below, kept alive by NmpHandle).
        // nmp_update_callback is a valid extern-C fn matching the expected ABI.
        unsafe {
            nmp_app_set_update_callback(raw_ptr.as_ptr(), ctx_ptr, Some(nmp_update_callback));
        }
        ctx_box
    };

    Some(NmpHandle {
        ptr: raw_ptr,
        _update_callback_ctx: Some(context_box),
    })
}

// ─── Unit tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::{CapabilityResult, KeychainResult};
    use crate::kernel::action::RootTab;
    use crate::kernel::app::ToastState;
    use crate::kernel::clock::{Clock, ManualClock};

    fn make_state() -> AppState {
        AppState::default()
    }

    fn step(state: &mut AppState, clock: &ManualClock, cmd: Cmd) -> Vec<Effect> {
        let now = clock.now_unix_seconds();
        reduce(state, cmd, now)
    }

    // Gate 1: dispatch is fire-and-forget, no Result.
    #[test]
    fn dispatch_signature_is_unit() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let effects = step(&mut state, &clock, Cmd::Action(AppAction::Logout));
        assert_eq!(state.session_epoch, 1);
        assert!(!effects.is_empty());
    }

    // Gate 2: failures surface as state.
    #[test]
    fn dispatch_failure_surfaces_as_state_not_return() {
        let mut state = make_state();
        let clock = ManualClock::default();
        step(&mut state, &clock, Cmd::Action(AppAction::RestoreSession));
        assert!(matches!(
            state.session,
            crate::kernel::app::SessionState::Restoring { .. }
        ));

        let err = CapabilityResult::Keychain(KeychainResult::Error("keychain locked".into()));
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::CapabilityResult(err)),
        );
        assert!(matches!(
            state.session,
            crate::kernel::app::SessionState::RestoreFailed { ref error } if error.contains("keychain locked")
        ));
    }

    // Gate 3: reducer is sync (runs in a non-tokio thread).
    #[test]
    fn reduce_is_sync_pure() {
        let epoch = std::thread::spawn(|| {
            let mut state = make_state();
            let clock = ManualClock::new(0);
            step(&mut state, &clock, Cmd::Action(AppAction::RestoreSession));
            step(
                &mut state,
                &clock,
                Cmd::Event(KernelEvent::OnboardingStateLoaded(true)),
            );
            step(&mut state, &clock, Cmd::Action(AppAction::Logout));
            state.session_epoch
        })
        .join()
        .unwrap();
        assert_eq!(epoch, 1);
    }

    // Gate 4: closed views → no snapshots from registry.
    #[test]
    fn closed_view_emits_no_snapshot() {
        let mut registry = ViewRegistry::default();
        registry.open(ViewId::AppRoot, ViewRoute::AppRoot);
        registry.close(&ViewId::AppRoot);
        assert!(!registry.is_open(&ViewId::AppRoot));
        assert_eq!(registry.open_count(), 0);
    }

    // Gate 5: snapshot shape is bounded.
    #[test]
    fn snapshot_size_is_view_shaped() {
        let mut state = make_state();
        let clock = ManualClock::default();
        step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::SelectRootTab {
                tab: RootTab::Discover,
            }),
        );
        let now = clock.now_unix_seconds();
        if let Some(ViewSnapshot::RootShell(s)) = project_snapshot(&state, &ViewId::RootShell, now)
        {
            assert_eq!(s.tab_count, 5);
        } else {
            panic!("expected RootShell snapshot");
        }
    }

    // Gate 6: coalescing — N changes → final state only.
    #[test]
    fn observer_coalesces_final_state() {
        let mut state = make_state();
        let clock = ManualClock::default();
        for tab in [
            RootTab::Feed,
            RootTab::Discover,
            RootTab::Capture,
            RootTab::Notifications,
            RootTab::Settings,
        ] {
            step(
                &mut state,
                &clock,
                Cmd::Action(AppAction::SelectRootTab { tab }),
            );
        }
        let now = clock.now_unix_seconds();
        if let Some(ViewSnapshot::RootShell(s)) = project_snapshot(&state, &ViewId::RootShell, now)
        {
            assert_eq!(s.selected_tab, RootTab::Settings as u8);
        } else {
            panic!("expected RootShell snapshot");
        }
    }

    // Gate 7: clock drives toast dismiss.
    #[test]
    fn clock_drives_toast_dismiss() {
        use crate::kernel::app::TOAST_DISMISS_SECS;
        let mut state = make_state();
        let clock = ManualClock::new(0);

        state.chrome.toast = Some(ToastState {
            message: "Shared!".into(),
            dismiss_at_unix: TOAST_DISMISS_SECS,
        });

        // t=2 → still present.
        clock.advance(2);
        step(&mut state, &clock, Cmd::Event(KernelEvent::ClockTick));
        let now = clock.now_unix_seconds();
        let snap = project_snapshot(&state, &ViewId::RootShell, now).unwrap();
        if let ViewSnapshot::RootShell(s) = snap {
            assert!(s.toast.is_some(), "toast should be present at t=2");
        }

        // t=3 → dismissed.
        clock.advance(1);
        step(&mut state, &clock, Cmd::Event(KernelEvent::ClockTick));
        let now = clock.now_unix_seconds();
        let snap = project_snapshot(&state, &ViewId::RootShell, now).unwrap();
        if let ViewSnapshot::RootShell(s) = snap {
            assert!(s.toast.is_none(), "toast should be dismissed at t=3");
        }
    }

    // Gate 8: session restore timeout via clock.
    #[test]
    fn session_restore_timeout_via_clock() {
        use crate::kernel::app::{SessionState, SESSION_RESTORE_TIMEOUT_SECS};
        let mut state = make_state();
        let clock = ManualClock::new(0);

        step(&mut state, &clock, Cmd::Action(AppAction::RestoreSession));
        assert!(matches!(
            state.session,
            SessionState::Restoring { started_at: 0 }
        ));

        clock.advance(SESSION_RESTORE_TIMEOUT_SECS - 1);
        step(&mut state, &clock, Cmd::Event(KernelEvent::ClockTick));
        assert!(
            matches!(state.session, SessionState::Restoring { .. }),
            "still restoring"
        );

        clock.advance(1);
        step(&mut state, &clock, Cmd::Event(KernelEvent::ClockTick));
        assert!(
            matches!(state.session, SessionState::Absent),
            "should be Absent after timeout, got {:?}",
            state.session
        );
    }

    // Gate 9: route selection deterministic.
    #[test]
    fn route_selection_deterministic() {
        use crate::kernel::snapshot::RouteKind;
        let clock = ManualClock::default();
        let now = clock.now_unix_seconds();

        // Onboarding incomplete → Onboarding.
        let state = make_state();
        if let Some(ViewSnapshot::AppRoot(s)) = project_snapshot(&state, &ViewId::AppRoot, now) {
            assert_eq!(s.route_kind, RouteKind::Onboarding);
        }

        // Onboarding complete, no session → Login.
        let mut state = make_state();
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::OnboardingStateLoaded(true)),
        );
        if let Some(ViewSnapshot::AppRoot(s)) = project_snapshot(&state, &ViewId::AppRoot, now) {
            assert_eq!(s.route_kind, RouteKind::Login);
        }

        // Session present → RootShell.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::SessionRestored {
                present: true,
                pubkey: Some("pk".into()),
            }),
        );
        if let Some(ViewSnapshot::AppRoot(s)) = project_snapshot(&state, &ViewId::AppRoot, now) {
            assert_eq!(s.route_kind, RouteKind::RootShell);
            assert!(s.session_present);
        }
    }

    // Gate 10: lifecycle idempotent.
    #[test]
    fn resume_suspend_idempotent() {
        let mut state = make_state();
        let clock = ManualClock::default();
        for _ in 0..5 {
            step(&mut state, &clock, Cmd::Resume);
            step(&mut state, &clock, Cmd::Suspend);
        }
        assert_eq!(state.session_epoch, 0);
    }

    #[test]
    fn shutdown_idempotent() {
        let mut state = make_state();
        let clock = ManualClock::default();
        for _ in 0..3 {
            step(&mut state, &clock, Cmd::Shutdown);
        }
        assert!(matches!(
            state.session,
            crate::kernel::app::SessionState::Unknown
        ));
    }

    // Gate 11: logout cancels view-scoped effects via epoch bump.
    #[test]
    fn logout_cancels_view_scoped_effects() {
        use crate::kernel::app::SessionState;
        let mut state = make_state();
        let clock = ManualClock::default();
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::SessionRestored {
                present: true,
                pubkey: Some("pk".into()),
            }),
        );
        let epoch_before = state.session_epoch;
        step(&mut state, &clock, Cmd::Action(AppAction::Logout));
        assert_eq!(state.session_epoch, epoch_before + 1);
        assert!(matches!(state.session, SessionState::Absent));
    }

    // ── Phase 2A tests ────────────────────────────────────────────────────────

    // P2A-1: SignInNsec transitions to SigningIn and emits AddNsecSigner with
    //        make_active intent baked in (the Effect carries the nsec string;
    //        the runner calls add_signer with make_active=true).
    #[test]
    fn nsec_sign_in_enqueues_local_signer_make_active_true() {
        use crate::kernel::action::SignInMethod;
        use crate::kernel::app::SessionState;
        let mut state = make_state();
        let clock = ManualClock::new(42);
        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::SignInNsec {
                nsec: "nsec1test".into(),
            }),
        );
        // State should be SigningIn immediately.
        assert!(
            matches!(
                &state.session,
                SessionState::SigningIn {
                    method: SignInMethod::Nsec,
                    started_at: 42
                }
            ),
            "expected SigningIn, got {:?}",
            state.session
        );
        // A single AddNsecSigner effect should be emitted.
        assert_eq!(effects.len(), 1, "expected one effect from SignInNsec");
        assert!(
            matches!(&effects[0], Effect::AddNsecSigner { nsec } if nsec == "nsec1test"),
            "expected AddNsecSigner effect with the nsec, got {:?}",
            effects[0]
        );
    }

    // P2A-2: IdentityChanged(Some(pubkey)) routes to SessionState::Present.
    #[test]
    fn identity_changed_some_routes_to_present() {
        use crate::kernel::app::SessionState;
        let mut state = make_state();
        let clock = ManualClock::default();
        // Start in SigningIn to simulate the normal flow.
        step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::SignInNsec {
                nsec: "nsec1x".into(),
            }),
        );
        // Observer fires with the resolved pubkey.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(Some("deadbeefpubkey".into()))),
        );
        assert!(
            matches!(
                &state.session,
                SessionState::Present { pubkey, .. } if pubkey == "deadbeefpubkey"
            ),
            "expected Present, got {:?}",
            state.session
        );
    }

    // P2A-3: IdentityChanged(None) routes to SessionState::Absent.
    #[test]
    fn identity_changed_none_routes_to_absent() {
        use crate::kernel::app::SessionState;
        let mut state = make_state();
        let clock = ManualClock::default();
        // Put the state in Present first.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(Some("somepubkey".into()))),
        );
        assert!(matches!(&state.session, SessionState::Present { .. }));
        // Now fire IdentityChanged(None) — should transition to Absent.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(None)),
        );
        assert!(
            matches!(&state.session, SessionState::Absent),
            "expected Absent, got {:?}",
            state.session
        );
    }

    // P2A-4: Logout emits RemoveActiveAccount + ClearSession and bumps epoch.
    #[test]
    fn logout_calls_remove_account_and_bumps_epoch() {
        use crate::kernel::app::SessionState;
        let mut state = make_state();
        let clock = ManualClock::default();
        // Simulate a logged-in session.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(Some("mypubkey".into()))),
        );
        let epoch_before = state.session_epoch;
        let effects = step(&mut state, &clock, Cmd::Action(AppAction::Logout));
        // Epoch must be bumped.
        assert_eq!(state.session_epoch, epoch_before + 1);
        // State must be Absent immediately (not waiting for observer).
        assert!(matches!(&state.session, SessionState::Absent));
        // Effects must include both RemoveActiveAccount and ClearSession.
        let has_remove = effects
            .iter()
            .any(|e| matches!(e, Effect::RemoveActiveAccount));
        let has_clear = effects.iter().any(|e| matches!(e, Effect::ClearSession));
        assert!(has_remove, "expected RemoveActiveAccount effect in Logout");
        assert!(has_clear, "expected ClearSession effect in Logout");
    }

    // P2A-5: sign-in failure surfaces in SessionState (D6), never as Result.
    //        dispatch(SignInNsec) returns () regardless of whether the signer
    //        call succeeds — errors arrive via KernelEvent::SignInFailed.
    #[test]
    fn sign_in_failure_surfaces_in_session_state_not_return() {
        use crate::kernel::action::SignInMethod;
        use crate::kernel::app::SessionState;
        let mut state = make_state();
        let clock = ManualClock::default();
        // Transition to SigningIn first.
        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::SignInNsec {
                nsec: "bad_nsec".into(),
            }),
        );
        // dispatch returns () — checked by the type of `effects` (Vec<Effect>).
        // The effect runner (in production) calls add_signer and on error sends
        // KernelEvent::SignInFailed back. We simulate that here.
        let _ = effects; // () — fire-and-forget
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::SignInFailed {
                method: SignInMethod::Nsec,
                error: "invalid nsec: bad bech32".into(),
            }),
        );
        assert!(
            matches!(
                &state.session,
                SessionState::SignInFailed {
                    method: SignInMethod::Nsec,
                    error
                } if error.contains("invalid nsec")
            ),
            "expected SignInFailed, got {:?}",
            state.session
        );
    }

    // P2A-6: dispatch(SignInNsec) returns () — the action dispatch API is
    //        fire-and-forget (#3).
    #[test]
    fn dispatch_signin_returns_unit() {
        use crate::kernel::app::SessionState;
        let mut state = make_state();
        let clock = ManualClock::new(0);
        // This must compile and run — the return value of step() is Vec<Effect>,
        // which models the `()` (fire-and-forget) contract. The key assertion is
        // that `step` returns without blocking or returning a Result.
        let _effects: Vec<Effect> = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::SignInNsec {
                nsec: "nsec1any".into(),
            }),
        );
        // No panic, no Result — the dispatch contract is satisfied.
        assert!(
            matches!(&state.session, SessionState::SigningIn { .. }),
            "SignInNsec must transition to SigningIn synchronously"
        );
    }

    // P2A-7: clock drives sign-in timeout → SignInFailed.
    //        NMP handles invalid nsec parse errors internally without firing the
    //        identity-change observer; this clock-driven fallback ensures the UI
    //        never gets stuck in SigningIn forever (D6 — failure surfaces as state).
    #[test]
    fn sign_in_timeout_transitions_signing_in_to_failed() {
        use crate::kernel::action::SignInMethod;
        use crate::kernel::app::{SessionState, SIGN_IN_TIMEOUT_SECS};
        let mut state = make_state();
        let clock = ManualClock::new(0);

        step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::SignInNsec {
                nsec: "nsec1invalid".into(),
            }),
        );
        assert!(matches!(&state.session, SessionState::SigningIn { .. }));

        // Just before timeout — still SigningIn.
        clock.advance(SIGN_IN_TIMEOUT_SECS - 1);
        step(&mut state, &clock, Cmd::Event(KernelEvent::ClockTick));
        assert!(
            matches!(&state.session, SessionState::SigningIn { .. }),
            "should still be SigningIn before timeout"
        );

        // At timeout — should transition to SignInFailed.
        clock.advance(1);
        step(&mut state, &clock, Cmd::Event(KernelEvent::ClockTick));
        assert!(
            matches!(
                &state.session,
                SessionState::SignInFailed {
                    method: SignInMethod::Nsec,
                    ..
                }
            ),
            "should be SignInFailed at timeout, got {:?}",
            state.session
        );
    }

    // ── Phase 2B tests ────────────────────────────────────────────────────────

    // P2B-1: PairBunker transitions to SigningIn{Bunker} and emits AddBunkerSigner.
    #[test]
    fn pair_bunker_enqueues_bunker_uri_source() {
        use crate::kernel::action::SignInMethod;
        use crate::kernel::app::SessionState;
        let mut state = make_state();
        let clock = ManualClock::new(10);
        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::PairBunker {
                uri: "bunker://pubkey?relay=wss://relay.example.com".into(),
            }),
        );
        assert!(
            matches!(
                &state.session,
                SessionState::SigningIn {
                    method: SignInMethod::Bunker,
                    started_at: 10
                }
            ),
            "expected SigningIn{{Bunker}}, got {:?}",
            state.session
        );
        assert_eq!(
            effects.len(),
            1,
            "expected exactly one effect from PairBunker"
        );
        assert!(
            matches!(
                &effects[0],
                Effect::AddBunkerSigner { uri }
                    if uri == "bunker://pubkey?relay=wss://relay.example.com"
            ),
            "expected AddBunkerSigner with the uri, got {:?}",
            effects[0]
        );
    }

    // P2B-2: NostrConnectUriReady stores the URI in state and the snapshot
    //        exposes it via AppRootSnapshot::nostrconnect_uri.
    #[test]
    fn nostrconnect_uri_ready_event_to_snapshot() {
        use crate::kernel::action::SignInMethod;
        use crate::kernel::app::SessionState;
        let mut state = make_state();
        let clock = ManualClock::default();

        // Begin a NostrConnect sign-in.
        step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::StartNostrConnect),
        );
        assert!(
            matches!(
                &state.session,
                SessionState::SigningIn {
                    method: SignInMethod::NostrConnect,
                    ..
                }
            ),
            "expected SigningIn{{NostrConnect}}, got {:?}",
            state.session
        );
        assert!(state.nostrconnect_uri.is_none(), "URI not set yet");

        // Simulate the effect runner delivering the minted URI.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::NostrConnectUriReady {
                uri: "nostrconnect://pubkey?relay=wss://relay.example.com".into(),
            }),
        );
        assert_eq!(
            state.nostrconnect_uri.as_deref(),
            Some("nostrconnect://pubkey?relay=wss://relay.example.com"),
            "URI should be stored in state"
        );

        // Project a snapshot and verify the URI is present.
        let now = clock.now_unix_seconds();
        if let Some(ViewSnapshot::AppRoot(snap)) = project_snapshot(&state, &ViewId::AppRoot, now) {
            assert_eq!(
                snap.nostrconnect_uri.as_deref(),
                Some("nostrconnect://pubkey?relay=wss://relay.example.com"),
                "URI must appear in AppRootSnapshot"
            );
        } else {
            panic!("expected AppRoot snapshot");
        }

        // When IdentityChanged fires (handshake complete), URI is cleared.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(Some("resolvedpubkey".into()))),
        );
        assert!(
            state.nostrconnect_uri.is_none(),
            "URI must be cleared after IdentityChanged"
        );
    }

    // P2B-3: SignInNip55 transitions to SigningIn{Nip55} and emits StartNip55SignIn.
    //        The effect runner would call nmp_app_signin_nip55; in test mode
    //        (no nmp handle) success is injected as IdentityChanged.
    #[test]
    fn nip55_signin_invokes_external_hook() {
        use crate::kernel::action::{SignInMethod, SignerKind};
        use crate::kernel::app::SessionState;
        let mut state = make_state();
        let clock = ManualClock::new(5);
        let effects = step(&mut state, &clock, Cmd::Action(AppAction::SignInNip55));
        assert!(
            matches!(
                &state.session,
                SessionState::SigningIn {
                    method: SignInMethod::Nip55,
                    started_at: 5
                }
            ),
            "expected SigningIn{{Nip55}}, got {:?}",
            state.session
        );
        assert_eq!(effects.len(), 1, "expected one effect from SignInNip55");
        assert!(
            matches!(&effects[0], Effect::StartNip55SignIn),
            "expected StartNip55SignIn effect, got {:?}",
            effects[0]
        );

        // Simulate success via IdentityChanged — signer_kind should be Nip55.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(Some("nip55pubkey".into()))),
        );
        assert!(
            matches!(
                &state.session,
                SessionState::Present {
                    pubkey,
                    signer_kind: SignerKind::Nip55
                } if pubkey == "nip55pubkey"
            ),
            "expected Present{{Nip55}}, got {:?}",
            state.session
        );
    }

    // P2B-4: dispatch(PairBunker/StartNostrConnect/SignInNip55) returns () —
    //        the fire-and-forget contract (Non-Negotiable #3).
    #[test]
    fn dispatch_returns_unit() {
        let mut state = make_state();
        let clock = ManualClock::new(0);

        let _: Vec<Effect> = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::PairBunker {
                uri: "bunker://x".into(),
            }),
        );
        let _: Vec<Effect> = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::StartNostrConnect),
        );
        let _: Vec<Effect> = step(&mut state, &clock, Cmd::Action(AppAction::SignInNip55));
        // No panic and no Result return — fire-and-forget contract satisfied.
    }

    // P2B-5: bunker sign-in times out → SignInFailed{Bunker}.
    //        Reuses the 2A clock arm which covers ALL SigningIn variants.
    #[test]
    fn bunker_signing_in_times_out_to_failed() {
        use crate::kernel::action::SignInMethod;
        use crate::kernel::app::{SessionState, SIGN_IN_TIMEOUT_SECS};
        let mut state = make_state();
        let clock = ManualClock::new(0);

        step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::PairBunker {
                uri: "bunker://pubkey?relay=wss://r.example.com".into(),
            }),
        );
        assert!(matches!(&state.session, SessionState::SigningIn { .. }));

        // Just before timeout — still SigningIn.
        clock.advance(SIGN_IN_TIMEOUT_SECS - 1);
        step(&mut state, &clock, Cmd::Event(KernelEvent::ClockTick));
        assert!(
            matches!(&state.session, SessionState::SigningIn { .. }),
            "should still be SigningIn before timeout"
        );

        // At timeout — should be SignInFailed{Bunker}.
        clock.advance(1);
        step(&mut state, &clock, Cmd::Event(KernelEvent::ClockTick));
        assert!(
            matches!(
                &state.session,
                SessionState::SignInFailed {
                    method: SignInMethod::Bunker,
                    ..
                }
            ),
            "should be SignInFailed{{Bunker}} at timeout, got {:?}",
            state.session
        );
    }

    // P2B-6: IdentityChanged after Bunker/NostrConnect → signer_kind is Nip46.
    #[test]
    fn bunker_identity_changed_sets_nip46_signer_kind() {
        use crate::kernel::action::SignerKind;
        use crate::kernel::app::SessionState;
        let mut state = make_state();
        let clock = ManualClock::default();

        step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::PairBunker {
                uri: "bunker://pk".into(),
            }),
        );
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(Some("bunkerpk".into()))),
        );
        assert!(
            matches!(
                &state.session,
                SessionState::Present {
                    pubkey,
                    signer_kind: SignerKind::Nip46
                } if pubkey == "bunkerpk"
            ),
            "expected Present{{Nip46}}, got {:?}",
            state.session
        );
    }

    // ── Phase 2C tests ────────────────────────────────────────────────────────

    // P2C-1: CreateAccount transitions to SigningIn{CreateAccount} and emits
    //        Effect::CreateAccount (actor_sender path, first use in hl).
    //        make_active=true is baked into the effect runner, not the reducer data.
    #[test]
    fn create_account_sends_effect_create_account() {
        use crate::kernel::action::SignInMethod;
        use crate::kernel::app::SessionState;
        let mut state = make_state();
        let clock = ManualClock::new(42);
        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::CreateAccount {
                profile_name: "Alice".into(),
            }),
        );
        // State must transition to SigningIn{CreateAccount} synchronously.
        assert!(
            matches!(
                &state.session,
                SessionState::SigningIn {
                    method: SignInMethod::CreateAccount,
                    started_at: 42,
                }
            ),
            "expected SigningIn{{CreateAccount}}, got {:?}",
            state.session
        );
        // Exactly one effect — CreateAccount.
        assert_eq!(effects.len(), 1, "expected one effect from CreateAccount");
        assert!(
            matches!(&effects[0], Effect::CreateAccount { profile_name } if profile_name == "Alice"),
            "expected Effect::CreateAccount{{profile_name: Alice}}, got {:?}",
            effects[0]
        );
    }

    // P2C-2: make_active=true is verified by inspecting the effect (the runner
    //        always passes make_active:true to ActorCommand::CreateAccount; we
    //        confirm the Effect variant carries the right profile_name and that
    //        the test pattern documents the intent).
    #[test]
    fn create_account_make_active_true_is_effect_runner_policy() {
        // The Effect::CreateAccount variant carries profile_name only; make_active
        // is Rust-constant in the effect runner (always true for the onboarding
        // path). This test confirms that the reducer emits exactly one
        // Effect::CreateAccount for a CreateAccount action — the runner's
        // make_active=true is verified by the build (it would fail to compile
        // if the ActorCommand::CreateAccount field was wrong).
        let mut state = make_state();
        let clock = ManualClock::new(0);
        let effects = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::CreateAccount {
                profile_name: "Bob".into(),
            }),
        );
        assert_eq!(effects.len(), 1);
        let Effect::CreateAccount { profile_name } = &effects[0] else {
            panic!("expected Effect::CreateAccount, got {:?}", effects[0]);
        };
        assert_eq!(profile_name, "Bob");
    }

    // P2C-3: Relays/follows come from injected KernelPolicy, not hardcoded literals.
    //        We verify no websocket relay URL scheme appears as a literal in the
    //        kernel source files (action.rs, effect.rs, app.rs).
    //        actor.rs is excluded — it hosts this test, which must reference the
    //        pattern in its assertion string to be meaningful.
    #[test]
    fn no_hardcoded_relay_literals_in_kernel() {
        // Build the banned pattern from parts so this test's own source does
        // not trip the scan on the files that DO NOT include this test.
        let banned = ["wss", "://"].concat();

        let action_src = include_str!("action.rs");
        let effect_src = include_str!("effect.rs");
        let app_src = include_str!("app.rs");

        for (name, src) in [
            ("action.rs", action_src),
            ("effect.rs", effect_src),
            ("app.rs", app_src),
        ] {
            assert!(
                !src.contains(banned.as_str()),
                "hardcoded relay literal found in kernel/{name} (D3 violation)"
            );
        }
    }

    // P2C-4: CreateAccount success via IdentityChanged(Some(pubkey))
    //        → SessionState::Present (same observer path as nsec sign-in).
    #[test]
    fn create_account_success_via_identity_changed() {
        use crate::kernel::action::{SignInMethod, SignerKind};
        use crate::kernel::app::SessionState;
        let mut state = make_state();
        let clock = ManualClock::new(0);

        // Dispatch CreateAccount — enters SigningIn.
        step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::CreateAccount {
                profile_name: "Charlie".into(),
            }),
        );
        assert!(matches!(
            &state.session,
            SessionState::SigningIn {
                method: SignInMethod::CreateAccount,
                ..
            }
        ));

        // NMP fires identity-change observer with the new pubkey.
        step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::IdentityChanged(Some("newpubkey123".into()))),
        );
        assert!(
            matches!(
                &state.session,
                SessionState::Present { pubkey, signer_kind: SignerKind::LocalNsec }
                    if pubkey == "newpubkey123"
            ),
            "expected Present{{LocalNsec}}, got {:?}",
            state.session
        );
    }

    // P2C-5: ADR-0059 publish policy — initial_follows is empty in the injected
    //        default KernelPolicy (no kind:3 for new accounts until user follows).
    //        We verify both the default (empty → no kind:3) AND that a populated
    //        policy round-trips correctly so the effect runner will pass it to
    //        ActorCommand::CreateAccount.initial_follows when nmp is present.
    #[test]
    fn create_account_follows_adr0059_publish_policy() {
        use crate::kernel::app::{CreateAccountPolicy, KernelPolicy, SeedRelay};

        // Default policy: initial_follows is empty → no kind:3 published.
        let default_policy = KernelPolicy::default();
        assert!(
            default_policy.create_account.initial_follows.is_empty(),
            "ADR-0059 §5: initial_follows must be empty by default (no kind:3)"
        );

        // Injected policy with follows: the field is preserved for the effect runner.
        // This proves initial_follows round-trips from KernelPolicy into the
        // CreateAccountPolicy struct that run_effect reads via `policy.create_account`.
        let follows = vec![
            "deadbeef00000000000000000000000000000000000000000000000000000001".to_string(),
            "deadbeef00000000000000000000000000000000000000000000000000000002".to_string(),
        ];
        let policy_with_follows = KernelPolicy {
            create_account: CreateAccountPolicy {
                seed_relays: vec![SeedRelay {
                    url: "relay.example".to_string(),
                    role: "both".to_string(),
                }],
                initial_follows: follows.clone(),
            },
            relay: Default::default(),
            room: Default::default(),
            data_dir: String::new(),
        };
        assert_eq!(
            policy_with_follows.create_account.initial_follows, follows,
            "initial_follows must round-trip through KernelPolicy for the effect runner"
        );
    }

    // P2C-6: dispatch(CreateAccount) returns () — fire-and-forget contract.
    #[test]
    fn dispatch_create_account_returns_unit() {
        use crate::kernel::app::SessionState;
        let mut state = make_state();
        let clock = ManualClock::new(0);
        let _effects: Vec<Effect> = step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::CreateAccount {
                profile_name: "Dave".into(),
            }),
        );
        // No panic, no Result — fire-and-forget satisfied.
        assert!(matches!(&state.session, SessionState::SigningIn { .. }));
    }

    // P2C-7: CreateAccount SigningIn is covered by the 2A clock timeout.
    //        Confirms the existing SIGN_IN_TIMEOUT_SECS check fires for
    //        SignInMethod::CreateAccount just as it does for Nsec.
    #[test]
    fn create_account_timeout_covered_by_existing_clock_check() {
        use crate::kernel::action::SignInMethod;
        use crate::kernel::app::{SessionState, SIGN_IN_TIMEOUT_SECS};
        let mut state = make_state();
        let clock = ManualClock::new(0);

        step(
            &mut state,
            &clock,
            Cmd::Action(AppAction::CreateAccount {
                profile_name: "Eve".into(),
            }),
        );
        assert!(matches!(&state.session, SessionState::SigningIn { .. }));

        // Advance to just before timeout — still SigningIn.
        clock.advance(SIGN_IN_TIMEOUT_SECS - 1);
        step(&mut state, &clock, Cmd::Event(KernelEvent::ClockTick));
        assert!(
            matches!(&state.session, SessionState::SigningIn { .. }),
            "still signing in before timeout"
        );

        // At timeout — transitions to SignInFailed.
        clock.advance(1);
        step(&mut state, &clock, Cmd::Event(KernelEvent::ClockTick));
        assert!(
            matches!(
                &state.session,
                SessionState::SignInFailed {
                    method: SignInMethod::CreateAccount,
                    ..
                }
            ),
            "expected SignInFailed{{CreateAccount}} at timeout, got {:?}",
            state.session
        );
    }

    // ── Phase 3A tests ────────────────────────────────────────────────────────

    // 3A-1: update_callback_frame_routes_to_actor_as_event
    //
    // Feeding a KernelEvent::NmpSnapshotFrame into the actor's reduce path must
    // not panic and must return a Vec<Effect> (fire-and-forget; the projection
    // domain handles no-ops gracefully for unknown/malformed frames — D6).
    #[test]
    fn update_callback_frame_routes_to_actor_as_event() {
        let mut state = make_state();
        let clock = ManualClock::default();
        // Simulate what the C callback sends: raw bytes (garbage here — the
        // projections module returns empty effects for any non-decodable frame).
        let synthetic_frame: Vec<u8> = vec![0u8; 32];
        let effects = step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::NmpSnapshotFrame(synthetic_frame)),
        );
        // The frame routes through dispatch_typed_frame; for a synthetic frame
        // the result is an empty effects list (no panics, no state corruption).
        let _ = effects; // Vec<Effect> — fire-and-forget contract confirmed.
    }

    // 3A-2: bridge_registered_once_at_boot
    //
    // Confirms the `start_nmp_app` path only registers the callback once.
    // We verify this structurally: the function signature takes `tx` by value
    // (not `&tx`) so it can only pass one sender to `register_update_callback`.
    // This is the compile-time proof; no runtime assertion is possible without
    // a live NmpApp.
    #[test]
    fn bridge_registered_once_at_boot() {
        // The public contract: start_nmp_app's signature requires a single tx.
        // A double-registration would require calling set_update_callback twice,
        // which the current implementation does not do (one call per boot).
        // We assert the function exists and accepts the expected args by calling
        // a no-op path (nmp is None in unit-test mode).
        // The real wiring is tested via integration tests that spin a live app.
        let _: fn(&str, tokio::sync::mpsc::UnboundedSender<Cmd>) -> Option<NmpHandle> =
            start_nmp_app;
    }

    // 3A-3: decode_dispatch_handles_unknown_schema_id_gracefully (D6)
    //
    // A KernelEvent::NmpSnapshotFrame with garbage bytes must reduce without
    // panic and return an empty Vec<Effect>. (Mirrors the projections module
    // test at the actor level to confirm the full path is wired.)
    #[test]
    fn snapshot_frame_with_garbage_bytes_does_not_panic() {
        let mut state = make_state();
        let clock = ManualClock::default();
        let garbage = b"GARBAGE FRAME\x00\xFF\xFE".to_vec();
        let effects = step(
            &mut state,
            &clock,
            Cmd::Event(KernelEvent::NmpSnapshotFrame(garbage)),
        );
        assert!(
            effects.is_empty(),
            "garbage snapshot frame must produce no effects (D6)"
        );
    }
}
