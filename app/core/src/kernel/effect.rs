//! Effect types — what the actor asks the effect runner to do after a
//! reduce pass. Effects are idempotent and cancellable by view/session-epoch
//! (plan line 157; Non-Negotiable #4).

use crate::capabilities::CapabilityRequest;

/// An instruction from the reducer to the async effect runner.
///
/// Effects are pure data — the reducer never `.await`s anything
/// (Non-Negotiable #2 / plan line 156). The actor's tokio side executes
/// each effect and feeds results back as `KernelEvent`s.
///
/// Append-only: new variants at the bottom keep rebases mechanical.
#[derive(Debug, Clone)]
pub enum Effect {
    /// Read `OnboardingStore::is_complete()` and feed back
    /// `KernelEvent::OnboardingStateLoaded(bool)`.
    LoadOnboardingFlag,
    /// Ask the native shell for the persisted session secret by emitting
    /// `CapabilityRequest::Keychain(KeychainOp::LoadSession)`.
    RestoreSessionSecret,
    /// Ask the native shell to delete the persisted session secret.
    ClearSession,
    /// Forward a capability request to the registered observer.
    EmitCapabilityRequest(CapabilityRequest),

    // ── Phase 2A additions (append-only) ─────────────────────────────────────
    /// Call `nmp.add_signer(LocalNsec(nsec), make_active: true)`.
    ///
    /// NMP auto-persists the nsec to its keyring when `make_active` is true
    /// and the source is `LocalNsec` — hl does NOT separately store the nsec
    /// after this call. Success is signalled by the identity-change observer
    /// firing `KernelEvent::IdentityChanged(Some(pubkey))`. Errors are fed
    /// back as `KernelEvent::SignInFailed` (never as a `Result`).
    AddNsecSigner { nsec: String },

    /// Read the active pubkey from `nmp.active_account_handle()` then call
    /// `nmp.remove_account(pubkey)`. Fire-and-forget — success is observed
    /// via `KernelEvent::IdentityChanged(None)`.
    RemoveActiveAccount,

    // ── Phase 2B additions (append-only) ─────────────────────────────────────
    /// Call `nmp.add_signer(BunkerUri(uri), make_active: true)` which routes
    /// through the NIP-46 broker state machine. Requires
    /// `nmp_signer_broker_init` to have run at boot. Fire-and-forget: the
    /// broker resolves the signer async; success arrives as
    /// `KernelEvent::IdentityChanged(Some(pubkey))`.
    AddBunkerSigner { uri: String },
    /// Call `nmp_app_nostrconnect_uri` to mint a fresh `nostrconnect://` URI.
    /// The raw URI is fed back as `KernelEvent::NostrConnectUriReady` so the
    /// snapshot can expose it to the iOS QR sheet. The broker then awaits the
    /// remote signer to connect; success arrives as `IdentityChanged(Some)`.
    MintNostrConnectUri,
    /// Call `nmp_app_signin_nip55` to begin a NIP-55 external-signer sign-in.
    /// Fire-and-forget: the host capability bridge exchanges with the signer
    /// app async; success arrives as `KernelEvent::IdentityChanged(Some)`.
    StartNip55SignIn,

    // ── Phase 2C additions (append-only) ─────────────────────────────────────
    /// Call `nmp.actor_sender().send(ActorCommand::CreateAccount{...})`.
    ///
    /// Profile metadata, relays, and initial_follows come from the kernel's
    /// injected `KernelPolicy` — never from hardcoded literals (D3). Bootstrap
    /// publish semantics follow ADR-0059: kind:0 and kind:10002 are published;
    /// kind:3 is skipped when `initial_follows` is empty. Fire-and-forget:
    /// success arrives via `KernelEvent::IdentityChanged(Some(pubkey))`.
    /// The 2A clock-driven timeout (SIGN_IN_TIMEOUT_SECS) covers SigningIn.
    CreateAccount {
        /// Display name for the fresh account's kind:0 profile.
        profile_name: String,
    },

    // ── Phase 2D additions (append-only) ─────────────────────────────────────
    /// Call `nmp.actor_sender().send(ActorCommand::AddRelay { url, role })`.
    ///
    /// `role` is the canonical wire string produced by `RelayRole::normalize()`
    /// (e.g. `"both,indexer"`). Fire-and-forget: nmp updates the active
    /// account's kind:10002 relay list asynchronously. D3: no wss-scheme literals
    /// in the kernel — the URL comes from the caller, the role from the
    /// normalized `RelayRole` variant.
    AddRelay { url: String, role: String },
    /// Call `nmp.actor_sender().send(ActorCommand::RemoveRelay { url })`.
    ///
    /// Fire-and-forget: nmp removes the relay from the active account's
    /// kind:10002 list. D6: no-op if the relay is not present.
    RemoveRelay { url: String },
    /// Change role: equivalent to a `RemoveRelay` followed by `AddRelay` in
    /// nmp's T66a relay-edit model. Implemented by the effect runner as a
    /// single `ActorCommand::AddRelay` with the new role (nmp upserts).
    /// Fire-and-forget. D3: no wss-scheme literals.
    SetRelayRole { url: String, role: String },
    /// Sign and publish a kind:30078 app-data event via
    /// `ActorCommand::PublishRawEvent`.
    ///
    /// Used to persist the hl rooms relay list under the hl-owned d-tag
    /// `"com.highlighter.relays"`. The kernel builds the JSON `content` and
    /// the `["d", "com.highlighter.relays"]` tag; the active signer signs it
    /// through nmp's standard publish path. Fire-and-forget: nmp handles relay
    /// routing via `PublishTarget::Auto` (NIP-65 outbox; D3). No wss-scheme
    /// literals in the kernel — relay URLs are embedded inside `content` only.
    PublishRoomsRelayList {
        /// JSON-serialized rooms relay list to embed in the event content.
        content: String,
    },

    // ── Phase 3B additions (append-only) ─────────────────────────────────────
    /// Call `nmp_nip29::register::wire_joined_groups(nmp_ref, pubkey, "")`.
    ///
    /// Registers (or re-registers) the `JoinedGroupsProjection` event observer
    /// and typed snapshot closure under `"nmp.nip29.joined_groups"`. Must be
    /// emitted at boot (via `start_nmp_app`) and on every
    /// `IdentityChanged(Some(pubkey))` so the projection follows account switches.
    /// Fire-and-forget: the snapshot update arrives via the NMP update callback
    /// as `KernelEvent::NmpSnapshotFrame` on the next projection tick.
    WireJoinedGroups {
        /// Hex pubkey of the account whose joined groups to project.
        pubkey: String,
    },

    // ── Phase 3C additions (append-only) ─────────────────────────────────────
    /// Call `nmp_app_dispatch_action` with `"nmp.follow"` or `"nmp.unfollow"`
    /// namespace and `{"pubkey":"<hex>"}` JSON. Fire-and-forget (D6, Non-
    /// Negotiable #3): the updated follow list arrives back through the
    /// `FollowListUpdated` projection event (via the NMP update callback).
    ///
    /// The `nmp.follow` / `nmp.unfollow` action namespaces (via
    /// `nmp_nip02::FollowModule` / `UnfollowModule`) enqueue
    /// `ActorCommand::Follow` / `Unfollow` on the nmp actor thread which
    /// rebuilds + re-publishes kind:3.
    DispatchFollowAction {
        /// `true` → `"nmp.follow"` namespace; `false` → `"nmp.unfollow"`.
        follow: bool,
        /// Raw 64-char lowercase hex pubkey to follow or unfollow.
        pubkey: String,
    },

    // ── Phase 3E additions (append-only) ─────────────────────────────────────
    /// Call `nmp_app_dispatch_action` with the given namespace and JSON payload.
    ///
    /// Used for NIP-29 write/subscribe actions (discover, join, create, etc.).
    /// Fire-and-forget (D6): the returned correlation_id JSON is freed and
    /// discarded. Results arrive via the relevant `KernelEvent::*Updated` event.
    DispatchNip29Action {
        /// NIP-29 action namespace (e.g. `"nmp.nip29.discover"`).
        namespace: String,
        /// JSON payload for the action (e.g. `{"relay_url":"..."}`).
        json: String,
    },
    /// Wire the `DiscoveredGroupsProjection` event observer + typed snapshot
    /// projection for `relay_url` into the live `NmpApp`.
    ///
    /// Called when `AppAction::StartRoomDiscovery` is dispatched. Registers the
    /// observer that accumulates kind:39000/39001/39002 events from the relay.
    /// Fire-and-forget: the snapshot arrives via the NMP update callback as
    /// `KernelEvent::NmpSnapshotFrame` on the next projection tick.
    WireGroupDiscovery {
        /// The discovery relay URL (opaque string; kernel never constructs URLs, D3).
        relay_url: String,
    },

    // ── Phase 3D additions (append-only) ─────────────────────────────────────
    /// Call `nmp_app_claim_profile(raw_ptr, pubkey, "hl.profile.<pubkey>",
    /// force:0, liveness:Live)`.
    ///
    /// Sent when the UI opens a `ViewId::Profile{pubkey}` view (triggered by
    /// `AppAction::ClaimProfile`). `Live` liveness (`c_int = 1`) keeps a
    /// `Tailing` kind:0 subscription open so profile edits arrive reactively
    /// while the view is on screen. The updated card arrives back through the
    /// `"claimed_profiles"` typed sidecar as `KernelEvent::ProfileCardUpdated`.
    /// Fire-and-forget (D6, Non-Negotiable #3): nmp handles the claim async.
    ClaimProfile {
        /// Raw 64-char lowercase hex pubkey to claim.
        pubkey: String,
    },

    /// Call `nmp_app_release_profile(raw_ptr, pubkey, "hl.profile.<pubkey>")`.
    ///
    /// Sent when the UI closes a `ViewId::Profile{pubkey}` view (triggered by
    /// `AppAction::ReleaseProfile`). Decrements the per-consumer refcount;
    /// when zero, NMP cancels the Tailing kind:0 subscription and removes the
    /// card from `claimed_profiles`. Fire-and-forget (D6).
    ReleaseProfile {
        /// Raw 64-char lowercase hex pubkey to release.
        pubkey: String,
    },

    // ── Phase 3F additions (append-only) ─────────────────────────────────────
    /// Call `nmp_nip29::register::wire_group_events(nmp_ref, GroupId{..})` to
    /// register the `GroupEventsProjection` observer + typed FlatBuffers sidecar
    /// under `"nmp.nip29.group_events"` for the given group.
    ///
    /// Sent when `Cmd::OpenView(ViewId::RoomHome{group_id})` arrives in the
    /// actor loop (via `room_home::lifecycle_effects_for_view_open`). The
    /// host_relay_url is resolved from `AppState::communities` at effect-run
    /// time (the effect runner is a no-op if the group is not yet in
    /// `communities`).
    ///
    /// Fire-and-forget (D6): events arrive via the NMP update callback as
    /// `KernelEvent::NmpSnapshotFrame` frames decoded by
    /// `projections::dispatch_typed_frame`.
    WireGroupEvents {
        /// NIP-29 local group id to wire the projection for.
        group_id: String,
    },

    /// Discard the hl-side event buffer for `group_id` from
    /// `AppState::room_home_events`.
    ///
    /// Sent when `Cmd::CloseView(ViewId::RoomHome{group_id})` arrives. The
    /// underlying `GroupEventsProjection` in nmp keeps running (singleton
    /// per-group observer); only the hl-side buffer is cleared to bound memory.
    /// Fire-and-forget (D6).
    ReleaseGroupEvents {
        /// NIP-29 local group id whose event buffer to discard.
        group_id: String,
    },

    // ── Phase 4E additions (append-only) ─────────────────────────────────────
    /// Call `nmp_app_dispatch_action` with either
    /// `"nmp.nip29.share_event_in_group"` (kind:11) or
    /// `"nmp.nip29.repost_in_group"` (kind:16) depending on `repost`.
    ///
    /// Payload is a `ShareEventInGroupInput` or `RepostInGroupInput` (identical
    /// shape, verified on pinned nmp b4404159):
    /// `{ group: { host_relay_url, local_id }, target: { event_id, author_pubkey? }, content: "", additional_tags: [] }`
    ///
    /// Built with `serde_json::json!` (never `format!`) to guarantee valid JSON
    /// even if any field contains quotes or backslashes (D-rule: serde, not format).
    /// Fire-and-forget (D6): returned correlation_id JSON is freed and discarded.
    /// D3: no relay URL literals in kernel — all URLs are opaque from the caller.
    DispatchShareToRoom {
        /// `"nmp.nip29.share_event_in_group"` or `"nmp.nip29.repost_in_group"`.
        namespace: String,
        /// JSON-serialized action payload (serde_json, not format!).
        json: String,
    },

    // ── Phase 4C additions (append-only) ─────────────────────────────────────
    /// Call `nmp_app_dispatch_action` with a NIP-51 bookmark namespace and the
    /// `BookmarkUpdateInput { account_pubkey, item }` JSON payload.
    ///
    /// Namespaces: `"nmp.nip51.add_bookmark"` or `"nmp.nip51.remove_bookmark"`
    /// (`AddBookmarkAction::NAMESPACE` and `RemoveBookmarkAction::NAMESPACE` in
    /// `nmp-nip51/src/bookmarks.rs:203,246`).
    ///
    /// Fire-and-forget (D6, Non-Negotiable #3): the returned correlation_id JSON
    /// is freed and discarded. The updated kind:10003 list arrives back through
    /// the `BookmarksUpdated` projection event via the NMP update callback.
    ///
    /// The kernel is the SOLE writer for kind:10003 on ported screens — no
    /// live-lane double-publish.
    DispatchBookmarkAction {
        /// NIP-51 bookmark action namespace.
        /// One of `"nmp.nip51.add_bookmark"` or `"nmp.nip51.remove_bookmark"`.
        namespace: String,
        /// Serialised `BookmarkUpdateInput { account_pubkey, item }` JSON.
        json: String,
    },

    // ── Phase 4B additions (append-only) ─────────────────────────────────────
    /// Call `nmp_app_dispatch_action` with the `"nmp.nip25.react"` or
    /// `"nmp.nip25.unreact"` namespace and the serialised NIP-25 action payload.
    ///
    /// `"nmp.nip25.react"` payload: `ReactAction { target_event_id, reaction,
    /// target_author_pubkey? }` — builds and publishes kind:7.
    /// `"nmp.nip25.unreact"` payload: `UnreactAction { reaction_event_id, reason }`
    /// — builds and publishes kind:5 deletion.
    ///
    /// Fire-and-forget (D6, Non-Negotiable #3): the returned correlation_id JSON
    /// is freed and discarded. The authoritative reaction state arrives back via
    /// `KernelEvent::ReactionStateUpdated` on the next `ReactionProjection` tick.
    ///
    /// The kernel is the sole kind:7 writer for ported screens (no live-lane
    /// double-publish for reactions on articles/highlights/artifacts).
    DispatchReactAction {
        /// NIP-25 action namespace (`"nmp.nip25.react"` or `"nmp.nip25.unreact"`).
        namespace: String,
        /// JSON-serialised NIP-25 action payload (`serde_json::to_string` — never
        /// `format!`). `ReactAction` or `UnreactAction` depending on `namespace`.
        json: String,
    },

    // ── Phase 4D additions (append-only) ─────────────────────────────────────
    /// Submit a NIP-50 relay search.
    ///
    /// The effect runner:
    ///   1. Calls `NmpApp::push_interest(LogicalInterest{ shape: interest_shape,
    ///      lifecycle: OneShot, scope: ActiveAccount, id: InterestId(search_id), .. })`
    ///      to cause the planner to issue NIP-50 REQ frames on connected
    ///      search-capable relays. nmp-nip50 has NO action namespace —
    ///      submission is via `push_interest` (confirmed b4404159
    ///      `crates/nmp-ffi/src/lib.rs:1828`).
    ///   2. Replaces the hl-owned `SearchResultsProjection` registered under
    ///      typed snapshot key `"hl.search"` with a fresh instance seeded from
    ///      `SearchRequest { query, scope, targets: UserPreferred, max_hits:
    ///      DEFAULT_MAX_SEARCH_HITS }`, clearing stale results from the previous
    ///      query. The closure registered with `register_typed_snapshot_projection`
    ///      captures the new projection Arc and serialises its `snapshot()` to
    ///      serde JSON on each tick.
    ///
    /// `interest_shape_json` is the serde-JSON of `InterestShape` built from
    /// `SearchRequest::interest_shape()`. The effect runner deserialises it to
    /// reconstruct the `InterestShape` without depending on nmp-nip50 types
    /// in the pure reducer. Alternatively the effect runner builds the
    /// `InterestShape` directly from `query`+`scope` — either approach is
    /// valid; we pass both to avoid a lossy round-trip.
    ///
    /// No-op if `nmp` is `None` (test mode — tests inject
    /// `KernelEvent::SearchResultsUpdated` directly).
    RunSearch {
        /// Plain-text search query (already trimmed by reducer; non-empty
        /// because the reducer no-ops empty strings).
        query: String,
        /// Serialised `nmp_nip50::SearchScope` (serde JSON). The effect runner
        /// deserialises this to build the `SearchRequest` → `InterestShape`.
        scope_json: String,
        /// Stable `InterestId` u64 for this search session; allows the planner
        /// to dedup or replace the prior search interest on re-query.
        interest_id: u64,
    },

    // ── Phase 4H additions (append-only) ─────────────────────────────────────
    /// Publish a NIP-84 kind:9802 highlight event via `ActorCommand::PublishRawEvent`.
    ///
    /// There is no dedicated nmp action namespace for kind:9802 at pinned nmp
    /// b4404159 — the kernel uses the same raw publish path as Phase 2D's rooms
    /// relay list. The `json` field is a serde_json-serialised event template
    /// (kind, content, tags) without `id`, `sig`, or `pubkey` — nmp's signer fills
    /// those before broadcasting. Built with `serde_json::json!` (never `format!`
    /// — D-rule: serde, not format). Fire-and-forget (D6): nmp handles relay routing
    /// via `PublishTarget::Auto` (NIP-65 outbox; D3).
    ///
    /// The kernel is the sole kind:9802 writer for ported screens.
    PublishHighlightEvent {
        /// serde_json-serialised event template: `{ kind: 9802, content, tags }`.
        json: String,
    },

    // ── Phase 4F additions (append-only) ─────────────────────────────────────
    /// Register a pull cursor with the nmp kernel for the named feed.
    ///
    /// Sends `ActorCommand::RegisterPullCursor` via `actor_sender()` with
    /// `mode: GapAllowed` and limits derived from `feed::FEED_PAGE_SIZE`. The
    /// `cursor_id` is minted deterministically from `key` via
    /// `feed::mint_cursor_id` so re-registering after a view close/re-open
    /// is idempotent (Replace-by-cursor_id semantics in the kernel).
    ///
    /// Fire-and-forget (D6). No-op when `nmp` is `None` (test mode — tests
    /// inject `KernelEvent::FeedPage` directly).
    RegisterFeedCursor {
        /// Stable feed key (e.g. `"hl.feed.articles"`, `"hl.feed.highlights"`,
        /// `"hl.feed.room.<group_id>"`). Must match the key used in `DrainFeed` /
        /// `ReleaseFeedCursor` / `KernelEvent::FeedPage`.
        key: String,
        /// Non-zero cursor id minted by `feed::mint_cursor_id(key)`.
        cursor_id: u64,
        /// The pull scope (`InterestShape` for article/highlight/room-lane feeds).
        /// The kernel filters the ingest log to matching entries.
        scope: nmp_core::PullScope,
    },

    /// Call `nmp_app_pull_page` for the named feed and emit
    /// `KernelEvent::FeedPage` with the decoded rows.
    ///
    /// The effect runner decodes the binary Page wire format, converts positive
    /// (Inserted/Replaced) entries to `KernelEvent`s, and sends
    /// `KernelEvent::FeedPage` back to the actor. On a `Gap` result the event
    /// carries `gap_rebased_to = Some(first_available_seq)` so the reducer can
    /// clear and rebase the cursor (ADR-0058 §10).
    ///
    /// Single `nmp_app_pull_page` call per effect (D5: bounded, D8: no polling).
    /// The consumer emits further `DrainFeed` effects for pagination (e.g.
    /// scroll-to-end). Fire-and-forget (D6). No-op when `nmp` is `None`.
    DrainFeed {
        /// Feed key identifying the cursor to drain.
        key: String,
    },

    /// Unregister the pull cursor for the named feed.
    ///
    /// Sends `ActorCommand::UnregisterPullCursor` via `actor_sender()`.
    /// The `cursor_id` for the unregister call is looked up from `AppState`
    /// inline in `actor_task` (same pattern as `ReleaseGroupEvents`).
    ///
    /// Fire-and-forget (D6). No-op when `nmp` is `None`.
    ReleaseFeedCursor {
        /// Feed key identifying the cursor to unregister.
        key: String,
    },

    // ── Phase 5A additions (append-only) ─────────────────────────────────────
    /// Load What's New entries from bundled JSON + seen-marker file, send KernelEvent::WhatsNewLoaded.
    LoadWhatsNewState,
    /// Persist the What's New seen marker. Monotonic (never moves backward). Fire-and-forget.
    PersistWhatsNewSeen { shipped_at_unix: u64 },

    // ── Phase 5C additions (append-only) ─────────────────────────────────────
    /// Fetch book metadata from `https://openlibrary.org/isbn/{isbn13}.json`
    /// (5 s timeout) and emit `KernelEvent::IsbnPreviewReady`.
    ///
    /// HTTP fetch is Rust-owned inline — no native capability (openlibrary.org is
    /// an audited, product-controlled host; D3 policy note in spec §3).
    /// On any network/parse failure the runner emits a partial preview with an
    /// error string rather than panicking (D6).
    ///
    /// Device-local result — the kernel caches it in
    /// `AppState::isbn` and persists it to `{data_dir}/isbn-preview-cache-v1.json`.
    LookupIsbn {
        /// Normalized 13-digit Bookland ISBN (normalization done in reducer).
        isbn13: String,
    },

    /// Load the ISBN preview cache from disk into `AppState::isbn.cache`.
    ///
    /// Emitted on the first `LookupIsbn` effect when `AppState::isbn.cache_loaded`
    /// is false. The file read is asynchronous; the result arrives as
    /// `KernelEvent::IsbnCacheLoaded`. Fire-and-forget (D6).
    LoadIsbnCache,

    /// Atomically persist the updated ISBN cache to
    /// `{data_dir}/isbn-preview-cache-v1.json` (write-to-tmp then rename).
    ///
    /// Emitted after a successful network fetch (cache miss path). Fire-and-forget
    /// (D6): cache persistence failure is logged but does not surface as an error
    /// to the caller (the in-memory cache is already updated).
    PersistIsbnCache {
        /// Snapshot of all (isbn13, entry) pairs at the time of the cache update.
        entries: Vec<(String, crate::kernel::domains::isbn::CachedIsbnEntry)>,
    },

    // ── Phase 5H additions (append-only) ─────────────────────────────────────
    /// Load the saved resume position for `guid` from the
    /// `{data_dir}/podcast-position-v1.json` store and emit
    /// `KernelEvent::PodcastPositionLoaded`.
    ///
    /// Emitted by `reduce_action_play` BEFORE the `Load` capability request so
    /// the runner can inject the saved position back into state for the
    /// immediate `AudioOp::Load` call.  In the actor the position is already in
    /// memory from a prior load, so this effect is a cheap synchronous read from
    /// `AppState::podcast_position_cache` (keyed by guid) in the effect runner.
    /// Fire-and-forget (D6). DEVICE-LOCAL — never published to nostr.
    LoadPodcastPosition {
        /// Podcast item GUID to look up.
        guid: String,
    },
    /// Atomically persist a podcast resume position to
    /// `{data_dir}/podcast-position-v1.json` (write-to-tmp then rename).
    ///
    /// Emitted by `reduce_capability_audio` on each 5 s `tick_projection` tick
    /// (while playing), and by `reduce_action_set_resume` on explicit app-
    /// background notification. Fire-and-forget (D6): I/O failure is logged
    /// but never surfaced as an error to the caller.
    ///
    /// DEVICE-LOCAL — NEVER a nostr event (`hl-app-state-vs-nostr-facts`).
    SavePodcastPosition {
        /// Podcast item GUID.
        guid: String,
        /// Current playback position in seconds (finite, ≥ 0).
        position_seconds: f64,
        /// Full `ArtifactRecord` snapshot (needed to reconstruct
        /// `PodcastPositionRecord` for cold-launch rehydration).
        /// Boxed to keep the `Effect` enum variant size manageable.
        artifact: Box<crate::models::ArtifactRecord>,
    },

    // ── Phase 5F additions (append-only) ─────────────────────────────────────
    /// Publish a kind:11 plain capture event via `ActorCommand::PublishRawEvent`.
    ///
    /// kind:11 plain capture via raw publish, no nmp.publish namespace needed for
    /// non-group captures. Same pattern as `Effect::PublishHighlightEvent`
    /// (Phase 4H): the kernel builds the event template (`kind`, `content`,
    /// `tags`) with `serde_json::json!`; nmp's signer fills `id`/`sig`/`pubkey`/
    /// `created_at` on publish. Fire-and-forget (D6); routed through the same
    /// `run_effect_publish_highlight` runner since both are just `PublishRawEvent`.
    PublishCaptureEvent {
        /// serde_json-serialised event template: `{ kind: 11, content, tags }`.
        json: String,
    },

    // ── Phase 5I additions (append-only) ─────────────────────────────────────
    /// Fetch and parse a transcript from `url` (HTTP GET, 8 MiB cap, 20 s timeout).
    ///
    /// Detects format (VTT/SRT/JSON) from Content-Type / extension / content sniff.
    /// On success: emits `KernelEvent::TranscriptReady(segments)`.
    /// On failure: emits `KernelEvent::TranscriptFetchFailed`. D6 — never panics.
    /// DEVICE-LOCAL — transcript content is never published to nostr.
    FetchTranscript {
        /// Transcript URL (HTTP or HTTPS). Validated before fetch.
        url: String,
    },

    // ── Phase 5G additions (append-only) ─────────────────────────────────────
    /// Dispatch `nmp.blossom.upload` via `nmp_app_dispatch_action`, recording
    /// the returned `correlation_id` so the `action_results` projection can
    /// route the blob descriptor back to the capture draft.
    ///
    /// Fire-and-dispatch: the effect runner calls `nmp_app_dispatch_action`,
    /// stores the correlation_id in AppState, and returns immediately.
    /// The upload settles asynchronously; the result arrives via
    /// `KernelEvent::BlossomUploadResult` when the `"action_results"` typed
    /// projection fires (next NmpSnapshotFrame tick).
    ///
    /// `image_handle` is the local disk path written by the iOS camera/OCR
    /// pipeline (5E). Never carries raw image bytes across FFI (D5 /
    /// Non-Negotiable #7). `servers` is the ordered BUD-02 server list; must
    /// be non-empty.
    BlossomUpload {
        /// Correlation id to thread through nmp so action_results can route
        /// the descriptor back. Generated in the action reducer (uuid v4 hex).
        correlation_id: String,
        /// Local disk path of the JPEG written by iOS capture/camera.
        image_handle: String,
        /// Ordered BUD-02 upload server URLs. Non-empty (validated in reducer).
        servers: Vec<String>,
    },

    /// Dispatch a capture-draft publish via `ActorCommand::PublishRawEvent`,
    /// carrying a `correlation_id` so the `action_results` projection can
    /// route the publish outcome to `KernelEvent::CaptureDraftPublishResult`.
    ///
    /// 5G replaces the fire-and-forget `PublishHighlightEvent` /
    /// `PublishCaptureEvent` effects for the CAPTURE path only (non-capture
    /// highlight publish remains fire-and-forget per Phase 4H). The correlation
    /// id is generated in `capture_draft::reduce_action_publish` and stored in
    /// `AppState::capture_draft.pending_publish_correlation_id` so the
    /// action_results routing can look it up by id.
    PublishCaptureWithCorrelation {
        /// serde_json-serialised event template: `{ kind, content, tags }`.
        json: String,
        /// Correlation id to thread through nmp for action_results routing.
        correlation_id: String,
    },

    // ── Phase 7 chat additions (append-only) ─────────────────────────────────
    /// Register a `ChatObserver` wrapping a fresh `GroupChatProjection` scoped to
    /// `group_id` as a `KernelEventObserver` against the live `NmpApp`.
    ///
    /// Sent when `hl.chat.open` is dispatched. The observer filters to kind:9,
    /// recovers `reply_to_event_id` from raw tags, and sends
    /// `KernelEvent::ChatRoomUpdated` into the actor channel on each accepted event.
    ///
    /// Fire-and-forget (D6). No-op when `nmp` is `None` (test mode).
    WireGroupChat {
        /// NIP-29 local group id.
        group_id: String,
        /// Host relay WebSocket URL for the `GroupId` construction.
        host_relay_url: String,
    },

    /// Remove the hl-side chat message buffer for `group_id` from
    /// `AppState::chat_rooms`.
    ///
    /// Sent when `hl.chat.close` is dispatched. The underlying `ChatObserver` in
    /// nmp keeps running (singleton per-group observer); only the hl-side buffer
    /// is cleared to bound memory. Fire-and-forget (D6).
    ReleaseChatRoom {
        /// NIP-29 local group id whose chat buffer to discard.
        group_id: String,
    },

    /// Call `nmp_app_dispatch_action` with `"nmp.nip29.post_chat_message"` namespace
    /// and the serialised `PostChatMessageInput` JSON payload.
    ///
    /// Payload shape: `{ group: { host_relay_url, local_id }, content,
    /// previous_event_id_prefixes: [], reply_to_event_id? }`.
    ///
    /// Fire-and-forget (D6, Non-Negotiable #3): the returned correlation_id JSON
    /// is freed and discarded. The authoritative message arrives back via
    /// `KernelEvent::ChatRoomUpdated` from the `ChatObserver` on the next
    /// kind:9 event (relay echo).
    ///
    /// The kernel is the sole kind:9 writer for ported screens.
    DispatchChatPost {
        /// Serialised `PostChatMessageInput` JSON payload (`serde_json::to_string`
        /// — never `format!`).
        json: String,
    },

    // ── Phase 7 additions (append-only) ─────────────────────────────────────
    /// Call `nmp_app_dispatch_action` with `"nmp.nip22.post_comment"` namespace
    /// and the serialised NIP-22 `PostCommentAction` JSON payload.
    ///
    /// `json` is a serde_json-serialised `PostCommentAction` object:
    /// `{ root_tag_name, root_tag_value, root_kind, content, parent_event_id?,
    ///   root_author_pubkey?, parent_author_pubkey? }`.
    ///
    /// Fire-and-forget (D6, Non-Negotiable #3): the returned correlation_id JSON
    /// is freed and discarded. The authoritative comment thread arrives back via
    /// `KernelEvent::CommentThreadUpdated` on the next `CommentObserver` tick.
    ///
    /// The kernel is the sole kind:1111 writer for ported screens — no live-lane
    /// double-publish for comments on articles/highlights/artifacts.
    DispatchCommentAction {
        /// Serialised `PostCommentAction` JSON payload (`serde_json::to_string`
        /// — never `format!`). The namespace is fixed: `"nmp.nip22.post_comment"`.
        json: String,
    },

    // ── Phase 7 feedback additions (append-only) ──────────────────────────────
    /// Call `nmp_app_dispatch_action` with `"nmp.nip22.post_comment"` namespace
    /// for a feedback-project-scoped NIP-22 comment.
    ///
    /// Identical C-ABI call as `DispatchCommentAction` but kept separate for
    /// audit clarity (feedback root is always `HIGHLIGHTER_PROJECT_COORDINATE`).
    /// `json` is a serde_json-serialised `PostCommentAction` object.
    ///
    /// Fire-and-forget (D6, Non-Negotiable #3). The authoritative thread arrives
    /// back via `KernelEvent::CommentThreadUpdated` on the next `CommentObserver`
    /// tick for `root_tag_value = HIGHLIGHTER_PROJECT_COORDINATE`.
    DispatchFeedbackCommentAction {
        /// Serialised `PostCommentAction` JSON — `serde_json::to_string` (not `format!`).
        json: String,
    },

    // ── Phase 5J additions (append-only) ─────────────────────────────────────
    /// Publish a podcast clip as a kind:9802 highlight via `ActorCommand::PublishRawEvent`,
    /// carrying a `correlation_id` so the `action_results` projection can route
    /// the publish outcome to `KernelEvent::ClipPublishActionResult`.
    ///
    /// Reuses the 5G correlation-aware publish path (same mechanism as
    /// `PublishCaptureWithCorrelation`). The `json` field is a serde_json-serialised
    /// event template: `{ kind: 9802, content, tags }` with NIP-73 i-tag +
    /// start/end/speaker/segment tags. nmp fills `id`/`sig`/`pubkey`/`created_at`.
    ///
    /// The correlation_id is stored in
    /// `AppState::podcast.pending_clip_publish_correlation_id` so
    /// `apply_action_result_row` in `blossom.rs` can route the verdict.
    /// NOT fire-and-forget — uses the 5G completion seam for real Done/Error.
    PublishClipWithCorrelation {
        /// serde_json-serialised kind:9802 event template.
        json: String,
        /// Correlation id to thread through nmp for action_results routing.
        correlation_id: String,
    },
}
