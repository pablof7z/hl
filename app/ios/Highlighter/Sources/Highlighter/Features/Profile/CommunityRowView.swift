import Kingfisher
import SwiftUI

/// Community row on a profile's Communities tab. Taps route through the
/// enclosing `NavigationStack`'s `.navigationDestination(for: String.self)`
/// into `RoomHomeView`, which already exists.
struct CommunityRowView: View {
    let community: CommunitySummary

    private var subtitleText: String? {
        if !community.about.isEmpty { return community.about }
        if let count = community.memberCount, count > 0 {
            return count == 1 ? "1 member" : "\(count) members"
        }
        return nil
    }

    var body: some View {
        let displayName = community.name.isEmpty ? community.id : community.name
        let pictureUrl: String? = community.picture.isEmpty ? nil : community.picture
        let projection = CommunityRowProjection(
            displayName: displayName,
            pictureUrl: pictureUrl,
            subtitle: subtitleText
        )

        HStack(spacing: 14) {
            thumbnail(projection)
                .frame(width: 52, height: 52)
                .clipShape(RoundedRectangle(cornerRadius: 12))

            VStack(alignment: .leading, spacing: 3) {
                Text(projection.displayName)
                    .font(.body.weight(.semibold))
                    .foregroundStyle(Color.highlighterInkStrong)
                    .lineLimit(1)

                if let subtitle = projection.subtitle {
                    Text(subtitle)
                        .font(.footnote)
                        .foregroundStyle(Color.highlighterInkMuted)
                        .lineLimit(2)
                }
            }

            Spacer()

            Image(systemName: "chevron.right")
                .font(.footnote.weight(.semibold))
                .foregroundStyle(Color.highlighterInkMuted.opacity(0.6))
        }
        .padding(.vertical, 10)
        .contentShape(Rectangle())
    }

    @ViewBuilder
    private func thumbnail(_ projection: CommunityRowProjection) -> some View {
        if let picture = projection.pictureUrl, let url = URL(string: picture) {
            KFImage(url)
                .placeholder { Color.highlighterRule.opacity(0.5) }
                .fade(duration: 0.15)
                .resizable()
                .scaledToFill()
        } else {
            RoundedRectangle(cornerRadius: 12)
                .fill(Color.highlighterRule.opacity(0.5))
                .overlay(
                    Image(systemName: "square.grid.2x2")
                        .foregroundStyle(Color.highlighterInkMuted)
                )
        }
    }
}
