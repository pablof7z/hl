import SwiftUI

struct OnboardingInterestsView: View {
    @Environment(HighlighterStore.self) private var store

    var body: some View {
        let onboarding = store.nmpState.onboarding

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
                    chipGrid(onboarding)
                        .padding(.horizontal, 20)
                        .padding(.bottom, 120)
                }

                Spacer(minLength: 0)
            }

            VStack {
                Spacer()

                VStack(spacing: 8) {
                    if onboarding.remainingSelectionCount > 0 {
                        Text("Choose \(onboarding.remainingSelectionCount) more")
                            .font(.caption)
                            .foregroundStyle(Color.highlighterInkMuted)
                            .transition(.opacity)
                    }

                    Button(action: finish) {
                        Group {
                            if onboarding.isFinishing {
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
                    .disabled(!onboarding.canFinish || onboarding.isFinishing)
                    .padding(.horizontal, 32)
                    .animation(.easeInOut(duration: 0.15), value: onboarding.remainingSelectionCount)
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
        .animation(.easeInOut(duration: 0.1), value: onboarding.selectedInterestIds)
    }

    private func chipGrid(_ onboarding: HighlighterOnboardingSnapshot) -> some View {
        FlowLayout(spacing: 10) {
            ForEach(onboarding.interests, id: \.id) { interest in
                chip(interest)
            }
        }
    }

    private func chip(_ interest: HighlighterOnboardingInterest) -> some View {
        let active = interest.selected
        return Button {
            store.toggleOnboardingInterest(id: interest.id)
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
        let onboarding = store.nmpState.onboarding
        guard onboarding.canFinish, !onboarding.isFinishing else { return }

        Task {
            await store.completeLogin()
            store.completeOnboarding()
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
