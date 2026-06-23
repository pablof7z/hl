import Kingfisher
import SwiftUI

/// Compact 1:1 tile used on the "Your rooms" shelf. Cover + name beneath,
/// sized for quick glanceability rather than discovery appeal.
struct RoomSquareTile: View {
    let room: CommunitySummary

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            cover
                .frame(width: 96, height: 96)
                .clipped()
                .clipShape(RoundedRectangle(cornerRadius: 14))
                .overlay(
                    RoundedRectangle(cornerRadius: 14)
                        .stroke(Color.highlighterRule, lineWidth: 0.5)
                )
            Text(room.name)
                .font(.caption.weight(.medium))
                .foregroundStyle(Color.highlighterInkStrong)
                .lineLimit(2)
                .multilineTextAlignment(.leading)
                .frame(width: 96, alignment: .leading)
        }
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
        ZStack {
            LinearGradient(
                colors: [
                    Color.highlighterAccent.opacity(0.36),
                    Color.highlighterAccent.opacity(0.16),
                ],
                startPoint: .topLeading,
                endPoint: .bottomTrailing
            )
            if !avatar.displayInitial.isEmpty {
                Text(avatar.displayInitial)
                    .font(.system(size: 40, weight: .semibold))
                    .foregroundStyle(Color.white.opacity(0.88))
            }
        }
    }

    private var avatarProjection: RoomAvatarProjection {
        // Inlined room_avatar_projection: pass-through picture, uppercase first char of name
        let initial = room.name.first.map { String($0).uppercased() } ?? ""
        return RoomAvatarProjection(pictureUrl: room.picture, displayInitial: initial)
    }
}
