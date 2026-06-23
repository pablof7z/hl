import Kingfisher
import SwiftUI

struct BookView: View {
    let catalogId: String

    @Environment(HighlighterStore.self) private var app
    @State private var loadedRoute: BookRoute?
    @State private var highlights: [HighlightRecord] = []
    @State private var descriptionExpanded = false

    private var bookRoute: BookRoute? {
        loadedRoute ?? app.core.getBookRoute(catalogId: catalogId)
    }

    private var preview: ArtifactPreview? {
        guard let isbn = bookRoute?.isbn else { return nil }
        return app.isbnPreviewCache[isbn]
    }

    var body: some View {
        ScrollView {
            VStack(spacing: 0) {
                heroSection
                infoSection
                if let desc = preview?.description, !desc.isEmpty {
                    descriptionSection(desc)
                }
                passagesSection
            }
        }
        .background(Color.highlighterPaper.ignoresSafeArea())
        .navigationTitle(preview?.title ?? "")
        .navigationBarTitleDisplayMode(.inline)
        .task(id: catalogId) { await load() }
    }

    // MARK: - Hero

    private var heroSection: some View {
        ZStack {
            heroBackground
                .frame(height: 300)
                .clipped()

            VStack(spacing: 0) {
                Spacer()
                bookCover
                    .padding(.bottom, 24)
            }
            .frame(height: 300)
        }
    }

    @ViewBuilder
    private var heroBackground: some View {
        if let img = preview?.image, !img.isEmpty, let url = URL(string: img) {
            KFImage(url)
                .resizable()
                .scaledToFill()
                .blur(radius: 28, opaque: true)
                .overlay(Color.black.opacity(0.45))
        } else {
            LinearGradient(
                colors: [Color.highlighterAccent.opacity(0.5), Color.highlighterInkStrong.opacity(0.7)],
                startPoint: .topLeading,
                endPoint: .bottomTrailing
            )
        }
    }

    @ViewBuilder
    private var bookCover: some View {
        if let img = preview?.image, !img.isEmpty, let url = URL(string: img) {
            KFImage(url)
                .resizable()
                .scaledToFit()
                .frame(width: 130, height: 195)
                .clipShape(RoundedRectangle(cornerRadius: 3, style: .continuous))
                .shadow(color: .black.opacity(0.5), radius: 16, x: -6, y: 10)
                .shadow(color: .black.opacity(0.25), radius: 4, x: 0, y: 2)
        } else {
            coverPlaceholder
                .frame(width: 130, height: 195)
                .clipShape(RoundedRectangle(cornerRadius: 3, style: .continuous))
                .shadow(color: .black.opacity(0.4), radius: 16, x: -6, y: 10)
        }
    }

    private var coverPlaceholder: some View {
        ZStack {
            LinearGradient(
                colors: [Color.highlighterAccent.opacity(0.4), Color.highlighterAccent.opacity(0.2)],
                startPoint: .topLeading,
                endPoint: .bottomTrailing
            )
            Image(systemName: "book.closed.fill")
                .font(.system(size: 40, weight: .light))
                .foregroundStyle(Color.white.opacity(0.6))
        }
    }

    // MARK: - Info

    private var infoSection: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(preview?.title ?? "")
                .font(.title2.weight(.semibold))
                .foregroundStyle(Color.highlighterInkStrong)
                .multilineTextAlignment(.leading)
                .frame(maxWidth: .infinity, alignment: .leading)

            if let author = preview?.author, !author.isEmpty {
                Text(author.uppercased())
                    .font(.caption.weight(.bold))
                    .tracking(0.8)
                    .foregroundStyle(Color.highlighterInkMuted)
            }
        }
        .padding(.horizontal, 20)
        .padding(.top, 20)
        .padding(.bottom, 16)
    }

    // MARK: - Description

    private func descriptionSection(_ desc: String) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Divider()
                .padding(.horizontal, 20)

            Text(desc)
                .font(.callout)
                .foregroundStyle(Color.highlighterInkStrong)
                .lineSpacing(3)
                .lineLimit(descriptionExpanded ? nil : 4)
                .multilineTextAlignment(.leading)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 20)

            Button {
                withAnimation(.easeInOut(duration: 0.2)) {
                    descriptionExpanded.toggle()
                }
            } label: {
                Text(descriptionExpanded ? "Less" : "More")
                    .font(.footnote.weight(.semibold))
                    .foregroundStyle(Color.highlighterAccent)
            }
            .padding(.horizontal, 20)
            .padding(.bottom, 4)
        }
        .padding(.bottom, 8)
    }

    // MARK: - Passages

    private var passagesSection: some View {
        VStack(alignment: .leading, spacing: 0) {
            Divider()
                .padding(.horizontal, 20)

            Text("Passages")
                .font(.system(.title3, design: .default).weight(.semibold))
                .foregroundStyle(Color.highlighterInkStrong)
                .padding(.horizontal, 20)
                .padding(.top, 20)
                .padding(.bottom, 12)

            if highlights.isEmpty {
                Text("No passages yet")
                    .font(.callout)
                    .foregroundStyle(Color.highlighterInkMuted)
                    .padding(.horizontal, 20)
                    .padding(.bottom, 32)
            } else {
                ForEach(highlights, id: \.eventId) { h in
                    passageRow(h)
                }
            }
        }
        .padding(.bottom, 40)
    }

    private func passageRow(_ h: HighlightRecord) -> some View {
        let trimmedImage = h.imageUrl.trimmingCharacters(in: .whitespaces)
        let quoteText = h.quote.trimmingCharacters(in: .whitespaces)
        let content = HighlightDetailContentProjection(
            quoteText: quoteText,
            noteText: h.note.trimmingCharacters(in: .whitespaces).isEmpty ? nil : h.note,
            pageImageUrl: trimmedImage.isEmpty ? nil : trimmedImage,
            shareMessage: quoteText
        )
        return HStack(alignment: .top, spacing: 14) {
            Rectangle()
                .fill(Color.highlighterAccent)
                .frame(width: 3)
                .clipShape(RoundedRectangle(cornerRadius: 1.5))

            VStack(alignment: .leading, spacing: 6) {
                Text(content.quoteText)
                    .font(.system(.body, design: .default).italic())
                    .foregroundStyle(Color.highlighterInkStrong)
                    .lineSpacing(3)
                    .fixedSize(horizontal: false, vertical: true)

                if let note = content.noteText {
                    Text(note)
                        .font(.footnote)
                        .foregroundStyle(Color.highlighterInkMuted)
                        .fixedSize(horizontal: false, vertical: true)
                }

                highlighterByline(h)
            }
        }
        .padding(.horizontal, 20)
        .padding(.vertical, 14)
        .overlay(alignment: .bottom) {
            Rectangle()
                .fill(Color.highlighterRule.opacity(0.5))
                .frame(height: 0.5)
                .padding(.leading, 37)
        }
    }

    @ViewBuilder
    private func highlighterByline(_ h: HighlightRecord) -> some View {
        let highlighter = highlighterDisplay(h.pubkey)

        HStack(spacing: 6) {
            AuthorAvatar(
                pubkey: h.pubkey,
                pictureURL: highlighter.pictureUrl,
                displayInitial: highlighter.displayInitial,
                size: 16,
                ringWidth: 0
            )
            Text(highlighter.displayName.uppercased())
                .font(.caption2.weight(.bold))
                .tracking(0.5)
                .foregroundStyle(Color.highlighterInkMuted)
            if let rel = relativeDate(h.createdAt) {
                Text("·").font(.caption2).foregroundStyle(Color.highlighterInkMuted)
                Text(rel).font(.caption2).foregroundStyle(Color.highlighterInkMuted)
            }
        }
        .task(id: h.pubkey) { await app.requestProfile(pubkeyHex: h.pubkey) }
    }

    // MARK: - Data loading

    private func load() async {
        let snapshot = await app.safeCore.getBookDetailSnapshot(catalogId: catalogId, limit: 64)
        let projection = app.safeCore.projectBookDetailSnapshotApply(
            input: BookDetailSnapshotApplyInput(
                route: snapshot.route,
                highlights: snapshot.highlights,
                error: snapshot.error
            )
        )
        if let isbn = projection.isbnPreviewRequest {
            await app.requestIsbnPreview(isbn: isbn)
        }
        await MainActor.run {
            loadedRoute = projection.route
            highlights = projection.highlights
        }
    }

    // MARK: - Helpers

    private func highlighterDisplay(_ pubkey: String) -> ProfileDisplayProjection {
        let profile = app.profileSnapshots[pubkey]
        let name = (profile?.displayName ?? "").isEmpty
            ? ((profile?.name ?? "").isEmpty ? String(pubkey.prefix(8)) : profile!.name)
            : profile!.displayName
        return ProfileDisplayProjection(
            displayName: name,
            displayInitial: name.first.map { String($0).uppercased() } ?? "?",
            pictureUrl: profile?.picture ?? ""
        )
    }

    private func relativeDate(_ seconds: UInt64?) -> String? {
        guard let seconds, seconds > 0 else { return nil }
        let now = UInt64(max(0, Date().timeIntervalSince1970))
        guard now >= seconds else { return nil }
        let delta = now - seconds
        if delta < 60 { return "just now" }
        switch delta {
        case 60 ..< 3600:   return "\(delta / 60)m"
        case 3600 ..< 86400:  return "\(delta / 3600)h"
        case 86400 ..< 604800:  return "\(delta / 86400)d"
        case 604800 ..< 2592000: return "\(delta / 604800)w"
        default: return "\(delta / 2592000)mo"
        }
    }
}
