import SwiftUI

/// Minimal login surface. Leans on iOS 26 Liquid Glass for chrome — almost no
/// custom styling, no heavy fills. Mirrors Olas's flow:
///   1. Detect known signer apps (Primal first).
///   2. If Primal present, surface a hero action: Scan / Paste / Show QR.
///   3. Always allow nsec paste + manual bunker URI paste as fallback.
struct LoginView: View {
    @Environment(HighlighterStore.self) private var store
    /// Phase 7 Part C: sign-in dispatches kernel auth actions; the kernel owns
    /// session/routing and surfaces failures via `appRoot.authError`. nsec/bunker
    /// also restore the live lane's `currentUser` via `store.bootstrap()` reading
    /// the same Keychain credential.
    @Environment(HighlighterAppKernel.self) private var kernel
    @Environment(\.openURL) private var openURL

    @State private var detectedSigner: KnownSigner?
    @State private var inputText: String = ""
    @State private var isWorking: Bool = false
    @State private var errorMessage: String?

    /// The inline error to show: the live-lane sign-in error takes precedence;
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
            .onChange(of: kernel.appRoot.nostrconnectUri) { _, uri in
                isWorking = false
                guard let uri, let url = URL(string: uri) else { return }
                openURL(url)
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
                Task { await submitManualInput() }
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

    /// Pure input classification (D1): trims an optional `nostr:` prefix and
    /// routes by bech32/URI prefix. Mirrors the kernel's parsing so no FFI
    /// round-trip is needed just to enable the button.
    private enum InputKind { case empty, nsec(String), bunker(String), invalid(String) }

    private func classifyInput(_ input: String) -> InputKind {
        let trimmed = input.trimmingCharacters(in: .whitespaces)
        let value = trimmed.hasPrefix("nostr:") ? String(trimmed.dropFirst(6)) : trimmed
        if value.isEmpty { return .empty }
        if value.hasPrefix("nsec1") { return .nsec(value) }
        if value.hasPrefix("bunker://") || value.hasPrefix("nostrconnect://") { return .bunker(value) }
        return .invalid("Enter an nsec1… or bunker:// URI.")
    }

    private var isManualInputEmpty: Bool {
        if case .empty = classifyInput(inputText) { return true }
        return false
    }

    private func submitManualInput() async {
        switch classifyInput(inputText) {
        case .empty:
            return
        case .nsec(let nsec):
            errorMessage = nil
            isWorking = true
            // Persist first so the live lane can restore `currentUser` from the
            // same Keychain entry; the kernel auths from the dispatched payload.
            _ = KeychainService.saveNsec(nsec)
            kernel.app.dispatch(.signInNsec(nsec: nsec))
            await store.bootstrap()
            isWorking = false
            if store.currentUser == nil {
                errorMessage = kernel.appRoot.authError ?? "Could not sign in with that key."
            }
        case .bunker(let uri):
            errorMessage = nil
            isWorking = true
            _ = KeychainService.saveBunkerURI(uri)
            kernel.app.dispatch(.pairBunker(uri: uri))
            await store.bootstrap()
            isWorking = false
            if store.currentUser == nil {
                errorMessage = kernel.appRoot.authError ?? "Could not pair with that bunker."
            }
        case .invalid(let message):
            errorMessage = message
        }
    }

    private func connectViaPrimalApp() {
        errorMessage = nil
        isWorking = true
        // The kernel mints the nostrconnect:// URI; it arrives on
        // `kernel.appRoot.nostrconnectUri` and is opened by the `.onChange`
        // above. Pairing completes on the kernel's relay subscription, which
        // flips `appRoot.sessionPresent` → `RootSceneView` routes to the shell.
        kernel.app.dispatch(.startNostrConnect)
    }
}
