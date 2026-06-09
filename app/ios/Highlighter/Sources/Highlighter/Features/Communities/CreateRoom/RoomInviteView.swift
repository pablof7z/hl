import CoreImage.CIFilterBuiltins
import Kingfisher
import SwiftUI

extension RoomInviteChip: Identifiable {
    public var id: String { pubkeyHex }
}

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

    @State private var query: String = ""
    @State private var inviteSnapshot: RoomInviteSnapshot?
    @State private var selected: [Candidate] = []
    @State private var sending = false
    @State private var error: String?
    @State private var sentToast: String?
    @State private var sentToastResetTimer = OneShotUITimer()

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
        .task(id: inviteSnapshotRequestKey) {
            await refreshInviteSnapshot(requestProfiles: true)
        }
        .task(id: inviteProfileRefreshKey) {
            guard inviteSnapshot != nil else { return }
            await refreshInviteSnapshot(requestProfiles: false)
        }
        .alert("Couldn't add", isPresented: errorBinding, actions: {
            Button("OK") { error = nil }
        }, message: { if let error { Text(error) } })
        .overlay(alignment: .top) {
            if let toast = sentToast {
                Text(toast)
                    .font(.subheadline.weight(.medium))
                    .foregroundStyle(.white)
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
        if !selected.isEmpty {
            FlowChips(items: inviteProjection.selectedChips) { chip in
                Chip(chip: chip, profile: profile(for: chip.pubkeyHex)) {
                    applySelection(
                        candidate: RoomInviteCandidate(
                            pubkeyHex: chip.pubkeyHex,
                            source: chip.source
                        ),
                        action: .remove
                    )
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
                text: $query,
                prompt: Text("Search follows or paste an npub")
                    .foregroundColor(Color.highlighterInkMuted.opacity(0.7))
            )
            .textInputAutocapitalization(.never)
            .autocorrectionDisabled()
            .submitLabel(.done)
            .onSubmit { acceptPasteIfAny() }
            if !query.isEmpty {
                Button {
                    query = ""
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
        let projection = inviteProjection
        if let resolved = projection.resolvedCandidate {
            VStack(spacing: 0) {
                personRow(
                    pubkeyHex: resolved.pubkeyHex,
                    profile: profile(for: resolved.pubkeyHex),
                    displayName: resolved.displayName,
                    secondary: resolved.label,
                    isSelected: resolved.isSelected
                ) {
                    add(Candidate(pubkeyHex: resolved.pubkeyHex, source: resolved.source))
                    query = ""
                }
            }
            .padding(.horizontal, 22)
        } else {
            let visible = projection.visibleFollows
            if projection.showEmptyFollowMessage {
                Text("No matching follow — paste an npub to invite anyone.")
                    .font(.subheadline)
                    .foregroundStyle(Color.highlighterInkMuted)
                    .padding(.horizontal, 22)
                    .padding(.top, 8)
            } else {
                LazyVStack(spacing: 0) {
                    ForEach(visible, id: \.pubkeyHex) { row in
                        personRow(
                            pubkeyHex: row.pubkeyHex,
                            profile: profile(for: row.pubkeyHex),
                            displayName: row.displayName,
                            secondary: row.secondaryLabel,
                            isSelected: row.isSelected
                        ) {
                            toggle(Candidate(pubkeyHex: row.pubkeyHex, source: row.source))
                        }
                        if row.pubkeyHex != visible.last?.pubkeyHex {
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
        if !selected.isEmpty {
            VStack(spacing: 0) {
                LinearGradient(
                    colors: [Color.highlighterPaper.opacity(0), Color.highlighterPaper],
                    startPoint: .top,
                    endPoint: .bottom
                )
                .frame(height: 24)

                Button(action: send) {
                    ZStack {
                        if sending {
                            ProgressView().tint(.white)
                        } else {
                            Text(selectionChrome.addButtonLabel)
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
                .disabled(sending)
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
        displayName: String,
        secondary: String,
        isSelected: Bool,
        onTap: @escaping () -> Void
    ) -> some View {
        Button(action: onTap) {
            HStack(spacing: 14) {
                AvatarView(profile: profile, pubkeyHex: pubkeyHex, size: 44)

                VStack(alignment: .leading, spacing: 2) {
                    Text(displayName)
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
                await appStore.requestProfile(pubkeyHex: pubkeyHex)
            }
        }
    }

    // MARK: - State helpers

    private var cachedRoom: CommunitySummary? {
        appStore.joinedCommunities.first(where: { $0.id == groupId })
    }

    private var inviteProjection: RoomInviteProjection {
        inviteSnapshot?.projection ?? Self.emptyInviteProjection
    }

    private var selectionChrome: RoomInviteSelectionChromeProjection {
        appStore.safeCore.projectRoomInviteSelectionChrome(
            input: RoomInviteSelectionChromeInput(selectedCount: UInt64(selected.count))
        )
    }

    private var inviteSnapshotRequestKey: String {
        let selectedKey = selected
            .map { "\($0.pubkeyHex):\($0.source)" }
            .joined(separator: ",")
        return "\(query)|\(selectedKey)|\(appStore.currentUser?.pubkey ?? "")"
    }

    private var inviteProfileRefreshKey: String {
        (inviteSnapshot?.profilePubkeysToRequest ?? [])
            .map { pubkey in
                guard let profile = appStore.profileSnapshots[pubkey] else {
                    return "\(pubkey):"
                }
                return [
                    pubkey,
                    profile.name,
                    profile.displayName,
                    profile.nip05,
                    profile.picture
                ].joined(separator: ":")
            }
            .joined(separator: "|")
    }

    private static let emptyInviteProjection = RoomInviteProjection(
        selectedChips: [],
        visibleFollows: [],
        resolvedCandidate: nil,
        showEmptyFollowMessage: false
    )

    private func profile(for pubkey: String) -> ProfileMetadata? {
        appStore.profileSnapshots[pubkey]
    }

    private func toggle(_ candidate: Candidate) {
        applySelection(candidate: candidate.coreCandidate, action: .toggle)
    }

    private func add(_ candidate: Candidate) {
        applySelection(candidate: candidate.coreCandidate, action: .add)
    }

    private func applySelection(
        candidate: RoomInviteCandidate,
        action: RoomInviteSelectionAction
    ) {
        let projection = appStore.safeCore.projectRoomInviteSelection(
            input: RoomInviteSelectionInput(
                selected: selected.map(\.coreCandidate),
                candidate: candidate,
                currentUserPubkey: appStore.currentUser?.pubkey ?? "",
                action: action
            )
        )
        let previousCount = selected.count
        selected = projection.selected.map(Candidate.init(core:))
        if !projection.errorMessage.isEmpty {
            error = projection.errorMessage
        }
        if projection.selectionChanged, projection.selected.count > previousCount {
            UISelectionFeedbackGenerator().selectionChanged()
        }
    }

    private func acceptPasteIfAny() {
        guard let resolved = inviteProjection.resolvedCandidate else { return }
        add(Candidate(pubkeyHex: resolved.pubkeyHex, source: resolved.source))
        query = ""
    }

    private var errorBinding: Binding<Bool> {
        Binding(get: { error != nil }, set: { if !$0 { error = nil } })
    }

    // MARK: - Loading + actions

    @MainActor
    private func refreshInviteSnapshot(requestProfiles: Bool) async {
        let requestKey = inviteSnapshotRequestKey
        let snapshot = await appStore.safeCore.getRoomInviteSnapshot(
            input: RoomInviteSnapshotInput(
                query: query,
                profiles: Array(appStore.profileSnapshots.values),
                selected: selected.map(\.coreCandidate),
                limit: 50
            )
        )
        guard !Task.isCancelled, requestKey == inviteSnapshotRequestKey else { return }
        inviteSnapshot = snapshot

        guard requestProfiles else { return }
        for pubkey in snapshot.profilePubkeysToRequest {
            guard !Task.isCancelled else { return }
            await appStore.requestProfile(pubkeyHex: pubkey)
        }
    }

    private func send() {
        guard !sending, !selected.isEmpty else { return }
        sending = true
        let toAdd = selected
        Task {
            defer { Task { @MainActor in sending = false } }
            let result = await appStore.safeCore.sendRoomInvites(
                groupId: groupId,
                selected: toAdd.map(\.coreCandidate)
            )
            await MainActor.run {
                if result.allSucceeded {
                    selected.removeAll()
                    sentToast = result.successToast
                    UINotificationFeedbackGenerator().notificationOccurred(.success)
                    sentToastResetTimer.schedule(after: 2) {
                        sentToast = nil
                    }
                } else if result.allFailed {
                    error = result.errorMessage
                } else {
                    selected = result.remainingSelected.map(Candidate.init(core:))
                    error = result.errorMessage
                }
            }
        }
    }
}

// MARK: - Models

private struct Candidate: Identifiable, Equatable {
    let pubkeyHex: String
    let source: RoomInviteCandidateSource
    var id: String { pubkeyHex }

    var coreCandidate: RoomInviteCandidate {
        RoomInviteCandidate(pubkeyHex: pubkeyHex, source: source)
    }

    init(pubkeyHex: String, source: RoomInviteCandidateSource) {
        self.pubkeyHex = pubkeyHex
        self.source = source
    }

    init(core: RoomInviteCandidate) {
        pubkeyHex = core.pubkeyHex
        source = core.source
    }
}

// MARK: - Avatar

private struct AvatarView: View {
    let profile: ProfileMetadata?
    let pubkeyHex: String
    let size: CGFloat

    @Environment(HighlighterStore.self) private var appStore

    var body: some View {
        let avatar = avatarProjection
        let url = URL(string: avatar.pictureUrl)
        ZStack {
            if let url, let _ = url.scheme {
                KFImage(url)
                    .placeholder { fallback(avatar) }
                    .resizable()
                    .scaledToFill()
            } else {
                fallback(avatar)
            }
        }
        .frame(width: size, height: size)
        .clipShape(Circle())
        .overlay(
            Circle().stroke(Color.highlighterRule, lineWidth: 1)
        )
    }

    private func fallback(_ avatar: RoomInviteAvatarProjection) -> some View {
        ZStack {
            Color.highlighterTintPale
            Text(avatar.displayInitial)
                .font(.system(size: size * 0.4, weight: .semibold, design: .default))
                .foregroundStyle(Color.highlighterInkStrong)
        }
    }

    private var avatarProjection: RoomInviteAvatarProjection {
        appStore.safeCore.getRoomInviteAvatarProjection(
            input: RoomInviteAvatarProjectionInput(
                pubkeyHex: pubkeyHex,
                profile: profile
            )
        )
    }
}

// MARK: - Chip

private struct Chip: View {
    let chip: RoomInviteChip
    let profile: ProfileMetadata?
    let onRemove: () -> Void

    var body: some View {
        HStack(spacing: 8) {
            AvatarView(profile: profile, pubkeyHex: chip.pubkeyHex, size: 22)
            Text(chip.displayName)
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

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
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

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
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
