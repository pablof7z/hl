import SwiftUI

struct OnboardingCreateAccountView: View {
    @Environment(HighlighterStore.self) private var store

    @State private var navigateToInterests = false
    @FocusState private var focusedField: Field?

    private enum Field { case displayName }

    private var account: HighlighterCreateAccountSnapshot {
        store.nmpState.createAccount
    }

    var body: some View {
        ZStack {
            Color.highlighterPaper.ignoresSafeArea()

            VStack(alignment: .leading, spacing: 0) {
                Spacer()

                VStack(alignment: .leading, spacing: 8) {
                    Text("What should we call you?")
                        .font(.system(.title, design: .default).weight(.semibold))
                        .foregroundStyle(Color.highlighterInkStrong)

                    Text("Display name is visible to everyone. You can change it later in Settings.")
                        .font(.callout)
                        .foregroundStyle(Color.highlighterInkMuted)
                        .lineSpacing(2)
                }
                .padding(.horizontal, 32)
                .padding(.bottom, 32)

                VStack(spacing: 12) {
                    TextField("Display name", text: displayNameBinding)
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

                if let message = account.errorMessage {
                    Text(message)
                        .font(.footnote)
                        .foregroundStyle(.red)
                        .padding(.horizontal, 32)
                        .padding(.top, 8)
                }

                Spacer()

                VStack(spacing: 12) {
                    Button(action: createAccount) {
                        Group {
                            if account.isCreating {
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
                    .disabled(!account.canSubmit || account.isCreating)
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
        .onChange(of: account.createdUser?.pubkey) { _, pubkey in
            if pubkey != nil {
                navigateToInterests = true
            }
        }
    }

    private var displayNameBinding: Binding<String> {
        Binding(
            get: { account.displayName },
            set: { store.setCreateAccountDisplayName($0) }
        )
    }

    private func createAccount() {
        guard account.canSubmit, !account.isCreating else { return }
        store.submitCreateAccount()
    }
}
