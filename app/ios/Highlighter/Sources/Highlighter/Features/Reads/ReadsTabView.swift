import SwiftUI

struct ReadsTabView: View {
    @Environment(HighlighterStore.self) private var app
    @State private var store: ReadsStore?
    @State private var shareTarget: ShareToCommunityTarget?

    var body: some View {
        NavigationStack {
            Group {
                if let store {
                    content(store: store)
                } else {
                    ProgressView()
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                }
            }
            .navigationTitle("Reads")
            .navigationDestination(for: ArticleReaderTarget.self) { target in
                ArticleReaderView(target: target)
            }
            .navigationDestination(for: ProfileDestination.self) { destination in
                if case .pubkey(let pk) = destination {
                    ProfileView(pubkey: pk)
                }
            }
            .globalUserToolbar()
        }
        .sheet(item: $shareTarget) { target in
            ShareToCommunitySheet(target: target)
                .presentationDetents([.medium, .large])
        }
        .task {
            guard store == nil else { return }
            let s = ReadsStore(safeCore: app.safeCore, eventBridge: app.eventBridge)
            store = s
            await s.start()
        }
        .onDisappear {
            store?.stop()
        }
    }

    @ViewBuilder
    private func content(store: ReadsStore) -> some View {
        if store.isLoadingInitial {
            ProgressView()
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else if store.items.isEmpty {
            emptyState
        } else {
            feedList(store: store)
        }
    }

    private func feedList(store: ReadsStore) -> some View {
        ScrollView {
            LazyVStack(spacing: 0) {
                ForEach(Array(store.items.enumerated()), id: \.element.article.eventId) { index, item in
                    NavigationLink(value: ArticleReaderTarget(
                        pubkey: item.article.pubkey,
                        dTag: item.article.identifier,
                        seed: item.article
                    )) {
                        ReadingFeedCardView(item: item)
                    }
                    .buttonStyle(.plain)
                    .contextMenu {
                        Button {
                            shareTarget = .article(item.article)
                        } label: {
                            Label("Share to community", systemImage: "square.and.arrow.up")
                        }
                    }

                    if index < store.items.count - 1 {
                        Rectangle()
                            .fill(Color.highlighterRule)
                            .frame(height: 1)
                    }
                }
            }
            .padding(.horizontal, 20)
        }
        .background(Color.highlighterPaper.ignoresSafeArea())
    }

    private var emptyState: some View {
        VStack(spacing: 16) {
            Image(systemName: "book.pages")
                .font(.system(size: 48))
                .foregroundStyle(Color.highlighterInkMuted.opacity(0.7))
            Text("Nothing yet")
                .font(.system(.headline, design: .serif))
                .foregroundStyle(Color.highlighterInkStrong)
            Text("Articles written or read by people you follow will appear here.")
                .font(.subheadline)
                .foregroundStyle(Color.highlighterInkMuted)
                .multilineTextAlignment(.center)
                .padding(.horizontal, 32)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Color.highlighterPaper.ignoresSafeArea())
    }
}
