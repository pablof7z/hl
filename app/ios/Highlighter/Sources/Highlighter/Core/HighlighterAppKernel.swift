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

    // MARK: - Kernel handle

    /// The Rust-side kernel object. Callers may dispatch actions and manage
    /// the lifecycle (resume/suspend/shutdown) via this handle.
    let app: HighlighterApp

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

        // Phase 2E (network settings / relay diagnostics) — handled elsewhere.
        case .networkSettings, .relayDiagnostics:
            break

        // Phase 4+ snapshots — managed by their owning views / stores via
        // `current_snapshot`; the observer push is handled by those stores
        // directly. No-op here (the actor still pushes; non-resident views
        // are closed before they can receive stale data — D5).
        case .bookmarks, .articleReader, .search, .articleFeed,
             .highlightFeed, .homeFeed, .whatsNew, .bookPicker, .shareComposer:
            break

        // Phase 5+ snapshots (podcast, OCR capture) — managed by their owning
        // views / stores; no-op here (same pattern as Phase 4+ above).
        case .podcastListening, .capture:
            break
        }
    }

    // MARK: - Capability fulfillment (called on main actor by KernelObserver)

    fileprivate func fulfill(request: CapabilityRequest) {
        switch request {
        case .keychain(let op):
            fulfillKeychain(op)
        // Phase 5K: share-extension App Group capability — handled by
        // the share capability bridge registered at startup; the observer
        // here is a fallback no-op so the switch remains exhaustive (D6).
        case .share:
            break
        // Phase 5+ native capabilities (audio, OCR, camera) — handled by
        // their respective capability bridges registered at startup; the
        // observer here is a fallback no-op so the switch remains exhaustive (D6).
        case .audio, .ocr, .camera:
            break
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
