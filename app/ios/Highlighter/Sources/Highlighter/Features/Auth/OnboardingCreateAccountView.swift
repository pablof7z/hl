import SwiftUI

struct OnboardingCreateAccountView: View {
    @Environment(HighlighterStore.self) private var store

    @State private var displayName: String = ""
    @State private var isWorking = false
    @State private var errorMessage: String?
    @State private var createdAccount: GeneratedAccount?
    @State private var navigateToInterests = false

    @FocusState private var focusedField: Field?

    private enum Field { case displayName }

    var body: some View {
        ZStack {
            Color.highlighterPaper.ignoresSafeArea()

            VStack(alignment: .leading, spacing: 0) {
                Spacer()

                VStack(alignment: .leading, spacing: 8) {
                    Text("What should we call you?")
                        .font(.system(.title, design: .default).weight(.semibold))
                        .foregroundStyle(Color.highlighterInkStrong)

                    Text("Your display name is visible to everyone. You can edit your profile later.")
                        .font(.callout)
                        .foregroundStyle(Color.highlighterInkMuted)
                        .lineSpacing(2)
                }
                .padding(.horizontal, 32)
                .padding(.bottom, 32)

                VStack(spacing: 12) {
                    TextField("Display name", text: $displayName)
                        .font(.title3)
                        .textInputAutocapitalization(.words)
                        .autocorrectionDisabled()
                        .padding(.horizontal, 20)
                        .padding(.vertical, 16)
                        .background(.thinMaterial, in: .rect(cornerRadius: 16))
                        .padding(.horizontal, 32)
                        .focused($focusedField, equals: .displayName)
                        .onSubmit { createAccount() }
                }

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
                    .disabled(!createProjection.canContinue)
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
        .onAppear { focusedField = .displayName }
    }

    private var createProjection: OnboardingCreateAccountProjection {
        store.safeCore.projectOnboardingCreateAccount(input: OnboardingCreateAccountProjectionInput(
            displayName: displayName,
            username: "",
            usernameAvailable: false,
            isWorking: isWorking
        ))
    }

    private func createAccount() {
        let projection = createProjection
        let name = projection.displayName
        guard projection.canContinue else { return }

        isWorking = true
        errorMessage = nil

        Task {
            defer { isWorking = false }
            let accountSnapshot = await store.safeCore.generateAccount()
            guard accountSnapshot.succeeded, let account = accountSnapshot.account else {
                errorMessage = accountSnapshot.errorMessage
                return
            }
            let storage = AppSessionStore.shared.persistAccountInstructions(
                accountSnapshot,
                core: store.safeCore
            )
            guard storage.succeeded else {
                errorMessage = storage.errorMessage
                return
            }

            Task {
                _ = await store.safeCore.updateProfile(
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
        }
    }
}
