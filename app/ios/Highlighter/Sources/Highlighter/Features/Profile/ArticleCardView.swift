import Kingfisher
import SwiftUI

/// Article row on a profile's Writing tab. Mirrors the web `ArticleCard`:
/// title + summary (2-line clamp) on the left, 96×72 thumbnail on the
/// right, metadata row underneath.
struct ArticleCardView: View {
    @Environment(HighlighterStore.self) private var app

    let article: ArticleRecord

    var body: some View {
        let projection = cardProjection

        HStack(alignment: .top, spacing: 16) {
            VStack(alignment: .leading, spacing: 8) {
                Text(projection.title)
                    .font(.title3.weight(.semibold))
                    .foregroundStyle(
                        projection.titleIsFallback
                            ? Color.highlighterInkMuted
                            : Color.highlighterInkStrong
                    )
                    .lineLimit(3)

                if !article.summary.isEmpty {
                    Text(article.summary)
                        .font(.subheadline)
                        .foregroundStyle(Color.highlighterInkMuted)
                    .lineLimit(2)
                }

                metadataRow(projection)
            }
            .frame(maxWidth: .infinity, alignment: .leading)

            if let url = thumbnailURL {
                KFImage(url)
                    .placeholder { Color.highlighterRule.opacity(0.4) }
                    .fade(duration: 0.15)
                    .resizable()
                    .scaledToFill()
                    .frame(width: 96, height: 72)
                    .clipShape(RoundedRectangle(cornerRadius: 8))
            }
        }
        .padding(.vertical, 14)
    }

    private var thumbnailURL: URL? {
        guard !article.image.isEmpty else { return nil }
        return URL(string: article.image)
    }

    private func metadataRow(_ projection: ArticleProfileCardProjection) -> some View {
        HStack(spacing: 10) {
            if let date = displayDate(projection.displayUnixSeconds) {
                Text(date)
            }
            if let hashtags = projection.hashtagSummary {
                Text("·")
                    .foregroundStyle(Color.highlighterInkMuted)
                Text(hashtags)
                    .lineLimit(1)
            }
        }
        .font(.caption)
        .foregroundStyle(Color.highlighterInkMuted)
    }

    private var cardProjection: ArticleProfileCardProjection {
        let titleIsFallback = article.title.isEmpty
        let title = titleIsFallback ? "Untitled" : article.title
        let rawSeconds = article.publishedAt ?? article.createdAt
        let displayUnixSeconds: UInt64? = rawSeconds.flatMap { $0 > 0 ? $0 : nil }
        let hashtagSummary: String? = article.hashtags.isEmpty
            ? nil
            : article.hashtags.prefix(2).map { "#\($0)" }.joined(separator: " ")
        return ArticleProfileCardProjection(
            title: title,
            titleIsFallback: titleIsFallback,
            displayUnixSeconds: displayUnixSeconds,
            hashtagSummary: hashtagSummary
        )
    }

    private func displayDate(_ seconds: UInt64?) -> String? {
        guard let seconds else { return nil }
        let date = Date(timeIntervalSince1970: TimeInterval(seconds))
        let formatter = DateFormatter()
        formatter.dateStyle = .medium
        formatter.timeStyle = .none
        return formatter.string(from: date)
    }
}
