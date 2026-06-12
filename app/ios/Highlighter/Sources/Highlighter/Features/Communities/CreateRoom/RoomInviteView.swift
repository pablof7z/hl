import CoreImage.CIFilterBuiltins
import Kingfisher
import SwiftUI

/// Add-people screen used both right after creating a room (welcome mode)
/// and from the room's overflow menu (manage mode).
///
/// Mental model: there is no segmented "manual vs link" picker — both
/// exist on one canvas. The share card at the top is "whoever shows up"
/// and the search field below is "specifically these people". A unified
/// search field auto-detects npub / nprofile / hex pubkey on paste, and
/// otherwise filters the user's follow list. Selected invitees collect as
/// paper chips above the field; a sticky "Add (N)" button only appears
/// when chips exist.
struct RoomInviteView: View {
    enum Mode {
        case welcome
        case manage
    }

    let groupId: String
    let mode: Mode
    let onClose: (() -> Void)?

    @Environment(HighlighterStore.self) private var appStore
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        ZStack(alignment: .bottom) {
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 24) {
                    if mode == .welcome {
                        welcomeHeader
                            .padding(.horizontal, 22)
                            .padding(.top, 8)
                    }

                    RoomShareCard(groupId: groupId, room: cachedRoom)
                        .padding(.horizontal, 22)

                    sectionHeader("Add specific people")
                        .padding(.horizontal, 22)

                    chipsZone
                        .padding(.horizontal, 22)

                    searchField
                        .padding(.horizontal, 22)

                    suggestionsList

                    Spacer(minLength: 140)
                }
                .padding(.top, 8)
            }
            .scrollDismissesKeyboard(.interactively)

            stickyAddBar
        }
        .background(Color.highlighterPaper.ignoresSafeArea())
        .navigationTitle(mode == .welcome ? "" : "Add people")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            if mode == .welcome {
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Done") {
                        if let onClose { onClose() } else { dismiss() }
                    }
                    .foregroundStyle(Color.highlighterInkStrong)
                }
            }
        }
        .task(id: groupId) {
            appStore.openRoomInvite(groupId: groupId)
        }
        .onDisappear {
            appStore.closeRoomInvite()
        }
        .alert("Couldn't add", isPresented: errorBinding, actions: {
            Button("OK") { appStore.clearRoomInviteAddError() }
        }, message: { if let addError { Text(addError) } })
        .overlay(alignment: .top) {
            if let toast = inviteToast {
                HStack(spacing: 10) {
                    Text(toast)
                        .font(.subheadline.weight(.medium))
                        .foregroundStyle(.white)
                    Button {
                        withAnimation(.easeIn(duration: 0.2)) { appStore.clearRoomInviteToast() }
                    } label: {
                        Image(systemName: "xmark")
                            .font(.caption.weight(.bold))
                            .foregroundStyle(.white.opacity(0.9))
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel("Dismiss")
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 10)
                .background(Color.highlighterInkStrong, in: Capsule())
                .padding(.top, 8)
                .transition(.move(edge: .top).combined(with: .opacity))
            }
        }
    }

    // MARK: - Sections

    private var welcomeHeader: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("Your room is open.")
                .font(.system(.title2, design: .default).italic())
                .foregroundStyle(Color.highlighterInkStrong)
            Text("Invite the first guests below — or share the link.")
                .font(.subheadline)
                .foregroundStyle(Color.highlighterInkMuted)
        }
    }

    private func sectionHeader(_ title: String) -> some View {
        Text(title.uppercased())
            .font(.footnote.weight(.semibold))
            .tracking(1.2)
            .foregroundStyle(Color.highlighterInkMuted)
    }

    @ViewBuilder
    private var chipsZone: some View {
        if !selectedCandidates.isEmpty {
            FlowChips(items: selectedCandidates) { candidate in
                Chip(candidate: candidate, profile: profile(for: candidate.pubkeyHex)) {
                    remove(candidate)
                }
            }
        }
    }

    private var searchField: some View {
        HStack(spacing: 10) {
            Image(systemName: "magnifyingglass")
                .foregroundStyle(Color.highlighterInkMuted)
            TextField(
                "",
                text: roomInviteQueryBinding,
                prompt: Text("Search follows or paste an npub")
                    .foregroundColor(Color.highlighterInkMuted.opacity(0.7))
            )
            .textInputAutocapitalization(.never)
            .autocorrectionDisabled()
            .submitLabel(.done)
            .onSubmit { acceptPasteIfAny() }
            if !appStore.roomInvite.query.isEmpty {
                Button {
                    appStore.setRoomInviteQuery("")
                } label: {
                    Image(systemName: "xmark.circle.fill")
                        .foregroundStyle(Color.highlighterInkMuted)
                }
                .buttonStyle(.plain)
            }
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 12)
        .background(
            RoundedRectangle(cornerRadius: 14)
                .stroke(Color.highlighterRule, lineWidth: 1)
        )
    }

    @ViewBuilder
    private var suggestionsList: some View {
        if let resolved = appStore.roomInvite.pastedCandidate {
            VStack(spacing: 0) {
                personRow(
                    pubkeyHex: resolved.pubkeyHex,
                    profile: profile(for: resolved.pubkeyHex),
                    secondary: resolved.kind.inviteLabel,
                    isSelected: isSelected(resolved.pubkeyHex)
                ) {
                    appStore.acceptRoomInvitePastedCandidate()
                }
            }
            .padding(.horizontal, 22)
        } else {
            let visible = appStore.roomInvite.visibleFollows
            if visible.isEmpty && !appStore.roomInvite.query.isEmpty && !appStore.roomInvite.isLoadingFollows {
                Text("No matching follow — paste an npub to invite anyone.")
                    .font(.subheadline)
                    .foregroundStyle(Color.highlighterInkMuted)
                    .padding(.horizontal, 22)
                    .padding(.top, 8)
            } else {
                LazyVStack(spacing: 0) {
                    ForEach(visible, id: \.self) { pubkey in
                        personRow(
                            pubkeyHex: pubkey,
                            profile: profile(for: pubkey),
                            secondary: "Following",
                            isSelected: isSelected(pubkey)
                        ) {
                            appStore.toggleRoomInviteCandidate(pubkeyHex: pubkey, source: .follow)
                        }
                        if pubkey != visible.last {
                            Divider().overlay(Color.highlighterRule)
                                .padding(.leading, 70)
                        }
                    }
                }
                .padding(.horizontal, 22)
            }
        }
    }

    @ViewBuilder
    private var stickyAddBar: some View {
        if !selectedCandidates.isEmpty {
            VStack(spacing: 0) {
                LinearGradient(
                    colors: [Color.highlighterPaper.opacity(0), Color.highlighterPaper],
                    startPoint: .top,
                    endPoint: .bottom
                )
                .frame(height: 24)

                Button {
                    appStore.submitRoomInviteMembers()
                } label: {
                    ZStack {
                        if appStore.roomInvite.isAddingMembers {
                            ProgressView().tint(.white)
                        } else {
                            Text(selectedCandidates.count == 1 ? "Add 1 person" : "Add \(selectedCandidates.count) people")
                                .font(.headline)
                                .foregroundStyle(.white)
                        }
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 16)
                    .background(
                        RoundedRectangle(cornerRadius: 16)
                            .fill(Color.highlighterAccent)
                    )
                }
                .buttonStyle(.plain)
                .disabled(appStore.roomInvite.isAddingMembers)
                .padding(.horizontal, 22)
                .padding(.bottom, 24)
                .background(Color.highlighterPaper)
            }
        }
    }

    // MARK: - Person row

    @ViewBuilder
    private func personRow(
        pubkeyHex: String,
        profile: ProfileMetadata?,
        secondary: String,
        isSelected: Bool,
        onTap: @escaping () -> Void
    ) -> some View {
        Button(action: onTap) {
            HStack(spacing: 14) {
                AvatarView(profile: profile, pubkeyHex: pubkeyHex, size: 44)

                VStack(alignment: .leading, spacing: 2) {
                    Text(displayName(profile: profile, fallback: pubkeyHex))
                        .font(.body.weight(.medium))
                        .foregroundStyle(Color.highlighterInkStrong)
                        .lineLimit(1)
                    Text(secondary)
                        .font(.caption)
                        .foregroundStyle(Color.highlighterInkMuted)
                        .lineLimit(1)
                }
                Spacer(minLength: 0)
                Image(systemName: isSelected ? "checkmark.circle.fill" : "plus.circle")
                    .font(.title3)
                    .foregroundStyle(isSelected ? Color.highlighterAccent : Color.highlighterInkMuted)
            }
            .padding(.vertical, 12)
        }
        .buttonStyle(.plain)
        .task {
            if profile == nil {
                appStore.requestProfile(pubkeyHex: pubkeyHex)
            }
        }
    }

    // MARK: - State helpers

    private var cachedRoom: CommunitySummary? {
        appStore.joinedCommunities.first(where: { $0.id == groupId })
    }

    private func profile(for pubkey: String) -> ProfileMetadata? {
        appStore.profile(pubkeyHex: pubkey)
    }

    private func isSelected(_ pubkey: String) -> Bool {
        selectedCandidates.contains(where: { $0.pubkeyHex == pubkey })
    }

    private func remove(_ candidate: HighlighterRoomInviteCandidate) {
        appStore.removeRoomInviteCandidate(pubkeyHex: candidate.pubkeyHex)
    }

    private func acceptPasteIfAny() {
        guard appStore.roomInvite.pastedCandidate != nil else { return }
        appStore.acceptRoomInvitePastedCandidate()
    }

    private var errorBinding: Binding<Bool> {
        Binding(
            get: { addError != nil },
            set: { if !$0 { appStore.clearRoomInviteAddError() } }
        )
    }

    private var roomInviteQueryBinding: Binding<String> {
        Binding(
            get: { appStore.roomInvite.query },
            set: { appStore.setRoomInviteQuery($0) }
        )
    }

    private var selectedCandidates: [HighlighterRoomInviteCandidate] {
        appStore.roomInvite.selected
    }

    private var addError: String? {
        appStore.roomInvite.addErrorMessage
    }

    private var inviteToast: String? {
        appStore.roomInvite.toastMessage
    }

    private func displayName(profile: ProfileMetadata?, fallback hex: String) -> String {
        if let displayName = profile?.displayName, !displayName.isEmpty { return displayName }
        if let name = profile?.name, !name.isEmpty { return name }
        return shortPubkey(hex)
    }

    private func shortPubkey(_ hex: String) -> String {
        guard hex.count > 12 else { return hex }
        let prefix = hex.prefix(6)
        let suffix = hex.suffix(4)
        return "\(prefix)…\(suffix)"
    }
}

extension HighlighterRoomInviteCandidate: Identifiable {
    public var id: String { pubkeyHex }
}

private extension HighlighterRoomInvitePastedKind {
    var inviteLabel: String {
        switch self {
        case .npub: return "Pasted npub"
        case .nprofile: return "Pasted nprofile"
        case .hex: return "Pasted pubkey"
        }
    }
}

// MARK: - Avatar

private struct AvatarView: View {
    let profile: ProfileMetadata?
    let pubkeyHex: String
    let size: CGFloat

    var body: some View {
        let url = URL(string: profile?.picture ?? "")
        ZStack {
            if let url, let _ = url.scheme {
                KFImage(url)
                    .placeholder { fallback }
                    .resizable()
                    .scaledToFill()
            } else {
                fallback
            }
        }
        .frame(width: size, height: size)
        .clipShape(Circle())
        .overlay(
            Circle().stroke(Color.highlighterRule, lineWidth: 1)
        )
    }

    private var fallback: some View {
        ZStack {
            Color.highlighterTintPale
            Text(initial)
                .font(.system(size: size * 0.4, weight: .semibold, design: .default))
                .foregroundStyle(Color.highlighterInkStrong)
        }
    }

    private var initial: String {
        let name = profile?.name ?? ""
        if let first = name.first { return String(first).uppercased() }
        return String(pubkeyHex.prefix(1)).uppercased()
    }
}

// MARK: - Chip

private struct Chip: View {
    let candidate: HighlighterRoomInviteCandidate
    let profile: ProfileMetadata?
    let onRemove: () -> Void

    var body: some View {
        HStack(spacing: 8) {
            AvatarView(profile: profile, pubkeyHex: candidate.pubkeyHex, size: 22)
            Text(displayName)
                .font(.subheadline.weight(.medium))
                .foregroundStyle(Color.highlighterInkStrong)
                .lineLimit(1)
            Button(action: onRemove) {
                Image(systemName: "xmark")
                    .font(.caption2.weight(.semibold))
                    .foregroundStyle(Color.highlighterInkMuted)
            }
            .buttonStyle(.plain)
        }
        .padding(.leading, 6)
        .padding(.trailing, 10)
        .padding(.vertical, 5)
        .background(
            Capsule().fill(Color.highlighterTintPale)
        )
        .overlay(
            Capsule().stroke(Color.highlighterRule, lineWidth: 1)
        )
    }

    private var displayName: String {
        if let name = profile?.name, !name.isEmpty { return name }
        return String(candidate.pubkeyHex.prefix(8))
    }
}

// MARK: - Flow chips layout

private struct FlowChips<Item: Identifiable, Content: View>: View {
    let items: [Item]
    @ViewBuilder let content: (Item) -> Content

    var body: some View {
        FlowLayout(spacing: 8) {
            ForEach(items) { item in
                content(item)
            }
        }
    }
}

private struct FlowLayout: Layout {
    var spacing: CGFloat = 8

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache _: inout ()) -> CGSize {
        let width = proposal.width ?? .infinity
        var rowWidth: CGFloat = 0
        var totalHeight: CGFloat = 0
        var rowHeight: CGFloat = 0
        for subview in subviews {
            let size = subview.sizeThatFits(.unspecified)
            if rowWidth + size.width > width {
                totalHeight += rowHeight + spacing
                rowWidth = size.width + spacing
                rowHeight = size.height
            } else {
                rowWidth += size.width + spacing
                rowHeight = max(rowHeight, size.height)
            }
        }
        return CGSize(width: width, height: totalHeight + rowHeight)
    }

    func placeSubviews(in bounds: CGRect, proposal _: ProposedViewSize, subviews: Subviews, cache _: inout ()) {
        var x = bounds.minX
        var y = bounds.minY
        var rowHeight: CGFloat = 0
        for subview in subviews {
            let size = subview.sizeThatFits(.unspecified)
            if x + size.width > bounds.maxX {
                x = bounds.minX
                y += rowHeight + spacing
                rowHeight = 0
            }
            subview.place(at: CGPoint(x: x, y: y), proposal: ProposedViewSize(size))
            x += size.width + spacing
            rowHeight = max(rowHeight, size.height)
        }
    }
}
