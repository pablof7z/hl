import SwiftUI

/// Editorial card in the Following Reads feed. Wraps the shared
/// `ReadingCard` presentation with a social-signal trailing slot and
/// pubkey-driven profile lookup for the author avatar.
struct ReadingFeedCardView: View {
    @Environment(HighlighterStore.self) private var app

    let item: ReadingFeedItem

    var body: some View {
        let author = authorDisplay
        let projection = cardProjection

        ReadingCard(
            title: item.article.title,
            summary: item.article.summary,
            imageURL: coverURL,
            authorName: author.displayName,
            authorPubkey: item.article.pubkey,
            relativeDate: relativeDate(projection.relativeUnixSeconds),
            metaBits: projection.metaBits,
            showTrailing: projection.showSocialSignal,
            avatar: {
                AuthorAvatar(
                    pubkey: item.article.pubkey,
                    pictureURL: author.pictureUrl,
                    displayInitial: author.displayInitial,
                    size: 22
                )
            },
            trailing: { socialBadge(projection) }
        )
        .task(id: item.article.pubkey) {
            await app.requestProfile(pubkeyHex: item.article.pubkey)
        }
        .task(id: projection.primaryInteractorPubkey ?? "") {
            guard let pk = projection.primaryInteractorPubkey else { return }
            await app.requestProfile(pubkeyHex: pk)
        }
    }

    // MARK: - Social signal

    @ViewBuilder
    private func socialBadge(_ projection: ReadingFeedCardProjection) -> some View {
        HStack(spacing: 6) {
            if !projection.visibleInteractorPubkeys.isEmpty {
                HStack(spacing: -6) {
                    ForEach(projection.visibleInteractorPubkeys, id: \.self) { pk in
                        AuthorAvatar(pubkey: pk, size: 18, ringWidth: 1.5)
                    }
                }
            }
            Text(projection.socialText)
                .font(.caption)
                .foregroundStyle(Color.highlighterInkMuted)
                .lineLimit(1)
        }
    }

    // MARK: - Author display

    private var authorDisplay: ProfileDisplayProjection {
        app.safeCore.projectProfileDisplay(
            input: ProfileDisplayProjectionInput(
                pubkey: item.article.pubkey,
                profile: app.profileSnapshots[item.article.pubkey],
                fallback: .pubkey10
            )
        )
    }

    private var cardProjection: ReadingFeedCardProjection {
        app.safeCore.projectReadingFeedCard(
            input: ReadingFeedCardProjectionInput(
                item: item,
                interactorProfiles: item.interactorPubkeys.map { pubkey in
                    ReadingFeedInteractorProfile(
                        pubkey: pubkey,
                        profile: app.profileSnapshots[pubkey]
                    )
                }
            )
        )
    }

    // MARK: - Derived bits

    private var coverURL: URL? {
        guard !item.article.image.isEmpty else { return nil }
        return URL(string: item.article.image)
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
