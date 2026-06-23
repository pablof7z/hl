import Kingfisher
import SwiftUI
import UIKit

/// Full-screen NIP-23 long-form reader. Handles the gorgeous header (cover,
/// serif title, author row, metadata), renders the body via `NostrContentView`
/// in article mode, and orchestrates the text-selection → highlight flow.
struct ArticleReaderView: View {
    let target: ArticleReaderTarget

    @Environment(HighlighterStore.self) private var app
    @Environment(HighlighterAppKernel.self) private var kernel
    @State private var store: ArticleReaderStore?
    @State private var pendingHighlight: PendingHighlight?
    @State private var highlightDetail: HighlightRecord?
    @State private var toast: String?
    @State private var shareTarget: ShareToCommunityTarget?
    @State private var toastResetTimer = OneShotUITimer()

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
                        shareTarget = ShareToCommunityTarget.article(article, core: app.core)
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
                contentTreeJson: kernel.articleReader[target.address]?.contentTreeJson ?? "",
                authorProfile: app.profileSnapshots[target.pubkey],
                highlights: store.highlights,
                onPublishHighlight: { quote, context in
                    Task { await publish(quote: quote, context: context, note: "") }
                },
                onRequestNote: { quote, context in
                    pendingHighlight = PendingHighlight(quote: quote, context: context)
                },
                onHighlightTap: { highlightDetail = $0 }
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
        // Phase 7: kernel publish is fire-and-forget (returns nil); always show
        // success toast. Errors surface via the kernel event bus, not here.
        await store.publishHighlight(quote: quote, note: note, context: context)
        withAnimation(.easeOut(duration: 0.2)) {
            toast = "Highlighted"
        }
        toastResetTimer.schedule(after: 1.8) {
            withAnimation(.easeIn(duration: 0.2)) { toast = nil }
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
        let address = target.address
        if !address.isEmpty {
            content.commentsAttachment(
                scope: CommentScope(rootTagName: "A", rootTagValue: address, rootKind: 30023),
                artifactAuthorPubkey: target.pubkey
            )
        } else {
            content
        }
    }
}

// MARK: - Scroll container composing header + body

private struct ReaderScroll: View {
    let article: ArticleRecord
    let contentTreeJson: String
    let authorProfile: ProfileMetadata?
    let highlights: [HighlightRecord]
    var onPublishHighlight: (String, String) -> Void
    var onRequestNote: (String, String) -> Void
    var onHighlightTap: (HighlightRecord) -> Void

    @State private var contentTree: ContentTreeWire?
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

                if let tree = contentTree {
                    let highlightDecorations = highlights.map { h in
                        NostrContentDecoration(
                            id: h.eventId,
                            quote: h.quote,
                            color: Color.highlighterAccent.opacity(0.25)
                        )
                    }
                    NostrContentView(
                        tree: tree,
                        decorations: highlightDecorations,
                        selectionEnabled: true
                    )
                    .padding(.horizontal, 20)
                    .nostrContentRenderer(NostrContentRenderer(
                        textColor: Color.highlighterInkStrong,
                        secondaryTextColor: Color.highlighterInkMuted,
                        mentionColor: Color.highlighterAccent,
                        hashtagColor: Color.highlighterAccent,
                        linkColor: Color.highlighterAccent,
                        quoteBorderColor: Color.highlighterRule,
                        codeBackgroundColor: Color.highlighterRule.opacity(0.15),
                        placeholderColor: Color.highlighterInkMuted.opacity(0.6),
                        callbacks: NostrContentCallbacks(
                            onMentionTap: { pubkey in
                                profileNavPubkey = pubkey
                                profileNavActive = true
                            },
                            onLinkTap: { url in UIApplication.shared.open(url) },
                            onImageTap: { url in imageToOpen = IdentifiableURL(url: url) },
                            onEventRefTap: { _ in },
                            onTextSelected: { quote, context in
                                onPublishHighlight(quote, context)
                            },
                            onDecorationTap: { id in
                                if let h = highlights.first(where: { $0.eventId == id.raw }) {
                                    onHighlightTap(h)
                                }
                            },
                            onTextSelectedWithNote: { quote, context in
                                onRequestNote(quote, context)
                            }
                        )
                    ))
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
        .task(id: contentTreeJson) {
            guard !contentTreeJson.isEmpty,
                  let data = contentTreeJson.data(using: .utf8),
                  let tree = try? JSONDecoder().decode(ContentTreeWire.self, from: data)
            else { return }
            contentTree = tree
        }
    }
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
        let profile = authorProfile
        let pubkey = article.pubkey
        let rawName = profile.map { !$0.displayName.isEmpty ? $0.displayName : $0.name } ?? ""
        let name = rawName.isEmpty ? String(pubkey.prefix(10)) : rawName
        return ProfileDisplayProjection(
            displayName: name,
            displayInitial: String(name.prefix(1)),
            pictureUrl: profile?.picture ?? ""
        )
    }

    private var headerProjection: ArticleReaderHeaderProjection {
        let title = article.title.isEmpty ? "Untitled" : article.title
        let hashtagLabels = article.hashtags.map { "#\($0)" }
        let displayUnixSeconds = article.publishedAt ?? article.createdAt
        let wordCount = article.content.split(separator: " ").count
        let mins = wordCount / 200
        return ArticleReaderHeaderProjection(
            title: title,
            hashtagLabels: hashtagLabels,
            displayUnixSeconds: displayUnixSeconds,
            readTimeMinutes: mins > 0 ? UInt32(mins) : nil
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
        let pubkey = highlight.pubkey
        let rawName = profile.map { !$0.displayName.isEmpty ? $0.displayName : $0.name } ?? ""
        let name = rawName.isEmpty ? String(pubkey.prefix(10)) : rawName
        return ProfileDisplayProjection(
            displayName: name,
            displayInitial: String(name.prefix(1)),
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
