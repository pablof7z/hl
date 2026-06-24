import SwiftUI

struct RootSceneView: View {
    @Environment(HighlighterStore.self) private var store
    @Environment(HighlighterAppKernel.self) private var kernel
    @Environment(\.scenePhase) private var scenePhase
    @State private var feedbackPresented: Bool = false
    /// Debounce repeated `motionEnded` callbacks for the same physical shake;
    /// iOS often delivers two within ~250ms.
    @State private var lastShakeAt: Date = .distantPast

    var body: some View {
        Group {
            // Phase 1b: route decision reads from the Rust kernel snapshot.
            // Child views are unchanged — MainTabView/LoginView/OnboardingView
            // continue to consume the live lane (HighlighterStore).
            switch kernel.appRoot.routeKind {
            case .rootShell:
                MainTabView()
            case .login:
                NavigationStack { LoginView() }
            case .onboarding:
                NavigationStack { OnboardingView() }
            }
        }
        .task {
            await store.bootstrap()
        }
        .onChange(of: scenePhase) { _, newPhase in
            if newPhase == .active {
                ShareQueueProcessor.drain(app: store, kernel: kernel)
                // Relay foreground refresh is handled by kernel.app.resume()
                // in App.swift's scenePhase onChange (belt-and-suspenders).
            }
        }
        // Belt-and-suspenders: mirror live-lane logout/onboarding state changes
        // into the kernel so both lanes agree during Phase 1 coexistence.
        .onChange(of: store.isLoggedIn) { _, isLoggedIn in
            if !isLoggedIn {
                kernel.app.dispatch(.logout)
            }
        }
        .onChange(of: store.isOnboardingComplete) { _, complete in
            if complete {
                kernel.app.dispatch(.completeOnboarding)
            }
        }
        // Kernel-owned toast: auto-dismissed by the Rust clock, no Swift Timer.
        .overlay(alignment: .top) {
            if let toast = kernel.rootShell.toast {
                KernelToastBanner(text: toast.message)
                    .padding(.top, 8)
                    .transition(.move(edge: .top).combined(with: .opacity))
            }
        }
        .animation(.easeInOut(duration: 0.25), value: kernel.rootShell.toast?.message)
        // Live-lane toast: ShareToastBanner with its own Swift OneShotUITimer,
        // kept in place during Phase 1 for non-kernel share toasts.
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

/// Displays a kernel-originated toast. Dismiss timing is Rust-clock-driven
/// (the snapshot's `toast` field becomes `nil` when the kernel auto-clears it)
/// so no Swift `OneShotUITimer` is needed here.
private struct KernelToastBanner: View {
    let text: String

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
    }
}

private struct ShareToastBanner: View {
    let text: String
    let onDismiss: () -> Void

    @State private var dismissTimer = OneShotUITimer()

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
        .onAppear {
            dismissTimer.schedule(after: 3) {
                onDismiss()
            }
        }
        .onDisappear {
            dismissTimer.cancel()
        }
    }
}
