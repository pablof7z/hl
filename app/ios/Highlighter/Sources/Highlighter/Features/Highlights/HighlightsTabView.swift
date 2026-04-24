import SwiftUI

/// Home tab surfacing highlights from people the user follows and from
/// rooms they've joined — a single chronological feed. Every row is
/// rendered as an editorial pull-quote via `HighlightFeedCardView`.
///
/// Tap a quote → opens the source article in the reader (anchored on
/// that highlight). Long-press → share the source to a community.
struct HighlightsTabView: View {
    @Environment(HighlighterStore.self) private var app
    @State private var store: HighlightsStore?
    @State private var shareTarget: ShareToCommunityTarget?
    @State private var capturePresented: Bool = false

    var body: some View {
        NavigationStack {
            Group {
                if let store {
                    content(store: store)
                } else {
                    ProgressView()
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                }
            }
            .navigationTitle("Highlights")
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button {
                        capturePresented = true
                    } label: {
                        Image(systemName: "plus")
                    }
                    .accessibilityLabel("Capture highlight")
                }
            }
            .navigationDestination(for: ArticleReaderTarget.self) { target in
                ArticleReaderView(target: target)
            }
            .navigationDestination(for: WebReaderTarget.self) { target in
                WebReaderView(target: target)
            }
            .navigationDestination(for: ProfileDestination.self) { destination in
                if case .pubkey(let pk) = destination {
                    ProfileView(pubkey: pk)
                }
            }
        }
        .sheet(item: $shareTarget) { target in
            ShareToCommunitySheet(target: target)
                .presentationDetents([.medium, .large])
        }
        .captureFlow(isPresented: $capturePresented)
        .task {
            guard store == nil else { return }
            let s = HighlightsStore(safeCore: app.safeCore, eventBridge: app.eventBridge)
            store = s
            await s.start()
        }
        .onDisappear {
            store?.stop()
        }
    }

    @ViewBuilder
    private func content(store: HighlightsStore) -> some View {
        if store.isLoadingInitial {
            ProgressView()
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else if store.items.isEmpty {
            emptyState(store: store)
        } else {
            feedList(store: store)
        }
    }

    private func feedList(store: HighlightsStore) -> some View {
        ScrollView {
            LazyVStack(spacing: 0) {
                ForEach(Array(store.items.enumerated()), id: \.element.highlight.eventId) { index, item in
                    tappableRow(for: item)
                        .contextMenu {
                            if let target = shareTargetForMenu(item) {
                                Button {
                                    shareTarget = target
                                } label: {
                                    Label("Share to community", systemImage: "square.and.arrow.up")
                                }
                            }
                            Button {
                                UIPasteboard.general.string = item.highlight.quote
                            } label: {
                                Label("Copy quote", systemImage: "doc.on.doc")
                            }
                        }

                    if index < store.items.count - 1 {
                        Rectangle()
                            .fill(Color.highlighterRule)
                            .frame(height: 1)
                    }
                }
            }
            .padding(.horizontal, 20)
        }
        .background(Color.highlighterPaper.ignoresSafeArea())
    }

    private func emptyState(store: HighlightsStore) -> some View {
        VStack(alignment: .leading, spacing: 14) {
            Rectangle()
                .fill(Color.highlighterAccent.opacity(0.6))
                .frame(width: 3, height: 28)
                .clipShape(RoundedRectangle(cornerRadius: 1.5))
            Text("No highlights yet")
                .font(.system(.title2, design: .serif).weight(.semibold))
                .foregroundStyle(Color.highlighterInkStrong)
            Text("Quotes surfaced by people you follow, your rooms, and your own highlights will land here.")
                .font(.system(.subheadline, design: .serif))
                .foregroundStyle(Color.highlighterInkMuted)
                .lineSpacing(3)
                .fixedSize(horizontal: false, vertical: true)
        }
        .frame(maxWidth: 360, alignment: .leading)
        .padding(.horizontal, 32)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .center)
        .background(Color.highlighterPaper.ignoresSafeArea())
    }

    /// Tap-through row: picks the right destination for the highlight's
    /// source. NIP-23 articles go to the in-app reader; web URLs go to the
    /// web reader (which anchors on the quote). Highlights with neither
    /// render as non-tappable cards.
    @ViewBuilder
    private func tappableRow(for item: HydratedHighlight) -> some View {
        if let articleTarget = articleReaderTarget(for: item) {
            NavigationLink(value: articleTarget) {
                HighlightFeedCardView(item: item)
            }
            .buttonStyle(.plain)
        } else if let webTarget = webReaderTarget(for: item) {
            NavigationLink(value: webTarget) {
                HighlightFeedCardView(item: item)
            }
            .buttonStyle(.plain)
        } else {
            HighlightFeedCardView(item: item)
        }
    }

    private func articleReaderTarget(for item: HydratedHighlight) -> ArticleReaderTarget? {
        let addr = item.highlight.artifactAddress.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !addr.isEmpty else { return nil }
        let parts = addr.split(separator: ":", maxSplits: 2, omittingEmptySubsequences: false)
        guard parts.count == 3, parts[0] == "30023" else { return nil }
        let pubkey = String(parts[1])
        let dTag = String(parts[2])
        guard !pubkey.isEmpty, !dTag.isEmpty else { return nil }
        return ArticleReaderTarget(pubkey: pubkey, dTag: dTag, seed: nil)
    }

    private func webReaderTarget(for item: HydratedHighlight) -> WebReaderTarget? {
        let raw = item.highlight.sourceUrl.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !raw.isEmpty, let url = URL(string: raw) else { return nil }
        // Only navigate on http(s) to avoid odd schemes (magnet:, mailto:, etc.)
        guard let scheme = url.scheme?.lowercased(),
              scheme == "http" || scheme == "https" else { return nil }
        return WebReaderTarget(url: url, highlightQuote: item.highlight.quote)
    }

    /// Share-to-community target. Only supported for NIP-23 article
    /// highlights today — we reshare the source article, not the quote.
    private func shareTargetForMenu(_ item: HydratedHighlight) -> ShareToCommunityTarget? {
        if let existing = item.artifact {
            return .artifact(existing)
        }
        // Synthesize a minimal ArtifactPreview from the highlight's
        // reference tag so the share path works even without a hydrated
        // artifact. Only article references are supported for now.
        let addr = item.highlight.artifactAddress.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !addr.isEmpty else { return nil }
        let parts = addr.split(separator: ":", maxSplits: 2, omittingEmptySubsequences: false)
        guard parts.count == 3, parts[0] == "30023" else { return nil }
        let dTag = String(parts[2])
        let preview = ArtifactPreview(
            id: dTag,
            url: "",
            title: "",
            author: "",
            image: "",
            description: "",
            source: "article",
            domain: "",
            catalogId: "",
            catalogKind: "",
            podcastGuid: "",
            podcastShowTitle: "",
            audioUrl: "",
            audioPreviewUrl: "",
            transcriptUrl: "",
            feedUrl: "",
            publishedAt: "",
            durationSeconds: nil,
            referenceTagName: "a",
            referenceTagValue: addr,
            referenceKind: "30023",
            highlightTagName: "a",
            highlightTagValue: addr,
            highlightReferenceKey: "a:\(addr)"
        )
        return ShareToCommunityTarget(
            preview: preview,
            displayTitle: "Article",
            displaySubtitle: item.highlight.quote,
            imageURL: nil
        )
    }
}
