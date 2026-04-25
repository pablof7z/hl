import SwiftUI

/// The Liquid Glass capsule that lives at the bottom of every reader.
/// At rest: avatar trio + count + accent send glyph.
/// When the parent reader is scrolled past the threshold: a small
/// trailing-corner pill with a single avatar + text-xs count.
///
/// Tapping it triggers `onTap` — the parent owns sheet presentation.
struct CommentsToolbar: View {
    let count: Int
    let recentCommenterPubkeys: [String]
    /// `true` shrinks the capsule into the trailing-corner pill state.
    let shrunk: Bool
    let onTap: () -> Void

    @Environment(HighlighterStore.self) private var app

    var body: some View {
        Button(action: onTap) {
            if shrunk {
                shrunkPill
            } else {
                fullCapsule
            }
        }
        .buttonStyle(.plain)
        .padding(.horizontal, shrunk ? 12 : 16)
        .padding(.bottom, 6)
        .frame(maxWidth: .infinity, alignment: shrunk ? .trailing : .center)
        .animation(.spring(response: 0.38, dampingFraction: 0.82), value: shrunk)
        .accessibilityLabel("\(count) comments")
        .accessibilityAddTraits(.isButton)
    }

    // MARK: - States

    private var fullCapsule: some View {
        HStack(spacing: 12) {
            avatarTrio(size: 24, overlap: 8)
            VStack(alignment: .leading, spacing: 1) {
                Text(countLabel)
                    .font(.system(size: 15, weight: .semibold, design: .rounded))
                    .foregroundStyle(Color.highlighterInkStrong)
                Text(captionLabel)
                    .font(.system(size: 11, weight: .medium))
                    .foregroundStyle(Color.highlighterInkMuted)
            }
            Spacer(minLength: 0)
            sendGlyph(size: 28)
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
        .frame(height: 56)
        .frame(maxWidth: 540)
        .glassCapsule(cornerRadius: 28)
        .contentShape(Capsule())
    }

    private var shrunkPill: some View {
        HStack(spacing: 6) {
            avatarTrio(size: 18, overlap: 6, max: 1)
            Text("\(count)")
                .font(.system(size: 12, weight: .semibold, design: .rounded))
                .foregroundStyle(Color.highlighterInkStrong)
                .monospacedDigit()
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 7)
        .frame(height: 32)
        .glassCapsule(cornerRadius: 16)
        .contentShape(Capsule())
    }

    // MARK: - Bits

    private func avatarTrio(size: CGFloat, overlap: CGFloat, max: Int = 3) -> some View {
        HStack(spacing: -overlap) {
            ForEach(Array(recentCommenterPubkeys.prefix(max).enumerated()), id: \.offset) { _, pubkey in
                AuthorAvatar(
                    pubkey: pubkey,
                    pictureURL: app.profileCache[pubkey]?.picture ?? "",
                    displayInitial: initial(for: pubkey),
                    size: size,
                    ringWidth: 1.5
                )
            }
        }
    }

    private func sendGlyph(size: CGFloat) -> some View {
        ZStack {
            Circle()
                .fill(Color.highlighterAccent)
            Image(systemName: "bubble.left.and.pencil")
                .font(.system(size: size * 0.42, weight: .semibold))
                .foregroundStyle(.white)
        }
        .frame(width: size, height: size)
    }

    private var countLabel: String {
        if count == 0 { return "Be the first" }
        if count == 1 { return "1 comment" }
        return "\(count) comments"
    }

    private var captionLabel: String {
        recentCommenterPubkeys.isEmpty
            ? "Start the thread"
            : "Tap to read & reply"
    }

    private func initial(for pubkey: String) -> String {
        let profile = app.profileCache[pubkey]
        if let dn = profile?.displayName, let c = dn.first { return String(c).uppercased() }
        if let n = profile?.name, let c = n.first { return String(c).uppercased() }
        return ""
    }
}

// MARK: - Liquid Glass capsule modifier

private struct GlassCapsule: ViewModifier {
    let cornerRadius: CGFloat

    func body(content: Content) -> some View {
        content
            .background(
                .regularMaterial,
                in: RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)
            )
            .overlay(
                RoundedRectangle(cornerRadius: cornerRadius, style: .continuous)
                    .strokeBorder(
                        LinearGradient(
                            colors: [
                                Color.white.opacity(0.42),
                                Color.white.opacity(0.06)
                            ],
                            startPoint: .top,
                            endPoint: .bottom
                        ),
                        lineWidth: 1
                    )
            )
            .shadow(color: Color.black.opacity(0.18), radius: 18, x: 0, y: 8)
    }
}

private extension View {
    func glassCapsule(cornerRadius: CGFloat) -> some View {
        modifier(GlassCapsule(cornerRadius: cornerRadius))
    }
}
