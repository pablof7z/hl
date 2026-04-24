import Kingfisher
import SwiftUI

/// Book lane. A paper-warm surface carrying the book's compact identity,
/// a hero pull-quote rendered as marginalia, and — if more highlights
/// exist — a horizontal strip of supporting quotes along the page edge.
///
/// The book is intentionally quiet (small cover, serif title). The loud
/// thing on screen is the community's pull-quote. That's the point.
struct BookLaneView: View {
    @Environment(HighlighterStore.self) private var app

    let lane: Lane
    let onShareToCommunity: (ArtifactRecord) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
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
        .padding(.vertical, lane.highlights.isEmpty ? 14 : 28)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(Color.laneBookPaper)
    }

    // MARK: - Book identity

    private var identity: some View {
        HStack(alignment: .top, spacing: 14) {
            cover
                .frame(width: 48, height: 72)

            VStack(alignment: .leading, spacing: 4) {
                Text(lane.artifact.preview.title.isEmpty ? "Untitled" : lane.artifact.preview.title)
                    .font(.system(.headline, design: .serif).weight(.semibold))
                    .foregroundStyle(Color.highlighterInkStrong)
                    .lineLimit(2)
                    .fixedSize(horizontal: false, vertical: true)

                if !lane.artifact.preview.author.isEmpty {
                    Text(lane.artifact.preview.author)
                        .font(.system(.subheadline, design: .serif).italic())
                        .foregroundStyle(Color.highlighterInkMuted)
                        .lineLimit(1)
                }
            }

            Spacer(minLength: 0)
        }
        .padding(.horizontal, 24)
    }

    @ViewBuilder
    private var cover: some View {
        let image = lane.artifact.preview.image
        Group {
            if !image.isEmpty, let url = URL(string: image) {
                KFImage(url)
                    .placeholder { coverPlaceholder }
                    .fade(duration: 0.15)
                    .resizable()
                    .scaledToFill()
            } else {
                coverPlaceholder
            }
        }
        .clipShape(RoundedRectangle(cornerRadius: 3, style: .continuous))
        .shadow(color: .black.opacity(0.18), radius: 5, x: 0, y: 2)
    }

    private var coverPlaceholder: some View {
        LinearGradient(
            colors: [Color.laneBookGutter.opacity(0.9), Color.laneBookGutter.opacity(0.45)],
            startPoint: .topLeading,
            endPoint: .bottomTrailing
        )
        .overlay(
            Image(systemName: "book.closed")
                .font(.footnote)
                .foregroundStyle(Color.highlighterInkMuted.opacity(0.75))
        )
    }

    // MARK: - Hero pull-quote (marginalia)

    private func heroPullQuote(_ h: HydratedHighlight) -> some View {
        HStack(alignment: .top, spacing: 16) {
            Rectangle()
                .fill(Color.laneBookGutter)
                .frame(width: 1)

            VStack(alignment: .leading, spacing: 14) {
                Text("\u{201C}\(h.highlight.quote)\u{201D}")
                    .font(.system(.title3, design: .serif).italic())
                    .foregroundStyle(Color.highlighterInkStrong)
                    .lineSpacing(4)
                    .multilineTextAlignment(.leading)
                    .fixedSize(horizontal: false, vertical: true)

                if !h.highlight.note.isEmpty {
                    Text(h.highlight.note)
                        .font(.system(.subheadline, design: .serif).italic())
                        .foregroundStyle(Color.highlighterInkMuted)
                        .lineSpacing(2)
                        .fixedSize(horizontal: false, vertical: true)
                }

                marginalia(for: h)
            }
        }
        .padding(.horizontal, 24)
        .task(id: h.highlight.pubkey) {
            await app.requestProfile(pubkeyHex: h.highlight.pubkey)
        }
    }

    private func marginalia(for h: HydratedHighlight) -> some View {
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
                    BookLaneSupportingCard(highlight: h)
                }
            }
            .padding(.horizontal, 24)
        }
    }

    // MARK: - Helpers

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

/// Supporting quote card shown in the book lane's horizontal strip after
/// the hero pull-quote. Same typographic idiom (serif), quieter scale.
struct BookLaneSupportingCard: View {
    @Environment(HighlighterStore.self) private var app
    let highlight: HydratedHighlight

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            Rectangle()
                .fill(Color.laneBookGutter.opacity(0.75))
                .frame(width: 1)

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
                        .font(.caption.italic())
                        .foregroundStyle(Color.highlighterInkMuted)
                        .lineLimit(1)
                }
            }
        }
        .padding(.vertical, 6)
        .frame(width: 240, height: 150, alignment: .topLeading)
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
