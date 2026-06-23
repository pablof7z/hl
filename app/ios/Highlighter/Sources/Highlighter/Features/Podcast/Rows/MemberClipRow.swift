import SwiftUI

struct MemberClipRow: View {
    @Environment(HighlighterStore.self) private var app

    let highlight: HighlightRecord
    let rangeLabel: String
    let state: TimelineRowState
    let onSeek: (Double) -> Void

    private var isExpanded: Bool {
        app.podcastPlayer.expandedClipId == highlight.eventId
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // -- Collapsed header (always visible) --
            Button {
                if let start = highlight.clipStartSeconds {
                    onSeek(start)
                }
                withAnimation(.easeInOut(duration: 0.2)) {
                    if isExpanded {
                        app.podcastPlayer.expandedClipId = nil
                    } else {
                        app.podcastPlayer.expandedClipId = highlight.eventId
                    }
                }
            } label: {
                let author = authorDisplay

                HStack(alignment: .top, spacing: 14) {
                    Text(rangeLabel)
                        .font(.caption.weight(.medium).monospacedDigit())
                        .foregroundStyle(.secondary)
                        .frame(width: 48, alignment: .leading)

                    VStack(alignment: .leading, spacing: 8) {
                        HStack(alignment: .top, spacing: 10) {
                            AuthorAvatar(
                                pubkey: highlight.pubkey,
                                pictureURL: author.pictureUrl,
                                displayInitial: author.displayInitial,
                                size: 28
                            )

                            VStack(alignment: .leading, spacing: 2) {
                                Text(author.displayName)
                                    .font(.footnote.weight(.semibold))
                                    .foregroundStyle(.primary)
                                    .lineLimit(1)
                                Text(rangeLabel)
                                    .font(.caption2.monospacedDigit())
                                    .foregroundStyle(.secondary)
                            }

                            Spacer(minLength: 0)

                            Image(systemName: isExpanded ? "chevron.up" : "chevron.down")
                                .font(.caption2)
                                .foregroundStyle(.secondary)
                        }

                        if !highlight.quote.isEmpty {
                            HStack(alignment: .top, spacing: 8) {
                                Rectangle()
                                    .fill(Color.highlighterAccent)
                                    .frame(width: 2)
                                Text("\u{201C}\(highlight.quote)\u{201D}")
                                    .font(.system(.subheadline).italic())
                                    .foregroundStyle(.primary.opacity(0.9))
                                    .lineLimit(isExpanded ? nil : 3)
                                    .multilineTextAlignment(.leading)
                                    .fixedSize(horizontal: false, vertical: true)
                            }
                        }

                        if isExpanded && !highlight.note.isEmpty {
                            Text(highlight.note)
                                .font(.footnote)
                                .foregroundStyle(.secondary)
                                .fixedSize(horizontal: false, vertical: true)
                        }
                    }
                }
                .padding(.horizontal, 16)
                .padding(.vertical, 12)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(
                    state == .active
                        ? Color(.separator).opacity(0.3)
                        : Color.clear
                )
                .opacity(state == .future ? 0.55 : 1.0)
            }
            .buttonStyle(.plain)

            // -- Thread expansion --
            if isExpanded {
                ClipThreadView(clipEventId: highlight.eventId)
                    .transition(.opacity.combined(with: .move(edge: .top)))
            }
        }
        .task(id: highlight.pubkey) {
            await app.requestProfile(pubkeyHex: highlight.pubkey)
        }
        .onChange(of: isExpanded) { _, expanded in
            guard expanded else { return }
            let id = highlight.eventId
            guard app.podcastPlayer.comments[id] == nil else { return }
            Task {
                let scopeSnapshot = app.safeCore.getHighlightCommentScope(eventIdHex: id)
                guard scopeSnapshot.attach, let scope = scopeSnapshot.scope else {
                    app.podcastPlayer.comments[id] = []
                    return
                }
                let snapshot = await app.safeCore.getCommentThreadSnapshot(scope: scope, limit: 200)
                app.podcastPlayer.comments[id] = snapshot.error.trimmingCharacters(in: .whitespaces).isEmpty ? snapshot.records : []
            }
        }
    }

    private var authorDisplay: ProfileDisplayProjection {
        {
            let profile = app.profileSnapshots[highlight.pubkey]
            let name = (profile?.displayName ?? "").isEmpty
                ? ((profile?.name ?? "").isEmpty ? String(highlight.pubkey.prefix(10)) : profile!.name)
                : profile!.displayName
            return ProfileDisplayProjection(
                displayName: name,
                displayInitial: name.first.map { String($0).uppercased() } ?? "?",
                pictureUrl: profile?.picture ?? ""
            )
        }()
    }
}
