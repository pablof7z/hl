import SwiftUI

private struct RoomLibraryArticleCardProjection {
    let displayTitle: String
    let titleIsFallback: Bool
    let imageUrl: String?
    let articleAuthorPubkey: String?
    let avatarPubkey: String
    let authorProfilePubkey: String
    let relativeUnixSeconds: UInt64?
    let metaText: String?
}

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
        // D1: inline article_card_projection — article author extracted from the NIP-23
        // "a"-tag reference in the artifact preview; domain and comment-count form meta text.
        let preview = artifact.preview
        let displayTitle = preview.title.isEmpty ? "Untitled" : preview.title
        let titleIsFallback = preview.title.isEmpty

        // Article author pubkey: only for "article" source with a valid "a"-tag reference.
        let articleAuthorPubkey: String? = {
            guard preview.source.trimmingCharacters(in: .whitespaces).lowercased() == "article"
            else { return nil }
            return Self.articleAuthorPubkeyFrom(preview: preview)
        }()

        let avatarPubkey = articleAuthorPubkey ?? artifact.pubkey
        let authorProfilePubkey = articleAuthorPubkey ?? ""
        let relativeUnixSeconds: UInt64? = artifact.createdAt.flatMap { $0 > 0 ? $0 : nil }

        var metaBits: [String] = []
        if !preview.domain.isEmpty { metaBits.append(preview.domain) }
        let cc = commentCount
        if cc > 0 { metaBits.append("\(cc) comment\(cc == 1 ? "" : "s")") }
        let metaText: String? = metaBits.isEmpty ? nil : metaBits.joined(separator: " · ")

        return RoomLibraryArticleCardProjection(
            displayTitle: displayTitle,
            titleIsFallback: titleIsFallback,
            imageUrl: preview.image.isEmpty ? nil : preview.image,
            articleAuthorPubkey: articleAuthorPubkey,
            avatarPubkey: avatarPubkey,
            authorProfilePubkey: authorProfilePubkey,
            relativeUnixSeconds: relativeUnixSeconds,
            metaText: metaText
        )
    }

    /// Extract the article author pubkey from an ArtifactPreview by reading either the
    /// `highlightTagName`/`highlightTagValue` or `referenceTagName`/`referenceTagValue`
    /// fields (mirrors artifact_detail.rs `reference_value_for(preview, "a")`).
    private static func articleAuthorPubkeyFrom(preview: ArtifactPreview) -> String? {
        let address: String
        if preview.highlightTagName.caseInsensitiveCompare("a") == .orderedSame,
           !preview.highlightTagValue.trimmingCharacters(in: .whitespaces).isEmpty {
            address = preview.highlightTagValue
        } else if preview.referenceTagName.caseInsensitiveCompare("a") == .orderedSame,
                  !preview.referenceTagValue.trimmingCharacters(in: .whitespaces).isEmpty {
            address = preview.referenceTagValue
        } else {
            return nil
        }
        let parts = address.trimmingCharacters(in: .whitespaces)
            .split(separator: ":", maxSplits: 2, omittingEmptySubsequences: false)
        guard parts.count == 3,
              parts[0] == "30023",
              !String(parts[1]).isEmpty,
              !String(parts[2]).isEmpty else { return nil }
        return String(parts[1])
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
