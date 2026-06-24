import Kingfisher
import SwiftUI
import UIKit

/// Full-screen NIP-23 long-form reader. Handles the gorgeous header (cover,
/// serif title, author row, metadata), renders the body via `ArticleBodyView`,
/// and orchestrates the text-selection → highlight flow.
struct ArticleReaderView: View {
    let target: ArticleReaderTarget

    @Environment(HighlighterStore.self) private var app
    @Environment(HighlighterAppKernel.self) private var kernel
    @State private var store: ArticleReaderStore?
    @State private var pendingHighlight: PendingHighlight?
    @State private var highlightDetail: HighlightRecord?
    @State private var toast: String?
    @State private var scrollAnchor: ScrollAnchor = .idle
    @State private var shareTarget: ShareToCommunityTarget?
    @State private var toastResetTimer = OneShotUITimer()

    enum ScrollAnchor: Equatable {
        case idle
        case footnote(number: Int)
        case footnoteBack(number: Int)
    }

    struct PendingHighlight: Identifiable {
        let id = UUID()
        let quote: String
        let context: String
    }

    var body: some View {
        Group {
            if let store {
                content(store: store)
            } else {
                ProgressView()
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                    .background(Color.highlighterPaper)
            }
        }
        .background(Color.highlighterPaper.ignoresSafeArea())
        .navigationBarTitleDisplayMode(.inline)
        .toolbar(.hidden, for: .tabBar)
        .toolbarBackground(.hidden, for: .navigationBar)
        .toolbar {
            if let article = store?.article {
                ToolbarItem(placement: .topBarTrailing) {
                    BookmarkMenuButton(articleAddress: article.address)
                }
                ToolbarItem(placement: .topBarTrailing) {
                    Button {
                        shareTarget = ShareToCommunityTarget.article(article, core: app.safeCore)
                    } label: {
                        Image(systemName: "square.and.arrow.up")
                    }
                    .accessibilityLabel("Share to community")
                }
            }
        }
        .sheet(item: $shareTarget) { target in
            ShareToCommunitySheet(target: target)
                .presentationDetents([.medium, .large])
        }
        .task(id: target) {
            if store == nil {
                let s = ArticleReaderStore(
                    target: target,
                    safeCore: app.safeCore,
                    eventBridge: app.eventBridge,
                    kernel: kernel
                )
                store = s
                await s.start()
                s.applyKernelSnapshot()
            }
        }
        .task(id: target.pubkey) {
            await app.requestProfile(pubkeyHex: target.pubkey)
        }
        .onChange(of: kernel.articleReader[target.address]) { _, _ in
            store?.applyKernelSnapshot()
        }
        .onDisappear {
            store?.stop()
        }
        .sheet(item: $pendingHighlight) { pending in
            NoteComposerSheet(
                quote: pending.quote,
                onCancel: { pendingHighlight = nil },
                onSave: { note in
                    Task { await publish(quote: pending.quote, context: pending.context, note: note) }
                    pendingHighlight = nil
                }
            )
            .presentationDetents([.medium])
        }
        .sheet(item: Binding(
            get: { highlightDetail.map { IdentifiedHighlight(record: $0) } },
            set: { highlightDetail = $0?.record }
        )) { ih in
            HighlightDetailSheet(highlight: ih.record)
                .presentationDetents([.medium, .large])
        }
        .safeAreaInset(edge: .bottom) {
            if let toast {
                Text(toast)
                    .font(.footnote.weight(.medium))
                    .foregroundStyle(.white)
                    .padding(.horizontal, 14)
                    .padding(.vertical, 10)
                    .background(Color.highlighterAccent.opacity(0.95), in: Capsule())
                    .padding(.horizontal, 20)
                    .padding(.bottom, 12)
                    .transition(.move(edge: .bottom).combined(with: .opacity))
            }
        }
        .modifier(ArticleCommentsAttachmentModifier(article: store?.article, target: target))
    }

    // MARK: - Content

    @ViewBuilder
    private func content(store: ArticleReaderStore) -> some View {
        if store.isLoadingInitial && store.article == nil {
            ProgressView()
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else if let article = store.article {
            ReaderScroll(
                article: article,
                contentTree: store.contentTree,
                authorProfile: app.profileSnapshots[target.pubkey] ?? store.authorProfile,
                highlights: store.highlights,
                scrollAnchor: scrollAnchor,
                onPublishHighlight: { quote, context in
                    Task { await publish(quote: quote, context: context, note: "") }
                },
                onRequestNote: { quote, context in
                    pendingHighlight = PendingHighlight(quote: quote, context: context)
                },
                onHighlightTap: { highlightDetail = $0 },
                onFootnoteTap: { number in
                    scrollAnchor = .footnote(number: number)
                },
                onFootnoteBackTap: { number in
                    scrollAnchor = .footnoteBack(number: number)
                }
            )
        } else {
            ContentUnavailableView(
                "Couldn't load this article",
                systemImage: "doc.text",
                description: Text("We'll keep listening — it may arrive over the network in a moment.")
            )
        }
    }

    // MARK: - Actions

    private func publish(quote: String, context: String, note: String) async {
        guard let store else { return }
        let submitNote = note.trimmingCharacters(in: .whitespaces)
        let outcome = await store.publishHighlight(
            quote: quote,
            note: submitNote,
            context: context
        )
        let error = (outcome ?? "").trimmingCharacters(in: .whitespaces)
        let isSuccess = error.isEmpty
        let toastMessage = isSuccess
            ? (submitNote.isEmpty ? "Highlighted" : "Highlighted with note")
            : "Couldn't save — \(error)"
        if isSuccess {
            withAnimation(.easeOut(duration: 0.2)) {
                toast = toastMessage
            }
            toastResetTimer.schedule(after: 1.8) {
                withAnimation(.easeIn(duration: 0.2)) { toast = nil }
            }
        } else {
            withAnimation(.easeOut(duration: 0.2)) {
                toast = toastMessage
            }
            toastResetTimer.schedule(after: 2.8) {
                withAnimation(.easeIn(duration: 0.2)) { toast = nil }
            }
        }
    }
}

// MARK: - Comments attachment

/// Tiny adapter that mounts the premium NIP-22 comments toolbar + sheet
/// against an article. The article's address (`30023:<pubkey>:<d>`) is
/// the NIP-22 root scope, so we always have the artifact ref even
/// before the body finishes loading.
private struct ArticleCommentsAttachmentModifier: ViewModifier {
    let article: ArticleRecord?
    let target: ArticleReaderTarget

    @Environment(HighlighterStore.self) private var app

    @ViewBuilder
    func body(content: Content) -> some View {
        // D1: CommentScope is a plain struct — rootTagName/rootTagValue/rootKind are always
        // deterministic for an article address (NIP-23 kind:30023) and need no Rust call.
        // Articles always attach comments.
        let scope = CommentScope(rootTagName: "A", rootTagValue: target.address, rootKind: 30023)
        content.commentsAttachment(
            scope: scope,
            artifactAuthorPubkey: target.pubkey
        )
    }
}

// MARK: - Scroll container composing header + body

private struct ReaderScroll: View {
    let article: ArticleRecord
    /// The article body as the nmp `content_tree` (#22). `nil` until the kernel
    /// snapshot's `content_tree` arrives — the body shows nothing until then
    /// (D6, same cold-start window as the bespoke empty-body read).
    let contentTree: ContentTreeWire?
    let authorProfile: ProfileMetadata?
    let highlights: [HighlightRecord]
    let scrollAnchor: ArticleReaderView.ScrollAnchor
    var onPublishHighlight: (String, String) -> Void
    var onRequestNote: (String, String) -> Void
    var onHighlightTap: (HighlightRecord) -> Void
    var onFootnoteTap: (Int) -> Void
    var onFootnoteBackTap: (Int) -> Void

    @State private var rendered: MarkdownRenderer.Output?
    @State private var imageToOpen: IdentifiableURL?
    @State private var profileNavPubkey: String?
    @State private var profileNavActive = false
    @Environment(HighlighterStore.self) private var app

    private struct IdentifiableURL: Identifiable {
        let url: URL
        var id: String { url.absoluteString }
    }

    private var coverURL: URL? {
        guard !article.image.isEmpty else { return nil }
        return URL(string: article.image)
    }

    /// Re-render key for the body task: the article id plus the content-tree
    /// node count so the body re-renders when the kernel `content_tree` arrives
    /// (cold-start → populated) without re-rendering on every unrelated tick.
    private var treeRenderKey: String {
        "\(article.eventId)-\(contentTree?.nodes.count ?? 0)"
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                if let coverURL {
                    HeroImage(url: coverURL)
                }

                Header(article: article, authorProfile: authorProfile)
                    .padding(.horizontal, 20)
                    .padding(.top, coverURL == nil ? 10 : 20)
                    .padding(.bottom, 12)

                if let rendered {
                    bodySegments(rendered)
                }

                NavigationLink(
                    destination: Group {
                        if let pk = profileNavPubkey {
                            ProfileView(pubkey: pk)
                        }
                    },
                    isActive: $profileNavActive
                ) { EmptyView() }
                    .hidden()
            }
        }
        .ignoresSafeArea(edges: coverURL == nil ? [] : .top)
        .fullScreenCover(item: $imageToOpen) { item in
            ImageZoomView(url: item.url, onDismiss: { imageToOpen = nil })
        }
        // #22: render the body from the nmp `content_tree` (kernel snapshot)
        // via `ContentTreeBodyRenderer` — replacing the bespoke
        // `MarkdownRenderer.render(content: article.content)` markdown read. The
        // native select-to-highlight overlay (`ArticleBodyView`) renders on top
        // of the resulting attributed body unchanged.
        .task(id: "\(treeRenderKey)-\(highlights.count)-\(app.profileSnapshots.count)") {
            guard let contentTree else {
                rendered = nil
                return
            }
            let safeCore = app.safeCore
            let profileSnapshot = Dictionary(
                uniqueKeysWithValues: app.profileSnapshots.map { (pk, meta) -> (String, String) in
                    let name = meta.displayName.isEmpty
                        ? (meta.name.isEmpty ? String(pk.prefix(8)) : meta.name)
                        : meta.displayName
                    return (pk, name)
                }
            )
            rendered = await Task.detached(priority: .userInitiated) {
                ContentTreeBodyRenderer.render(
                    tree: contentTree,
                    highlights: highlights,
                    accent: UIColor(Color.highlighterAccent),
                    tint: UIColor(Color.highlighterAccent),
                    ink: UIColor(Color.highlighterInkStrong),
                    muted: UIColor(Color.highlighterInkMuted),
                    highlightContent: { highlight in
                        let quoteText = highlight.quote.trimmingCharacters(in: .whitespaces)
                        let noteText = highlight.note.trimmingCharacters(in: .whitespaces)
                        let imageUrl = highlight.imageUrl.trimmingCharacters(in: .whitespaces)
                        return HighlightDetailContentProjection(
                            quoteText: quoteText,
                            noteText: noteText.isEmpty ? nil : highlight.note,
                            pageImageUrl: imageUrl.isEmpty ? nil : imageUrl,
                            shareMessage: quoteText
                        )
                    },
                    profileNames: profileSnapshot,
                    // Resolve standalone `nostr:` event refs into the app's
                    // resolving entity card. `standaloneNostrEntity` is
                    // `nonisolated`, so it's safe to call from this detached
                    // off-main render task (#22 entity-card fidelity).
                    resolveEntity: { uri in safeCore.standaloneNostrEntity(uri) }
                )
            }.value
        }
    }

    @ViewBuilder
    private func bodySegments(_ output: MarkdownRenderer.Output) -> some View {
        ForEach(Array(output.segments.enumerated()), id: \.offset) { idx, segment in
            switch segment {
            case .text(let attrStr):
                let isLast = idx == output.segments.count - 1
                ArticleBodyView(
                    attributedText: isLast ? withFootnotes(attrStr, output) : attrStr,
                    footnoteAnchors: isLast ? output.footnoteAnchors : [:],
                    footnoteBackAnchors: [:],
                    highlightsById: output.highlightsById,
                    paperColor: UIColor(Color.highlighterPaper),
                    safeCore: app.safeCore,
                    onPublishHighlight: onPublishHighlight,
                    onRequestNote: onRequestNote,
                    onHighlightTap: onHighlightTap,
                    onFootnoteTap: onFootnoteTap,
                    onFootnoteBackTap: onFootnoteBackTap,
                    onImageTap: { url in imageToOpen = IdentifiableURL(url: url) },
                    onProfileTap: { pk in
                        profileNavPubkey = pk
                        profileNavActive = true
                    }
                )
                .frame(maxWidth: .infinity)
            case .image(let url, let alt):
                InlineArticleImage(url: url, alt: alt)
            case .nostrEntity(let ref):
                NostrEntityCard(entity: ref)
                    .padding(.horizontal, 20)
                    .padding(.vertical, 4)
            case .media(let urls, let kind):
                // Video / audio block — reuse `NostrContentView`'s native media
                // affordance (VideoPlayer / audio row) by rendering a one-node
                // wire slice. Full fidelity for media nodes (#22); these blocks
                // are interactive (playback), not selectable text.
                ContentTreeBlockSlice(node: .media(urls: urls, kind: kind))
                    .padding(.horizontal, 20)
                    .padding(.vertical, 4)
            case .placeholder(let reason):
                // Preserve `content_tree` placeholders — reuse `NostrContentView`'s
                // placeholder chip rather than dropping the node (#22).
                ContentTreeBlockSlice(node: .placeholder(reason: reason))
                    .padding(.horizontal, 20)
                    .padding(.vertical, 4)
            }
        }
    }

    private func withFootnotes(_ body: NSAttributedString, _ output: MarkdownRenderer.Output) -> NSAttributedString {
        guard output.footnotes.length > 0 else { return body }
        let out = NSMutableAttributedString(attributedString: body)
        out.append(NSAttributedString(
            string: "\n———\n\n",
            attributes: [
                .font: UIFont.systemFont(ofSize: 14, weight: .semibold),
                .foregroundColor: UIColor(Color.highlighterInkMuted)
            ]
        ))
        out.append(NSAttributedString(
            string: "Footnotes\n\n",
            attributes: [
                .font: UIFont.systemFont(ofSize: 12, weight: .bold),
                .foregroundColor: UIColor(Color.highlighterInkMuted),
                .kern: 0.6
            ]
        ))
        out.append(output.footnotes)
        return out
    }
}

// MARK: - Content-tree rich block slice

/// Renders a single `content_tree` block node (video / audio media, placeholder)
/// by wrapping it in a one-node `ContentTreeWire` and handing it to the vendored
/// `NostrContentView` — so the article body reuses the exact native media player
/// / audio row / placeholder chip `NostrContentView` ships rather than
/// reinventing them. Themed to the reader's serif/ink palette via a
/// `NostrContentRenderer` environment value. (#22 fidelity.)
private struct ContentTreeBlockSlice: View {
    let node: NostrWireNode

    var body: some View {
        NostrContentView(tree: ContentTreeWire(nodes: [node], roots: [0]))
            .nostrContentRenderer(Self.readerRenderer)
    }

    private static let readerRenderer = NostrContentRenderer(
        textColor: .highlighterInkStrong,
        secondaryTextColor: .highlighterInkMuted,
        mentionColor: .highlighterAccent,
        hashtagColor: .highlighterAccent,
        linkColor: .highlighterAccent,
        quoteBorderColor: .highlighterRule,
        codeBackgroundColor: Color.highlighterInkMuted.opacity(0.1),
        placeholderColor: .highlighterInkMuted,
        callbacks: NostrContentCallbacks(
            onLinkTap: { url in UIApplication.shared.open(url) }
        )
    )
}

// MARK: - Inline image

private struct InlineArticleImage: View {
    let url: URL
    let alt: String

    @State private var showFullScreen = false

    var body: some View {
        KFImage(url)
            .placeholder {
                Color.highlighterRule.opacity(0.4)
                    .frame(height: 200)
            }
            .fade(duration: 0.2)
            .resizable()
            .scaledToFit()
            .frame(maxWidth: .infinity)
            .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
            .contentShape(Rectangle())
            .onTapGesture { showFullScreen = true }
            .padding(.horizontal, 20)
            .padding(.vertical, 8)
            .fullScreenCover(isPresented: $showFullScreen) {
                ImageZoomView(url: url, onDismiss: { showFullScreen = false })
            }
    }
}

// MARK: - Hero image

/// Full-bleed cover that extends behind the status bar / notch. Sized by
/// GeometryReader so it scales to the device width even when the parent
/// ScrollView is `.ignoresSafeArea(.top)`.
private struct HeroImage: View {
    let url: URL

    @State private var showFullScreen = false

    var body: some View {
        GeometryReader { proxy in
            KFImage(url)
                .placeholder { Color.highlighterRule.opacity(0.5) }
                .fade(duration: 0.2)
                .resizable()
                .scaledToFill()
                .frame(width: proxy.size.width, height: proxy.size.height)
                .clipped()
                .onTapGesture { showFullScreen = true }
        }
        .frame(height: 320)
        .fullScreenCover(isPresented: $showFullScreen) {
            ImageZoomView(url: url, onDismiss: { showFullScreen = false })
        }
    }
}

// MARK: - Header

private struct Header: View {
    let article: ArticleRecord
    let authorProfile: ProfileMetadata?

    @Environment(HighlighterStore.self) private var app

    var body: some View {
        let projection = headerProjection

        VStack(alignment: .leading, spacing: 14) {
            Text(projection.title)
                .font(.largeTitle.weight(.bold))
                .foregroundStyle(Color.highlighterInkStrong)
                .fixedSize(horizontal: false, vertical: true)

            if !article.summary.isEmpty {
                Text(article.summary)
                    .font(.system(.title3, design: .default))
                    .foregroundStyle(Color.highlighterInkMuted)
                    .fixedSize(horizontal: false, vertical: true)
            }

            authorRow(projection)

            if !projection.hashtagLabels.isEmpty {
                ScrollView(.horizontal, showsIndicators: false) {
                    HStack(spacing: 8) {
                        ForEach(projection.hashtagLabels, id: \.self) { tag in
                            Text(tag)
                                .font(.caption.weight(.medium))
                                .foregroundStyle(Color.highlighterAccent)
                                .padding(.horizontal, 10)
                                .padding(.vertical, 4)
                                .background(
                                    Capsule().fill(Color.highlighterAccent.opacity(0.08))
                                )
                        }
                    }
                }
            }

            Rectangle()
                .fill(Color.highlighterRule)
                .frame(height: 1)
                .padding(.top, 6)
        }
    }

    @ViewBuilder
    private func authorRow(_ projection: ArticleReaderHeaderProjection) -> some View {
        let author = authorDisplay

        NavigationLink(value: ProfileDestination.pubkey(article.pubkey)) {
            HStack(spacing: 12) {
                AuthorAvatar(
                    pubkey: article.pubkey,
                    pictureURL: author.pictureUrl,
                    displayInitial: author.displayInitial,
                    size: 40,
                    ringWidth: 2
                )

                VStack(alignment: .leading, spacing: 2) {
                    Text(author.displayName)
                        .font(.subheadline.weight(.semibold))
                        .foregroundStyle(Color.highlighterInkStrong)
                    HStack(spacing: 6) {
                        if let date = displayDate(projection.displayUnixSeconds) {
                            Text(date)
                        }
                        if let mins = projection.readTimeMinutes {
                            Text("·")
                            Text("\(mins) min read")
                        }
                    }
                    .font(.caption)
                    .foregroundStyle(Color.highlighterInkMuted)
                }
                Spacer(minLength: 0)
            }
        }
        .buttonStyle(.plain)
    }

    private var authorDisplay: ProfileDisplayProjection {
        let name = (authorProfile?.displayName ?? "").isEmpty
            ? ((authorProfile?.name ?? "").isEmpty ? String(article.pubkey.prefix(10)) : authorProfile!.name)
            : authorProfile!.displayName
        return ProfileDisplayProjection(
            displayName: name,
            displayInitial: name.first.map { String($0).uppercased() } ?? "?",
            pictureUrl: authorProfile?.picture ?? ""
        )
    }

    private var headerProjection: ArticleReaderHeaderProjection {
        let words = article.content.split(whereSeparator: \.isWhitespace).count
        let readTime: UInt32? = words > 60 ? UInt32(max(1, words / 240)) : nil
        let displaySeconds = (article.publishedAt ?? article.createdAt).flatMap { $0 > 0 ? $0 : nil }
        return ArticleReaderHeaderProjection(
            title: article.title.isEmpty ? "Untitled" : article.title,
            hashtagLabels: Array(article.hashtags.prefix(12)).map { "#\($0)" },
            displayUnixSeconds: displaySeconds,
            readTimeMinutes: readTime
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

// MARK: - Note composer sheet

private struct NoteComposerSheet: View {
    let quote: String
    var onCancel: () -> Void
    var onSave: (String) -> Void

    @State private var note: String = ""
    @FocusState private var focused: Bool

    var body: some View {
        NavigationStack {
            VStack(alignment: .leading, spacing: 12) {
                Text(quote)
                    .font(.system(.body, design: .default).italic())
                    .foregroundStyle(Color.highlighterInkStrong)
                    .padding(12)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(Color.highlighterAccent.opacity(0.12), in: RoundedRectangle(cornerRadius: 10))

                TextField("Add a note…", text: $note, axis: .vertical)
                    .lineLimit(3...8)
                    .focused($focused)
                    .textFieldStyle(.roundedBorder)

                Spacer(minLength: 0)
            }
            .padding(.horizontal, 20)
            .padding(.top, 20)
            .navigationTitle("Highlight")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    Button("Cancel", action: onCancel)
                }
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Save") { onSave(note) }
                        .fontWeight(.semibold)
                }
            }
            .onAppear { focused = true }
        }
    }
}

// MARK: - Highlight detail sheet

private struct IdentifiedHighlight: Identifiable {
    var id: String { record.eventId }
    let record: HighlightRecord
}

private struct HighlightDetailSheet: View {
    let highlight: HighlightRecord

    @Environment(HighlighterStore.self) private var app
    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    authorRow
                    quoteBlock
                    if !highlight.note.isEmpty {
                        noteBlock
                    }
                    if let ts = highlight.createdAt {
                        Text(Date(timeIntervalSince1970: TimeInterval(ts)).formatted(date: .abbreviated, time: .shortened))
                            .font(.caption)
                            .foregroundStyle(Color.highlighterInkMuted)
                    }
                }
                .padding(.horizontal, 20)
                .padding(.top, 24)
                .padding(.bottom, 40)
            }
            .background(Color.highlighterPaper)
            .navigationBarTitleDisplayMode(.inline)
        }
        .task(id: highlight.pubkey) {
            await app.requestProfile(pubkeyHex: highlight.pubkey)
        }
    }

    @ViewBuilder
    private var authorRow: some View {
        let author = authorDisplay

        HStack(spacing: 12) {
            AuthorAvatar(
                pubkey: highlight.pubkey,
                pictureURL: author.pictureUrl,
                displayInitial: author.displayInitial,
                size: 40,
                ringWidth: 2
            )
            VStack(alignment: .leading, spacing: 2) {
                Text(author.displayName)
                    .font(.subheadline.weight(.semibold))
                    .foregroundStyle(Color.highlighterInkStrong)
                Text("highlighted")
                    .font(.caption)
                    .foregroundStyle(Color.highlighterInkMuted)
            }
            Spacer(minLength: 0)
        }
    }

    private var authorDisplay: ProfileDisplayProjection {
        let profile = app.profileSnapshots[highlight.pubkey]
        let name = (profile?.displayName ?? "").isEmpty
            ? ((profile?.name ?? "").isEmpty ? String(highlight.pubkey.prefix(10)) : profile!.name)
            : profile!.displayName
        return ProfileDisplayProjection(
            displayName: name,
            displayInitial: name.first.map { String($0).uppercased() } ?? "?",
            pictureUrl: profile?.picture ?? ""
        )
    }

    private var quoteBlock: some View {
        HStack(alignment: .top, spacing: 0) {
            Rectangle()
                .fill(Color.highlighterAccent)
                .frame(width: 3)
            Text(highlight.quote)
                .font(.system(.body, design: .default))
                .foregroundStyle(Color.highlighterInkStrong)
                .padding(14)
                .frame(maxWidth: .infinity, alignment: .leading)
                .fixedSize(horizontal: false, vertical: true)
        }
        .background(Color.highlighterAccent.opacity(0.08), in: RoundedRectangle(cornerRadius: 10))
    }

    private var noteBlock: some View {
        Text(highlight.note)
            .font(.body)
            .foregroundStyle(Color.highlighterInkMuted)
            .frame(maxWidth: .infinity, alignment: .leading)
            .fixedSize(horizontal: false, vertical: true)
    }

}
