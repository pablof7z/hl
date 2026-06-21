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
        let projection = app.safeCore.projectSearchHighlightRow(
            input: SearchHighlightRowProjectionInput(highlight: h)
        )
        if let route = projection.articleRoute {
            NavigationLink(value: ArticleReaderTarget(route: route)) {
                SeeAllHighlightRow(
                    highlight: h,
                    query: store.query,
                    pageImageUrl: projection.pageImageUrl,
                    safeCore: app.safeCore
                )
            }
            .buttonStyle(.plain)
        } else {
            SeeAllHighlightRow(
                highlight: h,
                query: store.query,
                pageImageUrl: projection.pageImageUrl,
                safeCore: app.safeCore
            )
        }
    }

    // MARK: - Articles

    private var articlesList: some View {
        // Switched from LazyVStack to List so `.swipeActions` on
        // `articleRowActions` activates. Styled heavily to preserve the
        // editorial look of the rest of the app.
        List {
            ForEach(store.articles, id: \.eventId) { a in
                NavigationLink(value: ArticleReaderTarget(article: a)) {
                    ArticleCardView(article: a)
                }
                .listRowBackground(Color.highlighterPaper)
                .listRowInsets(EdgeInsets(top: 0, leading: 20, bottom: 0, trailing: 20))
                .listRowSeparatorTint(Color.highlighterRule)
                .articleRowActions(article: a)
            }
        }
        .listStyle(.plain)
        .scrollContentBackground(.hidden)
        .background(Color.highlighterPaper)
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

}

// MARK: - See-all row variants (a touch denser than the preview rows)

private struct SeeAllHighlightRow: View {
    let highlight: HighlightRecord
    let query: String
    let pageImageUrl: String?
    let safeCore: SafeHighlighterCore

    var body: some View {
        HStack(alignment: .top, spacing: 14) {
            if let pageURL = pageImageURL {
                HighlightPageImage(url: pageURL, treatment: .row)
            } else {
                Rectangle()
                    .fill(Color.highlighterAccent)
                    .frame(width: 2.5)
                    .clipShape(RoundedRectangle(cornerRadius: 1.25))
            }
            VStack(alignment: .leading, spacing: 6) {
                Text(matched(highlight.quote, query, safeCore: safeCore))
                    .font(.system(size: 17, design: .default).italic())
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

    private var pageImageURL: URL? {
        pageImageUrl.flatMap(URL.init(string:))
    }
}


private struct SeeAllCommunityRow: View {
    let community: CommunitySummary

    @Environment(HighlighterStore.self) private var app

    var body: some View {
        let avatar = avatarProjection
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
                    if !avatar.pictureUrl.isEmpty, let url = URL(string: avatar.pictureUrl) {
                        AsyncImage(url: url) { phase in
                            if case .success(let img) = phase {
                                img.resizable().aspectRatio(contentMode: .fill)
                            }
                        }
                        .frame(width: 56, height: 56)
                        .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
                    } else {
                        Text(avatar.displayInitial)
                            .font(.system(size: 22, design: .default).weight(.semibold))
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

    private var avatarProjection: RoomAvatarProjection {
        app.safeCore.projectRoomAvatar(
            input: RoomAvatarProjectionInput(
                name: community.name,
                pictureUrl: community.picture,
                uppercaseInitial: false
            )
        )
    }
}

private struct SeeAllPersonRow: View {
    @Environment(HighlighterStore.self) private var app

    let profile: ProfileSearchRow

    var body: some View {
        let metaForDisplay = ProfileMetadata(
            pubkey: profile.pubkey, name: profile.name,
            displayName: profile.displayName, about: profile.about,
            picture: profile.picture, banner: "",
            nip05: profile.nip05, website: "", lud16: "",
            createdAt: profile.createdAt
        )
        let display = app.safeCore.projectProfileDisplay(
            input: ProfileDisplayProjectionInput(
                pubkey: profile.pubkey,
                profile: metaForDisplay,
                fallback: .pubkey8
            )
        )

        HStack(spacing: 14) {
            AuthorAvatar(
                pubkey: profile.pubkey,
                pictureURL: display.pictureUrl,
                displayInitial: display.displayInitial,
                size: 46
            )
            VStack(alignment: .leading, spacing: 2) {
                Text(display.displayName)
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

/// Build an `AttributedString` highlighting every case-insensitive occurrence
/// of `query` within `text`. Free function so every row view can reuse it.
fileprivate func matched(
    _ text: String,
    _ query: String,
    safeCore: SafeHighlighterCore
) -> AttributedString {
    var out = AttributedString(text)
    let projection = safeCore.projectSearchTextMatches(
        input: SearchTextMatchesProjectionInput(text: text, query: query)
    )
    for span in projection.spans {
        let chars = out.characters
        var s = out.startIndex
        var e = out.startIndex
        var idx = 0
        while idx < Int(span.start), s < out.endIndex { s = chars.index(after: s); idx += 1 }
        idx = 0
        e = s
        while idx < Int(span.end - span.start), e < out.endIndex { e = chars.index(after: e); idx += 1 }
        if s < e {
            out[s..<e].foregroundColor = .highlighterAccent
            out[s..<e].backgroundColor = Color.laneArticleHighlightFill
        }
    }
    return out
}
