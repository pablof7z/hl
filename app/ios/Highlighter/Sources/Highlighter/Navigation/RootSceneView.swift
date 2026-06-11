import SwiftUI

struct RootSceneView: View {
    @Environment(HighlighterStore.self) private var store
    @Environment(\.scenePhase) private var scenePhase
    @State private var feedbackPresented: Bool = false
    /// Debounce repeated `motionEnded` callbacks for the same physical shake;
    /// iOS often delivers two within ~250ms.
    @State private var lastShakeAt: Date = .distantPast

    var body: some View {
        Group {
            if store.isLoggedIn && store.nmpState.onboarding.isComplete {
                MainTabView()
            } else if store.isLoggedIn {
                NavigationStack { OnboardingInterestsView() }
            } else if store.nmpState.onboarding.isComplete {
                NavigationStack { LoginView() }
            } else {
                NavigationStack { OnboardingView() }
            }
        }
        .task {
            await store.bootstrap()
        }
        .onChange(of: scenePhase) { _, newPhase in
            if newPhase == .active {
                Task { await ShareQueueProcessor.drain(app: store) }
                store.appForegrounded()
            }
        }
        .overlay(alignment: .top) {
            if let toast = store.shareToast {
                ShareToastBanner(text: toast) {
                    store.clearToast()
                }
                .padding(.top, 8)
                .transition(.move(edge: .top).combined(with: .opacity))
            }
        }
        .animation(.easeInOut(duration: 0.25), value: store.shareToast)
        .onShake { handleShake() }
        .sheet(isPresented: $feedbackPresented) {
            FeedbackThreadsView()
        }
    }

    private func handleShake() {
        guard store.isLoggedIn else { return }
        let now = Date()
        if now.timeIntervalSince(lastShakeAt) < 1.0 { return }
        lastShakeAt = now
        if !feedbackPresented {
            feedbackPresented = true
        }
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
            Button(action: onDismiss) {
                Image(systemName: "xmark")
                    .font(.caption.weight(.bold))
                    .foregroundStyle(.white.opacity(0.9))
                    .accessibilityLabel("Dismiss")
            }
            .buttonStyle(.plain)
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
        .background(Color.green.opacity(0.9), in: .capsule)
        .shadow(radius: 6)
    }
}
