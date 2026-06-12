import Foundation

/// Drains `ShareQueue` and publishes each pending share through the Rust
/// core. Runs inside the main app — the extension never publishes. Called
/// from `.onOpenURL(highlighter://process-share)` and on scene activation,
/// so a queued share is picked up whether the user tapped through from the
/// extension or came back to the app later.
@MainActor
enum ShareQueueProcessor {
    /// Returns the number of shares successfully published in this run.
    /// Failures are re-queued so the next run can retry. Toast is posted
    /// on the app store when at least one share succeeds.
    @discardableResult
    static func drain(app: HighlighterStore) async -> Int {
        guard app.isLoggedIn else { return 0 }

        let pending = ShareQueue.drain()
        if pending.isEmpty { return 0 }

        var requeue: [PendingShare] = []
        var successCount = 0
        var lastSuccessCommunity: String?

        for share in pending {
            let published = await app.publishQueuedUrlShare(
                url: share.url,
                groupId: share.groupId,
                note: share.note.isEmpty ? nil : share.note
            )
            if published {
                successCount += 1
                if let community = app.joinedCommunities.first(where: { $0.id == share.groupId }) {
                    lastSuccessCommunity = community.name
                } else {
                    lastSuccessCommunity = share.groupId
                }
            } else {
                requeue.append(share)
            }
        }

        if !requeue.isEmpty {
            ShareQueue.replace(requeue)
        }

        if successCount > 0 {
            let label = lastSuccessCommunity ?? "community"
            app.shareToast = successCount == 1
                ? "Shared to \(label)"
                : "Shared \(successCount) items"
        }

        return successCount
    }
}
