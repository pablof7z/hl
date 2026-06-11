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

    private var isBusy: Bool { isWorking || store.isAuthenticating }

    private var displayedError: String? {
        if let errorMessage {
            return errorMessage
        }
        if let toast = store.nmpState.toast, toast.kind == .error {
            return toast.message
        }
        return nil
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
            .onAppear {
                store.setExternalUrlHandler { urlString, completion in
                    guard let url = URL(string: urlString) else {
                        completion(false)
                        return
                    }
                    openURL(url, completion: completion)
                }
            }
            .onDisappear {
                store.clearExternalUrlHandler()
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
            .disabled(isBusy)
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
        .disabled(isBusy)
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
                Text(isBusy ? "Signing in…" : "Sign in")
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 10)
            }
            .buttonStyle(.glassProminent)
            .disabled(isBusy || inputText.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)

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

    private func submitManualInput() async {
        let trimmed = inputText.trimmingCharacters(in: .whitespacesAndNewlines)
        let normalized = trimmed.hasPrefix("nostr:") ? String(trimmed.dropFirst(6)) : trimmed
        guard !normalized.isEmpty else { return }

        isWorking = true
        errorMessage = nil
        defer { isWorking = false }

        if normalized.hasPrefix("nsec1") {
            store.signInNsec(normalized)
        } else if normalized.hasPrefix("bunker://") || normalized.hasPrefix("nostrconnect://") {
            store.pairBunker(normalized)
        } else {
            errorMessage = "Enter an nsec1… or bunker:// URI."
        }
    }

    private func connectViaPrimalApp() async {
        errorMessage = nil
        store.startNostrConnect(callbackUrl: "highlighter://nip46")
    }
}
