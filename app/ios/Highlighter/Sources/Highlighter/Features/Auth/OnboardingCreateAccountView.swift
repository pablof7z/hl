import SwiftUI

struct OnboardingCreateAccountView: View {
    @Environment(HighlighterStore.self) private var store

    @State private var navigateToInterests = false
    @FocusState private var focusedField: Field?

    private enum Field { case displayName, username }

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

                    Text("Display name is visible to everyone. Username lets others find you on Nostr.")
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
                        .onSubmit { focusedField = .username }

                    usernameField
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
                    .disabled(!account.canSubmit || account.isCreating || account.usernameStatus == .checking)
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

    private var usernameBinding: Binding<String> {
        Binding(
            get: { account.username },
            set: { store.setCreateAccountUsername($0) }
        )
    }

    private var usernameField: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 0) {
                TextField("username", text: usernameBinding)
                    .font(.title3)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .keyboardType(.asciiCapable)
                    .focused($focusedField, equals: .username)
                    .onSubmit { createAccount() }

                usernameTrailingIndicator
                    .frame(width: 28)
            }
            .padding(.horizontal, 20)
            .padding(.vertical, 16)
            .background(.thinMaterial, in: .rect(cornerRadius: 16))
            .padding(.horizontal, 32)

            usernameCaption
                .padding(.horizontal, 36)
                .animation(.easeInOut(duration: 0.15), value: account.usernameStatus)
        }
    }

    @ViewBuilder
    private var usernameTrailingIndicator: some View {
        switch account.usernameStatus {
        case .checking:
            ProgressView().scaleEffect(0.7)
        case .available:
            Image(systemName: "checkmark.circle.fill")
                .foregroundStyle(.green)
        case .taken:
            Image(systemName: "xmark.circle.fill")
                .foregroundStyle(.red)
        case .invalid, .error:
            Image(systemName: "exclamationmark.circle.fill")
                .foregroundStyle(.orange)
        case .idle:
            EmptyView()
        }
    }

    @ViewBuilder
    private var usernameCaption: some View {
        switch account.usernameStatus {
        case .available:
            Text(account.usernameIdentifier)
                .font(.caption)
                .foregroundStyle(.green)
        case .taken:
            Text("Already taken")
                .font(.caption)
                .foregroundStyle(.red)
        case .invalid:
            Text("Only letters, numbers, - and _")
                .font(.caption)
                .foregroundStyle(.orange)
        default:
            EmptyView()
        }
    }

    private func createAccount() {
        guard account.canSubmit, !account.isCreating else { return }
        store.submitCreateAccount()
    }
}
