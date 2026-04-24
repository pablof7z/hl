import SwiftUI

/// Full-bleed "all results of this kind" sub-screen. Pushed when the user
/// taps "See all" on a section header in `SearchView`. Reads directly from
/// the active `SearchStore` so the list stays in sync with the live query —
/// if relay results stream in while this screen is open, they appear here
/// too.
struct SearchSeeAllView: View {
    let target: SearchSeeAllTarget
    let store: SearchStore
    @Environment(HighlighterStore.self) private var app

    var body: some View {
        Group {
            switch target {
            case .highlights:
                highlightsList
            case .articles:
                articlesList
            case .communities:
                communitiesList
            case .people:
                peopleList
            }
        }
        .background(Color.highlighterPaper.ignoresSafeArea())
        .navigationTitle(target.title)
        .navigationBarTitleDisplayMode(.large)
    }

    // MARK: - Highlights

    private var highlightsList: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 0) {
                ForEach(Array(store.highlights.enumerated()), id: \.element.eventId) { idx, h in
                    row(for: h)
                    if idx < store.highlights.count - 1 {
                        Rectangle()
                            .fill(Color.highlighterRule)
                            .frame(height: 0.5)
                    }
                }
            }
            .padding(.horizontal, 20)
            .padding(.vertical, 12)
        }
    }

    @ViewBuilder
    private func row(for h: HighlightRecord) -> some View {
        if let target = articleReaderTarget(for: h) {
            NavigationLink(value: target) {
                SeeAllHighlightRow(highlight: h, query: target.dTag.isEmpty ? store.query : store.query)
            }
            .buttonStyle(.plain)
        } else {
            SeeAllHighlightRow(highlight: h, query: store.query)
        }
    }

    // MARK: - Articles

    private var articlesList: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 0) {
                ForEach(Array(store.articles.enumerated()), id: \.element.eventId) { idx, a in
                    NavigationLink(value: ArticleReaderTarget(pubkey: a.pubkey, dTag: a.identifier, seed: nil)) {
                        SeeAllArticleRow(article: a, query: store.query)
                    }
                    .buttonStyle(.plain)
                    if idx < store.articles.count - 1 {
                        Rectangle()
                            .fill(Color.highlighterRule)
                            .frame(height: 0.5)
                    }
                }
            }
            .padding(.horizontal, 20)
            .padding(.vertical, 12)
        }
    }

    // MARK: - Communities

    private var communitiesList: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 0) {
                ForEach(Array(store.communities.enumerated()), id: \.element.id) { idx, c in
                    NavigationLink(value: c.id) {
                        SeeAllCommunityRow(community: c)
                    }
                    .buttonStyle(.plain)
                    if idx < store.communities.count - 1 {
                        Rectangle()
                            .fill(Color.highlighterRule)
                            .frame(height: 0.5)
                    }
                }
            }
            .padding(.horizontal, 20)
            .padding(.vertical, 12)
        }
    }

    // MARK: - People

    private var peopleList: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 0) {
                ForEach(Array(store.profiles.enumerated()), id: \.element.pubkey) { idx, p in
                    NavigationLink(value: ProfileDestination.pubkey(p.pubkey)) {
                        SeeAllPersonRow(profile: p)
                    }
                    .buttonStyle(.plain)
                    if idx < store.profiles.count - 1 {
                        Rectangle()
                            .fill(Color.highlighterRule)
                            .frame(height: 0.5)
                    }
                }
            }
            .padding(.horizontal, 20)
            .padding(.vertical, 12)
        }
    }

    // MARK: - Helpers

    private func articleReaderTarget(for h: HighlightRecord) -> ArticleReaderTarget? {
        let addr = h.artifactAddress.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !addr.isEmpty else { return nil }
        let parts = addr.split(separator: ":", maxSplits: 2, omittingEmptySubsequences: false)
        guard parts.count == 3, parts[0] == "30023" else { return nil }
        return ArticleReaderTarget(pubkey: String(parts[1]), dTag: String(parts[2]), seed: nil)
    }
}

// MARK: - See-all row variants (a touch denser than the preview rows)

private struct SeeAllHighlightRow: View {
    let highlight: HighlightRecord
    let query: String

    var body: some View {
        HStack(alignment: .top, spacing: 14) {
            Rectangle()
                .fill(Color.highlighterAccent)
                .frame(width: 2.5)
                .clipShape(RoundedRectangle(cornerRadius: 1.25))
            VStack(alignment: .leading, spacing: 6) {
                Text(matched(highlight.quote, query))
                    .font(.system(size: 17, design: .serif).italic())
                    .foregroundStyle(Color.highlighterInkStrong)
                    .lineSpacing(3)
                    .lineLimit(6)
                if !highlight.note.isEmpty {
                    Text("— " + highlight.note)
                        .font(.footnote.italic())
                        .foregroundStyle(Color.highlighterInkMuted)
                        .lineLimit(2)
                }
            }
        }
        .padding(.vertical, 14)
        .contentShape(Rectangle())
    }
}

private struct SeeAllArticleRow: View {
    @Environment(HighlighterStore.self) private var app
    let article: ArticleRecord
    let query: String

    var body: some View {
        HStack(alignment: .top, spacing: 14) {
            articleThumb
            VStack(alignment: .leading, spacing: 6) {
                Text(matched(article.title.isEmpty ? "Untitled" : article.title, query))
                    .font(.system(size: 17, design: .serif).weight(.semibold))
                    .foregroundStyle(Color.highlighterInkStrong)
                    .lineLimit(2)
                if !article.summary.isEmpty {
                    Text(matched(article.summary, query))
                        .font(.footnote)
                        .foregroundStyle(Color.highlighterInkMuted)
                        .lineLimit(3)
                }
                HStack(spacing: 6) {
                    let name = app.profileCache[article.pubkey]?.bestName ?? String(article.pubkey.prefix(8))
                    Text(name)
                        .font(.caption.weight(.semibold))
                        .foregroundStyle(Color.highlighterInkMuted)
                    if let date = article.publishedAt ?? article.createdAt {
                        Text("·")
                            .foregroundStyle(Color.highlighterInkMuted)
                        Text(Date(timeIntervalSince1970: TimeInterval(date)), style: .date)
                            .font(.caption)
                            .foregroundStyle(Color.highlighterInkMuted)
                    }
                }
            }
            Spacer()
        }
        .padding(.vertical, 12)
        .contentShape(Rectangle())
        .task(id: article.pubkey) {
            await app.requestProfile(pubkeyHex: article.pubkey)
        }
    }

    @ViewBuilder
    private var articleThumb: some View {
        if let url = URL(string: article.image), !article.image.isEmpty {
            AsyncImage(url: url) { phase in
                if case .success(let img) = phase {
                    img.resizable().aspectRatio(contentMode: .fill)
                } else {
                    Color.laneArticlePage
                }
            }
            .frame(width: 64, height: 80)
            .clipShape(RoundedRectangle(cornerRadius: 4, style: .continuous))
        } else {
            RoundedRectangle(cornerRadius: 4, style: .continuous)
                .fill(Color.laneArticlePage)
                .frame(width: 64, height: 80)
                .overlay {
                    RoundedRectangle(cornerRadius: 4, style: .continuous)
                        .stroke(Color.highlighterRule, lineWidth: 0.5)
                }
        }
    }
}

private struct SeeAllCommunityRow: View {
    let community: CommunitySummary

    var body: some View {
        HStack(spacing: 14) {
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .fill(
                    LinearGradient(
                        colors: [
                            Color.highlighterAccent.opacity(0.35),
                            Color.highlighterTintPale
                        ],
                        startPoint: .topLeading,
                        endPoint: .bottomTrailing
                    )
                )
                .frame(width: 56, height: 56)
                .overlay {
                    if !community.picture.isEmpty, let url = URL(string: community.picture) {
                        AsyncImage(url: url) { phase in
                            if case .success(let img) = phase {
                                img.resizable().aspectRatio(contentMode: .fill)
                            }
                        }
                        .frame(width: 56, height: 56)
                        .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
                    } else {
                        Text(String(community.name.prefix(1)))
                            .font(.system(size: 22, design: .serif).weight(.semibold))
                            .foregroundStyle(Color.highlighterInkStrong.opacity(0.8))
                    }
                }
                .overlay {
                    RoundedRectangle(cornerRadius: 10, style: .continuous)
                        .stroke(Color.highlighterRule, lineWidth: 0.5)
                }
            VStack(alignment: .leading, spacing: 3) {
                Text(community.name)
                    .font(.callout.weight(.semibold))
                    .foregroundStyle(Color.highlighterInkStrong)
                    .lineLimit(1)
                if !community.about.isEmpty {
                    Text(community.about)
                        .font(.footnote)
                        .foregroundStyle(Color.highlighterInkMuted)
                        .lineLimit(2)
                }
            }
            Spacer()
            Image(systemName: "chevron.right")
                .font(.caption.weight(.semibold))
                .foregroundStyle(Color.highlighterInkMuted.opacity(0.6))
        }
        .padding(.vertical, 10)
        .contentShape(Rectangle())
    }
}

private struct SeeAllPersonRow: View {
    let profile: ProfileMetadata

    var body: some View {
        HStack(spacing: 14) {
            AuthorAvatar(
                pubkey: profile.pubkey,
                pictureURL: profile.picture,
                displayInitial: String(profile.bestName.prefix(1)),
                size: 46
            )
            VStack(alignment: .leading, spacing: 2) {
                Text(profile.bestName)
                    .font(.callout.weight(.semibold))
                    .foregroundStyle(Color.highlighterInkStrong)
                if !profile.nip05.isEmpty {
                    Text(profile.nip05)
                        .font(.caption)
                        .foregroundStyle(Color.highlighterInkMuted)
                        .lineLimit(1)
                } else if !profile.about.isEmpty {
                    Text(profile.about)
                        .font(.caption)
                        .foregroundStyle(Color.highlighterInkMuted)
                        .lineLimit(2)
                }
            }
            Spacer()
            Image(systemName: "chevron.right")
                .font(.caption.weight(.semibold))
                .foregroundStyle(Color.highlighterInkMuted.opacity(0.6))
        }
        .padding(.vertical, 10)
        .contentShape(Rectangle())
    }
}

// MARK: - Shared helpers

private extension ProfileMetadata {
    var bestName: String {
        if !displayName.isEmpty { return displayName }
        if !name.isEmpty { return name }
        return String(pubkey.prefix(8))
    }
}

/// Build an `AttributedString` highlighting every case-insensitive occurrence
/// of `query` within `text`. Free function so every row view can reuse it.
fileprivate func matched(_ text: String, _ query: String) -> AttributedString {
    var out = AttributedString(text)
    let trimmed = query.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !trimmed.isEmpty else { return out }

    let lowerText = text.lowercased()
    let lowerQuery = trimmed.lowercased()
    var searchRange = lowerText.startIndex..<lowerText.endIndex
    while let match = lowerText.range(of: lowerQuery, range: searchRange) {
        let startOffset = lowerText.distance(from: lowerText.startIndex, to: match.lowerBound)
        let endOffset = lowerText.distance(from: lowerText.startIndex, to: match.upperBound)
        let chars = out.characters
        var s = out.startIndex
        var e = out.startIndex
        var idx = 0
        while idx < startOffset, s < out.endIndex { s = chars.index(after: s); idx += 1 }
        idx = 0
        e = s
        while idx < (endOffset - startOffset), e < out.endIndex { e = chars.index(after: e); idx += 1 }
        if s < e {
            out[s..<e].foregroundColor = .highlighterAccent
            out[s..<e].backgroundColor = Color.laneArticleHighlightFill
        }
        searchRange = match.upperBound..<lowerText.endIndex
    }
    return out
}
