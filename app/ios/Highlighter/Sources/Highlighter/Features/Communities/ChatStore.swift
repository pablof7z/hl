import Foundation
import Observation

/// Lightweight presence probe for "does this room have chat activity?".
/// Distinct from `ChatStore` because the answer drives whether the Chat
/// tab is even shown — we want to know without spinning up the full chat
/// list view first.
///
/// The probe asks Rust for a tiny presence snapshot on `start` and installs
/// the same room-chat subscription as `ChatStore` so that a freshly-arriving
/// kind:9 unhides the tab live. Once activity is signalled the probe stays
/// subscribed (cheap) until `stop`.
@MainActor
@Observable
final class ChatPresenceProbe {
    @ObservationIgnored private var groupId: String?
    @ObservationIgnored private var core: SafeHighlighterCore?
    @ObservationIgnored private weak var bridge: EventBridge?
    @ObservationIgnored private var subscriptionHandle: UInt64?
    @ObservationIgnored private var onActivity: (() -> Void)?

    func start(
        groupId: String,
        core: SafeHighlighterCore,
        bridge: EventBridge?,
        onActivity: @escaping () -> Void
    ) async {
        if self.groupId != nil, self.groupId != groupId {
            stop()
        }
        self.groupId = groupId
        self.core = core
        self.bridge = bridge
        self.onActivity = onActivity

        // Cache peek first — instant if any kind:9 is already locally cached.
        let presence = await core.getChatPresenceSnapshot(groupId: groupId)
        if presence.hasActivity {
            onActivity()
        }

        guard subscriptionHandle == nil else { return }
        let presenceStart = await core.subscribeRoomChat(groupId: groupId)
        let presenceProjection = core.projectViewSubscriptionStart(
            input: ViewSubscriptionStartProjectionInput(start: presenceStart)
        )
        guard presenceProjection.shouldRegister else {
            // No live promotion if the subscription failed; the cache peek
            // result still applies.
            return
        }
        subscriptionHandle = presenceProjection.handle
        bridge?.registerChatPresence(self, handle: presenceProjection.handle)
    }

    func stop() {
        if let handle = subscriptionHandle, let core {
            Task { await core.unsubscribe(handle) }
            bridge?.unregister(handle: handle)
        }
        subscriptionHandle = nil
        onActivity = nil
    }

    /// Called by `EventBridge` for the first `ChatMessageUpserted` after
    /// `start`. Idempotent — repeat calls just re-fire the closure (harmless
    /// because the consumer flips a Bool to true).
    func notifyActivity() {
        onActivity?()
    }
}

/// View-scoped reactive state for a room's Chat tab. Rust owns the bounded
/// chat snapshot; this store keeps only view lifecycle and scroll activity
/// state around that snapshot.
@MainActor
@Observable
final class ChatStore {
    private(set) var rows: [ChatMessageRowProjection] = []
    private(set) var isLoading: Bool = true
    private(set) var isLoadingMore: Bool = false
    private(set) var hasMore: Bool = false
    private(set) var activityRevision: UInt64 = 0
    private(set) var activityDelta: Int = 0
    var sendError: String?

    @ObservationIgnored private var groupId: String?
    @ObservationIgnored private var core: SafeHighlighterCore?
    @ObservationIgnored private weak var bridge: EventBridge?
    @ObservationIgnored private var subscriptionHandle: UInt64?
    @ObservationIgnored private var loadedPageCount: UInt32 = 1

    func start(groupId: String, core: SafeHighlighterCore, bridge: EventBridge?) async {
        if self.groupId != nil, self.groupId != groupId {
            stop()
        }
        self.groupId = groupId
        self.core = core
        self.bridge = bridge
        loadedPageCount = 1
        isLoading = true
        await reloadSnapshot(pageCount: loadedPageCount)
        isLoading = false

        guard subscriptionHandle == nil else { return }
        let outcome = await core.subscribeRoomChat(groupId: groupId)
        let projection = core.projectViewSubscriptionStart(
            input: ViewSubscriptionStartProjectionInput(start: outcome)
        )
        guard projection.shouldRegister else {
            // Subscription failure leaves cache-only rendering working.
            return
        }
        subscriptionHandle = projection.handle
        bridge?.registerChat(self, handle: projection.handle)
    }

    /// Expand the loaded window by one page. Replaces `messages` with a
    /// larger slice from the DB; the caller is responsible for restoring
    /// the scroll position to the previously-topmost visible message.
    func loadMore() async {
        guard let core else { return }
        let projection = core.projectChatLoadMore(
            input: ChatLoadMoreProjectionInput(
                isLoadingMore: isLoadingMore,
                hasMore: hasMore,
                currentPageCount: loadedPageCount
            )
        )
        guard projection.shouldLoad else { return }
        isLoadingMore = projection.isLoadingMore
        await reloadSnapshot(pageCount: projection.requestedPageCount)
        isLoadingMore = false
    }

    func stop() {
        if let handle = subscriptionHandle, let core {
            Task { await core.unsubscribe(handle) }
            bridge?.unregister(handle: handle)
        }
        subscriptionHandle = nil
    }

    func reloadFromCache(activityEventId: String? = nil) async {
        let activityProjection = core?.projectChatActivityReload(
            input: ChatActivityReloadProjectionInput(
                activityEventId: activityEventId ?? "",
                visibleEventIds: rows.map(\.message.eventId),
                currentActivityRevision: activityRevision
            )
        )
        await reloadSnapshot(pageCount: loadedPageCount)
        if let activityProjection, activityProjection.shouldMarkActivity {
            activityDelta = Int(activityProjection.activityDelta)
            activityRevision = activityProjection.activityRevision
        }
    }

    /// Send a chat message into the room. Rust publishes and returns the
    /// refreshed bounded snapshot, including the signed record if the relay
    /// echo has not landed locally yet.
    func send(text: String, replyTo: ChatMessageRecord? = nil) async {
        guard let groupId, let core else { return }
        sendError = nil
        let outcome = await core.publishChatMessageSnapshot(
            groupId: groupId,
            content: text,
            replyToEventId: replyTo?.eventId,
            pageCount: loadedPageCount
        )
        let result = core.projectChatPublishResult(
            input: ChatPublishResultInput(error: outcome.error)
        )
        guard result.didPublish else {
            sendError = result.errorMessage
            return
        }
        apply(snapshot: outcome.snapshot)
    }

    private func reloadSnapshot(pageCount: UInt32) async {
        guard let groupId, let core else { return }
        let snapshot = await core.getChatSnapshot(groupId: groupId, pageCount: pageCount)
        apply(snapshot: snapshot)
    }

    private func apply(snapshot: ChatSnapshot) {
        rows = snapshot.rows
        hasMore = snapshot.hasMore
        loadedPageCount = snapshot.pageCount
    }
}
