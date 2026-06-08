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
        ReadingCard(
            title: artifact.preview.title,
            summary: artifact.preview.description,
            imageURL: coverURL,
            authorName: authorDisplayName,
            authorPubkey: articleAuthorPubkey,
            relativeDate: relativeDate,
            metaBits: metaBits,
            showTrailing: false,
            avatar: {
                let pubkey = articleAuthorPubkey ?? artifact.pubkey
                AuthorAvatar(
                    pubkey: pubkey,
                    pictureURL: app.profileSnapshots[pubkey]?.picture ?? "",
                    displayInitial: authorInitial,
                    size: 22
                )
            },
            trailing: { EmptyView() }
        )
        .task(id: articleAuthorPubkey ?? "") {
            guard let pk = articleAuthorPubkey else { return }
            await app.requestProfile(pubkeyHex: pk)
        }
    }

    // MARK: - Derived bits

    private var coverURL: URL? {
        guard !artifact.preview.image.isEmpty else { return nil }
        return URL(string: artifact.preview.image)
    }

    private var articleAuthorPubkey: String? {
        let route = app.core.getArtifactDetailRoute(artifact: artifact)
        guard route.target == .article, !route.articlePubkey.isEmpty else { return nil }
        return route.articlePubkey
    }

    private var authorDisplayName: String {
        let profile = articleAuthorPubkey.flatMap { app.profileSnapshots[$0] }
        if let dn = profile?.displayName, !dn.isEmpty { return dn }
        if let n = profile?.name, !n.isEmpty { return n }
        if !artifact.preview.author.isEmpty { return artifact.preview.author }
        if let pk = articleAuthorPubkey { return String(pk.prefix(10)) }
        return "Unknown"
    }

    private var authorInitial: String {
        authorDisplayName.first.map { String($0).uppercased() } ?? ""
    }

    private var relativeDate: String? {
        guard let seconds = artifact.createdAt, seconds > 0 else { return nil }
        let date = Date(timeIntervalSince1970: TimeInterval(seconds))
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        formatter.dateTimeStyle = .numeric
        return formatter.localizedString(for: date, relativeTo: Date())
    }

    private var metaBits: [String] {
        var out: [String] = []
        if !artifact.preview.domain.isEmpty { out.append(artifact.preview.domain) }
        if commentCount > 0 {
            out.append("\(commentCount) comment\(commentCount == 1 ? "" : "s")")
        }
        return out
    }
}
