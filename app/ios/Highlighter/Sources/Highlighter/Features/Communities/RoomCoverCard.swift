import Kingfisher
import SwiftUI

/// Portrait (3:4) cover card used by the curated and "new" shelves. Shelves
/// wrap this in a fixed-width container; the grid uses it with flexible
/// widths via `GridItem(.flexible())`. The name uses sans-serif weight.
struct RoomCoverCard: View {
    let room: CommunitySummary
    /// When `nil`, the card fills the width its container gives it.
    let fixedWidth: CGFloat?

    @Environment(HighlighterStore.self) private var store

    init(room: CommunitySummary, width: CGFloat? = nil) {
        self.room = room
        self.fixedWidth = width
    }

    var body: some View {
        let projection = cardProjection
        VStack(alignment: .leading, spacing: 8) {
            Color.clear
                .aspectRatio(3 / 4, contentMode: .fit)
                .overlay(cover)
                .clipShape(RoundedRectangle(cornerRadius: 14))
                .overlay(
                    RoundedRectangle(cornerRadius: 14)
                        .stroke(Color.highlighterRule, lineWidth: 0.5)
                )

            VStack(alignment: .leading, spacing: 2) {
                Text(room.name)
                    .font(.subheadline.weight(.semibold))
                    .foregroundStyle(Color.highlighterInkStrong)
                    .lineLimit(2)
                    .multilineTextAlignment(.leading)

                Text(projection.subtitle)
                    .font(.caption)
                    .foregroundStyle(Color.highlighterInkMuted)
                    .lineLimit(1)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .frame(width: fixedWidth)
    }

    private var cardProjection: RoomCoverCardProjection {
        store.safeCore.projectRoomCoverCard(
            input: RoomCoverCardProjectionInput(room: room)
        )
    }

    @ViewBuilder
    private var cover: some View {
        let avatar = avatarProjection
        if let url = URL(string: avatar.pictureUrl), !avatar.pictureUrl.isEmpty {
            KFImage(url)
                .placeholder { coverFallback(avatar) }
                .fade(duration: 0.15)
                .resizable()
                .scaledToFill()
        } else {
            coverFallback(avatar)
        }
    }

    private func coverFallback(_ avatar: RoomAvatarProjection) -> some View {
        GeometryReader { geo in
            ZStack {
                LinearGradient(
                    colors: [
                        Color.highlighterAccent.opacity(0.42),
                        Color.highlighterAccent.opacity(0.18),
                    ],
                    startPoint: .topLeading,
                    endPoint: .bottomTrailing
                )
                if !avatar.displayInitial.isEmpty {
                    Text(avatar.displayInitial)
                        .font(.system(size: geo.size.width * 0.42, weight: .semibold))
                        .foregroundStyle(Color.white.opacity(0.88))
                }
            }
        }
    }

    private var avatarProjection: RoomAvatarProjection {
        store.safeCore.projectRoomAvatar(
            input: RoomAvatarProjectionInput(
                name: room.name,
                pictureUrl: room.picture,
                uppercaseInitial: true
            )
        )
    }
}
