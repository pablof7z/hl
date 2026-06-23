import Kingfisher
import SwiftUI

/// Modal presented when a card on the explorer is tapped. Starts at
/// `.medium` with the hero, description, and Join button; "Peek inside"
/// expands the sheet to `.large` and streams the room's recent artifacts
/// inline — no dismissal, no navigation. "Open full room" is available
/// from the expanded state for when the user wants the real deal.
struct RoomPreviewSheet: View {
    let room: CommunitySummary
    let onJoin: () -> Void
    var onOpenRoom: (() -> Void)? = nil

    @Environment(HighlighterStore.self) private var appStore
    @Environment(HighlighterAppKernel.self) private var kernel
    @Environment(\.dismiss) private var dismiss

    @State private var detent: PresentationDetent = .medium
    @State private var roomStore: RoomStore?

    private var isExpanded: Bool { detent == .large }

    private var actionProjection: RoomPreviewActionProjection {
        let access = room.access.trimmingCharacters(in: .whitespaces)
        let roomId = room.id.trimmingCharacters(in: .whitespaces)
        let alreadyJoined = appStore.joinedCommunities.contains { $0.id.trimmingCharacters(in: .whitespaces) == roomId }
        let secondaryAction: RoomPreviewSecondaryAction = alreadyJoined || access != "open" ? .none : isExpanded ? .openFullRoom : .peekInside
        let primaryLabel = alreadyJoined ? "Open room" : access == "closed" ? "Request to join" : "Join room"
        return RoomPreviewActionProjection(alreadyJoined: alreadyJoined, primaryLabel: primaryLabel, secondaryAction: secondaryAction)
    }

    private var headerProjection: RoomPreviewHeaderProjection {
        let accessIsOpen = room.access == "open"
        let memberCountLabel: String? = {
            guard let count = room.memberCount, count > 0 else { return nil }
            return count == 1 ? "1 member" : "\(count) members"
        }()
        return RoomPreviewHeaderProjection(
            accessLabel: accessIsOpen ? "Open" : "Closed",
            accessIconSystemName: accessIsOpen ? "lock.open" : "lock",
            accessIsOpen: accessIsOpen,
            memberCountLabel: memberCountLabel
        )
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 22) {
                heroBackdrop

                VStack(alignment: .leading, spacing: 10) {
                    Text(room.name)
                        .font(.system(.title2, design: .default).weight(.semibold))
                        .foregroundStyle(Color.highlighterInkStrong)

                    meta

                    if !room.about.isEmpty {
                        NostrRichText(content: room.about, font: .body)
                            .padding(.top, 4)
                    }
                }
                .padding(.horizontal, 20)

                if isExpanded {
                    insideSection
                        .padding(.horizontal, 20)
                        .transition(.opacity.combined(with: .move(edge: .bottom)))
                }

                Spacer(minLength: 12)

                actionStack
                    .padding(.horizontal, 20)
                    .padding(.bottom, 20)
            }
            .animation(.easeInOut(duration: 0.25), value: isExpanded)
        }
        .background(Color.highlighterPaper.ignoresSafeArea())
        .presentationDetents([.medium, .large], selection: $detent)
        .presentationDragIndicator(.visible)
        .onChange(of: isExpanded) { _, expanded in
            if expanded { startRoomStoreIfNeeded() }
        }
        .onChange(of: kernel.roomHomeSnapshots[room.id]) { _, _ in
            roomStore?.applyKernelSnapshot()
        }
        .onDisappear {
            // Only release the kernel view if we actually opened it (the sheet
            // opens room-home lazily on expand).
            if roomStore != nil {
                roomStore?.stop()
                kernel.closeRoomHome(groupId: room.id)
            }
        }
    }

    // MARK: - Sections

    private var heroBackdrop: some View {
        ZStack(alignment: .bottomLeading) {
            if let url = URL(string: room.picture), !room.picture.isEmpty {
                KFImage(url)
                    .placeholder { coverFallback }
                    .fade(duration: 0.2)
                    .resizable()
                    .scaledToFill()
            } else {
                coverFallback
            }

            LinearGradient(
                colors: [
                    .black.opacity(0.0),
                    .black.opacity(0.35),
                ],
                startPoint: .top,
                endPoint: .bottom
            )
        }
        .frame(height: 220)
        .frame(maxWidth: .infinity)
        .clipped()
    }

    private var meta: some View {
        let projection = headerProjection
        return HStack(spacing: 10) {
            accessBadge(projection)
            if let memberCountLabel = projection.memberCountLabel {
                Label {
                    Text(memberCountLabel)
                } icon: {
                    Image(systemName: "person.2")
                }
                .labelStyle(.titleAndIcon)
                .font(.caption.weight(.medium))
                .foregroundStyle(Color.highlighterInkMuted)
            }
        }
    }

    private func accessBadge(_ projection: RoomPreviewHeaderProjection) -> some View {
        let isOpen = projection.accessIsOpen
        return HStack(spacing: 4) {
            Image(systemName: projection.accessIconSystemName)
                .font(.caption2.weight(.semibold))
            Text(projection.accessLabel)
                .font(.caption.weight(.semibold))
        }
        .foregroundStyle(Color.highlighterInkStrong)
        .padding(.horizontal, 10)
        .padding(.vertical, 5)
        .background(
            Capsule().fill(
                isOpen ? Color.highlighterTintPale : Color.highlighterRule.opacity(0.45)
            )
        )
    }

    @ViewBuilder
    private var insideSection: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("RECENT")
                .font(.caption.weight(.semibold))
                .tracking(1.2)
                .foregroundStyle(Color.highlighterInkMuted)

            if let store = roomStore, !store.artifacts.isEmpty {
                let previewRows: [RoomPreviewArtifactRowProjection] = {
                    let visible = Array(store.artifacts.prefix(8))
                    return visible.enumerated().map { (index, artifact) in
                        let rawTitle = artifact.preview.title.trimmingCharacters(in: .whitespaces)
                        let title = rawTitle.isEmpty ? "Untitled" : rawTitle
                        let subtitle: String? = !artifact.preview.author.isEmpty ? artifact.preview.author : !artifact.preview.domain.isEmpty ? artifact.preview.domain : nil
                        return RoomPreviewArtifactRowProjection(artifact: artifact, title: title, subtitle: subtitle, showsDivider: index < visible.count - 1)
                    }
                }()
                VStack(spacing: 0) {
                    ForEach(previewRows, id: \.artifact.shareEventId) { row in
                        InsideArtifactRow(row: row)
                        if row.showsDivider {
                            Divider().overlay(Color.highlighterRule)
                        }
                    }
                }
                .background(
                    RoundedRectangle(cornerRadius: 14)
                        .stroke(Color.highlighterRule, lineWidth: 1)
                )
            } else if roomStore?.isLoading == true || roomStore == nil {
                HStack(spacing: 10) {
                    ProgressView().controlSize(.small)
                    Text("Pulling recent content…")
                        .font(.subheadline)
                        .foregroundStyle(Color.highlighterInkMuted)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.vertical, 18)
            } else {
                Text("Nothing shared here yet.")
                    .font(.subheadline)
                    .foregroundStyle(Color.highlighterInkMuted)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.vertical, 12)
            }
        }
    }

    @ViewBuilder
    private var actionStack: some View {
        let projection = actionProjection
        if projection.alreadyJoined {
            Button {
                if let onOpenRoom {
                    onOpenRoom()
                } else {
                    dismiss()
                }
            } label: {
                Text(projection.primaryLabel)
                    .font(.headline)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 14)
                    .background(
                        RoundedRectangle(cornerRadius: 14)
                            .fill(Color.highlighterAccent)
                    )
                    .foregroundStyle(.white)
            }
            .buttonStyle(.plain)
        } else {
            VStack(spacing: 10) {
                Button(action: onJoin) {
                    Text(projection.primaryLabel)
                        .font(.headline)
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 14)
                        .background(
                            RoundedRectangle(cornerRadius: 14)
                                .fill(Color.highlighterAccent)
                        )
                        .foregroundStyle(.white)
                }
                .buttonStyle(.plain)

                switch projection.secondaryAction {
                case .openFullRoom:
                    Button {
                        if let onOpenRoom {
                            onOpenRoom()
                        } else {
                            dismiss()
                        }
                    } label: {
                        Text("Open full room")
                            .font(.subheadline.weight(.medium))
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 12)
                            .foregroundStyle(Color.highlighterInkStrong)
                            .overlay(
                                RoundedRectangle(cornerRadius: 14)
                                    .stroke(Color.highlighterRule, lineWidth: 1)
                            )
                    }
                    .buttonStyle(.plain)
                case .peekInside:
                    Button {
                        detent = .large
                    } label: {
                        Text("Peek inside")
                            .font(.subheadline.weight(.medium))
                            .frame(maxWidth: .infinity)
                            .padding(.vertical, 12)
                            .foregroundStyle(Color.highlighterInkStrong)
                            .overlay(
                                RoundedRectangle(cornerRadius: 14)
                                    .stroke(Color.highlighterRule, lineWidth: 1)
                            )
                    }
                    .buttonStyle(.plain)
                case .none:
                    EmptyView()
                }
            }
        }
    }

    private var coverFallback: some View {
        LinearGradient(
            colors: [
                Color.highlighterAccent.opacity(0.72),
                Color.highlighterAccent.opacity(0.36),
            ],
            startPoint: .topLeading,
            endPoint: .bottomTrailing
        )
    }

    // MARK: - Private

    private func startRoomStoreIfNeeded() {
        guard roomStore == nil else { return }
        // Open the kernel room-home view so the kernel pushes the aggregated
        // snapshot, then mirror it into a fresh store.
        kernel.openRoomHome(groupId: room.id)
        let store = RoomStore()
        roomStore = store
        store.start(groupId: room.id, kernel: kernel)
    }
}

/// Compact artifact row used inside the peek sheet. Just the essentials —
/// title, source, author. Full detail is a tap-through on the room page.
private struct InsideArtifactRow: View {
    let row: RoomPreviewArtifactRowProjection

    private var artifact: ArtifactRecord { row.artifact }

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            cover
                .frame(width: 44, height: 44)
                .clipShape(RoundedRectangle(cornerRadius: 8))

            VStack(alignment: .leading, spacing: 2) {
                Text(row.title)
                    .font(.subheadline.weight(.medium))
                    .foregroundStyle(Color.highlighterInkStrong)
                    .lineLimit(2)
                    .multilineTextAlignment(.leading)

                if let subtitle = row.subtitle {
                    Text(subtitle)
                        .font(.caption)
                        .foregroundStyle(Color.highlighterInkMuted)
                        .lineLimit(1)
                }
            }
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 12)
    }

    @ViewBuilder
    private var cover: some View {
        if let url = URL(string: artifact.preview.image), !artifact.preview.image.isEmpty {
            KFImage(url)
                .placeholder { coverFallback }
                .fade(duration: 0.15)
                .resizable()
                .scaledToFill()
        } else {
            coverFallback
        }
    }

    private var coverFallback: some View {
        LinearGradient(
            colors: [
                Color.highlighterAccent.opacity(0.32),
                Color.highlighterAccent.opacity(0.12),
            ],
            startPoint: .topLeading,
            endPoint: .bottomTrailing
        )
    }
}
