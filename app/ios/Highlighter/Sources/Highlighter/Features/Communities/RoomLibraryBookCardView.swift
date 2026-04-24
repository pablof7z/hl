import Kingfisher
import SwiftUI

struct RoomLibraryBookCardView: View {
    @Environment(HighlighterStore.self) private var app

    let artifact: ArtifactRecord

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            attributionRow

            HStack(alignment: .top, spacing: 16) {
                VStack(alignment: .leading, spacing: 6) {
                    Text(artifact.preview.title.isEmpty ? "Untitled" : artifact.preview.title)
                        .font(.system(.title3, design: .serif).weight(.semibold))
                        .foregroundStyle(
                            artifact.preview.title.isEmpty
                                ? Color.highlighterInkMuted
                                : Color.highlighterInkStrong
                        )
                        .lineLimit(3)
                        .fixedSize(horizontal: false, vertical: true)

                    if !artifact.preview.author.isEmpty {
                        Text(artifact.preview.author)
                            .font(.subheadline)
                            .foregroundStyle(Color.highlighterInkMuted)
                            .lineLimit(1)
                    }

                    if !artifact.preview.description.isEmpty {
                        Text(artifact.preview.description)
                            .font(.subheadline)
                            .foregroundStyle(Color.highlighterInkMuted)
                            .lineLimit(2)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)

                bookCover
            }
        }
        .padding(.vertical, 18)
        .contentShape(Rectangle())
        .task(id: artifact.pubkey) {
            await app.requestProfile(pubkeyHex: artifact.pubkey)
        }
    }

    private var attributionRow: some View {
        HStack(spacing: 8) {
            AuthorAvatar(
                pubkey: artifact.pubkey,
                pictureURL: app.profileCache[artifact.pubkey]?.picture ?? "",
                displayInitial: sharerInitial,
                size: 22
            )

            Text(sharerName)
                .font(.footnote.weight(.semibold))
                .foregroundStyle(Color.highlighterInkStrong)
                .lineLimit(1)

            if let date = relativeDate {
                Text("·")
                    .foregroundStyle(Color.highlighterInkMuted)
                Text(date)
                    .font(.footnote)
                    .foregroundStyle(Color.highlighterInkMuted)
                    .lineLimit(1)
            }

            Spacer(minLength: 0)
        }
    }

    @ViewBuilder
    private var bookCover: some View {
        let image = artifact.preview.image
        Group {
            if !image.isEmpty, let url = URL(string: image) {
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

    private var sharerName: String {
        let profile = app.profileCache[artifact.pubkey]
        if let dn = profile?.displayName, !dn.isEmpty { return dn }
        if let n = profile?.name, !n.isEmpty { return n }
        return String(artifact.pubkey.prefix(10))
    }

    private var sharerInitial: String {
        sharerName.first.map { String($0).uppercased() } ?? ""
    }

    private var relativeDate: String? {
        guard let seconds = artifact.createdAt, seconds > 0 else { return nil }
        let date = Date(timeIntervalSince1970: TimeInterval(seconds))
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        formatter.dateTimeStyle = .numeric
        return formatter.localizedString(for: date, relativeTo: Date())
    }
}
