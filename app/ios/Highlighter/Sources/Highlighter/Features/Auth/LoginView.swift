import SwiftUI

/// Minimal login surface. Leans on iOS 26 Liquid Glass for chrome — almost no
/// custom styling, no heavy fills. Mirrors Olas's flow:
///   1. Detect known signer apps (Primal first).
///   2. If Primal present, surface a hero action: Scan / Paste / Show QR.
///   3. Always allow nsec paste + manual bunker URI paste as fallback.
struct LoginView: View {
    @Environment(HighlighterStore.self) private var store
    /// Phase 7: sign-in is now kernel-dispatched. Auth failures arrive via
    /// `appRoot.authError`; success transitions `appRoot.routeKind` to
    /// `.rootShell`, at which point `RootSceneView` unmounts this view.
    @Environment(HighlighterAppKernel.self) private var kernel
    @Environment(\.openURL) private var openURL

    @State private var detectedSigner: KnownSigner?
    @State private var inputText: String = ""
    @State private var isWorking: Bool = false
    @State private var errorMessage: String?

    /// The inline error to show: a local classification error takes precedence;
    /// otherwise surface a kernel-side restore/sign-in failure.
    private var displayedError: String? {
        errorMessage ?? kernel.appRoot.authError
    }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 32) {
                    header

                    if let detected = detectedSigner, detected == .primal {
                        primalHero
                    } else if let detected = detectedSigner {
                        genericSignerButton(detected)
                    }

                    manualEntry

                    if let displayedError {
                        Text(displayedError)
                            .font(.footnote)
                            .foregroundStyle(.red)
                    }
                }
                .padding(24)
            }
            .task {
                detectedSigner = KnownSigner.detect()
            }
            .onChange(of: kernel.appRoot.authError) { _, error in
                // Auth failed — re-enable the form so the user can retry.
                if error != nil { isWorking = false }
            }
            .onChange(of: kernel.appRoot.nostrconnectUri) { _, uri in
                // Kernel minted a nostrconnect:// URI — open the signer app.
                guard let uri, let url = URL(string: uri) else { return }
                openURL(url)
                isWorking = false
            }
            .navigationTitle("")
        }
    }

    // MARK: - Subviews

    private var header: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text("Highlighter")
                .font(.largeTitle.weight(.medium))
            Text("Sign in with your Nostr identity")
                .font(.subheadline)
                .foregroundStyle(.secondary)
        }
    }

    private var primalHero: some View {
        VStack(spacing: 12) {
            Button {
                connectViaPrimalApp()
            } label: {
                HStack(spacing: 12) {
                    Image(systemName: "bolt.fill")
                        .font(.title2)
                    VStack(alignment: .leading, spacing: 2) {
                        Text("Continue with Primal")
                            .font(.headline)
                        Text("Opens your Primal app to approve.")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                    Spacer()
                    Image(systemName: "arrow.up.forward")
                        .font(.subheadline)
                        .foregroundStyle(.secondary)
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 14)
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .buttonStyle(.glass)
            .disabled(isWorking)
        }
    }

    private func genericSignerButton(_ signer: KnownSigner) -> some View {
        Button {
            connectViaPrimalApp()  // same flow, different scheme
        } label: {
            HStack {
                Text("Continue with \(signer.name)")
                Spacer()
                Image(systemName: "arrow.up.forward")
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 14)
            .frame(maxWidth: .infinity)
        }
        .buttonStyle(.glass)
        .disabled(isWorking)
    }

    private var manualEntry: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Or paste a key or bunker URI")
                .font(.caption)
                .foregroundStyle(.secondary)

            TextField("nsec1… or bunker://… or nostrconnect://…", text: $inputText, axis: .vertical)
                .lineLimit(1...3)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .padding(.horizontal, 14)
                .padding(.vertical, 12)
                .background(.thinMaterial, in: .rect(cornerRadius: 14))

            Button {
                submitManualInput()
            } label: {
                Text(isWorking ? "Signing in…" : "Sign in")
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 10)
            }
            .buttonStyle(.glassProminent)
            .disabled(isWorking || isManualInputEmpty)

            NavigationLink {
                OnboardingCreateAccountView()
            } label: {
                Text("Create a new account")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, alignment: .center)
                    .padding(.top, 4)
            }
        }
    }

    // MARK: - Actions

    private func classifyInput(_ raw: String) -> LoginInputAction {
        let trimmed = raw.trimmingCharacters(in: .whitespaces)
        let s = trimmed.hasPrefix("nostr:") ? String(trimmed.dropFirst(6)) : trimmed
        if s.isEmpty { return .empty }
        if s.hasPrefix("nsec1") { return .nsec(nsec: s) }
        if s.hasPrefix("bunker://") || s.hasPrefix("nostrconnect://") { return .bunker(uri: s) }
        return .invalid(message: "Enter an nsec1… or bunker:// URI.")
    }

    private var isManualInputEmpty: Bool {
        if case .empty = classifyInput(inputText) { return true }
        return false
    }

    private func submitManualInput() {
        let action = classifyInput(inputText)
        errorMessage = nil

        switch action {
        case .empty:
            return
        case .nsec(let nsec):
            isWorking = true
            store.kernel?.app.dispatch(.signInNsec(nsec: nsec))
            // NMP adds the signer and auto-persists to its own keyring.
            // Result arrives via kernel.appRoot: routeKind → .rootShell on
            // success, authError set on failure (isWorking reset via .onChange).
        case .bunker(let uri):
            isWorking = true
            store.kernel?.app.dispatch(.pairBunker(uri: uri))
            // NIP-46 broker handles the handshake async; result arrives via
            // kernel.appRoot (same pattern as nsec above).
        case .invalid(let message):
            errorMessage = message
        }
    }

    private func connectViaPrimalApp() {
        errorMessage = nil
        isWorking = true
        store.kernel?.app.dispatch(.startNostrConnect)
        // Kernel mints a nostrconnect:// URI → sets appRoot.nostrconnectUri.
        // .onChange above opens the URL in the signer app and resets isWorking.
        // The signer responds on the relay; IdentityChanged fires → routeKind
        // transitions to .rootShell and RootSceneView unmounts this view.
    }
}
