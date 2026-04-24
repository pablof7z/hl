import Kingfisher
import SwiftUI

/// Article lane. Clean magazine surface. Publication domain as a
/// small-caps kicker. Article title in serif. The hero pull-quote sits
/// in a highlighter-stroke underlay so the community's emphasis reads
/// the way a pen mark would on paper.
struct ArticleLaneView: View {
    @Environment(HighlighterStore.self) private var app

    let lane: Lane
    let onShareToCommunity: (ArtifactRecord) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            NavigationLink(value: lane.artifact) {
                identity
            }
            .buttonStyle(.plain)
            .contextMenu {
                Button {
                    onShareToCommunity(lane.artifact)
                } label: {
                    Label("Share to community", systemImage: "square.and.arrow.up")
                }
            }

            if let hero = lane.highlights.first {
                heroPullQuote(hero)
            }

            if lane.highlights.count > 1 {
                supportingStrip
            }
        }
        .padding(.vertical, 28)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.laneArticlePage)
    }

    // MARK: - Article identity

    private var identity: some View {
        VStack(alignment: .leading, spacing: 10) {
            if !lane.artifact.preview.domain.isEmpty {
                Text(lane.artifact.preview.domain.uppercased())
                    .font(.caption.weight(.semibold))
                    .tracking(1.4)
                    .foregroundStyle(Color.highlighterInkMuted)
            }

            HStack(alignment: .top, spacing: 14) {
                VStack(alignment: .leading, spacing: 4) {
                    Text(lane.artifact.preview.title.isEmpty ? "Untitled" : lane.artifact.preview.title)
                        .font(.system(.title3, design: .serif).weight(.semibold))
                        .foregroundStyle(Color.highlighterInkStrong)
                        .lineLimit(3)
                        .fixedSize(horizontal: false, vertical: true)

                    if !byline.isEmpty {
                        Text(byline)
                            .font(.subheadline)
                            .foregroundStyle(Color.highlighterInkMuted)
                            .lineLimit(1)
                    }
                }

                Spacer(minLength: 0)

                if let url = URL(string: lane.artifact.preview.image), !lane.artifact.preview.image.isEmpty {
                    KFImage(url)
                        .fade(duration: 0.15)
                        .resizable()
                        .scaledToFill()
                        .frame(width: 72, height: 72)
                        .clipShape(RoundedRectangle(cornerRadius: 4, style: .continuous))
                }
            }
        }
        .padding(.horizontal, 24)
    }

    // MARK: - Hero pull-quote

    private func heroPullQuote(_ h: HydratedHighlight) -> some View {
        VStack(alignment: .leading, spacing: 14) {
            Text(h.highlight.quote)
                .font(.system(.title3, design: .serif))
                .foregroundStyle(Color.highlighterInkStrong)
                .lineSpacing(4)
                .multilineTextAlignment(.leading)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.horizontal, 6)
                .padding(.vertical, 3)
                .background(Color.laneArticleHighlightFill)

            if !h.highlight.note.isEmpty {
                Text(h.highlight.note)
                    .font(.subheadline.italic())
                    .foregroundStyle(Color.highlighterInkMuted)
                    .lineSpacing(2)
                    .fixedSize(horizontal: false, vertical: true)
            }

            attribution(for: h)
        }
        .padding(.horizontal, 24)
        .task(id: h.highlight.pubkey) {
            await app.requestProfile(pubkeyHex: h.highlight.pubkey)
        }
    }

    private func attribution(for h: HydratedHighlight) -> some View {
        HStack(spacing: 8) {
            AuthorAvatar(
                pubkey: h.highlight.pubkey,
                pictureURL: app.profileCache[h.highlight.pubkey]?.picture ?? "",
                displayInitial: initial(for: h.highlight.pubkey),
                size: 20
            )
            Text(name(for: h.highlight.pubkey))
                .font(.footnote.weight(.semibold))
                .foregroundStyle(Color.highlighterInkStrong)
                .lineLimit(1)
            if let t = relative(h.highlight.createdAt) {
                Text("·").foregroundStyle(Color.highlighterInkMuted)
                Text(t)
                    .font(.footnote)
                    .foregroundStyle(Color.highlighterInkMuted)
                    .lineLimit(1)
            }
            Spacer(minLength: 0)
        }
    }

    // MARK: - Supporting highlights

    private var supportingStrip: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(alignment: .top, spacing: 16) {
                ForEach(Array(lane.highlights.dropFirst()), id: \.highlight.eventId) { h in
                    ArticleLaneSupportingCard(highlight: h)
                }
            }
            .padding(.horizontal, 24)
        }
    }

    // MARK: - Helpers

    private var byline: String {
        let profile = articleAuthorPubkey.flatMap { app.profileCache[$0] }
        if let dn = profile?.displayName, !dn.isEmpty { return "by " + dn }
        if let n = profile?.name, !n.isEmpty { return "by " + n }
        if !lane.artifact.preview.author.isEmpty { return "by " + lane.artifact.preview.author }
        return ""
    }

    private var articleAuthorPubkey: String? {
        let raw: String
        if lane.artifact.preview.highlightTagName == "a",
           !lane.artifact.preview.highlightTagValue.isEmpty {
            raw = lane.artifact.preview.highlightTagValue
        } else if lane.artifact.preview.referenceTagName == "a",
                  !lane.artifact.preview.referenceTagValue.isEmpty {
            raw = lane.artifact.preview.referenceTagValue
        } else {
            return nil
        }
        let parts = raw.split(separator: ":", maxSplits: 2, omittingEmptySubsequences: false)
        guard parts.count == 3, parts[0] == "30023" else { return nil }
        let pk = String(parts[1])
        return pk.isEmpty ? nil : pk
    }

    private func name(for pubkey: String) -> String {
        let profile = app.profileCache[pubkey]
        if let dn = profile?.displayName, !dn.isEmpty { return dn }
        if let n = profile?.name, !n.isEmpty { return n }
        return String(pubkey.prefix(10))
    }

    private func initial(for pubkey: String) -> String {
        name(for: pubkey).first.map { String($0).uppercased() } ?? ""
    }

    private func relative(_ seconds: UInt64?) -> String? {
        guard let s = seconds, s > 0 else { return nil }
        let date = Date(timeIntervalSince1970: TimeInterval(s))
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        formatter.dateTimeStyle = .numeric
        return formatter.localizedString(for: date, relativeTo: Date())
    }
}

/// Supporting pull-quote in the article lane's horizontal strip. No
/// highlighter underlay — reserved for the hero — but same editorial
/// serif treatment, quieter scale.
struct ArticleLaneSupportingCard: View {
    @Environment(HighlighterStore.self) private var app
    let highlight: HydratedHighlight

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(highlight.highlight.quote)
                .font(.system(.callout, design: .serif))
                .foregroundStyle(Color.highlighterInkStrong)
                .lineSpacing(2)
                .lineLimit(5)
                .multilineTextAlignment(.leading)

            Spacer(minLength: 0)

            HStack(spacing: 6) {
                AuthorAvatar(
                    pubkey: highlight.highlight.pubkey,
                    pictureURL: app.profileCache[highlight.highlight.pubkey]?.picture ?? "",
                    displayInitial: initial,
                    size: 16
                )
                Text(name)
                    .font(.caption2)
                    .foregroundStyle(Color.highlighterInkMuted)
                    .lineLimit(1)
            }
        }
        .padding(14)
        .frame(width: 240, height: 150, alignment: .topLeading)
        .overlay(
            Rectangle()
                .fill(Color.highlighterRule)
                .frame(height: 1),
            alignment: .top
        )
        .task(id: highlight.highlight.pubkey) {
            await app.requestProfile(pubkeyHex: highlight.highlight.pubkey)
        }
    }

    private var name: String {
        let profile = app.profileCache[highlight.highlight.pubkey]
        if let dn = profile?.displayName, !dn.isEmpty { return dn }
        if let n = profile?.name, !n.isEmpty { return n }
        return String(highlight.highlight.pubkey.prefix(10))
    }

    private var initial: String {
        name.first.map { String($0).uppercased() } ?? ""
    }
}
