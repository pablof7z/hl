import SwiftUI

struct RootSceneView: View {
    @Environment(HighlighterStore.self) private var store
    @Environment(\.scenePhase) private var scenePhase

    var body: some View {
        Group {
            if store.isLoggedIn {
                MainTabView()
            } else {
                LoginView()
            }
        }
        .task {
            await store.bootstrap()
        }
        .onChange(of: scenePhase) { _, newPhase in
            if newPhase == .active {
                Task { await ShareQueueProcessor.drain(app: store) }
            }
        }
        .overlay(alignment: .top) {
            if let toast = store.shareToast {
                ShareToastBanner(text: toast) {
                    store.shareToast = nil
                }
                .padding(.top, 8)
                .transition(.move(edge: .top).combined(with: .opacity))
            }
        }
        .animation(.easeInOut(duration: 0.25), value: store.shareToast)
    }
}

private struct ShareToastBanner: View {
    let text: String
    let onDismiss: () -> Void

    var body: some View {
        HStack {
            Image(systemName: "checkmark.circle.fill")
                .foregroundStyle(.white)
            Text(text)
                .foregroundStyle(.white)
                .font(.subheadline.weight(.medium))
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
        .background(Color.green.opacity(0.9), in: .capsule)
        .shadow(radius: 6)
        .task {
            try? await Task.sleep(nanoseconds: 3 * 1_000_000_000)
            onDismiss()
        }
    }
}
