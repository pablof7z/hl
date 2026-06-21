import Foundation

/// Drains the iOS Share Extension queue and publishes each pending share —
/// entirely through the kernel (#21: kernel is the sole writer for kind:11
/// artifact shares). Runs inside the main app; the extension never publishes.
/// Called from `.onOpenURL(highlighter://process-share)` and on scene
/// activation, so a queued share is picked up whether the user tapped through
/// from the extension or came back to the app later.
///
/// A single kernel action: `hl.share.drain_queue` emits the native
/// `ShareOp::DrainQueue` capability; native reads the App Group
/// `pending-shares-v1.json` and returns the items. When that capability result
/// lands, the kernel dedupes the items into `share_queue.pending` AND publishes
/// each as a host-pinned kind:11 artifact share (`build_preview` →
/// `build_artifact_share_tags` → `ActorCommand::PublishRawEvent`) — all in
/// `reduce_event_share_queue_drained`. The kernel is the sole writer; the drain
/// and publish are one atomic kernel step (no Swift-side ordering race).
@MainActor
enum ShareQueueProcessor {
    static func drain(app: HighlighterStore, kernel: HighlighterAppKernel) {
        guard app.isLoggedIn else { return }
        kernel.app.dispatch(.drainShareQueue)
    }
}
