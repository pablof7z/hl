import Kingfisher
import SwiftUI

/// Founding-a-room flow. One `.large` sheet, no wizard. The name field is
/// the only serif on the screen — the room is being given an identity, and
/// the typeface honours that. Visibility is an inline row, not a segmented
/// control — the default (public · open) is sane, so most users never
/// touch it. A cover is set by pasting an image URL. On create, the room is
/// minted with a caller-supplied groupId and `RoomInviteView` is pushed in
/// welcome mode so adding the first guests feels like one continuous act.
struct CreateRoomSheet: View {
    @Environment(\.dismiss) private var dismiss
    @Environment(HighlighterStore.self) private var appStore

    @State private var name: String = ""
    @State private var about: String = ""
    @State private var visibility: RoomVisibility = .public
    @State private var access: RoomAccess = .open
    @State private var visibilityPickerPresented = false
    @State private var coverURL: String = ""
    @State private var error: String?
    @State private var createdGroupId: String?

    @FocusState private var focused: Field?
    private enum Field { case name, about }

    private var projection: CreateRoomProjection {
        let createName = name.trimmingCharacters(in: .whitespaces)
        let createAbout = about.trimmingCharacters(in: .whitespaces)
        let (glyph, summary) = Self.visibilityDisplay(visibility, access)
        return CreateRoomProjection(
            canCreate: createName.count >= 2,
            createName: createName,
            createAbout: createAbout,
            visibilityGlyph: glyph,
            visibilitySummary: summary,
            visibilityOptions: Self.visibilityOptions(for: visibility, access: access)
        )
    }

    private static func visibilityDisplay(_ visibility: RoomVisibility, _ access: RoomAccess) -> (String, String) {
        switch (visibility, access) {
        case (.public, .open): return ("globe", "Public · Anyone can join")
        case (.public, .closed): return ("globe.badge.chevron.backward", "Public · You approve joins")
        case (.private, _): return ("lock", "Private · Invite only")
        }
    }

    private static func visibilityOptions(for selectedVisibility: RoomVisibility, access selectedAccess: RoomAccess) -> [CreateRoomVisibilityOption] {
        [
            CreateRoomVisibilityOption(id: "public-open", title: "Public", summary: "Anyone can find and join this room.", glyph: "globe", visibility: .public, access: .open, isSelected: selectedVisibility == .public && selectedAccess == .open),
            CreateRoomVisibilityOption(id: "public-closed", title: "Public · By approval", summary: "Anyone can find it, but you approve who joins.", glyph: "globe.badge.chevron.backward", visibility: .public, access: .closed, isSelected: selectedVisibility == .public && selectedAccess == .closed),
            CreateRoomVisibilityOption(id: "private", title: "Private", summary: "Hidden from the explorer. Invite only.", glyph: "lock", visibility: .private, access: .closed, isSelected: selectedVisibility == .private),
        ]
    }

    var body: some View {
        let currentProjection = projection

        NavigationStack {
            ZStack(alignment: .bottom) {
                ScrollView {
                    VStack(alignment: .leading, spacing: 24) {
                        coverPlate
                        identityFields
                            .padding(.horizontal, 22)
                        Divider().overlay(Color.highlighterRule)
                            .padding(.horizontal, 22)
                        visibilityRow(currentProjection)
                            .padding(.horizontal, 22)
                        Spacer(minLength: 120)
                    }
                    .padding(.top, 8)
                }
                .scrollDismissesKeyboard(.interactively)

                stickyCTA(currentProjection)
            }
            .background(Color.highlighterPaper.ignoresSafeArea())
            .navigationTitle("")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    Button("Cancel") { dismiss() }
                        .foregroundStyle(Color.highlighterInkStrong)
                }
            }
            .navigationDestination(item: $createdGroupId) { groupId in
                RoomInviteView(groupId: groupId, mode: .welcome) {
                    dismiss()
                }
            }
            .sheet(isPresented: $visibilityPickerPresented) {
                VisibilityPickerSheet(
                    visibility: $visibility,
                    access: $access,
                    options: currentProjection.visibilityOptions
                )
                .presentationDetents([.medium])
                .presentationDragIndicator(.visible)
            }
            .alert("Couldn't create room", isPresented: errorBinding, actions: {
                Button("OK") { error = nil }
            }, message: {
                if let error { Text(error) }
            })
        }
    }

    // MARK: - Sections

    private var coverPlate: some View {
        ZStack {
            if !coverURL.isEmpty, let url = URL(string: coverURL) {
                KFImage(url)
                    .resizable()
                    .scaledToFill()
                    .frame(maxWidth: .infinity)
                    .frame(height: 200)
                    .clipped()
            } else {
                LinearGradient(
                    colors: [
                        Color.highlighterAccent.opacity(0.85),
                        Color.highlighterAccent.opacity(0.45),
                    ],
                    startPoint: .topLeading,
                    endPoint: .bottomTrailing
                )
                .frame(height: 200)
                .overlay {
                    VStack(spacing: 8) {
                        Image(systemName: "photo.on.rectangle.angled")
                            .font(.title)
                            .foregroundStyle(.white.opacity(0.92))
                        Text("Paste a cover URL below")
                            .font(.subheadline.weight(.medium))
                            .foregroundStyle(.white.opacity(0.92))
                    }
                }
            }
        }
        .overlay(alignment: .topTrailing) {
            if !coverURL.isEmpty {
                Button {
                    coverURL = ""
                } label: {
                    Image(systemName: "xmark")
                        .font(.caption.weight(.semibold))
                        .foregroundStyle(.white)
                        .padding(8)
                        .background(.black.opacity(0.45), in: Circle())
                }
                .padding(12)
            }
        }
    }

    private var identityFields: some View {
        VStack(alignment: .leading, spacing: 18) {
            TextField(
                "",
                text: $name,
                prompt: Text("Name your room").foregroundColor(Color.highlighterInkMuted.opacity(0.7))
            )
            .font(.system(.largeTitle, design: .default).weight(.semibold))
            .foregroundStyle(Color.highlighterInkStrong)
            .focused($focused, equals: .name)
            .submitLabel(.next)
            .onSubmit { focused = .about }
            .lineLimit(2)

            TextField(
                "",
                text: $about,
                prompt: Text("What will you read together?")
                    .foregroundColor(Color.highlighterInkMuted.opacity(0.7)),
                axis: .vertical
            )
            .font(.body)
            .foregroundStyle(Color.highlighterInkStrong)
            .focused($focused, equals: .about)
            .lineLimit(3...8)

            TextField(
                "",
                text: $coverURL,
                prompt: Text("Cover image URL (optional)")
                    .foregroundColor(Color.highlighterInkMuted.opacity(0.7))
            )
            .font(.body)
            .foregroundStyle(Color.highlighterInkStrong)
            .textInputAutocapitalization(.never)
            .autocorrectionDisabled()
            .keyboardType(.URL)
        }
    }

    private func visibilityRow(_ projection: CreateRoomProjection) -> some View {
        Button {
            visibilityPickerPresented = true
        } label: {
            HStack(spacing: 12) {
                Image(systemName: projection.visibilityGlyph)
                    .font(.body.weight(.medium))
                    .foregroundStyle(Color.highlighterAccent)
                    .frame(width: 22)
                VStack(alignment: .leading, spacing: 2) {
                    Text("Visibility")
                        .font(.footnote.weight(.semibold))
                        .tracking(0.6)
                        .foregroundStyle(Color.highlighterInkMuted)
                    Text(projection.visibilitySummary)
                        .font(.body.weight(.medium))
                        .foregroundStyle(Color.highlighterInkStrong)
                }
                Spacer(minLength: 0)
                Image(systemName: "chevron.right")
                    .font(.footnote.weight(.semibold))
                    .foregroundStyle(Color.highlighterInkMuted)
            }
            .padding(.vertical, 6)
        }
        .buttonStyle(.plain)
    }

    private func stickyCTA(_ projection: CreateRoomProjection) -> some View {
        VStack(spacing: 0) {
            LinearGradient(
                colors: [Color.highlighterPaper.opacity(0), Color.highlighterPaper],
                startPoint: .top,
                endPoint: .bottom
            )
            .frame(height: 24)

            Button(action: create) {
                Text("Create Room")
                    .font(.headline)
                    .foregroundStyle(.white)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 16)
                    .background(
                        RoundedRectangle(cornerRadius: 16)
                            .fill(projection.canCreate ? Color.highlighterAccent : Color.highlighterAccent.opacity(0.35))
                    )
            }
            .buttonStyle(.plain)
            .disabled(!projection.canCreate)
            .padding(.horizontal, 22)
            .padding(.bottom, 24)
            .background(Color.highlighterPaper)
        }
    }

    // MARK: - Helpers

    private var errorBinding: Binding<Bool> {
        Binding(get: { error != nil }, set: { if !$0 { error = nil } })
    }

    private func create() {
        let draft = projection
        guard draft.canCreate else { return }

        guard appStore.joinedCommunities.contains(where: { !$0.relayUrl.isEmpty }) else {
            error = "No rooms relay configured. Join a room first."
            return
        }

        let groupId = String(UUID().uuidString.replacingOccurrences(of: "-", with: "").prefix(16)).lowercased()

        appStore.kernel?.app.dispatch(.createRoom(
            groupId: groupId,
            name: draft.createName,
            about: draft.createAbout.isEmpty ? nil : draft.createAbout
        ))

        focused = nil
        UINotificationFeedbackGenerator().notificationOccurred(.success)
        createdGroupId = groupId
    }
}

extension String: @retroactive Identifiable {
    public var id: String { self }
}

// MARK: - Visibility picker

private struct VisibilityPickerSheet: View {
    @Binding var visibility: RoomVisibility
    @Binding var access: RoomAccess
    let options: [CreateRoomVisibilityOption]
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(spacing: 0) {
                    ForEach(options, id: \.id) { option in
                        Button {
                            visibility = option.visibility
                            access = option.access
                            UISelectionFeedbackGenerator().selectionChanged()
                            dismiss()
                        } label: {
                            HStack(alignment: .top, spacing: 14) {
                                Image(systemName: option.glyph)
                                    .font(.title3)
                                    .foregroundStyle(Color.highlighterAccent)
                                    .frame(width: 28)
                                    .padding(.top, 2)
                                VStack(alignment: .leading, spacing: 4) {
                                    Text(option.title)
                                        .font(.body.weight(.semibold))
                                        .foregroundStyle(Color.highlighterInkStrong)
                                    Text(option.summary)
                                        .font(.subheadline)
                                        .foregroundStyle(Color.highlighterInkMuted)
                                        .multilineTextAlignment(.leading)
                                }
                                Spacer(minLength: 0)
                                if option.isSelected {
                                    Image(systemName: "checkmark")
                                        .font(.body.weight(.semibold))
                                        .foregroundStyle(Color.highlighterAccent)
                                }
                            }
                            .padding(.horizontal, 22)
                            .padding(.vertical, 16)
                        }
                        .buttonStyle(.plain)
                        if option.id != options.last?.id {
                            Divider().overlay(Color.highlighterRule)
                                .padding(.leading, 64)
                        }
                    }
                }
            }
            .background(Color.highlighterPaper.ignoresSafeArea())
            .navigationTitle("Visibility")
            .navigationBarTitleDisplayMode(.inline)
        }
    }
}
