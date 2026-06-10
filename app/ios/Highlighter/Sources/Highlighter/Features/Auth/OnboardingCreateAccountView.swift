import SwiftUI

private enum UsernameState: Equatable {
    case idle
    case checking
    case available(identifier: String, domain: String)
    case taken
    case invalid
}

struct OnboardingCreateAccountView: View {
    @Environment(HighlighterStore.self) private var store

    @State private var displayName: String = ""
    @State private var username: String = ""
    @State private var usernameState: UsernameState = .idle
    @State private var isWorking = false
    @State private var errorMessage: String?
    @State private var createdAccount: GeneratedAccount?
    @State private var navigateToInterests = false
    @State private var usernameCheckTask: Task<Void, Never>?

    @FocusState private var focusedField: Field?

    private enum Field { case displayName, username }

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
                    TextField("Display name", text: $displayName)
                        .font(.title3)
                        .textInputAutocapitalization(.words)
                        .autocorrectionDisabled()
                        .padding(.horizontal, 20)
                        .padding(.vertical, 16)
                        .background(.thinMaterial, in: .rect(cornerRadius: 16))
                        .padding(.horizontal, 32)
                        .focused($focusedField, equals: .displayName)
                        .onSubmit { focusedField = .username }
                        .onChange(of: displayName) { _, new in
                            if username.isEmpty {
                                let suggested = store.safeCore.suggestNip05Username(displayName: new)
                                if !suggested.isEmpty {
                                    username = suggested
                                    scheduleCheck(for: suggested)
                                }
                            }
                        }

                    usernameField
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

    private var usernameField: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 0) {
                TextField("username", text: $username)
                    .font(.title3)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .keyboardType(.asciiCapable)
                    .focused($focusedField, equals: .username)
                    .onChange(of: username) { _, new in
                        let normalized = store.safeCore.normalizeNip05Username(new)
                        if normalized != new { username = normalized }
                        scheduleCheck(for: normalized)
                    }
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
                .animation(.easeInOut(duration: 0.15), value: usernameState)
        }
    }

    @ViewBuilder
    private var usernameTrailingIndicator: some View {
        switch usernameState {
        case .checking:
            ProgressView().scaleEffect(0.7)
        case .available:
            Image(systemName: "checkmark.circle.fill")
                .foregroundStyle(.green)
        case .taken:
            Image(systemName: "xmark.circle.fill")
                .foregroundStyle(.red)
        case .invalid:
            Image(systemName: "exclamationmark.circle.fill")
                .foregroundStyle(.orange)
        case .idle:
            EmptyView()
        }
    }

    @ViewBuilder
    private var usernameCaption: some View {
        switch usernameState {
        case .available(let identifier, _):
            Text("\(identifier)")
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

    private var createProjection: OnboardingCreateAccountProjection {
        store.safeCore.projectOnboardingCreateAccount(input: OnboardingCreateAccountProjectionInput(
            displayName: displayName,
            username: username,
            usernameAvailable: usernameAvailable,
            isWorking: isWorking
        ))
    }

    private var usernameAvailable: Bool {
        if case .available = usernameState { return true }
        return false
    }

    private func scheduleCheck(for name: String) {
        usernameCheckTask?.cancel()
        usernameState = .idle
        let projection = store.safeCore.projectOnboardingUsernameCheck(username: name)
        guard projection.hasUsername else { return }

        guard projection.valid else {
            usernameState = .invalid
            return
        }

        usernameState = .checking
        let checkName = projection.username

        usernameCheckTask = Task {
            guard !Task.isCancelled else { return }
            let current = store.safeCore.projectOnboardingUsernameCheck(username: username)
            guard current.username == checkName else { return }
            await checkAvailability(name: checkName)
        }
    }

    private func checkAvailability(name: String) async {
        let snapshot = await store.safeCore.checkNip05Availability(name: name)
        let current = store.safeCore.projectOnboardingUsernameCheck(username: username)
        guard current.username == name else { return }
        switch snapshot.state {
        case .idle:
            usernameState = .idle
        case .invalid:
            usernameState = .invalid
        case .available:
            usernameState = .available(identifier: snapshot.identifier, domain: snapshot.domain)
        case .taken:
            usernameState = .taken
        }
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
            AppSessionStore.shared.persistAccountInstructions(accountSnapshot)

            let claimedUsername: String
            if case .available(let identifier, let domain) = usernameState,
               !projection.username.isEmpty {
                let registerSnapshot = await store.safeCore.registerNip05(
                    name: projection.username,
                    domain: domain
                )
                guard registerSnapshot.succeeded else {
                    errorMessage = registerSnapshot.errorMessage
                    return
                }
                claimedUsername = registerSnapshot.identifier.isEmpty ? identifier : registerSnapshot.identifier
            } else {
                claimedUsername = ""
            }

            Task {
                _ = await store.safeCore.updateProfile(
                    name: "",
                    displayName: name,
                    about: "",
                    picture: "",
                    banner: "",
                    nip05: claimedUsername,
                    website: "",
                    lud16: ""
                )
            }

            createdAccount = account
            navigateToInterests = true
        }
    }
}
