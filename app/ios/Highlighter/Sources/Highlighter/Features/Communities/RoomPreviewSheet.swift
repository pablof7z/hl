import Kingfisher
import SwiftUI

/// Modal presented when a card on the explorer is tapped. Large cover
/// backdrop, room name, description, a primary Join button, and a
/// secondary "Peek inside" link for open rooms. Dismisses itself after
/// firing the join; the toast on the root scene confirms.
struct RoomPreviewSheet: View {
    let room: CommunitySummary
    let onJoin: () -> Void

    @Environment(HighlighterStore.self) private var appStore
    @Environment(\.dismiss) private var dismiss

    private var alreadyJoined: Bool {
        appStore.joinedCommunities.contains(where: { $0.id == room.id })
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
                        Text(room.about)
                            .font(.body)
                            .foregroundStyle(Color.highlighterInkStrong)
                            .padding(.top, 4)
                    }
                }
                .padding(.horizontal, 20)

                Spacer(minLength: 12)

                actionStack
                    .padding(.horizontal, 20)
                    .padding(.bottom, 20)
            }
        }
        .background(Color.highlighterPaper.ignoresSafeArea())
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
        HStack(spacing: 10) {
            accessBadge
            if let count = room.memberCount, count > 0 {
                Label {
                    Text(count == 1 ? "1 member" : "\(count) members")
                } icon: {
                    Image(systemName: "person.2")
                }
                .labelStyle(.titleAndIcon)
                .font(.caption.weight(.medium))
                .foregroundStyle(Color.highlighterInkMuted)
            }
        }
    }

    private var accessBadge: some View {
        let isOpen = room.access == "open"
        return HStack(spacing: 4) {
            Image(systemName: isOpen ? "lock.open" : "lock")
                .font(.caption2.weight(.semibold))
            Text(isOpen ? "Open" : "Closed")
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
    private var actionStack: some View {
        if alreadyJoined {
            NavigationLink {
                RoomHomeView(groupId: room.id)
            } label: {
                Text("Open room")
                    .font(.headline)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 14)
                    .background(
                        RoundedRectangle(cornerRadius: 14)
                            .fill(Color.highlighterAccent)
                    )
                    .foregroundStyle(.white)
            }
            .simultaneousGesture(TapGesture().onEnded { dismiss() })
        } else {
            VStack(spacing: 10) {
                Button(action: onJoin) {
                    Text(room.access == "closed" ? "Request to join" : "Join room")
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

                if room.access == "open" {
                    NavigationLink {
                        RoomHomeView(groupId: room.id)
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
                    .simultaneousGesture(TapGesture().onEnded { dismiss() })
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
}
