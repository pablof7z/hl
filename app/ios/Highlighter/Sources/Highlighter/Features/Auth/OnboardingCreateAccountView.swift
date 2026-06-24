import SwiftUI

struct OnboardingCreateAccountView: View {
    @Environment(HighlighterAppKernel.self) private var kernel

    @State private var displayName: String = ""
    @State private var isWorking = false
    @State private var errorMessage: String?
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
                    .disabled(!canContinue)
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
            OnboardingInterestsView()
        }
        .onAppear { focusedField = .displayName }
    }

    private var trimmedDisplayName: String {
        displayName.trimmingCharacters(in: .whitespaces)
    }

    private var canContinue: Bool {
        !isWorking && !trimmedDisplayName.isEmpty
    }

    private func createAccount() {
        let name = trimmedDisplayName
        guard canContinue else { return }

        errorMessage = nil
        // The kernel generates keys inside NMP's keyring and publishes the
        // initial kind:0 with `profileName`. Success flips `appRoot.sessionPresent`;
        // navigate optimistically into the interests step (a local push) while the
        // kernel finishes — `finish()` there completes onboarding.
        kernel.app.dispatch(.createAccount(profileName: name))
        navigateToInterests = true
    }
}
