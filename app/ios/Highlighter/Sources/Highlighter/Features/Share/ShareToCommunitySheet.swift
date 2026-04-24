import SwiftUI

/// Identifiable payload for `.sheet(item:)` — callers construct one of these
/// from an `ArticleRecord` or `ArtifactRecord` and hand it to the sheet.
struct ShareToCommunityTarget: Identifiable {
    let id = UUID()
    let preview: ArtifactPreview
    let displayTitle: String
    let displaySubtitle: String
    let imageURL: URL?

    static func article(_ article: ArticleRecord) -> ShareToCommunityTarget {
        let preview = ArtifactPreviewBuilder.from(article: article)
        return ShareToCommunityTarget(
            preview: preview,
            displayTitle: article.title.isEmpty ? "Untitled" : article.title,
            displaySubtitle: article.summary,
            imageURL: article.image.isEmpty ? nil : URL(string: article.image)
        )
    }

    static func artifact(_ artifact: ArtifactRecord) -> ShareToCommunityTarget {
        let preview = ArtifactPreviewBuilder.from(artifact: artifact)
        return ShareToCommunityTarget(
            preview: preview,
            displayTitle: artifact.preview.title.isEmpty ? "Untitled" : artifact.preview.title,
            displaySubtitle: artifact.preview.description,
            imageURL: artifact.preview.image.isEmpty ? nil : URL(string: artifact.preview.image)
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
            .navigationTitle("Share to community")
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
                    .font(.system(.subheadline, design: .serif).weight(.semibold))
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
                AsyncImage(url: url) { phase in
                    switch phase {
                    case .success(let image):
                        image.resizable().scaledToFill()
                    default:
                        Color.highlighterRule.opacity(0.4)
                    }
                }
                .frame(width: 64, height: 64)
                .clipShape(RoundedRectangle(cornerRadius: 8))
            }
        }
    }

    // MARK: - Community row

    private func communityRow(_ community: CommunitySummary) -> some View {
        HStack(spacing: 12) {
            if let url = URL(string: community.picture), !community.picture.isEmpty {
                AsyncImage(url: url) { image in
                    image.resizable().scaledToFill()
                } placeholder: {
                    Color.highlighterRule.opacity(0.4)
                }
                .frame(width: 32, height: 32)
                .clipShape(RoundedRectangle(cornerRadius: 6))
            } else {
                Image(systemName: "square.grid.2x2")
                    .frame(width: 32, height: 32)
                    .foregroundStyle(Color.highlighterInkMuted)
            }

            Text(community.name.isEmpty ? community.id : community.name)
                .foregroundStyle(Color.highlighterInkStrong)

            Spacer()

            if publishingId == community.id {
                ProgressView()
            }
        }
    }

    // MARK: - Action

    private func publish(to groupId: String) {
        guard publishingId == nil else { return }
        publishingId = groupId
        let trimmedNote = note.trimmingCharacters(in: .whitespacesAndNewlines)
        Task {
            do {
                _ = try await app.safeCore.publishArtifact(
                    preview: target.preview,
                    groupId: groupId,
                    note: trimmedNote.isEmpty ? nil : trimmedNote
                )
                await MainActor.run { dismiss() }
            } catch {
                await MainActor.run {
                    publishingId = nil
                    errorMessage = error.localizedDescription
                }
            }
        }
    }
}
