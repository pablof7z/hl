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

    /// Pure-Swift replacement for `projectRoomLibraryArticleCard`.
    private var cardProjection: RoomLibraryArticleCardProjection {
        let preview = artifact.preview
        let displayTitle = preview.title.isEmpty ? "Untitled" : preview.title
        let titleIsFallback = preview.title.isEmpty
        let imageUrl: String? = preview.image.isEmpty ? nil : preview.image
        let articleAuthorPubkey = Self.articleAuthorPubkey(preview: preview)
        let authorProfilePubkey = articleAuthorPubkey ?? ""
        let avatarPubkey = articleAuthorPubkey ?? artifact.pubkey
        let relativeUnixSeconds: UInt64? = artifact.createdAt.flatMap { $0 > 0 ? $0 : nil }
        let metaText: String? = {
            var bits: [String] = []
            if !preview.domain.isEmpty { bits.append(preview.domain) }
            if commentCount > 0 {
                bits.append("\(commentCount) comment\(commentCount == 1 ? "" : "s")")
            }
            return bits.isEmpty ? nil : bits.joined(separator: " · ")
        }()
        return RoomLibraryArticleCardProjection(
            displayTitle: displayTitle,
            titleIsFallback: titleIsFallback,
            imageUrl: imageUrl,
            articleAuthorPubkey: articleAuthorPubkey,
            avatarPubkey: avatarPubkey,
            authorProfilePubkey: authorProfilePubkey,
            relativeUnixSeconds: relativeUnixSeconds,
            metaText: metaText
        )
    }

    /// Extract the NIP-23 article author pubkey from the preview's reference
    /// tags, mirroring `article_card_projection` in Rust.
    ///
    /// Returns a pubkey only when the source is "article" and there is a
    /// well-formed `"a"` reference of the form `"30023:<pubkey>:<d-tag>"`.
    private static func articleAuthorPubkey(preview: ArtifactPreview) -> String? {
        guard preview.source.trimmingCharacters(in: .whitespaces).lowercased() == "article" else {
            return nil
        }
        // reference_value_for(preview, "a"): highlight tag wins, then reference tag.
        let raw: String
        if preview.highlightTagName.caseInsensitiveCompare("a") == .orderedSame,
           !preview.highlightTagValue.trimmingCharacters(in: .whitespaces).isEmpty {
            raw = preview.highlightTagValue
        } else if preview.referenceTagName.caseInsensitiveCompare("a") == .orderedSame,
                  !preview.referenceTagValue.trimmingCharacters(in: .whitespaces).isEmpty {
            raw = preview.referenceTagValue
        } else {
            return nil
        }
        // parse_nip23_address: split at most at the first two colons (Rust splitn(3, ':')).
        let trimmed = raw.trimmingCharacters(in: .whitespaces)
        guard let c1 = trimmed.firstIndex(of: ":") else { return nil }
        let part0 = String(trimmed[..<c1])
        let rest = String(trimmed[trimmed.index(after: c1)...])
        guard let c2 = rest.firstIndex(of: ":") else { return nil }
        let part1 = String(rest[..<c2])
        let part2 = String(rest[rest.index(after: c2)...])
        guard part0 == "30023", !part1.isEmpty, !part2.isEmpty else { return nil }
        return part1
    }

    private func authorDisplay(
        _ projection: RoomLibraryArticleCardProjection
    ) -> ProfileDisplayProjection {
        let pubkey = projection.authorProfilePubkey
        return ProfileDisplayProjection.from(pubkey: pubkey, profile: pubkey.isEmpty ? nil : app.profileSnapshots[pubkey])
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
