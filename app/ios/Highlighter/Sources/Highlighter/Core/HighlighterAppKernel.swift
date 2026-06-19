import Foundation
import Observation

/// Thin Swift wrapper around the Rust `HighlighterApp` kernel (Phase 1 nmp-lane).
///
/// Owns the `HighlighterApp` instance, registers as its observer, and
/// re-publishes the latest `AppRootSnapshot` and `RootShellSnapshot` as
/// `@Observable` state for SwiftUI. Capability requests (keychain load/clear)
/// are fulfilled via the existing `KeychainService` helpers and the results
/// fed back to Rust via `provide_capability_result`.
///
/// One instance lives alongside `HighlighterStore` in `AppEntry`. Neither
/// instance is aware of the other — coexistence is ensured by reading from
/// separate storage sub-directories (Phase 1 / §3 of the build spec).
@MainActor
@Observable
final class HighlighterAppKernel {

    // MARK: - Published snapshots

    /// Latest projection for the app-root route decision surface.
    /// Defaults to `.onboarding` until the kernel delivers its first snapshot.
    private(set) var appRoot: AppRootSnapshot = AppRootSnapshot(
        routeKind: .onboarding,
        sessionPresent: false,
        onboardingComplete: false
    )

    /// Latest projection for the root-shell chrome (tabs, toast, sheet).
    private(set) var rootShell: RootShellSnapshot = RootShellSnapshot(
        selectedTab: 0,
        tabCount: 5,
        toast: nil,
        sheetId: nil
    )

    // MARK: - Kernel handle

    /// The Rust-side kernel object. Callers may dispatch actions and manage
    /// the lifecycle (resume/suspend/shutdown) via this handle.
    let app: HighlighterApp

    // MARK: - Init

    init() {
        let dataDir: String
        if let dir = FileManager.default
            .urls(for: .applicationSupportDirectory, in: .userDomainMask).first {
            dataDir = dir.path
        } else {
            dataDir = NSTemporaryDirectory()
        }

        let kernelApp = HighlighterApp.new(config: AppConfig(dataDir: dataDir))
        self.app = kernelApp

        // Register the observer BEFORE opening views so no initial snapshot
        // is dropped. The observer holds a weak back-reference which breaks
        // the retain cycle:
        //   HighlighterAppKernel → HighlighterApp → KernelObserver ⇢ (weak) HighlighterAppKernel
        let observer = KernelObserver(kernel: self)
        kernelApp.setObserver(observer: observer)

        // Open the two root projections. The kernel will push snapshots as
        // soon as the actor processes these commands.
        kernelApp.openView(viewId: .appRoot, route: .appRoot)
        kernelApp.openView(viewId: .rootShell, route: .rootShell)
    }

    // MARK: - Snapshot ingestion (called on main actor by KernelObserver)

    fileprivate func receive(viewId: ViewId, snapshot: ViewSnapshot) {
        switch snapshot {
        case .appRoot(let s):   appRoot = s
        case .rootShell(let s): rootShell = s
        }
    }

    // MARK: - Capability fulfillment (called on main actor by KernelObserver)

    fileprivate func fulfill(request: CapabilityRequest) {
        switch request {
        case .keychain(let op):
            fulfillKeychain(op)
        }
    }

    private func fulfillKeychain(_ op: KeychainOp) {
        let result: KeychainResult
        switch op {
        case .loadSession:
            // Prefer nsec; fall back to bunker URI — same priority as the
            // live lane's `AppSessionStore.restoreSession(into:)`.
            if let nsec = KeychainService.loadNsec() {
                result = .sessionSecret(nsec)
            } else if let uri = KeychainService.loadBunkerURI() {
                result = .sessionSecret(uri)
            } else {
                result = .sessionSecret(nil)
            }
        case .clearSession:
            KeychainService.deleteNsec()
            KeychainService.deleteBunkerURI()
            result = .cleared
        }
        app.provideCapabilityResult(result: .keychain(result))
    }
}

// MARK: - Observer implementation

/// Inner observer registered with Rust. Callbacks arrive on Rust's tokio
/// task thread; this class hops each call to the main actor before touching
/// `HighlighterAppKernel`.
///
/// `@unchecked Sendable` is safe here: the only mutable state is the
/// `weak var kernel`, and Swift's runtime guarantees that `weak` loads are
/// atomic — so cross-thread reads are well-defined.
private final class KernelObserver: HighlighterObserver, @unchecked Sendable {
    private weak var kernel: HighlighterAppKernel?

    init(kernel: HighlighterAppKernel) {
        self.kernel = kernel
    }

    func onSnapshot(viewId: ViewId, snapshot: ViewSnapshot) {
        Task { @MainActor [weak self] in
            self?.kernel?.receive(viewId: viewId, snapshot: snapshot)
        }
    }

    func onCapabilityRequest(request: CapabilityRequest) {
        Task { @MainActor [weak self] in
            self?.kernel?.fulfill(request: request)
        }
    }
}
