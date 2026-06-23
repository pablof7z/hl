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

        var attempts: [ShareQueueAttempt] = []

        for share in pending {
            attempts.append(await app.core.publishShareQueueItem(item: share.coreQueueItem))
        }

        let projection = app.core.projectShareQueueDrain(
            input: ShareQueueDrainProjectionInput(
                attempts: attempts,
                communities: app.joinedCommunities
            )
        )

        if !projection.requeue.isEmpty {
            ShareQueue.replace(projection.requeue.map(PendingShare.init(core:)))
        }

        if let toast = projection.toast {
            app.shareToast = toast
        }

        return Int(projection.successCount)
    }
}

private extension PendingShare {
    var coreQueueItem: ShareQueueItem {
        ShareQueueItem(
            id: id.uuidString,
            groupId: groupId,
            url: url,
            note: note,
            createdAtUnixSeconds: createdAt.timeIntervalSince1970
        )
    }

    init(core item: ShareQueueItem) {
        self.init(
            id: UUID(uuidString: item.id) ?? UUID(),
            groupId: item.groupId,
            url: item.url,
            note: item.note,
            createdAt: Date(timeIntervalSince1970: item.createdAtUnixSeconds)
        )
    }
}
