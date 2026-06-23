import SwiftUI

/// Medium-style card for an article artifact in a room's library. Mirrors
/// the reads-tab treatment by using the Rust artifact route so the real
/// article author drives the attribution avatar/profile lookup rather than
/// the sharer.
struct RoomLibraryArticleCardView: View {
    @Environment(HighlighterStore.self) private var app

    let artifact: ArtifactRecord
    var commentCount: Int = 0

    var body: some View {
        let projection = cardProjection
        let author = authorDisplay(projection)

        ReadingCard(
            title: projection.displayTitle,
            titleIsFallback: projection.titleIsFallback,
            summary: artifact.preview.description,
            imageURL: projection.imageUrl.flatMap { URL(string: $0) },
            authorName: author.displayName,
            authorPubkey: projection.articleAuthorPubkey,
            relativeDate: relativeDate(projection.relativeUnixSeconds),
            metaText: projection.metaText,
            showTrailing: false,
            avatar: {
                AuthorAvatar(
                    pubkey: projection.avatarPubkey,
                    pictureURL: author.pictureUrl,
                    displayInitial: author.displayInitial,
                    size: 22
                )
            },
            trailing: { EmptyView() }
        )
        .task(id: projection.articleAuthorPubkey ?? "") {
            guard let pk = projection.articleAuthorPubkey else { return }
            await app.requestProfile(pubkeyHex: pk)
        }
    }

    // MARK: - Derived bits

    private var cardProjection: RoomLibraryArticleCardProjection {
        app.safeCore.projectRoomLibraryArticleCard(
            input: RoomLibraryArticleCardProjectionInput(
                artifact: artifact,
                commentCount: UInt32(commentCount)
            )
        )
    }

    private func authorDisplay(
        _ projection: RoomLibraryArticleCardProjection
    ) -> ProfileDisplayProjection {
        let pubkey = projection.authorProfilePubkey
        let profile = pubkey.isEmpty ? nil : app.profileSnapshots[pubkey]
        let dn = profile?.displayName ?? ""
        let n = profile?.name ?? ""
        let label = artifact.preview.author
        let displayName: String
        let displayInitial: String
        if !dn.isEmpty {
            displayName = dn; displayInitial = String(dn.prefix(1))
        } else if !n.isEmpty {
            displayName = n; displayInitial = String(n.prefix(1))
        } else if !label.isEmpty {
            displayName = label; displayInitial = String(label.prefix(1))
        } else if !pubkey.isEmpty {
            displayName = String(pubkey.prefix(10)); displayInitial = String(pubkey.prefix(1))
        } else {
            displayName = "Unknown"; displayInitial = "U"
        }
        return ProfileDisplayProjection(
            displayName: displayName,
            displayInitial: displayInitial,
            pictureUrl: profile?.picture ?? ""
        )
    }

    private func relativeDate(_ seconds: UInt64?) -> String? {
        guard let seconds else { return nil }
        let date = Date(timeIntervalSince1970: TimeInterval(seconds))
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        formatter.dateTimeStyle = .numeric
        return formatter.localizedString(for: date, relativeTo: Date())
    }
}
