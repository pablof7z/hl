import Kingfisher
import SwiftUI

private struct RoomRecommendationAvatarProjection {
    let pubkey: String
    let pictureUrl: String
    let displayInitial: String
}

private struct RoomRecommendationCardProjection {
    let byline: String
    let visibleAvatars: [RoomRecommendationAvatarProjection]
    let preloadPubkeys: [String]
    let overflowLabel: String?
}

/// 3:4 cover card with an overlapping avatar cluster in the bottom-left —
/// the social-proof shelf for "Friends are here". Caption reads
/// "@alice, @bob + 2" or just "@alice + 1" depending on count.
struct FriendsOnRoomCard: View {
    let recommendation: RoomRecommendation

    @Environment(HighlighterStore.self) private var store

    private let width: CGFloat = 96

    var body: some View {
        let projection = cardProjection

        VStack(alignment: .leading, spacing: 8) {
            ZStack(alignment: .bottomLeading) {
                cover
                    .frame(width: width, height: width)
                    .clipped()
                    .clipShape(RoundedRectangle(cornerRadius: 14))
                    .overlay(
                        RoundedRectangle(cornerRadius: 14)
                            .stroke(Color.highlighterRule, lineWidth: 0.5)
                    )

                avatarCluster(projection)
                    .padding(8)
            }

            VStack(alignment: .leading, spacing: 2) {
                Text(recommendation.summary.name)
                    .font(.caption.weight(.medium))
                    .foregroundStyle(Color.highlighterInkStrong)
                    .lineLimit(2)
                    .multilineTextAlignment(.leading)

                Text(projection.byline)
                    .font(.caption2)
                    .foregroundStyle(Color.highlighterInkMuted)
                    .lineLimit(1)
            }
            .frame(width: width, alignment: .leading)
        }
        .task(id: projection.preloadPubkeys) {
            // Warm the profile cache for the friends shown in the cluster
            // so avatars render with actual pictures, not initials.
            for pubkey in projection.preloadPubkeys {
                await store.requestProfile(pubkeyHex: pubkey)
            }
        }
    }

    private var cardProjection: RoomRecommendationCardProjection {
        let reasonPubkeys = recommendation.reasonPubkeys
        let total = reasonPubkeys.count

        // Build byline using profile_handle order: name > displayName > pubkey[:6]
        let byline: String
        if let firstPubkey = reasonPubkeys.first {
            let profile = store.profileSnapshots[firstPubkey]
            let handle: String
            if let p = profile, !p.name.isEmpty {
                handle = p.name
            } else if let p = profile, !p.displayName.isEmpty {
                handle = p.displayName
            } else {
                handle = String(firstPubkey.prefix(6))
            }
            switch total {
            case 1:  byline = "@\(handle) is here"
            case 2:  byline = "@\(handle) + 1 you follow"
            default: byline = "@\(handle) + \(total - 1) you follow"
            }
        } else {
            let about = recommendation.summary.about.trimmingCharacters(in: .whitespaces)
            byline = about.isEmpty ? "Rooms you may like" : about
        }

        // Up to 3 visible avatars; displayInitial is always first char of pubkey
        let visiblePubkeys = Array(reasonPubkeys.prefix(3))
        let visibleAvatars: [RoomRecommendationAvatarProjection] = visiblePubkeys.map { pubkey in
            RoomRecommendationAvatarProjection(
                pubkey: pubkey,
                pictureUrl: store.profileSnapshots[pubkey]?.picture ?? "",
                displayInitial: String(pubkey.prefix(1))
            )
        }

        let overflowCount = total - 3
        let overflowLabel: String? = overflowCount > 0 ? "+\(overflowCount)" : nil

        return RoomRecommendationCardProjection(
            byline: byline,
            visibleAvatars: visibleAvatars,
            preloadPubkeys: visiblePubkeys,
            overflowLabel: overflowLabel
        )
    }

    @ViewBuilder
    private var cover: some View {
        if let url = URL(string: recommendation.summary.picture), !recommendation.summary.picture.isEmpty {
            KFImage(url)
                .placeholder { coverFallback }
                .fade(duration: 0.15)
                .resizable()
                .scaledToFill()
        } else {
            coverFallback
        }
    }

    private var coverFallback: some View {
        LinearGradient(
            colors: [
                Color.highlighterAccent.opacity(0.38),
                Color.highlighterAccent.opacity(0.14),
            ],
            startPoint: .topLeading,
            endPoint: .bottomTrailing
        )
    }

    private func avatarCluster(_ projection: RoomRecommendationCardProjection) -> some View {
        HStack(spacing: -8) {
            ForEach(Array(projection.visibleAvatars.enumerated()), id: \.offset) { item in
                let avatar = item.element
                AuthorAvatar(
                    pubkey: avatar.pubkey,
                    pictureURL: avatar.pictureUrl,
                    displayInitial: avatar.displayInitial,
                    size: 26
                )
                .overlay(
                    Circle().stroke(Color.white, lineWidth: 2)
                )
            }
            if let overflowLabel = projection.overflowLabel {
                ZStack {
                    Circle().fill(Color.black.opacity(0.55))
                    Text(overflowLabel)
                        .font(.caption2.weight(.bold))
                        .foregroundStyle(.white)
                }
                .frame(width: 26, height: 26)
                .overlay(Circle().stroke(Color.white, lineWidth: 2))
            }
        }
    }
}
