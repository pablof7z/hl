import SwiftUI

struct OnboardingInterestsView: View {
    let account: GeneratedAccount

    @Environment(HighlighterStore.self) private var store

    @State private var selectedIds: [String] = []
    @State private var isWorking = false

    private var projection: OnboardingInterestProjection {
        OnboardingInterestCatalog.projection(selectedIds: selectedIds)
    }

    var body: some View {
        let currentProjection = projection
        let selectionState = currentProjection.selection

        ZStack {
            Color.highlighterPaper.ignoresSafeArea()

            VStack(alignment: .leading, spacing: 0) {
                VStack(alignment: .leading, spacing: 8) {
                    Text("What do you read?")
                        .font(.system(.title, design: .default).weight(.semibold))
                        .foregroundStyle(Color.highlighterInkStrong)

                    Text("Pick at least three — we'll pre-fill your feed with highlights from readers like you.")
                        .font(.callout)
                        .foregroundStyle(Color.highlighterInkMuted)
                        .lineSpacing(2)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .padding(.horizontal, 24)
                .padding(.top, 32)
                .padding(.bottom, 24)

                ScrollView {
                    chipGrid(projection: currentProjection)
                        .padding(.horizontal, 20)
                        .padding(.bottom, 120)
                }

                Spacer(minLength: 0)
            }

            VStack {
                Spacer()

                VStack(spacing: 8) {
                    if selectionState.remaining > 0 {
                        Text("Choose \(selectionState.remaining) more")
                            .font(.caption)
                            .foregroundStyle(Color.highlighterInkMuted)
                            .transition(.opacity)
                    }

                    Button(action: finish) {
                        Group {
                            if isWorking {
                                ProgressView().tint(.white)
                            } else {
                                Text("Start exploring")
                                    .font(.headline)
                            }
                        }
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 14)
                    }
                    .buttonStyle(.glassProminent)
                    .disabled(!selectionState.canContinue || isWorking)
                    .padding(.horizontal, 32)
                    .animation(.easeInOut(duration: 0.15), value: selectionState.selectedCount)
                }
                .padding(.bottom, 48)
                .background(
                    LinearGradient(
                        colors: [Color.highlighterPaper.opacity(0), Color.highlighterPaper],
                        startPoint: .top,
                        endPoint: UnitPoint(x: 0.5, y: 0.6)
                    )
                    .ignoresSafeArea()
                )
            }
        }
        .navigationBarBackButtonHidden(true)
        .animation(.easeInOut(duration: 0.1), value: selectedIds)
    }

    private func chipGrid(projection: OnboardingInterestProjection) -> some View {
        FlowLayout(spacing: 10) {
            ForEach(projection.interests, id: \.id) { interest in
                chip(interest)
            }
        }
    }

    private func chip(_ interest: OnboardingInterestChip) -> some View {
        let active = interest.isSelected
        return Button {
            selectedIds = OnboardingInterestCatalog.toggle(
                selectedIds: selectedIds,
                interestId: interest.id
            )
        } label: {
            HStack(spacing: 6) {
                Text(interest.emoji)
                    .font(.body)
                Text(interest.label)
                    .font(.subheadline.weight(active ? .semibold : .regular))
                    .foregroundStyle(active ? Color.white : Color.highlighterInkStrong)
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 9)
            .background(active ? Color.highlighterAccent : Color.highlighterInkStrong.opacity(0.08),
                        in: .capsule)
        }
        .buttonStyle(.plain)
    }

    private func finish() {
        guard !isWorking else { return }
        isWorking = true
        let chosenIds = selectedIds

        Task {
            await store.completeLogin(user: account.user)
            let outcome = await store.completeOnboardingInterests(selectedIds: chosenIds)
            if !outcome.applied {
                // Follow-list publish failed (relays not yet connected on a fresh
                // account). Fall back to marking onboarding complete locally so
                // the user isn't stranded on the interests screen.
                let fallback = store.markOnboardingComplete()
                if !fallback.applied {
                    isWorking = false
                    return
                }
            }
        }
    }
}

// MARK: - FlowLayout

private struct FlowLayout: Layout {
    var spacing: CGFloat = 8

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let rows = computeRows(proposal: proposal, subviews: subviews)
        let height = rows.map(\.height).reduce(0, +) + CGFloat(max(rows.count - 1, 0)) * spacing
        return CGSize(width: proposal.width ?? 0, height: height)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let rows = computeRows(proposal: ProposedViewSize(width: bounds.width, height: nil), subviews: subviews)
        var y = bounds.minY
        for row in rows {
            var x = bounds.minX
            for item in row.items {
                item.view.place(at: CGPoint(x: x, y: y), proposal: ProposedViewSize(item.size))
                x += item.size.width + spacing
            }
            y += row.height + spacing
        }
    }

    private struct Row {
        var items: [(view: LayoutSubview, size: CGSize)] = []
        var height: CGFloat = 0
    }

    private func computeRows(proposal: ProposedViewSize, subviews: Subviews) -> [Row] {
        let maxWidth = proposal.width ?? .infinity
        var rows: [Row] = []
        var current = Row()
        var currentWidth: CGFloat = 0

        for view in subviews {
            let size = view.sizeThatFits(ProposedViewSize(width: nil, height: nil))
            if currentWidth + size.width > maxWidth, !current.items.isEmpty {
                rows.append(current)
                current = Row()
                currentWidth = 0
            }
            current.items.append((view, size))
            current.height = max(current.height, size.height)
            currentWidth += size.width + spacing
        }
        if !current.items.isEmpty { rows.append(current) }
        return rows
    }
}

// MARK: - Onboarding interest catalog (D1 inlined)
//
// Mirrors `app/core/src/onboarding.rs` (interest_catalog / interest_projection /
// toggle_interest_selection). The catalog is static seed data, so per the D1
// doctrine Swift owns this pure computation. Rust still owns the durable
// "onboarding complete" flag and the follow-list publish in
// `complete_onboarding_interests`.
private enum OnboardingInterestCatalog {
    private static let jackPubkey = "82341f882b6eabcd2ba7f1ef90aad961cf074af15b9ef44a09f9d2a8fbfbe6a2"
    private static let fiatjafPubkey = "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d"
    private static let minimumRequired: UInt32 = 3

    private struct Seed {
        let id: String
        let emoji: String
        let label: String
        let pubkeys: [String]
    }

    private static let interests: [Seed] = [
        Seed(id: "philosophy", emoji: "🧠", label: "Philosophy", pubkeys: [jackPubkey]),
        Seed(id: "science_fiction", emoji: "🚀", label: "Science Fiction", pubkeys: [jackPubkey]),
        Seed(id: "technology", emoji: "💻", label: "Technology", pubkeys: [fiatjafPubkey, jackPubkey]),
        Seed(id: "history", emoji: "📜", label: "History", pubkeys: [jackPubkey]),
        Seed(id: "economics", emoji: "📈", label: "Economics", pubkeys: [fiatjafPubkey]),
        Seed(id: "psychology", emoji: "🔬", label: "Psychology", pubkeys: [jackPubkey]),
        Seed(id: "literature", emoji: "📚", label: "Literature", pubkeys: [jackPubkey]),
        Seed(id: "politics", emoji: "🗳️", label: "Politics", pubkeys: []),
        Seed(id: "bitcoin", emoji: "₿", label: "Bitcoin", pubkeys: [jackPubkey, fiatjafPubkey]),
        Seed(id: "self_improvement", emoji: "🌱", label: "Self-improvement", pubkeys: [jackPubkey]),
        Seed(id: "science", emoji: "🔭", label: "Science", pubkeys: []),
        Seed(id: "art", emoji: "🎨", label: "Art", pubkeys: []),
        Seed(id: "music", emoji: "🎵", label: "Music", pubkeys: []),
        Seed(id: "design", emoji: "✏️", label: "Design", pubkeys: []),
        Seed(id: "writing", emoji: "✍️", label: "Writing", pubkeys: [jackPubkey]),
        Seed(id: "startups", emoji: "⚡️", label: "Startups", pubkeys: [jackPubkey]),
        Seed(id: "nostr", emoji: "🟣", label: "Nostr", pubkeys: [fiatjafPubkey]),
        Seed(id: "food", emoji: "🍳", label: "Food", pubkeys: []),
        Seed(id: "travel", emoji: "🗺️", label: "Travel", pubkeys: []),
        Seed(id: "health", emoji: "🏃", label: "Health", pubkeys: []),
    ]

    /// Renderable chips plus the selection summary for the current set of ids.
    static func projection(selectedIds: [String]) -> OnboardingInterestProjection {
        let requested = Set(selectedIds)
        let selected = Set(interests.lazy.filter { requested.contains($0.id) }.map(\.id))
        let chips = interests.map {
            OnboardingInterestChip(
                id: $0.id,
                emoji: $0.emoji,
                label: $0.label,
                isSelected: selected.contains($0.id)
            )
        }
        return OnboardingInterestProjection(interests: chips, selection: selection(for: selected))
    }

    /// Toggle a known interest id in/out, returning ids in catalog order.
    /// Unknown ids leave the selection unchanged (matches Rust).
    static func toggle(selectedIds: [String], interestId: String) -> [String] {
        let requested = Set(selectedIds)
        var selected = Set(interests.lazy.filter { requested.contains($0.id) }.map(\.id))
        if interests.contains(where: { $0.id == interestId }) {
            if selected.contains(interestId) {
                selected.remove(interestId)
            } else {
                selected.insert(interestId)
            }
        }
        return interests.lazy.filter { selected.contains($0.id) }.map(\.id)
    }

    private static func selection(for selected: Set<String>) -> OnboardingInterestSelection {
        let selectedCount = UInt32(selected.count)
        let remaining = selectedCount >= minimumRequired ? 0 : minimumRequired - selectedCount

        var seen = Set<String>()
        var followPubkeys: [String] = []
        for interest in interests where selected.contains(interest.id) {
            for pubkey in interest.pubkeys where seen.insert(pubkey).inserted {
                followPubkeys.append(pubkey)
            }
        }

        return OnboardingInterestSelection(
            minimumRequired: minimumRequired,
            selectedCount: selectedCount,
            remaining: remaining,
            canContinue: remaining == 0,
            followPubkeys: followPubkeys
        )
    }
}
