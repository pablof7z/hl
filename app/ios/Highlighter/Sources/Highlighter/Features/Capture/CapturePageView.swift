import SwiftUI
import UIKit

/// Review screen after capture. The corrected photo is the primary surface:
/// the user drags across text to underline and select it, then taps Next →
/// to proceed to the destination/metadata sheet.
///
/// Layout:
///   ┌─ close/title bar ─────────────────────┐
///   │  photo canvas (drag-to-select,        │
///   │               pinch-to-zoom)          │
///   │                                       │
///   │                   [ Next → ]          │
///   └───────────────────────────────────────┘
struct CapturePageView: View {
    @Bindable var store: CaptureStore
    let onDismiss: () -> Void

    @Environment(HighlighterStore.self) private var appStore

    // Drag-select state
    @State private var sortedLines: [OCRLine] = []
    @State private var selectionRange: ClosedRange<Int>? = nil

    // Spring-in animation
    @State private var imageScale: CGFloat = 0.88
    @State private var imageOpacity: Double = 0

    // Zoom / pan state — committed values
    @State private var zoomScale: CGFloat = 1.0
    @State private var zoomOffset: CGSize = .zero
    // Active values (updated live during gesture)
    @State private var activeZoomScale: CGFloat = 1.0
    @State private var activeZoomOffset: CGSize = .zero

    // Tracks whether a magnify gesture is in progress to suppress one-finger selection
    @GestureState private var isMagnifying: Bool = false

    // Metadata sheet
    @State private var showMetadataSheet = false

    var body: some View {
        content
            .sheet(isPresented: $showMetadataSheet) {
                CaptureMetadataSheet(store: store, onPublish: {
                    showMetadataSheet = false
                    store.publish()
                })
                .environment(appStore)
            }
            .overlay { publishingOverlay }
    }

    private var content: some View {
        ZStack(alignment: .bottom) {
            Color.black.ignoresSafeArea()

            VStack(spacing: 0) {
                titleBar
                photoCanvas
            }
            .ignoresSafeArea(edges: .bottom)

            nextButton
                .padding(.bottom, 48)
                .padding(.horizontal, 20)
        }
        .onAppear { setupLines() }
        .onAppear { triggerSpringIfReady() }
        .onChange(of: store.ocrLines) { _, lines in sortedLines = lines.sorted { $0.bbox.midY > $1.bbox.midY } }
        .onChange(of: store.thumbnail) { old, new in
            guard old == nil, new != nil else { return }
            springIn()
            zoomScale = 1.0
            zoomOffset = .zero
            activeZoomScale = 1.0
            activeZoomOffset = .zero
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
            // Zoom reset — visible only when zoomed
            if zoomScale > 1.01 {
                Button {
                    withAnimation(.spring(response: 0.35, dampingFraction: 0.75)) {
                        zoomScale = 1.0
                        zoomOffset = .zero
                        activeZoomScale = 1.0
                        activeZoomOffset = .zero
                    }
                } label: {
                    Image(systemName: "arrow.up.left.and.arrow.down.right")
                        .font(.caption.weight(.medium))
                        .foregroundStyle(.white)
                        .frame(width: 36, height: 36)
                        .background(.ultraThinMaterial, in: Circle())
                }
            } else {
                Color.clear.frame(width: 36, height: 36)
            }
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

    // MARK: - Next button

    private var nextButton: some View {
        Button {
            showMetadataSheet = true
        } label: {
            HStack(spacing: 8) {
                Image(systemName: store.stashedQuote != nil ? "highlighter" : "photo")
                Text("Next")
                    .fontWeight(.semibold)
                Image(systemName: "arrow.right")
            }
            .font(.body)
            .foregroundStyle(.white)
            .padding(.horizontal, 24)
            .padding(.vertical, 14)
            .background(
                store.canPublish ? Color.highlighterAccent : Color.black.opacity(0.55),
                in: Capsule()
            )
            .overlay(
                Capsule()
                    .stroke(Color.white.opacity(store.canPublish ? 0 : 0.25), lineWidth: 1)
            )
        }
        .frame(maxWidth: .infinity, alignment: .trailing)
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
                        .scaleEffect(activeZoomScale, anchor: .center)
                        .offset(activeZoomOffset)

                    // OCR overlay — follows the same zoom/pan transform
                    if !sortedLines.isEmpty {
                        Canvas { ctx, _ in
                            drawSelectionOverlay(ctx: ctx, dispSize: dispSize, dispOffset: dispOffset)
                        }
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                        .contentShape(Rectangle())
                        .scaleEffect(activeZoomScale, anchor: .center)
                        .offset(activeZoomOffset)
                        .gesture(
                            isMagnifying ? nil : canvasSelectionGesture(
                                containerSize: geo.size,
                                dispSize: dispSize,
                                dispOffset: dispOffset
                            )
                        )
                    }

                }
                .gesture(zoomGesture(containerSize: geo.size))
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

    // MARK: - Zoom gesture (two-finger pinch + pan)

    private func zoomGesture(containerSize: CGSize) -> some Gesture {
        MagnifyGesture()
            .updating($isMagnifying) { _, state, _ in state = true }
            .onChanged { value in
                activeZoomScale = (zoomScale * value.magnification).clamped(to: 1.0...5.0)
            }
            .onEnded { value in
                let newScale = (zoomScale * value.magnification).clamped(to: 1.0...5.0)
                zoomScale = newScale
                activeZoomScale = newScale
                if newScale <= 1.0 {
                    zoomOffset = .zero
                    activeZoomOffset = .zero
                }
            }
            .simultaneously(with:
                DragGesture(minimumDistance: 0)
                    .onChanged { value in
                        guard activeZoomScale > 1.01 else { return }
                        let maxOffset = maxAllowedOffset(containerSize: containerSize, scale: activeZoomScale)
                        activeZoomOffset = CGSize(
                            width: (zoomOffset.width + value.translation.width).clamped(to: -maxOffset.width...maxOffset.width),
                            height: (zoomOffset.height + value.translation.height).clamped(to: -maxOffset.height...maxOffset.height)
                        )
                    }
                    .onEnded { _ in
                        guard activeZoomScale > 1.01 else { return }
                        zoomOffset = activeZoomOffset
                    }
            )
    }

    private func maxAllowedOffset(containerSize: CGSize, scale: CGFloat) -> CGSize {
        CGSize(
            width: containerSize.width * (scale - 1) / 2,
            height: containerSize.height * (scale - 1) / 2
        )
    }

    // MARK: - OCR selection overlay drawing

    private func drawSelectionOverlay(ctx: GraphicsContext, dispSize: CGSize, dispOffset: CGPoint) {
        guard let range = selectionRange else { return }
        for idx in range {
            guard idx < sortedLines.count else { continue }
            let line = sortedLines[idx]
            let rect = visionToScreen(line.bbox, size: dispSize, offset: dispOffset)
            let underline = CGRect(x: rect.minX, y: rect.maxY - 4, width: rect.width, height: 4)
            ctx.fill(Path(underline), with: .color(Color.yellow.opacity(0.85)))
            ctx.fill(Path(rect), with: .color(Color.yellow.opacity(0.25)))
        }
    }

    // MARK: - One-finger selection gesture

    private func canvasSelectionGesture(
        containerSize: CGSize,
        dispSize: CGSize,
        dispOffset: CGPoint
    ) -> some Gesture {
        DragGesture(minimumDistance: 8)
            .onChanged { value in
                updateSelection(
                    start: value.startLocation,
                    current: value.location,
                    containerSize: containerSize,
                    dispSize: dispSize,
                    dispOffset: dispOffset
                )
            }
            .onEnded { _ in commitSelection() }
    }

    private func updateSelection(
        start: CGPoint, current: CGPoint,
        containerSize: CGSize,
        dispSize: CGSize, dispOffset: CGPoint
    ) {
        guard !sortedLines.isEmpty else { return }
        let vStart = screenToVision(start, containerSize: containerSize, dispSize: dispSize, dispOffset: dispOffset)
        let vCurrent = screenToVision(current, containerSize: containerSize, dispSize: dispSize, dispOffset: dispOffset)

        let anchor = nearestLineIndex(to: vStart)
        let cursor = nearestLineIndex(to: vCurrent)

        selectionRange = min(anchor, cursor)...max(anchor, cursor)
    }

    private func commitSelection() {
        guard let range = selectionRange, !sortedLines.isEmpty else {
            selectionRange = nil
            return
        }
        let selected = Array(sortedLines[range])
        let quote = selected.map { $0.text }.joined(separator: " ")
            .trimmingCharacters(in: .whitespacesAndNewlines)

        // Keep selectionRange so the yellow highlight stays visible.
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
        return sqrt((pt.x - cx) * (pt.x - cx) + (pt.y - cy) * (pt.y - cy))
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
    /// Inverts the active zoom/pan transform so a touch on the zoomed canvas
    /// maps back to the correct image-space coordinate.
    private func screenToVision(
        _ pt: CGPoint,
        containerSize: CGSize,
        dispSize: CGSize,
        dispOffset: CGPoint
    ) -> CGPoint {
        let cx = containerSize.width / 2
        let cy = containerSize.height / 2
        // Undo pan, then undo scale (pivot = container center)
        let unscaled = CGPoint(
            x: (pt.x - activeZoomOffset.width - cx) / activeZoomScale + cx,
            y: (pt.y - activeZoomOffset.height - cy) / activeZoomScale + cy
        )
        return CGPoint(
            x: (unscaled.x - dispOffset.x) / dispSize.width,
            y: 1.0 - (unscaled.y - dispOffset.y) / dispSize.height
        )
    }

}

// MARK: - Comparable clamped

private extension Comparable {
    func clamped(to range: ClosedRange<Self>) -> Self {
        min(max(self, range.lowerBound), range.upperBound)
    }
}
