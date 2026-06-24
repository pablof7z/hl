import SwiftUI

/// Discussions tab content for a room. Uses its own `DiscussionStore`.
struct DiscussionListView: View {
    let groupId: String
    @Binding var composerPresented: Bool

    @Environment(HighlighterStore.self) private var app
    @Environment(HighlighterAppKernel.self) private var kernel
    @State private var store = DiscussionStore()

    var body: some View {
        Group {
            if store.isLoading && store.discussions.isEmpty {
                ProgressView().controlSize(.large)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else if store.discussions.isEmpty {
                ContentUnavailableView(
                    "No discussions yet",
                    systemImage: "bubble.left.and.bubble.right",
                    description: Text("Start one to propose a new read, ask a question, or share thoughts.")
                )
            } else {
                ScrollView {
                    LazyVStack(spacing: 0) {
                        ForEach(store.discussions, id: \.eventId) { d in
                            NavigationLink(value: d) {
                                DiscussionRowView(discussion: d)
                            }
                            .buttonStyle(.plain)
                            Divider()
                                .background(Color.highlighterRule.opacity(0.4))
                                .padding(.leading, 68)
                        }
                    }
                    .padding(.horizontal, 20)
                }
                .background(Color.highlighterPaper.ignoresSafeArea())
            }
        }
        .task {
            await store.start(groupId: groupId, kernel: kernel)
        }
        .onChange(of: kernel.roomDiscussions[groupId]) { _, _ in
            store.applyKernelSnapshot()
        }
        .onDisappear { store.stop() }
        .sheet(isPresented: $composerPresented) {
            DiscussionComposerView(groupId: groupId) {
                Task { await store.reloadFromCache() }
            }
        }
    }
}

private struct DiscussionRowView: View {
    let discussion: DiscussionRecord

    @Environment(HighlighterStore.self) private var app

    var body: some View {
        let author = authorDisplay

        HStack(alignment: .top, spacing: 12) {
            AuthorAvatar(
                pubkey: discussion.pubkey,
                pictureURL: author.pictureUrl,
                displayInitial: author.displayInitial,
                size: 36
            )

            VStack(alignment: .leading, spacing: 4) {
                Text(discussion.title)
                    .font(.body.weight(.semibold))
                    .foregroundStyle(Color.highlighterInkStrong)
                    .lineLimit(2)
                    .multilineTextAlignment(.leading)

                if !discussion.body.isEmpty {
                    Text(discussion.body)
                        .font(.subheadline)
                        .foregroundStyle(Color.highlighterInkMuted)
                        .lineLimit(3)
                        .multilineTextAlignment(.leading)
                }

                if let attachment = discussion.attachment {
                    attachmentChip(attachment)
                }

                HStack(spacing: 4) {
                    Text(author.displayName)
                        .font(.caption.weight(.medium))
                        .foregroundStyle(Color.highlighterInkMuted)
                    if let ts = discussion.createdAt, ts > 0 {
                        Text("·")
                            .font(.caption)
                            .foregroundStyle(Color.highlighterInkMuted)
                        Text(relativeTime(ts))
                            .font(.caption)
                            .foregroundStyle(Color.highlighterInkMuted)
                    }
                }
                .padding(.top, 2)
            }

            Spacer(minLength: 0)
        }
        .padding(.vertical, 14)
        .task(id: discussion.pubkey) {
            await app.requestProfile(pubkeyHex: discussion.pubkey)
        }
    }

    private var authorDisplay: ProfileDisplayProjection {
        let profile = app.profileSnapshots[discussion.pubkey]
        let name: String
        if let p = profile, !p.displayName.isEmpty {
            name = p.displayName
        } else if let p = profile, !p.name.isEmpty {
            name = p.name
        } else {
            name = String(discussion.pubkey.prefix(8))
        }
        let displayInitial: String
        if let p = profile, !p.displayName.isEmpty {
            displayInitial = String(p.displayName.prefix(1))
        } else if let p = profile, !p.name.isEmpty {
            displayInitial = String(p.name.prefix(1))
        } else {
            displayInitial = String(discussion.pubkey.prefix(1))
        }
        return ProfileDisplayProjection(
            displayName: name,
            displayInitial: displayInitial,
            pictureUrl: profile?.picture ?? ""
        )
    }

    private func relativeTime(_ timestamp: UInt64) -> String {
        let date = Date(timeIntervalSince1970: TimeInterval(timestamp))
        let formatter = RelativeDateTimeFormatter()
        formatter.unitsStyle = .abbreviated
        return formatter.localizedString(for: date, relativeTo: Date())
    }

    @ViewBuilder
    private func attachmentChip(_ a: DiscussionAttachment) -> some View {
        let label: String? = !a.title.isEmpty ? a.title : (!a.url.isEmpty ? a.url : nil)
        if let label {
            HStack(spacing: 5) {
                Image(systemName: "link")
                    .font(.caption2.weight(.medium))
                    .foregroundStyle(Color.highlighterAccent)
                Text(label)
                    .font(.caption)
                    .foregroundStyle(Color.highlighterAccent)
                    .lineLimit(1)
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 4)
            .background(Color.highlighterAccent.opacity(0.08), in: RoundedRectangle(cornerRadius: 6, style: .continuous))
        }
    }
}
