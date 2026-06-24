import Foundation
import Observation

/// Lightweight presence probe for "does this room have chat activity?".
/// Distinct from `ChatStore` because the answer drives whether the Chat
/// tab is even shown — we want to know without spinning up the full chat
/// list view first.
///
/// Phase 7: reads the kernel `RoomChatSnapshot.hasActivity` flag. Opening the
/// kernel chat view wires the per-room `ChatObserver`, so a freshly-arriving
/// kind:9 flips `hasActivity` live. The probe keeps the view open (cheap —
/// bounded window) until `stop`.
@MainActor
@Observable
final class ChatPresenceProbe {
    @ObservationIgnored private var groupId: String?
    @ObservationIgnored private weak var kernel: HighlighterAppKernel?
    @ObservationIgnored private var onActivity: (() -> Void)?

    func start(
        groupId: String,
        hostRelayUrl: String,
        kernel: HighlighterAppKernel,
        onActivity: @escaping () -> Void
    ) async {
        if self.groupId != nil, self.groupId != groupId {
            stop()
        }
        self.groupId = groupId
        self.kernel = kernel
        self.onActivity = onActivity

        // Opening the kernel chat view wires the ChatObserver and streams a
        // RoomChatSnapshot into `kernel.roomChatSnapshots[groupId]`.
        kernel.openRoomChat(groupId: groupId, hostRelayUrl: hostRelayUrl)
        if kernel.roomChatSnapshots[groupId]?.hasActivity == true {
            onActivity()
        }
    }

    func stop() {
        if let groupId {
            kernel?.closeRoomChat(groupId: groupId)
        }
        groupId = nil
        onActivity = nil
    }

    /// Re-evaluate activity from the latest kernel snapshot. Called by the
    /// owning view's `onChange(of: kernel.roomChatSnapshots[groupId])`.
    func refreshActivity() {
        guard let groupId else { return }
        if kernel?.roomChatSnapshots[groupId]?.hasActivity == true {
            onActivity?()
        }
    }
}

/// View-scoped reactive state for a room's Chat tab. Phase 7: the kernel owns
/// the bounded chat snapshot (`ViewId.roomChat`); this store opens the kernel
/// view, mirrors `kernel.roomChatSnapshots[groupId]` into the view-model rows
/// the existing UI renders, and dispatches writes through the kernel envelope
/// actions (kernel is sole writer — no live-lane publish).
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
    @ObservationIgnored private var hostRelayUrl: String = ""
    @ObservationIgnored private weak var kernel: HighlighterAppKernel?
    @ObservationIgnored private var lastRevision: UInt64 = 0

    /// Bind to a room's kernel chat view. The kernel `roomChat` view lifecycle
    /// is owned by `ChatPresenceProbe` (resident for the whole RoomHome screen),
    /// so this store does NOT open/close it — it only mirrors the snapshot and
    /// dispatches writes. This avoids double open/close when the chat tab is
    /// shown and hidden while the room stays on screen.
    func start(groupId: String, hostRelayUrl: String, kernel: HighlighterAppKernel) async {
        self.groupId = groupId
        self.hostRelayUrl = hostRelayUrl
        self.kernel = kernel
        isLoading = true
        applyKernelSnapshot()
        isLoading = false
    }

    /// Expand the loaded window by one page. The kernel increments `page_count`
    /// (bounded) and pushes a fresh snapshot; `applyKernelSnapshot` mirrors it.
    func loadMore() async {
        guard let groupId, let kernel, hasMore, !isLoadingMore else { return }
        isLoadingMore = true
        kernel.app.dispatch(.chatLoadMore(groupId: groupId))
        // The kernel pushes a new snapshot asynchronously; the owning view's
        // onChange(of:) re-applies it and clears `isLoadingMore`.
    }

    func stop() {
        groupId = nil
        kernel = nil
    }

    /// Re-apply the latest kernel snapshot. Called by the owning view's
    /// `onChange(of: kernel.roomChatSnapshots[groupId])` so live kind:9 deltas
    /// and `load_more`/`post` results flow into the rendered rows.
    func reloadFromCache(activityEventId: String? = nil) async {
        applyKernelSnapshot()
    }

    /// Send a chat message into the room. The kernel publishes the kind:9 and
    /// streams the refreshed snapshot back (sole writer — no live-lane publish).
    func send(text: String, replyTo: ChatMessageRecord? = nil) async {
        guard let groupId, let kernel else { return }
        sendError = nil
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return }
        kernel.app.dispatch(.postChat(
            groupId: groupId,
            hostRelayUrl: hostRelayUrl,
            content: trimmed,
            replyToEventId: replyTo?.eventId
        ))
    }

    /// Mirror `kernel.roomChatSnapshots[groupId]` into the rendered view model.
    /// Builds `ChatMessageRowProjection` rows Swift-side from the raw kernel rows
    /// (D1: kernel emits raw data; Swift shapes the view model).
    func applyKernelSnapshot() {
        guard let groupId, let snapshot = kernel?.roomChatSnapshots[groupId] else { return }
        rows = snapshot.rows.map { row in
            ChatMessageRowProjection(
                message: ChatMessageRecord(
                    eventId: row.eventId,
                    groupId: groupId,
                    authorPubkey: row.authorPubkey,
                    content: row.content,
                    createdAt: row.createdAt,
                    replyToEventId: row.replyToEventId
                ),
                showHeader: row.showHeader,
                replyToMessage: row.replyTo.map { preview in
                    ChatMessageRecord(
                        eventId: preview.eventId,
                        groupId: groupId,
                        authorPubkey: preview.authorPubkey,
                        content: preview.content,
                        createdAt: preview.createdAt,
                        replyToEventId: nil
                    )
                }
            )
        }
        hasMore = snapshot.hasMore
        isLoadingMore = false
        // Surface a monotonic activity revision so the view can drive its
        // scroll-to-bottom / "new messages" pill (same semantics as before).
        if snapshot.activityRevision != lastRevision {
            activityDelta = max(0, Int(snapshot.activityRevision) - Int(lastRevision))
            lastRevision = snapshot.activityRevision
            activityRevision = snapshot.activityRevision
        }
    }
}
