import SwiftUI

/// New-user account creation: display name only, invisible key generation.
struct OnboardingCreateAccountView: View {
    @Environment(HighlighterStore.self) private var store

    @State private var displayName: String = ""
    @State private var isWorking = false
    @State private var errorMessage: String?
    @State private var createdAccount: GeneratedAccount?
    @State private var navigateToInterests = false

    @FocusState private var nameFocused: Bool

    var body: some View {
        ZStack {
            Color.highlighterPaper.ignoresSafeArea()

            VStack(alignment: .leading, spacing: 0) {
                Spacer()

                VStack(alignment: .leading, spacing: 8) {
                    Text("What should we call you?")
                        .font(.system(.title, design: .serif).weight(.semibold))
                        .foregroundStyle(Color.highlighterInkStrong)

                    Text("This is your display name — you can change it any time.")
                        .font(.callout)
                        .foregroundStyle(Color.highlighterInkMuted)
                        .lineSpacing(2)
                }
                .padding(.horizontal, 32)
                .padding(.bottom, 32)

                TextField("Display name", text: $displayName)
                    .font(.title3)
                    .textInputAutocapitalization(.words)
                    .autocorrectionDisabled()
                    .padding(.horizontal, 20)
                    .padding(.vertical, 16)
                    .background(.thinMaterial, in: .rect(cornerRadius: 16))
                    .padding(.horizontal, 32)
                    .focused($nameFocused)
                    .onSubmit { createAccount() }

                if let msg = errorMessage {
                    Text(msg)
                        .font(.footnote)
                        .foregroundStyle(.red)
                        .padding(.horizontal, 32)
                        .padding(.top, 8)
                }

                Spacer()

                VStack(spacing: 12) {
                    Button(action: createAccount) {
                        Group {
                            if isWorking {
                                ProgressView().tint(.white)
                            } else {
                                Text("Continue")
                                    .font(.headline)
                            }
                        }
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 14)
                    }
                    .buttonStyle(.glassProminent)
                    .disabled(isWorking || displayName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                    .padding(.horizontal, 32)

                    NavigationLink {
                        LoginView()
                    } label: {
                        Text("I already have an account")
                            .font(.footnote)
                            .foregroundStyle(Color.highlighterInkMuted)
                    }
                }
                .padding(.bottom, 48)
            }
        }
        .navigationDestination(isPresented: $navigateToInterests) {
            if let account = createdAccount {
                OnboardingInterestsView(account: account)
            }
        }
        .onAppear { nameFocused = true }
    }

    private func createAccount() {
        let name = displayName.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !name.isEmpty, !isWorking else { return }

        isWorking = true
        errorMessage = nil

        Task {
            defer { isWorking = false }
            do {
                let account = try await store.safeCore.generateAccount()
                AppSessionStore.shared.persistNsec(account.nsec)
                Task {
                    try? await store.safeCore.updateProfile(
                        name: "",
                        displayName: name,
                        about: "",
                        picture: "",
                        banner: "",
                        nip05: "",
                        website: "",
                        lud16: ""
                    )
                }
                createdAccount = account
                navigateToInterests = true
            } catch {
                errorMessage = error.localizedDescription
            }
        }
    }
}
