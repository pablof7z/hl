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
            title: artifact.preview.title,
            summary: artifact.preview.description,
            imageURL: coverURL,
            authorName: author.displayName,
            authorPubkey: projection.articleAuthorPubkey,
            relativeDate: relativeDate(projection.relativeUnixSeconds),
            metaBits: projection.metaBits,
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

    private var coverURL: URL? {
        guard !artifact.preview.image.isEmpty else { return nil }
        return URL(string: artifact.preview.image)
    }

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
        return app.safeCore.projectProfileDisplayWithLabel(
            input: ProfileDisplayWithLabelProjectionInput(
                pubkey: pubkey,
                profile: pubkey.isEmpty ? nil : app.profileSnapshots[pubkey],
                labelFallback: artifact.preview.author,
                pubkeyFallback: .pubkey10,
                emptyFallback: "Unknown"
            )
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
