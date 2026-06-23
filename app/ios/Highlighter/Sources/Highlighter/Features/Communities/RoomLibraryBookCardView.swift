import Kingfisher
import SwiftUI

struct RoomLibraryBookCardView: View {
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

                    if let author = projection.authorLabel {
                        Text(author.uppercased())
                            .font(.caption2.weight(.bold))
                            .tracking(0.6)
                            .foregroundStyle(Color.highlighterInkMuted)
                            .lineLimit(1)
                    }

                    if let summary = projection.summary {
                        Text(summary)
                            .font(.subheadline)
                            .foregroundStyle(Color.highlighterInkMuted)
                            .lineLimit(2)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)

                bookCover(projection)
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
    private func sharerRow(_ projection: RoomLibraryBookCardProjection) -> some View {
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
    private func bookCover(_ projection: RoomLibraryBookCardProjection) -> some View {
        Group {
            if let image = projection.imageUrl, let url = URL(string: image) {
                KFImage(url)
                    .placeholder { bookPlaceholder }
                    .fade(duration: 0.15)
                    .resizable()
                    .scaledToFill()
            } else {
                bookPlaceholder
            }
        }
        .frame(width: 64, height: 96)
        .clipShape(RoundedRectangle(cornerRadius: 4, style: .continuous))
        .shadow(color: .black.opacity(0.12), radius: 4, x: 0, y: 2)
    }

    private var bookPlaceholder: some View {
        LinearGradient(
            colors: [Color.highlighterRule.opacity(0.7), Color.highlighterRule.opacity(0.35)],
            startPoint: .topLeading,
            endPoint: .bottomTrailing
        )
        .overlay(
            Image(systemName: "book.closed")
                .font(.title3)
                .foregroundStyle(Color.highlighterInkMuted.opacity(0.7))
        )
    }

    private var cardProjection: RoomLibraryBookCardProjection {
        app.safeCore.projectRoomLibraryBookCard(
            input: RoomLibraryBookCardProjectionInput(
                artifact: artifact,
                commentCount: UInt32(commentCount)
            )
        )
    }

    private func sharerDisplay(_ projection: RoomLibraryBookCardProjection) -> ProfileDisplayProjection {
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
