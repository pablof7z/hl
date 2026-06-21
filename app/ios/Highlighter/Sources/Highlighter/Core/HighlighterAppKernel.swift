import Foundation
import Observation

/// Thin Swift wrapper around the Rust `HighlighterApp` kernel (Phase 1 nmp-lane).
///
/// Owns the `HighlighterApp` instance, registers as its observer, and
/// re-publishes the latest `AppRootSnapshot` and `RootShellSnapshot` as
/// `@Observable` state for SwiftUI. Capability requests (keychain load/clear)
/// are fulfilled via the existing `KeychainService` helpers and the results
/// fed back to Rust via `provide_capability_result`.
///
/// One instance lives alongside `HighlighterStore` in `AppEntry`. Neither
/// instance is aware of the other — coexistence is ensured by reading from
/// separate storage sub-directories (Phase 1 / §3 of the build spec).
///
/// Phase 3G: also owns Communities, RoomExplorer, Profile, and RoomHome
/// view-snapshot state. Each screen opens its kernel view on appear and
/// closes it on disappear; the observer pushes typed snapshots into these
/// `@Observable` fields so SwiftUI re-renders automatically.
@MainActor
@Observable
final class HighlighterAppKernel {

    // MARK: - Published snapshots (Phase 1 + Phase 2E)

    /// Latest projection for the app-root route decision surface.
    /// Defaults to `.onboarding` until the kernel delivers its first snapshot.
    private(set) var appRoot: AppRootSnapshot = AppRootSnapshot(
        routeKind: .onboarding,
        sessionPresent: false,
        onboardingComplete: false,
        nostrconnectUri: nil
    )

    /// Latest projection for the root-shell chrome (tabs, toast, sheet).
    private(set) var rootShell: RootShellSnapshot = RootShellSnapshot(
        selectedTab: 0,
        tabCount: 5,
        toast: nil,
        sheetId: nil
    )

    // MARK: - Published snapshots (Phase 3G additions)

    /// Joined-groups list for the active account (Communities screen).
    /// `nil` until the kernel delivers its first `CommunitiesSnapshot`.
    private(set) var communities: CommunitiesSnapshot?

    /// Room explorer / discovery shelves.
    /// `nil` until the kernel delivers its first `KernelRoomExplorerSnapshot`.
    private(set) var roomExplorer: KernelRoomExplorerSnapshot?

    /// Per-pubkey profile snapshots, keyed by raw hex pubkey.
    /// Populated while a `ViewId.profile(pubkey:)` view is open.
    /// Raw-data doctrine: Swift formats display strings from these fields.
    private(set) var profileSnapshots: [String: ProfileSnapshot] = [:]

    /// Per-group-id room home snapshots, keyed by NIP-29 local group id.
    /// Populated while a `ViewId.roomHome(groupId:)` view is open.
    private(set) var roomHomeSnapshots: [String: KernelRoomHomeSnapshot] = [:]

    // MARK: - Published snapshots (Phase 7 cutovers)

    /// Per-group-id room chat snapshots, keyed by NIP-29 local group id.
    /// Populated while a `ViewId.roomChat(groupId:)` view is open (the chat
    /// store opens it via `hl.chat.open`). The kernel owns the bounded message
    /// window; Swift renders rows and formats display strings (D1).
    private(set) var roomChatSnapshots: [String: RoomChatSnapshot] = [:]

    /// Per-root NIP-22 comment-thread snapshots, keyed by `root_tag_value`.
    /// Populated while a `ViewId.commentThread(rootTagValue:)` view is open. The
    /// kernel emits a flat `[CommentRecordRow]` (+ per-comment interaction
    /// fields); Swift builds the display tree (`CommentTreeBuilder`).
    private(set) var commentThreads: [String: CommentThreadKernelSnapshot] = [:]

    /// Per-group-id room discussions snapshots, keyed by NIP-29 local group id.
    /// Populated while a `ViewId.roomDiscussions(groupId:)` view is open. The
    /// kernel emits raw kind:11+discussion rows; Swift formats all display
    /// strings (title fallback, date, attachment chip).
    private(set) var roomDiscussions: [String: RoomDiscussionsSnapshot] = [:]

    /// Feedback thread-list snapshot (shake-to-share). `nil` until the
    /// `ViewId.feedbackThreads` view is open. Single resident list.
    private(set) var feedbackThreads: KernelFeedbackThreadsSnapshot?

    /// Per-root feedback thread-detail snapshots, keyed by root comment event id.
    /// Populated while a `ViewId.feedbackThread(rootEventId:)` view is open.
    private(set) var feedbackThread: [String: KernelFeedbackThreadSnapshot] = [:]

    /// Merged home feed snapshot (highlights + reads). `nil` until the
    /// `ViewId.homeFeed` view is open. Carries the rows + the artifact-preview
    /// slice (Phase 7); Swift renders cards from `artifactPreviews[coordinate]`,
    /// skeleton while pending.
    private(set) var homeFeed: KernelHomeFeedSnapshot?

    /// Per-article reader snapshots, keyed by article address (`30023:pubkey:d`).
    /// Populated while a `ViewId.articleReader(address:)` view is open. Carries
    /// the article fields + content_tree_bytes + the enriched overlay
    /// `highlights` (Phase 7). The article-reader store reads its overlay from
    /// here; the body render path is Swift-side.
    private(set) var articleReader: [String: KernelArticleReaderSnapshot] = [:]

    /// Capture (book/page scan → OCR → draft → publish) snapshot. `nil` until the
    /// `ViewId.capture` view is open. The kernel owns the OCR reconstruction,
    /// draft state, and publish FSM; native owns all pixel work (Q1).
    private(set) var captureSnapshot: KernelCaptureSnapshot?

    /// NIP-50 search snapshot. `nil` until the `ViewId.search` view is open.
    /// Carries the raw relay hits + the kernel-decoded highlight rows + the
    /// local communities bucket (Phase 7). SearchStore buckets `hits` by kind for
    /// articles, reads `highlights` directly, and `communities` directly; the
    /// profiles bucket stays on the live lane (nmp #1697).
    private(set) var searchSnapshot: SearchSnapshot?

    // MARK: - Kernel handle

    /// The Rust-side kernel object. Callers may dispatch actions and manage
    /// the lifecycle (resume/suspend/shutdown) via this handle.
    let app: HighlighterApp

    /// Native capability executor (Phase 7 — Part A) for the non-keychain
    /// capabilities: OCR (Vision), Audio (AVPlayer), Share (App Group), and
    /// Camera (presentation, via a registered presenter). Keychain stays inline
    /// in this class (Phase 1).
    let capabilityBridge: KernelCapabilityBridge

    // MARK: - Init

    init() {
        let dataDir: String
        if let dir = FileManager.default
            .urls(for: .applicationSupportDirectory, in: .userDomainMask).first {
            dataDir = dir.path
        } else {
            dataDir = NSTemporaryDirectory()
        }

        let kernelApp = HighlighterApp(config: AppConfig(dataDir: dataDir))
        self.app = kernelApp

        // Native capability executor. Holds a weak ref back to the kernel so it
        // can return results via `provideCapabilityResult` (Phase 7 — Part A).
        let bridge = KernelCapabilityBridge()
        bridge.app = kernelApp
        // Phase 7 Capture: the camera capability is a UI-presentation capability —
        // route it to native SwiftUI camera/barcode presentation over the key
        // window (native owns the pixels; the kernel drives the flow).
        bridge.cameraPresenter = { op in await CapturePresenter.present(op) }
        self.capabilityBridge = bridge

        // Register the observer BEFORE opening views so no initial snapshot
        // is dropped. The observer holds a weak back-reference which breaks
        // the retain cycle:
        //   HighlighterAppKernel → HighlighterApp → KernelObserver ⇢ (weak) HighlighterAppKernel
        let observer = KernelObserver(kernel: self)
        kernelApp.setObserver(observer: observer)

        // Open the two root projections. The kernel will push snapshots as
        // soon as the actor processes these commands.
        kernelApp.openView(viewId: .appRoot, route: .appRoot)
        kernelApp.openView(viewId: .rootShell, route: .rootShell)

        // Phase 3G: open the Communities view immediately — it is always
        // resident in the background tab shell (no open/close lifecycle).
        kernelApp.openView(viewId: .communities, route: .communities)

        // Note: RoomExplorer is NOT opened here. RoomExplorerView manages its
        // own open/close lifecycle (openRoomExplorer on .task, closeRoomExplorer
        // on .onDisappear). The kernel auto-starts discovery via the lifecycle
        // hook when the view is opened (Phase 3G).
    }

    // MARK: - Phase 3G: per-view lifecycle helpers

    /// Open the RoomExplorer view. The kernel's lifecycle hook auto-starts
    /// discovery on open (Phase 3G). Call from `RoomExplorerView.task`.
    func openRoomExplorer() {
        app.openView(viewId: .roomExplorer, route: .roomExplorer)
    }

    /// Close the RoomExplorer view and clear its cached snapshot.
    /// Call from `RoomExplorerView.onDisappear`.
    func closeRoomExplorer() {
        app.closeView(viewId: .roomExplorer)
        roomExplorer = nil
    }

    /// Re-trigger room discovery for pull-to-refresh.
    ///
    /// Re-sends `Cmd::OpenView(RoomExplorer)` to the actor. The registry
    /// treats it as idempotent (view stays open, existing snapshot preserved),
    /// but the actor's lifecycle hook re-fires
    /// `discovery::lifecycle_effects_for_view_open` which reads
    /// `state.room_policy.discovery_relay` — no relay URL literal in Swift (D3).
    func refreshRoomExplorer() {
        app.openView(viewId: .roomExplorer, route: .roomExplorer)
    }

    /// Open a profile view for `pubkey` and send `ClaimProfile` to NMP.
    /// Call from `ProfileView.task(id:)`. Idempotent — nop if already open.
    func openProfile(pubkey: String) {
        let viewId = ViewId.profile(pubkey: pubkey)
        app.openView(viewId: viewId, route: .profile(pubkey: pubkey))
        app.dispatch(.claimProfile(pubkey: pubkey))
    }

    /// Close a profile view for `pubkey` and send `ReleaseProfile` to NMP.
    /// Call from `ProfileView.onDisappear`. Clears the cached snapshot.
    func closeProfile(pubkey: String) {
        app.dispatch(.releaseProfile(pubkey: pubkey))
        app.closeView(viewId: .profile(pubkey: pubkey))
        profileSnapshots.removeValue(forKey: pubkey)
    }

    /// Open a room-home view for `groupId`. The kernel wires the
    /// `GroupEventsProjection` via `Effect::WireGroupEvents`.
    /// Call from `RoomHomeView.task`.
    func openRoomHome(groupId: String) {
        let viewId = ViewId.roomHome(groupId: groupId)
        app.openView(viewId: viewId, route: .roomHome(groupId: groupId))
    }

    /// Close a room-home view for `groupId`. Releases the `GroupEventsProjection`.
    /// Call from `RoomHomeView.onDisappear`.
    func closeRoomHome(groupId: String) {
        app.closeView(viewId: .roomHome(groupId: groupId))
        roomHomeSnapshots.removeValue(forKey: groupId)
    }

    // MARK: - Phase 7: home feed lifecycle

    /// Open the merged home feed view (highlights + reads). The kernel opens both
    /// underlying feed cursors and pushes `KernelHomeFeedSnapshot` (with its
    /// artifact-preview slice) into `homeFeed`. Call from `HighlightsTabView.task`.
    func openHomeFeed() {
        app.openView(viewId: .homeFeed, route: .homeFeed)
    }

    /// Close the home feed view (releases both feed cursors).
    func closeHomeFeed() {
        app.closeView(viewId: .homeFeed)
        homeFeed = nil
    }

    // MARK: - Phase 7: article reader lifecycle

    /// Open the article-reader view for `address` (`30023:pubkey:d`). The kernel
    /// projects the article body + registers the per-article highlight feed, then
    /// pushes `KernelArticleReaderSnapshot` (with the enriched overlay
    /// `highlights`) into `articleReader[address]`. Call from `ArticleReaderView.task`.
    func openArticleReader(address: String) {
        app.openView(viewId: .articleReader(address: address), route: .articleReader(address: address))
    }

    /// Close the article-reader view (releases the per-article highlight feed)
    /// and drop its cached snapshot.
    func closeArticleReader(address: String) {
        app.closeView(viewId: .articleReader(address: address))
        articleReader.removeValue(forKey: address)
    }

    // MARK: - Phase 7: room chat lifecycle

    /// Open a room-chat view for `groupId`. Opens the kernel view (so snapshots
    /// stream into `roomChatSnapshots`) and dispatches `hl.chat.open` to wire the
    /// per-room `ChatObserver`. Call from `ChatView.task`.
    func openRoomChat(groupId: String, hostRelayUrl: String) {
        app.openView(viewId: .roomChat(groupId: groupId), route: .roomChat(groupId: groupId))
        app.dispatch(.chatOpen(groupId: groupId, hostRelayUrl: hostRelayUrl))
    }

    /// Close a room-chat view for `groupId`. Dispatches `hl.chat.close` (releases
    /// the room buffer) and closes the kernel view. Call from `ChatView.onDisappear`.
    func closeRoomChat(groupId: String) {
        app.dispatch(.chatClose(groupId: groupId))
        app.closeView(viewId: .roomChat(groupId: groupId))
        roomChatSnapshots.removeValue(forKey: groupId)
    }

    // MARK: - Phase 7: comment thread lifecycle

    /// Open a NIP-22 comment thread view for `rootTagValue`. The global
    /// `CommentObserver` already routes all kind:1111 events, so opening the
    /// view just registers the projection so snapshots stream into
    /// `commentThreads[rootTagValue]`. Call from the comments view's `.task`.
    func openCommentThread(rootTagValue: String) {
        app.openView(viewId: .commentThread(rootTagValue: rootTagValue),
                     route: .commentThread(rootTagValue: rootTagValue))
    }

    /// Close a comment thread view for `rootTagValue`. The kernel keeps the
    /// content-addressed thread in `AppState`; this just stops snapshot pushes.
    func closeCommentThread(rootTagValue: String) {
        app.closeView(viewId: .commentThread(rootTagValue: rootTagValue))
        commentThreads.removeValue(forKey: rootTagValue)
    }

    // MARK: - Phase 7: room discussions lifecycle

    /// Open the discussions tab view for `groupId`. The kernel filters the
    /// room's kind:11+discussion events into `roomDiscussions[groupId]`.
    /// Call from `DiscussionListView.task`.
    func openRoomDiscussions(groupId: String) {
        app.openView(viewId: .roomDiscussions(groupId: groupId),
                     route: .roomDiscussions(groupId: groupId))
    }

    /// Close the discussions tab view for `groupId`. Call from `.onDisappear`.
    func closeRoomDiscussions(groupId: String) {
        app.closeView(viewId: .roomDiscussions(groupId: groupId))
        roomDiscussions.removeValue(forKey: groupId)
    }

    // MARK: - Phase 7: feedback (shake-to-share) lifecycle

    /// Open the feedback thread-list view. Dispatches `hl.feedback.open_list`
    /// (sets the UI flags) and opens the kernel view so snapshots stream into
    /// `feedbackThreads`. Call from `FeedbackThreadsView.task`.
    func openFeedbackThreads() {
        app.openView(viewId: .feedbackThreads, route: .feedbackThreads)
        app.dispatch(.feedbackOpenList)
    }

    /// Close the feedback thread-list view. Call from `.onDisappear`.
    func closeFeedbackThreads() {
        app.dispatch(.feedbackCloseList)
        app.closeView(viewId: .feedbackThreads)
        feedbackThreads = nil
    }

    /// Open a feedback thread-detail view for `rootEventId`. Dispatches
    /// `hl.feedback.open_thread` and opens the kernel view.
    func openFeedbackThread(rootEventId: String) {
        app.openView(viewId: .feedbackThread(rootEventId: rootEventId),
                     route: .feedbackThread(rootEventId: rootEventId))
        app.dispatch(.feedbackOpenThread(rootEventId: rootEventId))
    }

    /// Close a feedback thread-detail view for `rootEventId`.
    func closeFeedbackThread(rootEventId: String) {
        app.dispatch(.feedbackCloseThread)
        app.closeView(viewId: .feedbackThread(rootEventId: rootEventId))
        feedbackThread.removeValue(forKey: rootEventId)
    }

    // MARK: - Phase 7: capture lifecycle

    /// Open the capture view (book/page scan → OCR → draft → publish).
    /// Snapshots stream into `captureSnapshot`. Call from the capture screen's
    /// `.task`. Camera/OCR/blossom run as native capability round-trips.
    func openCapture() {
        app.openView(viewId: .capture, route: .capture)
    }

    /// Close the capture view and reset the draft (hl.capture.reset).
    func closeCapture() {
        app.dispatch(.captureReset)
        app.closeView(viewId: .capture)
        captureSnapshot = nil
    }

    // MARK: - Phase 7: search lifecycle

    /// Open the NIP-50 search view. Snapshots stream into `searchSnapshot` once a
    /// query is run via `dispatch(.runSearch(...))`. Call from the search screen's
    /// `.task`.
    func openSearch() {
        app.openView(viewId: .search, route: .search)
    }

    /// Close the search view (kernel clears `AppState::search_results` to bound
    /// memory).
    func closeSearch() {
        app.closeView(viewId: .search)
        searchSnapshot = nil
    }

    // MARK: - Snapshot ingestion (called on main actor by KernelObserver)

    fileprivate func receive(viewId: ViewId, snapshot: ViewSnapshot) {
        switch snapshot {
        case .appRoot(let s):
            appRoot = s
        case .rootShell(let s):
            rootShell = s

        // Phase 3G additions:
        case .communities(let s):
            communities = s
        case .roomExplorer(let s):
            roomExplorer = s
        case .profile(let s):
            profileSnapshots[s.pubkey] = s
        case .roomHome(let s):
            roomHomeSnapshots[s.groupId] = s

        // Phase 7 cutover: room chat snapshot (ChatStore reads this dict).
        case .roomChat(let s):
            roomChatSnapshots[s.groupId] = s

        // Phase 7 cutover: NIP-22 comment thread (CommentsStore reads this dict).
        case .commentThread(let s):
            commentThreads[s.rootTagValue] = s

        // Phase 7 cutover: room discussions (DiscussionStore reads this dict).
        case .roomDiscussions(let s):
            roomDiscussions[s.groupId] = s

        // Phase 7 cutover: feedback list + thread (FeedbackStore / FeedbackThreadStore).
        case .feedbackThreads(let s):
            feedbackThreads = s
        case .feedbackThread(let s):
            feedbackThread[s.rootEventId] = s

        // Phase 7 cutover: merged home feed (HomeFeedStore reads this).
        case .homeFeed(let s):
            homeFeed = s

        // Phase 7 cutover: capture (CaptureStore reads this).
        case .capture(let s):
            captureSnapshot = s

        // Phase 7 cutover: search (SearchStore reads this).
        case .search(let s):
            searchSnapshot = s

        // Phase 2E (network settings / relay diagnostics) — handled elsewhere.
        case .networkSettings, .relayDiagnostics:
            break

        // Phase 7 cutover: article reader (ArticleReaderStore reads its overlay
        // highlights from here; the body render path is Swift-side).
        case .articleReader(let s):
            articleReader[s.address] = s

        // Phase 4+ snapshots — managed by their owning views / stores via
        // `current_snapshot`; the observer push is handled by those stores
        // directly. No-op here (the actor still pushes; non-resident views
        // are closed before they can receive stale data — D5).
        case .bookmarks, .articleFeed,
             .highlightFeed, .whatsNew, .bookPicker, .shareComposer:
            break

        // Phase 5+ snapshots (podcast) — managed by their owning views / stores;
        // no-op here (same pattern as Phase 4+ above).
        case .podcastListening:
            break
        }
    }

    // MARK: - Capability fulfillment (called on main actor by KernelObserver)

    fileprivate func fulfill(request: CapabilityRequest) {
        switch request {
        case .keychain(let op):
            fulfillKeychain(op)
        // Phase 7 — Part A: OCR / Audio / Share / Camera capabilities are
        // executed by the native `KernelCapabilityBridge`, which runs the raw
        // OS capability and returns the result via `provideCapabilityResult`.
        case .ocr, .audio, .share, .camera:
            capabilityBridge.fulfill(request)
        }
    }

    private func fulfillKeychain(_ op: KeychainOp) {
        let result: KeychainResult
        switch op {
        case .loadSession:
            // Prefer nsec; fall back to bunker URI — same priority as the
            // live lane's `AppSessionStore.restoreSession(into:)`.
            if let nsec = KeychainService.loadNsec() {
                result = .sessionSecret(nsec)
            } else if let uri = KeychainService.loadBunkerURI() {
                result = .sessionSecret(uri)
            } else {
                result = .sessionSecret(nil)
            }
        case .clearSession:
            KeychainService.deleteNsec()
            KeychainService.deleteBunkerURI()
            result = .cleared
        }
        app.provideCapabilityResult(result: .keychain(result))
    }
}

// MARK: - Observer implementation

/// Inner observer registered with Rust. Callbacks arrive on Rust's tokio
/// task thread; this class hops each call to the main actor before touching
/// `HighlighterAppKernel`.
///
/// `@unchecked Sendable` is safe here: the only mutable state is the
/// `weak var kernel`, and Swift's runtime guarantees that `weak` loads are
/// atomic — so cross-thread reads are well-defined.
private final class KernelObserver: HighlighterObserver, @unchecked Sendable {
    private weak var kernel: HighlighterAppKernel?

    init(kernel: HighlighterAppKernel) {
        self.kernel = kernel
    }

    func onSnapshot(viewId: ViewId, snapshot: ViewSnapshot) {
        Task { @MainActor [weak self] in
            self?.kernel?.receive(viewId: viewId, snapshot: snapshot)
        }
    }

    func onCapabilityRequest(request: CapabilityRequest) {
        Task { @MainActor [weak self] in
            self?.kernel?.fulfill(request: request)
        }
    }
}

// MARK: - Phase 3G bridge helpers (raw-data → legacy presentation types)
//
// These bridges convert kernel snapshot types (raw data, D3) into the
// live-lane types (`CommunitySummary`, `ProfileMetadata`, `RoomRecommendation`)
// that the existing UI components expect. They live here — not in the view
// layer — so the view code does not need to understand both type systems.
//
// Swift formats ALL presentation strings here (raw-data doctrine, §7 of
// the Phase 3 build spec): member count labels, open/closed indicators,
// display name fallbacks, NIP-05 label normalisation, etc.
//
// Phase 7: delete these helpers when the live lane (`HighlighterCore`) is
// removed and UI components are updated to consume kernel types directly.

extension CommunityRow {
    /// Bridge to the legacy `CommunitySummary` expected by existing UI components.
    /// Swift formats presentation strings from raw kernel fields (D3).
    func asCommunitySummary() -> CommunitySummary {
        CommunitySummary(
            id: groupId,
            name: name ?? groupId,
            about: about ?? "",
            picture: picture ?? "",
            // Swift formats "open"/"closed" from the boolean (D3).
            access: `open` ? "open" : "closed",
            // Swift formats "public"/"private" from the boolean (D3).
            visibility: `public` ? "public" : "private",
            adminPubkeys: [],
            memberCount: UInt64(memberCount),
            relayUrl: hostRelayUrl,
            metadataEventId: "",
            createdAt: nil
        )
    }
}

extension DiscoveredRow {
    /// Bridge to `CommunitySummary` for display in existing room cards.
    func asCommunitySummary() -> CommunitySummary {
        CommunitySummary(
            id: groupId,
            name: name ?? groupId,
            about: about ?? "",
            picture: picture ?? "",
            access: `open` ? "open" : "closed",
            visibility: `public` ? "public" : "private",
            adminPubkeys: [],
            memberCount: UInt64(memberCount),
            relayUrl: hostRelayUrl,
            metadataEventId: "",
            createdAt: nil
        )
    }
}

extension RecommendationRow {
    /// Bridge to `RoomRecommendation` for display in existing friends/authors shelf cards.
    func asRoomRecommendation(reason: RoomRecommendationReason) -> RoomRecommendation {
        let summary = CommunitySummary(
            id: groupId,
            name: name ?? groupId,
            about: about ?? "",
            picture: picture ?? "",
            access: "open",
            visibility: "public",
            adminPubkeys: [],
            memberCount: UInt64(totalReasonCount),
            relayUrl: hostRelayUrl,
            metadataEventId: "",
            createdAt: nil
        )
        return RoomRecommendation(
            summary: summary,
            reasonPubkeys: reasonPubkeys,
            reasonKind: reason
        )
    }
}

extension ProfileSnapshot {
    /// Bridge to `ProfileMetadata` for display in existing profile UI components.
    /// Swift applies presentation decisions (D3):
    /// - display-name fallback order: displayName → name → first 12 chars of pubkey
    /// - NIP-05 `"_@example.com"` → `"example.com"` label strip is done in the view
    func asProfileMetadata() -> ProfileMetadata {
        ProfileMetadata(
            pubkey: pubkey,
            name: name ?? "",
            displayName: displayName ?? name ?? "",
            about: about,
            picture: pictureUrl ?? "",
            banner: banner ?? "",
            nip05: nip05,
            website: website ?? "",
            lud16: lud16 ?? "",
            createdAt: nil
        )
    }
}
