import Kingfisher
import SwiftUI

/// Identifiable payload for `.sheet(item:)`.
///
/// Two flavours:
/// - `.artifact` / `.article` → publish a kind:11 share of the underlying
///   article/book/podcast into the target room (existing flow). Uses the
///   `preview` payload to construct the kind:11 event.
/// - `.highlight` → publish a kind:16 generic repost referencing the
///   selected highlight directly. The repost carries `["e", id]`,
///   `["k", "9802"]`, `["p", author]`, `["h", target_group_id]`.
struct ShareToCommunityTarget: Identifiable {
    let id = UUID()
    let payload: Payload
    let displayTitle: String
    let displaySubtitle: String
    let imageURL: URL?

    enum Payload {
        /// Share the source artifact/article via kind:11.
        case artifactShare(preview: ArtifactPreview)
        /// Re-share an existing highlight via kind:16.
        case highlightRepost(eventId: String, authorPubkeyHex: String, relayHint: String)
    }

    static func article(_ article: ArticleRecord, core: SafeHighlighterCore) -> ShareToCommunityTarget {
        let projection = core.projectShareArticleTarget(
            input: ShareArticleTargetProjectionInput(article: article)
        )
        return ShareToCommunityTarget(
            payload: .artifactShare(preview: projection.preview),
            displayTitle: projection.displayTitle,
            displaySubtitle: projection.displaySubtitle,
            imageURL: projection.imageUrl.flatMap { URL(string: $0) }
        )
    }

    static func artifact(_ artifact: ArtifactRecord, core: SafeHighlighterCore) -> ShareToCommunityTarget {
        let projection = core.projectShareArtifactTarget(
            input: ShareArtifactTargetProjectionInput(artifact: artifact)
        )
        return ShareToCommunityTarget(
            payload: .artifactShare(preview: projection.preview),
            displayTitle: projection.displayTitle,
            displaySubtitle: projection.displaySubtitle,
            imageURL: projection.imageUrl.flatMap { URL(string: $0) }
        )
    }

    /// Share the highlight quote itself (not the source artifact). The
    /// repost references the kind:9802 highlight event by id, so anyone
    /// in the room sees the friend's quote with full attribution.
    static func highlight(
        _ highlight: HighlightRecord,
        relayHint: String = "",
        core: SafeHighlighterCore
    ) -> ShareToCommunityTarget {
        let projection = core.projectShareHighlightTarget(
            input: ShareHighlightTargetProjectionInput(
                highlight: highlight,
                relayHint: relayHint
            )
        )
        return ShareToCommunityTarget(
            payload: .highlightRepost(
                eventId: projection.eventId,
                authorPubkeyHex: projection.authorPubkeyHex,
                relayHint: projection.relayHint
            ),
            displayTitle: projection.displayTitle,
            displaySubtitle: projection.displaySubtitle,
            imageURL: projection.imageUrl.flatMap { URL(string: $0) }
        )
    }
}

/// Sheet that lets the user pick which community to publish an article / re-share
/// to, with an optional note.
struct ShareToCommunitySheet: View {
    @Environment(HighlighterStore.self) private var app
    @Environment(\.dismiss) private var dismiss

    let target: ShareToCommunityTarget

    @State private var note: String = ""
    @State private var publishingId: String?
    @State private var errorMessage: String?

    var body: some View {
        NavigationStack {
            List {
                Section {
                    headerCard
                        .listRowInsets(EdgeInsets(top: 12, leading: 16, bottom: 12, trailing: 16))
                }

                Section("Note (optional)") {
                    TextField("What caught your attention?", text: $note, axis: .vertical)
                        .lineLimit(2...6)
                }

                Section("Share to") {
                    if app.joinedCommunities.isEmpty {
                        Text("You haven't joined any communities yet.")
                            .foregroundStyle(Color.highlighterInkMuted)
                    } else {
                        ForEach(app.joinedCommunities, id: \.id) { community in
                            Button {
                                publish(to: community.id)
                            } label: {
                                communityRow(community)
                            }
                            .disabled(publishingId != nil)
                        }
                    }
                }
            }
            .navigationTitle(navigationTitle)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                        .disabled(publishingId != nil)
                }
            }
            .alert("Couldn't share", isPresented: Binding(
                get: { errorMessage != nil },
                set: { if !$0 { errorMessage = nil } }
            )) {
                Button("OK", role: .cancel) { errorMessage = nil }
            } message: {
                Text(errorMessage ?? "")
            }
        }
    }

    // MARK: - Header card

    private var headerCard: some View {
        HStack(alignment: .top, spacing: 12) {
            VStack(alignment: .leading, spacing: 6) {
                Text(target.displayTitle)
                    .font(.subheadline.weight(.semibold))
                    .foregroundStyle(Color.highlighterInkStrong)
                    .lineLimit(3)
                if !target.displaySubtitle.isEmpty {
                    Text(target.displaySubtitle)
                        .font(.caption)
                        .foregroundStyle(Color.highlighterInkMuted)
                        .lineLimit(2)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)

            if let url = target.imageURL {
                KFImage(url)
                    .placeholder { Color.highlighterRule.opacity(0.4) }
                    .fade(duration: 0.15)
                    .resizable()
                    .scaledToFill()
                    .frame(width: 64, height: 64)
                    .clipShape(RoundedRectangle(cornerRadius: 8))
            }
        }
    }

    // MARK: - Community row

    private func communityRow(_ community: CommunitySummary) -> some View {
        let projection = app.safeCore.projectCommunityRow(
            input: CommunityRowProjectionInput(community: community)
        )

        return HStack(spacing: 12) {
            if let picture = projection.pictureUrl, let url = URL(string: picture) {
                KFImage(url)
                    .placeholder { Color.highlighterRule.opacity(0.4) }
                    .fade(duration: 0.15)
                    .resizable()
                    .scaledToFill()
                    .frame(width: 32, height: 32)
                    .clipShape(RoundedRectangle(cornerRadius: 6))
            } else {
                Image(systemName: "square.grid.2x2")
                    .frame(width: 32, height: 32)
                    .foregroundStyle(Color.highlighterInkMuted)
            }

            Text(projection.displayName)
                .foregroundStyle(Color.highlighterInkStrong)

            Spacer()

            if publishingId == community.id {
                ProgressView()
            }
        }
    }

    // MARK: - Action

    private var navigationTitle: String {
        switch target.payload {
        case .artifactShare: return "Share to community"
        case .highlightRepost: return "Share highlight"
        }
    }

    private func publish(to groupId: String) {
        guard publishingId == nil else { return }
        publishingId = groupId
        let rawNote = note
        Task {
            let result: ShareToCommunityPublishResultProjection
            switch target.payload {
            case .artifactShare(let preview):
                let outcome = await app.safeCore.publishArtifact(
                    preview: preview,
                    groupId: groupId,
                    note: rawNote
                )
                result = app.safeCore.projectShareToCommunityPublishResult(
                    input: ShareToCommunityPublishResultInput(error: outcome.error)
                )
            case .highlightRepost(let eventId, let authorPubkey, let relayHint):
                let outcome = await app.safeCore.shareHighlightToRoom(
                    highlightId: eventId,
                    highlightAuthorPubkeyHex: authorPubkey,
                    highlightRelayUrl: relayHint,
                    targetGroupId: groupId
                )
                result = app.safeCore.projectShareToCommunityPublishResult(
                    input: ShareToCommunityPublishResultInput(error: outcome.error)
                )
            }
            if result.didPublish {
                await MainActor.run {
                    UINotificationFeedbackGenerator().notificationOccurred(.success)
                    dismiss()
                }
            } else {
                await MainActor.run {
                    publishingId = nil
                    errorMessage = result.errorMessage
                }
            }
        }
    }
}
