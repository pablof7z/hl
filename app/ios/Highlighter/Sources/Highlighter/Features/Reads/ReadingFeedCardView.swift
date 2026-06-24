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

    private var authorDisplay: ProfileDisplayProjection {
        {
            let profile = app.profileSnapshots[item.article.pubkey]
            let name = (profile?.displayName ?? "").isEmpty
                ? ((profile?.name ?? "").isEmpty ? String(item.article.pubkey.prefix(10)) : profile!.name)
                : profile!.displayName
            return ProfileDisplayProjection(
                displayName: name,
                displayInitial: name.first.map { String($0).uppercased() } ?? "?",
                pictureUrl: profile?.picture ?? ""
            )
        }()
    }

    private var cardProjection: ReadingFeedCardProjection {
        let article = item.article
        let titleIsFallback = article.title.isEmpty
        let displayTitle = titleIsFallback ? "Untitled" : article.title

        // reading_meta_bits: read-time estimate + first hashtag
        var metaBits: [String] = []
        let wordCount = article.content.split(whereSeparator: \.isWhitespace).count
        if wordCount > 60 {
            metaBits.append("\(max(wordCount / 240, 1)) min read")
        }
        if let tag = article.hashtags.first, !tag.isEmpty {
            metaBits.append("#\(tag)")
        }
        let metaText: String? = metaBits.isEmpty ? nil : metaBits.joined(separator: " · ")

        let interactorPubkeys = item.interactorPubkeys
        let interactorCount = interactorPubkeys.count
        let primaryPubkey = interactorPubkeys.first

        // interactor_display_name: profile display_name → name → pubkey prefix(10)
        func interactorName(_ pk: String) -> String {
            let p = app.profileSnapshots[pk]
            if let dn = p?.displayName, !dn.isEmpty { return dn }
            if let n = p?.name, !n.isEmpty { return n }
            return String(pk.prefix(10))
        }

        let primaryName = primaryPubkey.map { interactorName($0) } ?? "Someone"

        let socialText: String
        if item.authorFollowed && interactorPubkeys.isEmpty {
            socialText = "From someone you follow"
        } else {
            switch interactorCount {
            case 0:
                socialText = ""
            case 1 where item.authorFollowed:
                socialText = "\(primaryName) and the author liked this"
            case 1:
                socialText = "\(primaryName) liked this"
            case 2:
                socialText = "\(primaryName) and 1 other"
            default:
                socialText = "\(primaryName) and \(interactorCount - 1) others"
            }
        }

        let relativeUnixSeconds: UInt64? = [article.publishedAt, article.createdAt]
            .compactMap { $0 }
            .first { $0 > 0 }

        return ReadingFeedCardProjection(
            displayTitle: displayTitle,
            titleIsFallback: titleIsFallback,
            imageUrl: article.image.isEmpty ? nil : article.image,
            metaText: metaText,
            showSocialSignal: !interactorPubkeys.isEmpty || item.authorFollowed,
            visibleInteractorPubkeys: Array(interactorPubkeys.prefix(3)),
            primaryInteractorPubkey: primaryPubkey,
            socialText: socialText,
            relativeUnixSeconds: relativeUnixSeconds
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
