import SwiftUI

/// Minimal login surface. Leans on iOS 26 Liquid Glass for chrome — almost no
/// custom styling, no heavy fills. Mirrors Olas's flow:
///   1. Detect known signer apps (Primal first).
///   2. If Primal present, surface a hero action: Scan / Paste / Show QR.
///   3. Always allow nsec paste + manual bunker URI paste as fallback.
struct LoginView: View {
    @Environment(HighlighterStore.self) private var store
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
            .onChange(of: kernel.appRoot) { _, appRoot in
                guard isWorking else { return }
                if appRoot.sessionPresent {
                    isWorking = false
                } else if let error = appRoot.authError {
                    errorMessage = error
                    isWorking = false
                }
                if let uri = appRoot.nostrconnectUri, let url = URL(string: uri) {
                    openURL(url)
                    isWorking = false
                }
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
            connectViaPrimalApp()
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

    private var isManualInputEmpty: Bool {
        inputText.trimmingCharacters(in: .whitespaces).isEmpty
    }

    private func submitManualInput() {
        let trimmed = inputText.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty else { return }

        isWorking = true
        errorMessage = nil

        if trimmed.hasPrefix("nsec1") {
            _ = KeychainService.saveNsec(trimmed)
            kernel.app.dispatch(.signInNsec(nsec: trimmed))
        } else if trimmed.hasPrefix("bunker://") || trimmed.hasPrefix("nostrconnect://") {
            _ = KeychainService.saveBunkerURI(trimmed)
            kernel.app.dispatch(.pairBunker(uri: trimmed))
        } else {
            errorMessage = "Unrecognized input — paste an nsec1…, bunker://, or nostrconnect:// URI"
            isWorking = false
        }
    }

    private func connectViaPrimalApp() {
        isWorking = true
        errorMessage = nil
        kernel.app.dispatch(.startNostrConnect)
        // The NostrConnect URI arrives asynchronously via kernel.appRoot.nostrconnectUri;
        // the onChange above opens it and clears isWorking.
    }
}
