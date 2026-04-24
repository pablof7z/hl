import SwiftUI

/// Capture tab — the OCR + photo entry point. Tap "Take photo" → camera →
/// full-screen page review (typeset OCR markdown with native selection →
/// stash highlight + pick book & room → publish). Publishing produces either
/// a highlight (kind:9802 + kind:16 repost, photo attached via imeta) or a
/// kind:20 picture event when no quote is stashed. Photos always upload to
/// Blossom and accompany the publish.
struct CaptureTabView: View {
    @Environment(HighlighterStore.self) private var appStore
    @State private var store: CaptureStore?
    @State private var showCamera = false

    var body: some View {
        NavigationStack {
            Group {
                if let store {
                    content(store: store)
                } else {
                    ProgressView()
                }
            }
            .background(Color.highlighterPaper.ignoresSafeArea())
            .navigationTitle("Capture")
            .navigationBarTitleDisplayMode(.large)
        }
        .task {
            if store == nil {
                store = CaptureStore(safeCore: appStore.safeCore)
            }
        }
    }

    @ViewBuilder
    private func content(store: CaptureStore) -> some View {
        VStack(spacing: 32) {
            Spacer()
            Image(systemName: "camera.viewfinder")
                .font(.system(size: 56, weight: .light))
                .foregroundStyle(Color.highlighterInkMuted)
            VStack(spacing: 8) {
                Text("Snap a page")
                    .font(.title3.weight(.medium))
                    .foregroundStyle(Color.highlighterInkStrong)
                Text("We'll read the page and let you highlight the bits worth sharing. The photo always travels with your highlight.")
                    .font(.callout)
                    .foregroundStyle(Color.highlighterInkMuted)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal, 32)
            }
            Button {
                showCamera = true
            } label: {
                Label("Take photo", systemImage: "camera")
                    .font(.body.weight(.medium))
                    .padding(.horizontal, 32)
                    .padding(.vertical, 14)
                    .background(Color.highlighterAccent, in: Capsule())
                    .foregroundStyle(Color.white)
            }
            Spacer()
        }
        .fullScreenCover(isPresented: $showCamera) {
            CameraView { result in
                showCamera = false
                if case .captured(let image) = result {
                    store.handleCapturedImage(image)
                }
            }
            .ignoresSafeArea()
        }
        .fullScreenCover(isPresented: reviewBinding(store)) {
            CapturePageView(
                store: store,
                onDismiss: { store.reset(keepingPickerSelection: false) }
            )
            .environment(appStore)
        }
        .alert("Couldn't publish", isPresented: errorBinding(store), actions: {
            Button("OK") { store.reset(keepingPickerSelection: true) }
        }, message: {
            if case .error(let msg) = store.phase {
                Text(msg)
            }
        })
        .onChange(of: store.phase) { _, newValue in
            if case .done = newValue {
                store.reset(keepingPickerSelection: false)
            }
        }
    }

    private func reviewBinding(_ store: CaptureStore) -> Binding<Bool> {
        Binding(
            get: {
                switch store.phase {
                case .processing, .reviewing, .publishing: return true
                default: return false
                }
            },
            set: { presented in
                if !presented, case .reviewing = store.phase {
                    store.reset(keepingPickerSelection: false)
                }
            }
        )
    }

    private func errorBinding(_ store: CaptureStore) -> Binding<Bool> {
        Binding(
            get: {
                if case .error = store.phase { return true }
                return false
            },
            set: { _ in }
        )
    }
}
