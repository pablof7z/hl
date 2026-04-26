import Kingfisher
import SwiftUI
import UIKit

/// Review screen after capture. The corrected photo is the primary surface:
/// the user drags across text to underline and select it, then swipes up
/// (or taps Publish) to share.
///
/// Layout:
///   ┌─ close/title bar ─────────────────────┐
///   │  photo canvas (drag-to-select)        │
///   │   · yellow underlines for selected    │
///   │     OCR lines while dragging           │
///   ├───────────────────────────────────────┤
///   │  book pill  ·  community chip         │
///   │  note field                           │
///   │  ──── swipe up to publish ────        │
///   └───────────────────────────────────────┘
struct CapturePageView: View {
    @Bindable var store: CaptureStore
    let onDismiss: () -> Void

    @Environment(HighlighterStore.self) private var appStore

    // Drag-select state
    @State private var sortedLines: [OCRLine] = []
    @State private var selectionRange: ClosedRange<Int>? = nil
    @State private var dragAnchorIdx: Int? = nil

    // Image display geometry (populated by GeometryReader)
    @State private var imageDisplaySize: CGSize = .zero
    @State private var imageDisplayOffset: CGPoint = .zero

    // Spring-in animation
    @State private var imageScale: CGFloat = 0.88
    @State private var imageOpacity: Double = 0

    // Swipe-up publish
    @State private var swipeTranslation: CGFloat = 0
    @State private var gestureMode: DragMode? = nil

    // Sheets
    @State private var showBookPicker = false
    @State private var showCommunityPicker = false

    private enum DragMode { case selecting, publishing }

    var body: some View {
        content
            .sheet(isPresented: $showBookPicker) {
                BookPicker(selection: $store.selectedBook).environment(appStore)
            }
            .sheet(isPresented: $showCommunityPicker) {
                CommunityPicker(selection: $store.selectedGroupId).environment(appStore)
            }
            .overlay { publishingOverlay }
    }

    private var content: some View {
        ZStack(alignment: .bottom) {
            Color.black.ignoresSafeArea()

            VStack(spacing: 0) {
                titleBar
                photoCanvas
                bottomPanel
            }
            .ignoresSafeArea(edges: .bottom)
        }
        .onAppear { setupLines() }
        .onAppear { triggerSpringIfReady() }
        .onChange(of: store.ocrLines) { _, lines in sortedLines = lines.sorted { $0.bbox.midY > $1.bbox.midY } }
        .onChange(of: store.thumbnail) { old, new in
            guard old == nil, new != nil else { return }
            springIn()
        }
    }

    private func setupLines() {
        sortedLines = store.ocrLines.sorted { $0.bbox.midY > $1.bbox.midY }
    }

    private func triggerSpringIfReady() {
        if store.thumbnail != nil { springIn() }
    }

    private func springIn() {
        withAnimation(.spring(response: 0.45, dampingFraction: 0.72)) {
            imageScale = 1.0
            imageOpacity = 1.0
        }
    }

    @ViewBuilder
    private var publishingOverlay: some View {
        if store.phase == .publishing {
            ZStack {
                Color.black.opacity(0.35).ignoresSafeArea()
                VStack(spacing: 8) {
                    ProgressView().tint(.white)
                    Text("Publishing…").font(.footnote).foregroundStyle(.white)
                }
            }
            .transition(.opacity)
        }
    }

    // MARK: - Title bar

    private var titleBar: some View {
        HStack {
            Button(action: onDismiss) {
                Image(systemName: "xmark")
                    .font(.body.weight(.medium))
                    .foregroundStyle(.white)
                    .frame(width: 36, height: 36)
                    .background(.ultraThinMaterial, in: Circle())
            }
            Spacer()
            if store.phase == .processing {
                HStack(spacing: 6) {
                    ProgressView().scaleEffect(0.7).tint(.white)
                    Text("Reading the page…")
                        .font(.footnote.weight(.medium))
                        .foregroundStyle(.white.opacity(0.8))
                }
            } else if store.stashedQuote != nil {
                Text("Highlight ready")
                    .font(.subheadline.weight(.semibold))
                    .foregroundStyle(.white)
            } else if !sortedLines.isEmpty {
                Text("Drag to select")
                    .font(.subheadline)
                    .foregroundStyle(.white.opacity(0.7))
            }
            Spacer()
            // Spacer to balance the close button
            Color.clear.frame(width: 36, height: 36)
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 10)
        .background(
            LinearGradient(
                colors: [.black.opacity(0.6), .clear],
                startPoint: .top,
                endPoint: .bottom
            )
            .ignoresSafeArea(edges: .top)
        )
    }

    // MARK: - Photo canvas

    @ViewBuilder
    private var photoCanvas: some View {
        if store.phase == .processing && store.thumbnail == nil {
            loadingState
        } else if let thumbnail = store.thumbnail {
            GeometryReader { geo in
                let (dispSize, dispOffset) = computeLayout(thumbnail: thumbnail, container: geo.size)

                ZStack(alignment: .topLeading) {
                    Color.black

                    Image(uiImage: thumbnail)
                        .resizable()
                        .scaledToFit()
                        .frame(width: dispSize.width, height: dispSize.height)
                        .offset(x: dispOffset.x, y: dispOffset.y)
                        .scaleEffect(imageScale)
                        .opacity(imageOpacity)

                    // OCR selection overlay
                    if !sortedLines.isEmpty {
                        Canvas { ctx, _ in
                            drawSelectionOverlay(ctx: ctx, dispSize: dispSize, dispOffset: dispOffset)
                        }
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                        .contentShape(Rectangle())
                        .gesture(canvasGesture(dispSize: dispSize, dispOffset: dispOffset))
                    }

                }
                .onAppear {
                    imageDisplaySize = dispSize
                    imageDisplayOffset = dispOffset
                }
                .onChange(of: geo.size) { _, newSize in
                    let (s, o) = computeLayout(thumbnail: thumbnail, container: newSize)
                    imageDisplaySize = s
                    imageDisplayOffset = o
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else {
            emptyState
        }
    }

    private var loadingState: some View {
        VStack(spacing: 12) {
            ProgressView().tint(.white)
            Text("Reading the page…")
                .font(.footnote)
                .foregroundStyle(.white.opacity(0.7))
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    private var emptyState: some View {
        VStack(spacing: 12) {
            Image(systemName: "text.viewfinder")
                .font(.system(size: 40, weight: .light))
                .foregroundStyle(.white.opacity(0.5))
            Text("No text recognized")
                .font(.headline)
                .foregroundStyle(.white)
            Text("Drag to add a note, or share as a photo.")
                .font(.footnote)
                .foregroundStyle(.white.opacity(0.6))
                .multilineTextAlignment(.center)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    // MARK: - OCR selection

    private func drawSelectionOverlay(ctx: GraphicsContext, dispSize: CGSize, dispOffset: CGPoint) {
        guard let range = selectionRange else { return }
        for idx in range {
            guard idx < sortedLines.count else { continue }
            let line = sortedLines[idx]
            let rect = visionToScreen(line.bbox, size: dispSize, offset: dispOffset)
            // Yellow underline bar at the bottom of each text line
            let underline = CGRect(x: rect.minX, y: rect.maxY - 4, width: rect.width, height: 4)
            ctx.fill(
                Path(underline.insetBy(dx: 0, dy: 0)),
                with: .color(Color.yellow.opacity(0.85))
            )
            // Light yellow wash over the full line
            ctx.fill(
                Path(rect),
                with: .color(Color.yellow.opacity(0.25))
            )
        }
    }

    private func canvasGesture(dispSize: CGSize, dispOffset: CGPoint) -> some Gesture {
        DragGesture(minimumDistance: 8)
            .onChanged { value in
                let dx = abs(value.translation.width)
                let dy = abs(value.translation.height)

                if gestureMode == nil {
                    gestureMode = dx >= dy * 0.8 ? .selecting : .publishing
                    if gestureMode == .selecting { selectionRange = nil }
                }

                switch gestureMode {
                case .selecting:
                    updateSelection(
                        start: value.startLocation,
                        current: value.location,
                        dispSize: dispSize,
                        dispOffset: dispOffset
                    )
                case .publishing:
                    let up = min(0, value.translation.height)
                    swipeTranslation = up
                case nil:
                    break
                }
            }
            .onEnded { value in
                switch gestureMode {
                case .selecting:
                    commitSelection()
                case .publishing:
                    if swipeTranslation < -90, store.canPublish {
                        store.publish()
                    }
                    withAnimation(.spring(response: 0.3, dampingFraction: 0.8)) {
                        swipeTranslation = 0
                    }
                case nil:
                    break
                }
                gestureMode = nil
            }
    }

    private func updateSelection(
        start: CGPoint, current: CGPoint,
        dispSize: CGSize, dispOffset: CGPoint
    ) {
        guard !sortedLines.isEmpty else { return }
        let vStart = screenToVision(start, size: dispSize, offset: dispOffset)
        let vCurrent = screenToVision(current, size: dispSize, offset: dispOffset)

        let anchor = nearestLineIndex(to: vStart)
        let cursor = nearestLineIndex(to: vCurrent)
        dragAnchorIdx = anchor
        selectionRange = min(anchor, cursor)...max(anchor, cursor)
    }

    private func commitSelection() {
        guard let range = selectionRange, !sortedLines.isEmpty else {
            selectionRange = nil
            dragAnchorIdx = nil
            return
        }
        let selected = Array(sortedLines[range])
        let quote = selected.map { $0.text }.joined(separator: " ")
            .trimmingCharacters(in: .whitespacesAndNewlines)

        dragAnchorIdx = nil
        // Keep selectionRange so the highlight stays visible on the image.

        guard !quote.isEmpty else { return }
        store.stashHighlight(quote: quote, context: "")
    }

    private func nearestLineIndex(to pt: CGPoint) -> Int {
        sortedLines.indices.min(by: {
            pointToBBoxDistance(pt, bbox: sortedLines[$0].bbox)
            < pointToBBoxDistance(pt, bbox: sortedLines[$1].bbox)
        }) ?? 0
    }

    private func pointToBBoxDistance(_ pt: CGPoint, bbox: CGRect) -> CGFloat {
        let cx = min(max(pt.x, bbox.minX), bbox.maxX)
        let cy = min(max(pt.y, bbox.minY), bbox.maxY)
        let dx = pt.x - cx
        let dy = pt.y - cy
        return sqrt(dx * dx + dy * dy)
    }

    // MARK: - Coordinate helpers

    private func computeLayout(thumbnail: UIImage, container: CGSize) -> (size: CGSize, offset: CGPoint) {
        let iw = thumbnail.size.width
        let ih = thumbnail.size.height
        guard iw > 0, ih > 0 else { return (.zero, .zero) }
        let imageAR = iw / ih
        let containerAR = container.width / container.height

        let dispSize: CGSize
        if imageAR > containerAR {
            dispSize = CGSize(width: container.width, height: container.width / imageAR)
        } else {
            dispSize = CGSize(width: container.height * imageAR, height: container.height)
        }
        let offset = CGPoint(
            x: (container.width - dispSize.width) / 2,
            y: (container.height - dispSize.height) / 2
        )
        return (dispSize, offset)
    }

    /// Vision normalized coords (bottom-left origin) → screen rect.
    private func visionToScreen(_ bbox: CGRect, size: CGSize, offset: CGPoint) -> CGRect {
        CGRect(
            x: offset.x + bbox.minX * size.width,
            y: offset.y + (1.0 - bbox.maxY) * size.height,
            width: bbox.width * size.width,
            height: bbox.height * size.height
        )
    }

    /// Screen point → Vision normalized coords (bottom-left origin).
    private func screenToVision(_ pt: CGPoint, size: CGSize, offset: CGPoint) -> CGPoint {
        CGPoint(
            x: (pt.x - offset.x) / size.width,
            y: 1.0 - (pt.y - offset.y) / size.height
        )
    }

    // MARK: - Bottom panel

    private var bottomPanel: some View {
        VStack(spacing: 10) {
            // Upload status
            if store.isUploading {
                HStack(spacing: 6) {
                    ProgressView().scaleEffect(0.7).tint(Color.highlighterInkMuted)
                    Text("Uploading…")
                        .font(.caption)
                        .foregroundStyle(Color.highlighterInkMuted)
                    Spacer()
                }
            } else if let err = store.uploadError {
                HStack(spacing: 6) {
                    Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(.red)
                    Text(err).font(.caption).lineLimit(1)
                    Spacer()
                    Button("Retry") { store.retryUpload() }
                        .font(.caption.weight(.semibold))
                        .foregroundStyle(Color.highlighterAccent)
                }
            }

            // Book
            bookPill

            // Community
            communityRow

            // Note
            TextField("Add a note (optional)", text: $store.note, axis: .vertical)
                .lineLimit(1...3)
                .font(.callout)
                .padding(.horizontal, 14)
                .padding(.vertical, 10)
                .background(Color.highlighterPaper, in: RoundedRectangle(cornerRadius: 12))
                .overlay(RoundedRectangle(cornerRadius: 12).stroke(Color.highlighterRule, lineWidth: 1))

            // Publish / swipe-up area
            publishArea
        }
        .padding(.horizontal, 16)
        .padding(.top, 14)
        .padding(.bottom, 32)
        .background(
            Color.highlighterPaper
                .overlay(alignment: .top) {
                    Rectangle().fill(Color.highlighterRule).frame(height: 0.5)
                }
                .shadow(color: .black.opacity(0.08), radius: 10, x: 0, y: -3)
                .ignoresSafeArea(edges: .bottom)
        )
        .offset(y: swipeTranslation)
    }

    @ViewBuilder
    private var publishArea: some View {
        Button {
            store.publish()
        } label: {
            HStack(spacing: 8) {
                Image(systemName: store.stashedQuote != nil ? "highlighter" : "photo")
                Text(publishLabel).fontWeight(.semibold)
                Spacer()
                Image(systemName: "arrow.up")
            }
            .font(.body)
            .foregroundStyle(.white)
            .padding(.horizontal, 18)
            .padding(.vertical, 14)
            .background(
                store.canPublish ? Color.highlighterAccent : Color.highlighterInkMuted.opacity(0.4),
                in: RoundedRectangle(cornerRadius: 14)
            )
        }
        .disabled(!store.canPublish)

        if store.canPublish && store.stashedQuote != nil {
            Text("or swipe up on the photo")
                .font(.caption2)
                .foregroundStyle(Color.highlighterInkMuted)
        }
    }

    private var publishLabel: String {
        if let q = store.stashedQuote, !q.isEmpty {
            return store.selectedBook != nil ? "Publish highlight" : "Pick a book first"
        }
        return "Share photo"
    }

    // MARK: - Book pill

    @ViewBuilder
    private var bookPill: some View {
        Button { showBookPicker = true } label: {
            HStack(spacing: 10) {
                bookCover
                bookText
                Spacer(minLength: 0)
                Image(systemName: "chevron.right")
                    .font(.caption.weight(.medium))
                    .foregroundStyle(Color.highlighterInkMuted)
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 8)
            .background(Color.highlighterPaper, in: RoundedRectangle(cornerRadius: 12))
            .overlay(RoundedRectangle(cornerRadius: 12).stroke(Color.highlighterRule, lineWidth: 1))
        }
        .buttonStyle(.plain)
    }

    @ViewBuilder
    private var bookCover: some View {
        if let sel = store.selectedBook, !sel.coverURL.isEmpty, let url = URL(string: sel.coverURL) {
            KFImage(url)
                .placeholder { bookCoverPlaceholder }
                .fade(duration: 0.15)
                .resizable()
                .scaledToFill()
                .frame(width: 30, height: 42)
                .clipShape(RoundedRectangle(cornerRadius: 3))
        } else if store.selectedBook != nil {
            bookCoverPlaceholder
        } else {
            Image(systemName: "book.closed")
                .font(.body)
                .foregroundStyle(Color.highlighterInkMuted)
                .frame(width: 30, height: 42)
        }
    }

    private var bookCoverPlaceholder: some View {
        ZStack {
            Color.highlighterRule.opacity(0.5)
            Image(systemName: "book.closed").foregroundStyle(Color.highlighterInkMuted)
        }
        .frame(width: 30, height: 42)
        .clipShape(RoundedRectangle(cornerRadius: 3))
    }

    @ViewBuilder
    private var bookText: some View {
        if let sel = store.selectedBook {
            VStack(alignment: .leading, spacing: 2) {
                Text(sel.title.isEmpty ? "Untitled" : sel.title)
                    .font(.callout.weight(.semibold))
                    .foregroundStyle(Color.highlighterInkStrong)
                    .lineLimit(1)
                if !sel.author.isEmpty {
                    Text(sel.author)
                        .font(.caption)
                        .foregroundStyle(Color.highlighterInkMuted)
                        .lineLimit(1)
                }
            }
        } else {
            VStack(alignment: .leading, spacing: 2) {
                Text("Add a book")
                    .font(.callout.weight(.semibold))
                    .foregroundStyle(Color.highlighterInkStrong)
                Text("Optional — scan barcode or search")
                    .font(.caption)
                    .foregroundStyle(Color.highlighterInkMuted)
                    .lineLimit(1)
            }
        }
    }

    // MARK: - Community row

    private var communityRow: some View {
        Button { showCommunityPicker = true } label: {
            HStack(spacing: 8) {
                Image(systemName: "number")
                    .font(.caption)
                    .foregroundStyle(Color.highlighterInkMuted)
                    .frame(width: 18)
                Text("Room")
                    .font(.callout)
                    .foregroundStyle(Color.highlighterInkStrong)
                Spacer()
                Text(communityName.isEmpty ? "Optional" : communityName)
                    .font(.callout)
                    .foregroundStyle(communityName.isEmpty ? Color.highlighterInkMuted : Color.highlighterAccent)
                    .lineLimit(1)
                Image(systemName: "chevron.right")
                    .font(.caption.weight(.medium))
                    .foregroundStyle(Color.highlighterInkMuted)
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 10)
            .background(Color.highlighterPaper, in: RoundedRectangle(cornerRadius: 12))
            .overlay(RoundedRectangle(cornerRadius: 12).stroke(Color.highlighterRule, lineWidth: 1))
        }
        .buttonStyle(.plain)
    }

    private var communityName: String {
        guard let id = store.selectedGroupId else { return "" }
        return appStore.joinedCommunities.first(where: { $0.id == id })?.name ?? id
    }
}
