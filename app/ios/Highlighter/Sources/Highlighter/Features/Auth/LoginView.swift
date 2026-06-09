import SwiftUI

/// Minimal login surface. Leans on iOS 26 Liquid Glass for chrome — almost no
/// custom styling, no heavy fills. Mirrors Olas's flow:
///   1. Detect known signer apps (Primal first).
///   2. If Primal present, surface a hero action: Scan / Paste / Show QR.
///   3. Always allow nsec paste + manual bunker URI paste as fallback.
struct LoginView: View {
    @Environment(HighlighterStore.self) private var store
    @Environment(\.openURL) private var openURL

    @State private var detectedSigner: KnownSigner?
    @State private var inputText: String = ""
    @State private var isWorking: Bool = false
    @State private var errorMessage: String?

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

                    if let errorMessage {
                        Text(errorMessage)
                            .font(.footnote)
                            .foregroundStyle(.red)
                    }
                }
                .padding(24)
            }
            .task {
                detectedSigner = KnownSigner.detect()
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
                Task { await connectViaPrimalApp() }
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
            Task { await connectViaPrimalApp() }  // same flow, different scheme
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

    private var isManualInputEmpty: Bool {
        if case .empty = store.safeCore.classifyLoginInput(inputText) {
            return true
        }
        return false
    }

    private func submitManualInput() async {
        let action = store.safeCore.classifyLoginInput(inputText)

        isWorking = true
        errorMessage = nil
        defer { isWorking = false }

        switch action {
        case .empty:
            return
        case .nsec(let nsec):
            let snapshot = await store.safeCore.loginNsec(nsec)
            if snapshot.isAuthenticated, let user = snapshot.user {
                AppSessionStore.shared.persistNsec(nsec)
                await store.completeLogin(user: user)
            } else {
                errorMessage = snapshot.errorMessage
            }
        case .bunker(let uri):
            let snapshot = await store.safeCore.pairBunker(uri)
            if snapshot.isAuthenticated, let user = snapshot.user {
                AppSessionStore.shared.persistBunkerURI(uri)
                await store.completeLogin(user: user)
            } else {
                errorMessage = snapshot.errorMessage
            }
        case .invalid(let message):
            errorMessage = message
        }
    }

    private func connectViaPrimalApp() async {
        isWorking = true
        errorMessage = nil
        defer { isWorking = false }

        let outcome = await store.safeCore.startDefaultNostrConnect(callback: "highlighter://nip46")
        guard outcome.error.isEmpty else {
            errorMessage = outcome.error
            return
        }
        let uri = outcome.value
        guard !uri.isEmpty else {
            errorMessage = "Could not start Nostr Connect."
            return
        }

        if let url = URL(string: uri) {
            openURL(url)
        }
        // `EventBridge` receives `.signerConnected(user)` once the remote
        // signer responds on the relay and `completeLogin` runs from there.
    }
}
