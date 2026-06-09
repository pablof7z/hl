import Kingfisher
import SwiftUI

/// Community row on a profile's Communities tab. Taps route through the
/// enclosing `NavigationStack`'s `.navigationDestination(for: String.self)`
/// into `RoomHomeView`, which already exists.
struct CommunityRowView: View {
    @Environment(HighlighterStore.self) private var app

    let community: CommunitySummary

    var body: some View {
        let projection = app.safeCore.projectCommunityRow(
            input: CommunityRowProjectionInput(community: community)
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

                if !community.about.isEmpty {
                    Text(community.about)
                        .font(.footnote)
                        .foregroundStyle(Color.highlighterInkMuted)
                        .lineLimit(2)
                } else if let count = community.memberCount {
                    Text("\(count) member\(count == 1 ? "" : "s")")
                        .font(.footnote)
                        .foregroundStyle(Color.highlighterInkMuted)
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
