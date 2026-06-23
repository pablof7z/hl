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
            title: projection.displayTitle,
            titleIsFallback: projection.titleIsFallback,
            summary: item.article.summary,
            imageURL: projection.imageUrl.flatMap { URL(string: $0) },
            authorName: author.displayName,
            authorPubkey: item.article.pubkey,
            relativeDate: relativeDate(projection.relativeUnixSeconds),
            metaText: projection.metaText,
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

    /// Inline port of `profile_display_projection` — mirrors CommentRow.authorDisplay pattern.
    private var authorDisplay: ProfileDisplayProjection {
        let pubkey = item.article.pubkey
        let profile = app.profileSnapshots[pubkey]
        let name: String = {
            if let d = profile?.displayName, !d.isEmpty { return d }
            if let n = profile?.name, !n.isEmpty { return n }
            return String(pubkey.prefix(8))
        }()
        return ProfileDisplayProjection(
            displayName: name,
            displayInitial: String(name.prefix(1)),
            pictureUrl: profile?.picture ?? ""
        )
    }

    // MARK: - Card projection (inline port of reading_feed_card_projection in reads.rs)

    private var cardProjection: ReadingFeedCardProjection {
        let article = item.article

        // Title fallback
        let displayTitle = article.title.isEmpty ? "Untitled" : article.title
        let titleIsFallback = article.title.isEmpty

        // Image URL (nil when empty)
        let imageUrl: String? = article.image.isEmpty ? nil : article.image

        // Meta text: "N min read" and first hashtag
        var metaBits: [String] = []
        let words = article.content.split(separator: " ").count
        if words > 60 {
            let minutes = max(1, words / 240)
            metaBits.append("\(minutes) min read")
        }
        if let tag = article.hashtags.first(where: { !$0.isEmpty }) {
            metaBits.append("#\(tag)")
        }
        let metaText: String? = metaBits.isEmpty ? nil : metaBits.joined(separator: " · ")

        // Social signal
        let interactorPubkeys = item.interactorPubkeys
        let primaryInteractorPubkey = interactorPubkeys.first
        let visibleInteractorPubkeys = Array(interactorPubkeys.prefix(3))

        let primaryName: String = {
            guard let pk = primaryInteractorPubkey else { return "Someone" }
            let profile = app.profileSnapshots[pk]
            if let d = profile?.displayName, !d.isEmpty { return d }
            if let n = profile?.name, !n.isEmpty { return n }
            return String(pk.prefix(8))
        }()

        let interactorCount = interactorPubkeys.count
        let showSocialSignal = !interactorPubkeys.isEmpty
            || (item.authorFollowed && interactorPubkeys.isEmpty)

        let socialText: String = {
            if item.authorFollowed && interactorPubkeys.isEmpty {
                return "From someone you follow"
            }
            switch interactorCount {
            case 0: return ""
            case 1 where item.authorFollowed: return "\(primaryName) and the author liked this"
            case 1: return "\(primaryName) liked this"
            case 2: return "\(primaryName) and 1 other"
            default: return "\(primaryName) and \(interactorCount - 1) others"
            }
        }()

        // Timestamp: published_at preferred, then created_at
        let relativeUnixSeconds = article.publishedAt ?? article.createdAt

        return ReadingFeedCardProjection(
            displayTitle: displayTitle,
            titleIsFallback: titleIsFallback,
            imageUrl: imageUrl,
            metaText: metaText,
            showSocialSignal: showSocialSignal,
            visibleInteractorPubkeys: visibleInteractorPubkeys,
            primaryInteractorPubkey: primaryInteractorPubkey,
            socialText: socialText,
            relativeUnixSeconds: relativeUnixSeconds.flatMap { $0 > 0 ? $0 : nil }
        )
    }

    // MARK: - Derived bits

    private func relativeDate(_ seconds: UInt64?) -> String? {
        guard let seconds else { return nil }
        let date = Date(timeIntervalSince1970: TimeInterval(seconds))
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        formatter.dateTimeStyle = .numeric
        return formatter.localizedString(for: date, relativeTo: Date())
    }

}
