import SwiftUI
import UIKit

/// Full-screen review after a photo is snapped. Renders the OCR'd page as a
/// book-like attributed view the user reads from, selects from, and publishes
/// — replacing the old raw-TextEditor sheet.
///
/// Flow:
/// 1. User sees the page typeset as serif markdown; photo lives as a pinned
///    thumbnail in the top bar so it's reference-close but not consuming
///    vertical real estate.
/// 2. Select any span → edit menu shows "Highlight" → quote stashes with a
///    yellow highlight overlaid on the page.
/// 3. Bottom tray always visible with book + community pills, optional note,
///    and a single Publish CTA whose label reflects state
///    ("Share photo" vs "Publish highlight").
/// 4. Pencil icon opens a plain-text OCR escape hatch if structure is off.
struct CapturePageView: View {
    @Bindable var store: CaptureStore
    let onDismiss: () -> Void

    @Environment(HighlighterStore.self) private var appStore

    @State private var rendered: NSAttributedString?
    @State private var showBookPicker = false
    @State private var showCommunityPicker = false
    @State private var showOCREdit = false
    @State private var showPhotoZoom = false

    var body: some View {
        VStack(spacing: 0) {
            topBar
            pageScroll
        }
        .background(Color.highlighterPaper.ignoresSafeArea())
        .safeAreaInset(edge: .bottom, spacing: 0) { tray }
        .task(id: store.ocrMarkdown) { await renderMarkdown() }
        .sheet(isPresented: $showBookPicker) {
            BookPicker(selection: $store.selectedBook)
                .environment(appStore)
        }
        .sheet(isPresented: $showCommunityPicker) {
            CommunityPicker(selection: $store.selectedGroupId)
                .environment(appStore)
        }
        .sheet(isPresented: $showOCREdit) {
            OCREditSheet(markdown: $store.ocrMarkdown) { store.clearStash() }
        }
        .fullScreenCover(isPresented: $showPhotoZoom) {
            PhotoZoomView(image: store.thumbnail, onDismiss: { showPhotoZoom = false })
        }
        .overlay {
            if store.phase == .publishing {
                ZStack {
                    Color.black.opacity(0.2).ignoresSafeArea()
                    ProgressView("Publishing…")
                        .padding(20)
                        .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 16))
                }
            }
        }
    }

    // MARK: - Top bar

    private var topBar: some View {
        HStack(spacing: 16) {
            Button(action: onDismiss) {
                Image(systemName: "xmark")
                    .font(.body.weight(.medium))
                    .foregroundStyle(Color.highlighterInkStrong)
                    .frame(width: 36, height: 36)
                    .background(Color.highlighterRule.opacity(0.4), in: Circle())
            }
            Spacer(minLength: 0)
            Text(store.stashedQuote == nil ? "Review page" : "New highlight")
                .font(.system(.subheadline, design: .serif).weight(.semibold))
                .foregroundStyle(Color.highlighterInkStrong)
            Spacer(minLength: 0)
            Button {
                showOCREdit = true
            } label: {
                Image(systemName: "pencil")
                    .font(.body.weight(.medium))
                    .foregroundStyle(Color.highlighterInkStrong)
                    .frame(width: 36, height: 36)
                    .background(Color.highlighterRule.opacity(0.4), in: Circle())
            }
            .accessibilityLabel("Edit recognized text")
            if let image = store.thumbnail {
                Button {
                    showPhotoZoom = true
                } label: {
                    Image(uiImage: image)
                        .resizable()
                        .scaledToFill()
                        .frame(width: 36, height: 36)
                        .clipShape(RoundedRectangle(cornerRadius: 8))
                        .overlay(RoundedRectangle(cornerRadius: 8).stroke(Color.highlighterRule, lineWidth: 1))
                }
                .accessibilityLabel("View source photo")
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 10)
        .background(
            Color.highlighterPaper
                .overlay(alignment: .bottom) {
                    Rectangle().fill(Color.highlighterRule).frame(height: 0.5)
                }
        )
    }

    // MARK: - Page scroll

    @ViewBuilder
    private var pageScroll: some View {
        if store.phase == .processing && rendered == nil {
            VStack(spacing: 16) {
                ProgressView()
                Text("Reading the page…")
                    .font(.footnote)
                    .foregroundStyle(Color.highlighterInkMuted)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else if let rendered, !store.ocrMarkdown.isEmpty {
            ScrollView {
                CapturePageBodyView(
                    attributedText: rendered,
                    stashedQuote: store.stashedQuote,
                    accent: UIColor(Color.highlighterAccent),
                    highlightTint: UIColor(Color.highlighterAccent),
                    paperColor: UIColor(Color.highlighterPaper),
                    onStash: { quote, ctx in store.stashHighlight(quote: quote, context: ctx) }
                )
                .frame(maxWidth: .infinity)
            }
            .background(Color.highlighterPaper)
        } else {
            emptyState
        }
    }

    private var emptyState: some View {
        VStack(spacing: 12) {
            Image(systemName: "text.viewfinder")
                .font(.system(size: 42, weight: .light))
                .foregroundStyle(Color.highlighterInkMuted)
            Text("No text recognized")
                .font(.headline)
                .foregroundStyle(Color.highlighterInkStrong)
            Text("You can still share this as a photo, or tap the pencil to type the page yourself.")
                .font(.footnote)
                .foregroundStyle(Color.highlighterInkMuted)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 40)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    // MARK: - Bottom tray

    private var tray: some View {
        VStack(spacing: 12) {
            if let quote = store.stashedQuote {
                stashedCard(quote: quote)
            } else if !store.ocrMarkdown.isEmpty {
                hint("Select any text above and tap Highlight")
            }

            bookPill

            pickerRow(
                label: "Room",
                value: communityName,
                placeholder: "Required",
                icon: "number",
                action: { showCommunityPicker = true }
            )

            TextField("Add a note (optional)", text: $store.note, axis: .vertical)
                .lineLimit(1...4)
                .font(.callout)
                .padding(.horizontal, 14)
                .padding(.vertical, 12)
                .background(Color.highlighterPaper, in: RoundedRectangle(cornerRadius: 12))
                .overlay(
                    RoundedRectangle(cornerRadius: 12)
                        .stroke(Color.highlighterRule, lineWidth: 1)
                )

            if store.isUploading {
                HStack(spacing: 8) {
                    ProgressView().scaleEffect(0.8)
                    Text("Uploading photo…")
                        .font(.footnote)
                        .foregroundStyle(Color.highlighterInkMuted)
                    Spacer()
                }
            } else if let uploadError = store.uploadError {
                HStack(spacing: 8) {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .foregroundStyle(.red)
                    Text(uploadError)
                        .font(.footnote)
                        .foregroundStyle(Color.highlighterInkStrong)
                        .lineLimit(2)
                    Spacer()
                    Button("Retry") { store.retryUpload() }
                        .font(.footnote.weight(.semibold))
                }
            }

            Button {
                store.publish()
            } label: {
                HStack {
                    Image(systemName: store.stashedQuote == nil ? "photo" : "highlighter")
                    Text(publishLabel)
                        .fontWeight(.semibold)
                    Spacer()
                    Image(systemName: "arrow.up")
                }
                .font(.body)
                .foregroundStyle(Color.white)
                .padding(.horizontal, 18)
                .padding(.vertical, 14)
                .frame(maxWidth: .infinity)
                .background(
                    store.canPublish ? Color.highlighterAccent : Color.highlighterInkMuted.opacity(0.5),
                    in: RoundedRectangle(cornerRadius: 14)
                )
            }
            .disabled(!store.canPublish)
        }
        .padding(16)
        .background(
            Color.highlighterPaper
                .overlay(alignment: .top) {
                    Rectangle().fill(Color.highlighterRule).frame(height: 0.5)
                }
                .shadow(color: .black.opacity(0.06), radius: 8, x: 0, y: -2)
                .ignoresSafeArea(edges: .bottom)
        )
    }

    @ViewBuilder
    private func stashedCard(quote: String) -> some View {
        HStack(alignment: .top, spacing: 0) {
            Rectangle()
                .fill(Color.highlighterAccent)
                .frame(width: 3)
            VStack(alignment: .leading, spacing: 6) {
                Text(quote)
                    .font(.system(.callout, design: .serif))
                    .foregroundStyle(Color.highlighterInkStrong)
                    .lineLimit(4)
                    .frame(maxWidth: .infinity, alignment: .leading)
                Button {
                    store.clearStash()
                } label: {
                    Label("Remove", systemImage: "xmark.circle.fill")
                        .labelStyle(.iconOnly)
                        .foregroundStyle(Color.highlighterInkMuted)
                        .font(.footnote)
                }
            }
            .padding(12)
        }
        .background(Color.highlighterAccent.opacity(0.08), in: RoundedRectangle(cornerRadius: 10))
    }

    private func hint(_ text: String) -> some View {
        Text(text)
            .font(.footnote)
            .foregroundStyle(Color.highlighterInkMuted)
            .frame(maxWidth: .infinity, alignment: .leading)
    }

    /// Book-selection row. When a book is picked, shows its cover + title
    /// + author in a short-stack so the user sees what will travel with
    /// the highlight; otherwise falls back to a neutral "Add a book" pill.
    @ViewBuilder
    private var bookPill: some View {
        Button {
            showBookPicker = true
        } label: {
            HStack(spacing: 12) {
                bookPillLeading
                bookPillText
                Spacer(minLength: 0)
                Image(systemName: "chevron.right")
                    .font(.caption.weight(.medium))
                    .foregroundStyle(Color.highlighterInkMuted)
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 10)
            .background(Color.highlighterPaper, in: RoundedRectangle(cornerRadius: 12))
            .overlay(RoundedRectangle(cornerRadius: 12).stroke(Color.highlighterRule, lineWidth: 1))
        }
        .buttonStyle(.plain)
    }

    @ViewBuilder
    private var bookPillLeading: some View {
        if let selection = store.selectedBook {
            let cover = selection.coverURL
            if !cover.isEmpty, let url = URL(string: cover) {
                AsyncImage(url: url) { phase in
                    switch phase {
                    case .success(let img):
                        img.resizable().scaledToFill()
                    default:
                        bookPillFallback
                    }
                }
                .frame(width: 34, height: 48)
                .clipShape(RoundedRectangle(cornerRadius: 4))
            } else {
                bookPillFallback
            }
        } else {
            Image(systemName: "book.closed")
                .font(.body)
                .foregroundStyle(Color.highlighterInkMuted)
                .frame(width: 34, height: 48)
        }
    }

    private var bookPillFallback: some View {
        ZStack {
            Color.highlighterRule.opacity(0.6)
            Image(systemName: "book.closed")
                .foregroundStyle(Color.highlighterInkMuted)
        }
        .frame(width: 34, height: 48)
        .clipShape(RoundedRectangle(cornerRadius: 4))
    }

    @ViewBuilder
    private var bookPillText: some View {
        if let selection = store.selectedBook {
            VStack(alignment: .leading, spacing: 2) {
                Text(selection.title.isEmpty ? "Untitled book" : selection.title)
                    .font(.callout.weight(.semibold))
                    .foregroundStyle(Color.highlighterInkStrong)
                    .lineLimit(1)
                if !selection.author.isEmpty {
                    Text(selection.author)
                        .font(.caption)
                        .foregroundStyle(Color.highlighterInkMuted)
                        .lineLimit(1)
                } else if !selection.catalogId.isEmpty {
                    Text(selection.catalogId)
                        .font(.caption.monospacedDigit())
                        .foregroundStyle(Color.highlighterInkMuted)
                        .lineLimit(1)
                }
            }
        } else {
            VStack(alignment: .leading, spacing: 2) {
                Text("Add a book")
                    .font(.callout.weight(.semibold))
                    .foregroundStyle(Color.highlighterInkStrong)
                Text("Scan a barcode, search, or share as photo only")
                    .font(.caption)
                    .foregroundStyle(Color.highlighterInkMuted)
                    .lineLimit(1)
            }
        }
    }

    @ViewBuilder
    private func pickerRow(
        label: String,
        value: String,
        placeholder: String?,
        icon: String,
        action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            HStack(spacing: 10) {
                Image(systemName: icon)
                    .font(.footnote)
                    .foregroundStyle(Color.highlighterInkMuted)
                    .frame(width: 20)
                Text(label)
                    .font(.callout)
                    .foregroundStyle(Color.highlighterInkStrong)
                Spacer()
                Text(value.isEmpty ? (placeholder ?? "") : value)
                    .font(.callout)
                    .foregroundStyle(value.isEmpty ? Color.highlighterInkMuted : Color.highlighterAccent)
                    .lineLimit(1)
                Image(systemName: "chevron.right")
                    .font(.caption.weight(.medium))
                    .foregroundStyle(Color.highlighterInkMuted)
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 12)
            .background(Color.highlighterPaper, in: RoundedRectangle(cornerRadius: 12))
            .overlay(RoundedRectangle(cornerRadius: 12).stroke(Color.highlighterRule, lineWidth: 1))
        }
        .buttonStyle(.plain)
    }

    private var publishLabel: String {
        store.stashedQuote != nil && store.selectedBook != nil
            ? "Publish highlight"
            : (store.stashedQuote != nil ? "Pick a book to highlight" : "Share photo")
    }

    private var communityName: String {
        guard let id = store.selectedGroupId else { return "" }
        if let community = appStore.joinedCommunities.first(where: { $0.id == id }) {
            return community.name.isEmpty ? id : community.name
        }
        return id
    }

    private func renderMarkdown() async {
        let markdown = store.ocrMarkdown
        guard !markdown.isEmpty else {
            rendered = nil
            return
        }
        let accent = UIColor(Color.highlighterAccent)
        let ink = UIColor(Color.highlighterInkStrong)
        let muted = UIColor(Color.highlighterInkMuted)
        let output = await Task.detached(priority: .userInitiated) {
            MarkdownRenderer.render(
                content: markdown,
                highlights: [],
                accent: accent,
                tint: accent,
                ink: ink,
                muted: muted
            )
        }.value
        rendered = output.body
    }
}

// MARK: - OCR plain-text escape hatch

private struct OCREditSheet: View {
    @Binding var markdown: String
    let onChange: () -> Void

    @Environment(\.dismiss) private var dismiss
    @State private var working: String = ""

    var body: some View {
        NavigationStack {
            TextEditor(text: $working)
                .font(.system(.body, design: .monospaced))
                .padding(12)
                .background(Color.highlighterPaper)
                .navigationTitle("Edit text")
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .cancellationAction) {
                        Button("Cancel") { dismiss() }
                    }
                    ToolbarItem(placement: .confirmationAction) {
                        Button("Save") {
                            if working != markdown {
                                markdown = working
                                onChange()
                            }
                            dismiss()
                        }
                        .fontWeight(.semibold)
                    }
                }
        }
        .onAppear { working = markdown }
    }
}

// MARK: - Photo zoom

private struct PhotoZoomView: View {
    let image: UIImage?
    let onDismiss: () -> Void

    @State private var scale: CGFloat = 1
    @State private var lastScale: CGFloat = 1

    var body: some View {
        ZStack {
            Color.black.ignoresSafeArea()
            if let image {
                Image(uiImage: image)
                    .resizable()
                    .scaledToFit()
                    .scaleEffect(scale)
                    .gesture(
                        MagnificationGesture()
                            .onChanged { value in
                                scale = max(1, min(5, lastScale * value))
                            }
                            .onEnded { _ in lastScale = scale }
                    )
            }
            VStack {
                HStack {
                    Spacer()
                    Button(action: onDismiss) {
                        Image(systemName: "xmark")
                            .font(.body.weight(.semibold))
                            .foregroundStyle(.white)
                            .padding(12)
                            .background(.ultraThinMaterial, in: Circle())
                    }
                    .padding()
                }
                Spacer()
            }
        }
    }
}
