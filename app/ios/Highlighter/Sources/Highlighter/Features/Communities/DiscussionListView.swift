import SwiftUI

/// Discussions tab content for a room. Uses its own `DiscussionStore`.
struct DiscussionListView: View {
    let groupId: String
    @Binding var composerPresented: Bool

    @Environment(HighlighterStore.self) private var app
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
                List(store.discussions, id: \.eventId) { d in
                    DiscussionRow(discussion: d)
                }
                .listStyle(.plain)
            }
        }
        .task {
            await store.start(groupId: groupId, core: app.safeCore, bridge: app.eventBridge)
        }
        .onDisappear { store.stop() }
        .sheet(isPresented: $composerPresented) {
            DiscussionComposerView(groupId: groupId) { discussion in
                store.apply(discussion: discussion)
            }
        }
    }
}

private struct DiscussionRow: View {
    let discussion: DiscussionRecord

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(discussion.title)
                .font(.body.weight(.medium))
            if !discussion.body.isEmpty {
                Text(discussion.body)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(2)
            }
            if let attachment = discussion.attachment, !attachment.url.isEmpty {
                Text(attachment.url)
                    .font(.caption2)
                    .foregroundStyle(.tertiary)
                    .lineLimit(1)
            }
        }
        .padding(.vertical, 4)
    }
}
