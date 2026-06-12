import Kingfisher
import PhotosUI
import SwiftUI

/// Edit-profile flow for the current user. Rust owns field values, upload
/// state, save state, errors, and the saved profile projection.
struct EditProfileSheet: View {
    @Environment(\.dismiss) private var dismiss
    @Environment(HighlighterStore.self) private var appStore

    let initial: ProfileMetadata?
    let onSaved: (ProfileMetadata) -> Void

    @State private var pictureItem: PhotosPickerItem?
    @State private var bannerItem: PhotosPickerItem?

    private var draft: HighlighterEditProfileSnapshot {
        appStore.editProfile
    }

    private var isDirty: Bool {
        draft.displayName != (initial?.displayName ?? "")
            || draft.name != (initial?.name ?? "")
            || draft.about != (initial?.about ?? "")
            || draft.picture != (initial?.picture ?? "")
            || draft.banner != (initial?.banner ?? "")
            || draft.nip05 != (initial?.nip05 ?? "")
            || draft.website != (initial?.website ?? "")
            || draft.lud16 != (initial?.lud16 ?? "")
    }

    var body: some View {
        NavigationStack {
            ZStack(alignment: .bottom) {
                ScrollView {
                    VStack(alignment: .leading, spacing: 24) {
                        bannerPlate
                        avatarPlate
                            .padding(.horizontal, 22)
                            .padding(.top, -52)
                        identityFields
                            .padding(.horizontal, 22)
                        Divider().overlay(Color.highlighterRule)
                            .padding(.horizontal, 22)
                        contactFields
                            .padding(.horizontal, 22)
                        Spacer(minLength: 120)
                    }
                }
                .scrollDismissesKeyboard(.interactively)

                stickyCTA
            }
            .background(Color.highlighterPaper.ignoresSafeArea())
            .navigationTitle("Edit profile")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    Button("Cancel") { dismiss() }
                        .foregroundStyle(Color.highlighterInkStrong)
                }
            }
            .alert("Couldn't save", isPresented: errorBinding) {
                Button("OK") { appStore.clearEditProfileError() }
            } message: {
                if let error = draft.errorMessage { Text(error) }
            }
            .onAppear {
                appStore.openEditProfile(seed: initial)
            }
            .onDisappear {
                appStore.closeEditProfile()
            }
            .onChange(of: pictureItem) { _, item in
                guard let item else { return }
                Task { await upload(item: item, target: .picture) }
            }
            .onChange(of: bannerItem) { _, item in
                guard let item else { return }
                Task { await upload(item: item, target: .banner) }
            }
            .onChange(of: draft.savedProfile) { _, profile in
                guard let profile else { return }
                UINotificationFeedbackGenerator().notificationOccurred(.success)
                onSaved(profile)
                appStore.clearEditProfileResult()
                dismiss()
            }
        }
    }

    private var bannerPlate: some View {
        let bannerActionTitle = draft.banner.isEmpty ? "Add banner" : "Replace"

        return ZStack {
            if let url = URL(string: draft.banner), !draft.banner.isEmpty {
                KFImage(url)
                    .resizable()
                    .scaledToFill()
                    .frame(maxWidth: .infinity)
                    .frame(height: 160)
                    .clipped()
            } else {
                LinearGradient(
                    colors: [
                        Color.highlighterAccent.opacity(0.55),
                        Color.highlighterAccent.opacity(0.18),
                    ],
                    startPoint: .topLeading,
                    endPoint: .bottomTrailing
                )
                .frame(height: 160)
            }
        }
        .overlay(alignment: .topTrailing) {
            if !draft.banner.isEmpty {
                clearChip {
                    appStore.setEditProfileBanner("")
                    bannerItem = nil
                }
                .padding(12)
            }
        }
        .overlay(alignment: .bottomTrailing) {
            HStack(spacing: 6) {
                if draft.isBannerUploading {
                    ProgressView().controlSize(.small).tint(.white)
                }
                PhotosPicker(selection: $bannerItem, matching: .images) {
                    Label(bannerActionTitle, systemImage: "photo")
                        .font(.caption.weight(.semibold))
                        .foregroundStyle(.white)
                        .padding(.horizontal, 12)
                        .padding(.vertical, 7)
                        .background(.black.opacity(0.55), in: Capsule())
                }
            }
            .padding(12)
        }
    }

    private var avatarPlate: some View {
        let pictureActionTitle = draft.picture.isEmpty ? "Add photo" : "Replace photo"

        return HStack(spacing: 14) {
            ZStack {
                if let url = URL(string: draft.picture), !draft.picture.isEmpty {
                    KFImage(url)
                        .resizable()
                        .scaledToFill()
                } else {
                    LinearGradient(
                        colors: [
                            Color.highlighterTintPale,
                            Color.highlighterAccent.opacity(0.4),
                        ],
                        startPoint: .top,
                        endPoint: .bottom
                    )
                    .overlay {
                        Image(systemName: "person.fill")
                            .font(.system(size: 36))
                            .foregroundStyle(Color.highlighterInkMuted)
                    }
                }
            }
            .frame(width: 96, height: 96)
            .clipShape(Circle())
            .overlay(Circle().stroke(Color.highlighterPaper, lineWidth: 4))
            .overlay {
                if draft.isPictureUploading {
                    Circle().fill(.black.opacity(0.4))
                    ProgressView().controlSize(.regular).tint(.white)
                }
            }

            VStack(alignment: .leading, spacing: 8) {
                PhotosPicker(selection: $pictureItem, matching: .images) {
                    Label(pictureActionTitle, systemImage: "camera")
                        .font(.subheadline.weight(.medium))
                        .foregroundStyle(Color.highlighterInkStrong)
                        .padding(.horizontal, 12)
                        .padding(.vertical, 8)
                        .background(
                            Capsule().fill(Color.highlighterTintPale)
                        )
                }
                if !draft.picture.isEmpty {
                    Button {
                        appStore.setEditProfilePicture("")
                        pictureItem = nil
                    } label: {
                        Text("Remove")
                            .font(.subheadline)
                            .foregroundStyle(Color.highlighterInkMuted)
                    }
                }
            }
            Spacer(minLength: 0)
        }
    }

    private var identityFields: some View {
        VStack(alignment: .leading, spacing: 18) {
            field(
                label: "Display name",
                placeholder: "How you want to be addressed",
                text: Binding(
                    get: { draft.displayName },
                    set: { appStore.setEditProfileDisplayName($0) }
                )
            )
            field(
                label: "Username",
                placeholder: "lowercase, no spaces",
                text: Binding(
                    get: { draft.name },
                    set: { appStore.setEditProfileName($0) }
                ),
                autocap: .never,
                autocorrect: false
            )
            VStack(alignment: .leading, spacing: 6) {
                fieldLabel("About")
                TextField(
                    "",
                    text: Binding(
                        get: { draft.about },
                        set: { appStore.setEditProfileAbout($0) }
                    ),
                    prompt: Text("A line or two — what do you read?")
                        .foregroundColor(Color.highlighterInkMuted.opacity(0.7)),
                    axis: .vertical
                )
                .font(.body)
                .foregroundStyle(Color.highlighterInkStrong)
                .lineLimit(3 ... 8)
            }
        }
    }

    private var contactFields: some View {
        VStack(alignment: .leading, spacing: 18) {
            field(
                label: "NIP-05",
                placeholder: "you@example.com",
                text: Binding(
                    get: { draft.nip05 },
                    set: { appStore.setEditProfileNip05($0) }
                ),
                autocap: .never,
                keyboard: .emailAddress,
                autocorrect: false
            )
            field(
                label: "Website",
                placeholder: "https://…",
                text: Binding(
                    get: { draft.website },
                    set: { appStore.setEditProfileWebsite($0) }
                ),
                autocap: .never,
                keyboard: .URL,
                autocorrect: false
            )
            field(
                label: "Lightning address",
                placeholder: "you@walletofsatoshi.com",
                text: Binding(
                    get: { draft.lud16 },
                    set: { appStore.setEditProfileLud16($0) }
                ),
                autocap: .never,
                keyboard: .emailAddress,
                autocorrect: false
            )
        }
    }

    private func field(
        label: String,
        placeholder: String,
        text: Binding<String>,
        autocap: TextInputAutocapitalization = .sentences,
        keyboard: UIKeyboardType = .default,
        autocorrect: Bool = true
    ) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            fieldLabel(label)
            TextField(
                "",
                text: text,
                prompt: Text(placeholder)
                    .foregroundColor(Color.highlighterInkMuted.opacity(0.7))
            )
            .font(.body)
            .foregroundStyle(Color.highlighterInkStrong)
            .textInputAutocapitalization(autocap)
            .autocorrectionDisabled(!autocorrect)
            .keyboardType(keyboard)
        }
    }

    private func fieldLabel(_ text: String) -> some View {
        Text(text.uppercased())
            .font(.footnote.weight(.semibold))
            .tracking(0.6)
            .foregroundStyle(Color.highlighterInkMuted)
    }

    private func clearChip(action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: "xmark")
                .font(.caption.weight(.semibold))
                .foregroundStyle(.white)
                .padding(8)
                .background(.black.opacity(0.55), in: Circle())
        }
        .accessibilityLabel("Remove banner")
    }

    private var stickyCTA: some View {
        VStack(spacing: 0) {
            LinearGradient(
                colors: [Color.highlighterPaper.opacity(0), Color.highlighterPaper],
                startPoint: .top,
                endPoint: .bottom
            )
            .frame(height: 24)

            Button(action: { appStore.submitEditProfile() }) {
                ZStack {
                    if draft.isSaving {
                        ProgressView().tint(.white)
                    } else {
                        Text("Save")
                            .font(.headline)
                            .foregroundStyle(.white)
                    }
                }
                .frame(maxWidth: .infinity)
                .padding(.vertical, 16)
                .background(
                    RoundedRectangle(cornerRadius: 16)
                        .fill(canSave ? Color.highlighterAccent : Color.highlighterAccent.opacity(0.35))
                )
            }
            .buttonStyle(.plain)
            .disabled(!canSave)
            .padding(.horizontal, 22)
            .padding(.bottom, 24)
            .background(Color.highlighterPaper)
        }
    }

    private var canSave: Bool {
        isDirty && !draft.isSaving && !draft.isPictureUploading && !draft.isBannerUploading
    }

    private var errorBinding: Binding<Bool> {
        Binding(
            get: { draft.errorMessage != nil },
            set: { if !$0 { appStore.clearEditProfileError() } }
        )
    }

    private func upload(item: PhotosPickerItem, target: HighlighterEditProfileImageTarget) async {
        do {
            guard let data = try await item.loadTransferable(type: Data.self),
                  let image = UIImage(data: data)
            else {
                appStore.editProfileCapabilityFailed(message: "Couldn't read that image.")
                return
            }
            let prepared = await prepareForUpload(image: image)
            appStore.uploadEditProfileImage(
                target: target,
                bytes: prepared.data,
                mime: "image/jpeg",
                width: UInt32(prepared.width),
                height: UInt32(prepared.height),
                alt: ""
            )
        } catch {
            appStore.editProfileCapabilityFailed(message: "Couldn't read that image.")
        }
    }

    private struct PreparedImage { let data: Data; let width: Int; let height: Int }

    private func prepareForUpload(image: UIImage) async -> PreparedImage {
        let maxSide: CGFloat = 1600
        let scale = min(1, maxSide / max(image.size.width, image.size.height))
        let target = CGSize(width: image.size.width * scale, height: image.size.height * scale)
        let renderer = UIGraphicsImageRenderer(size: target)
        let scaled = renderer.image { _ in image.draw(in: CGRect(origin: .zero, size: target)) }
        let data = scaled.jpegData(compressionQuality: 0.85) ?? Data()
        return PreparedImage(data: data, width: Int(scaled.size.width), height: Int(scaled.size.height))
    }
}
