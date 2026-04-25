import SwiftUI

/// NIP-22 comment thread anchored to an artifact, rendered inside a lane
/// below the highlight module. Uses the standard ink palette (no per-lane
/// surface variants).
struct LaneCommentsSection: View {
    @Environment(HighlighterStore.self) private var app

    let comments: [CommentRecord]

    var body: some View {
        if comments.isEmpty {
            EmptyView()
        } else {
            VStack(alignment: .leading, spacing: 12) {
                ForEach(Array(comments.prefix(4)), id: \.eventId) { comment in
                    row(for: comment)
                    Divider().background(Color.highlighterRule)
                }
                if comments.count > 4 {
                    Text("+ \(comments.count - 4) more")
                        .font(.caption)
                        .foregroundStyle(Color.highlighterInkMuted)
                }
            }
            .padding(.horizontal, 24)
            .padding(.top, 8)
        }
    }

    private func row(for comment: CommentRecord) -> some View {
        HStack(alignment: .top, spacing: 10) {
            AuthorAvatar(
                pubkey: comment.pubkey,
                pictureURL: app.profileCache[comment.pubkey]?.picture ?? "",
                displayInitial: initial(for: comment.pubkey),
                size: 26
            )

            VStack(alignment: .leading, spacing: 4) {
                HStack(spacing: 6) {
                    Text(name(for: comment.pubkey))
                        .font(.footnote.weight(.semibold))
                        .foregroundStyle(Color.highlighterInkStrong)
                        .lineLimit(1)
                    if let t = relative(comment.createdAt) {
                        Text("·").foregroundStyle(Color.highlighterInkMuted)
                        Text(t)
                            .font(.footnote)
                            .foregroundStyle(Color.highlighterInkMuted)
                            .lineLimit(1)
                    }
                    Spacer(minLength: 0)
                }
                NostrRichText(content: comment.body, font: .subheadline, ink: Color.highlighterInkStrong)
                    .multilineTextAlignment(.leading)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .task(id: comment.pubkey) {
            await app.requestProfile(pubkeyHex: comment.pubkey)
        }
    }

    private func name(for pubkey: String) -> String {
        let profile = app.profileCache[pubkey]
        if let dn = profile?.displayName, !dn.isEmpty { return dn }
        if let n = profile?.name, !n.isEmpty { return n }
        return String(pubkey.prefix(10))
    }

    private func initial(for pubkey: String) -> String {
        name(for: pubkey).first.map { String($0).uppercased() } ?? ""
    }

    private func relative(_ seconds: UInt64?) -> String? {
        guard let s = seconds, s > 0 else { return nil }
        let date = Date(timeIntervalSince1970: TimeInterval(s))
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        formatter.dateTimeStyle = .numeric
        return formatter.localizedString(for: date, relativeTo: Date())
    }
}
