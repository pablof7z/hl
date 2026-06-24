import Kingfisher
import SwiftUI

struct RoomLibraryPodcastCardView: View {
    @Environment(HighlighterStore.self) private var app

    let artifact: ArtifactRecord
    var commentCount: Int = 0

    var body: some View {
        let projection = cardProjection

        VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .top, spacing: 16) {
                VStack(alignment: .leading, spacing: 6) {
                    Text(projection.title)
                        .font(.title3.weight(.semibold))
                        .foregroundStyle(
                            projection.titleIsFallback
                                ? Color.highlighterInkMuted
                                : Color.highlighterInkStrong
                        )
                        .lineLimit(3)
                        .fixedSize(horizontal: false, vertical: true)

                    HStack(spacing: 4) {
                        if let showLabel = projection.showLabel {
                            Text(showLabel.uppercased())
                                .font(.caption2.weight(.bold))
                                .tracking(0.6)
                                .foregroundStyle(Color.highlighterInkMuted)
                                .lineLimit(1)
                        }
                        if let duration = projection.durationLabel, projection.showLabel != nil {
                            Text("·")
                                .font(.caption2)
                                .foregroundStyle(Color.highlighterInkMuted)
                            Text(duration)
                                .font(.caption2)
                                .foregroundStyle(Color.highlighterInkMuted)
                        } else if let duration = projection.durationLabel {
                            Text(duration)
                                .font(.caption2)
                                .foregroundStyle(Color.highlighterInkMuted)
                        }
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)

                podcastArtwork(projection)
            }

            sharerRow(projection)
        }
        .padding(.vertical, 18)
        .contentShape(Rectangle())
        .task(id: projection.sharerPubkey) {
            await app.requestProfile(pubkeyHex: projection.sharerPubkey)
        }
    }

    @ViewBuilder
    private func sharerRow(_ projection: RoomLibraryPodcastCardProjection) -> some View {
        let sharer = sharerDisplay(projection)

        HStack(spacing: 6) {
            AuthorAvatar(
                pubkey: projection.sharerPubkey,
                pictureURL: sharer.pictureUrl,
                displayInitial: sharer.displayInitial,
                size: 18
            )

            Text(sharer.displayName.uppercased())
                .font(.caption2.weight(.bold))
                .tracking(0.6)
                .foregroundStyle(Color.highlighterInkMuted)
                .lineLimit(1)

            if let date = relativeDate(projection.relativeUnixSeconds) {
                Text("·")
                    .font(.caption2)
                    .foregroundStyle(Color.highlighterInkMuted)
                Text(date)
                    .font(.caption2)
                    .foregroundStyle(Color.highlighterInkMuted)
                    .lineLimit(1)
            }

            Spacer(minLength: 0)

            if let commentBadge = projection.commentBadgeLabel {
                HStack(spacing: 3) {
                    Image(systemName: "bubble.left")
                        .font(.caption2)
                    Text(commentBadge)
                        .font(.caption2.weight(.semibold))
                }
                .foregroundStyle(Color.highlighterInkMuted)
            }
        }
    }

    @ViewBuilder
    private func podcastArtwork(_ projection: RoomLibraryPodcastCardProjection) -> some View {
        Group {
            if let image = projection.imageUrl, let url = URL(string: image) {
                KFImage(url)
                    .placeholder { artworkPlaceholder }
                    .fade(duration: 0.15)
                    .resizable()
                    .scaledToFill()
            } else {
                artworkPlaceholder
            }
        }
        .frame(width: 96, height: 96)
        .clipShape(RoundedRectangle(cornerRadius: 6, style: .continuous))
    }

    private var artworkPlaceholder: some View {
        LinearGradient(
            colors: [Color.highlighterRule.opacity(0.7), Color.highlighterRule.opacity(0.35)],
            startPoint: .topLeading,
            endPoint: .bottomTrailing
        )
        .overlay(
            Image(systemName: "waveform")
                .font(.title3)
                .foregroundStyle(Color.highlighterInkMuted.opacity(0.7))
        )
    }

    private var cardProjection: RoomLibraryPodcastCardProjection {
        // D1: derive all display fields from ArtifactPreview + ArtifactRecord directly.
        let title = artifact.preview.title
        return RoomLibraryPodcastCardProjection(
            title: title.isEmpty ? artifact.preview.catalogId : title,
            titleIsFallback: title.isEmpty,
            showLabel: artifact.preview.author.isEmpty ? nil : artifact.preview.author,
            durationLabel: nil,
            imageUrl: artifact.preview.image.isEmpty ? nil : artifact.preview.image,
            sharerPubkey: artifact.pubkey,
            relativeUnixSeconds: artifact.createdAt,
            commentBadgeLabel: commentCount > 0 ? "\(commentCount)" : nil
        )
    }

    private func sharerDisplay(
        _ projection: RoomLibraryPodcastCardProjection
    ) -> ProfileDisplayProjection {
        {
            let profile = app.profileSnapshots[projection.sharerPubkey]
            let name = (profile?.displayName ?? "").isEmpty
                ? ((profile?.name ?? "").isEmpty ? String(projection.sharerPubkey.prefix(10)) : profile!.name)
                : profile!.displayName
            return ProfileDisplayProjection(
                displayName: name,
                displayInitial: name.first.map { String($0).uppercased() } ?? "?",
                pictureUrl: profile?.picture ?? ""
            )
        }()
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
