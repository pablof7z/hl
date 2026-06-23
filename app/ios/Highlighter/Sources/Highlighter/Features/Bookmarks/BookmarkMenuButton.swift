import SwiftUI

/// Toolbar button that bundles the simple bookmark toggle (kind:10003)
/// with a long-press menu that lets the user add the same artifact to
/// any of their kind:30004 curation sets — including a "New
/// collection…" entry that prompts for a title and adds the artifact in
/// one shot.
///
/// Uses SwiftUI's `Menu(primaryAction:)` so a tap stays one-tap-fast and
/// long-press surfaces the curation choices. Curation state is derived
/// live from the kernel's BookmarksSnapshot; write operations dispatch
/// kernel actions (addToSet / removeFromSet / createAndAddToSet).
struct BookmarkMenuButton: View {
    /// NIP-33 a-tag value — `"30023:<pubkey>:<d>"`.
    let articleAddress: String

    @Environment(HighlighterStore.self) private var app
    @Environment(HighlighterAppKernel.self) private var kernel

    @State private var newCollectionPresented: Bool = false

    var body: some View {
        Menu {
            curationsSection
            Divider()
            Button {
                newCollectionPresented = true
            } label: {
                Label("New collection…", systemImage: "plus")
            }
        } label: {
            Image(systemName: bookmarkChrome.toolbarSystemImage)
                .foregroundStyle(
                    bookmarkChrome.usesAccentColor ? Color.highlighterAccent : Color.highlighterInkStrong
                )
        } primaryAction: {
            Task { await app.toggleBookmark(articleAddress: articleAddress) }
        }
        .accessibilityLabel(bookmarkChrome.accessibilityLabel)
        .sheet(isPresented: $newCollectionPresented) {
            NewCollectionSheet(
                onCancel: { newCollectionPresented = false },
                onCreate: { title in
                    newCollectionPresented = false
                    createAndAdd(title: title)
                }
            )
            .presentationDetents([.medium])
        }
    }

    @ViewBuilder
    private var curationsSection: some View {
        if curationItems.isEmpty {
            // Header-only section so the menu still reads as the
            // collection picker before any sets exist.
            Text("No collections yet")
                .font(.footnote)
        } else {
            Section("Add to collection") {
                ForEach(curationItems, id: \.id) { item in
                    Button {
                        toggleInCuration(item)
                    } label: {
                        if item.isMember {
                            Label(item.title, systemImage: "checkmark")
                        } else {
                            Text(item.title)
                        }
                    }
                }
            }
        }
    }

    private var isBookmarked: Bool {
        app.isBookmarked(articleAddress: articleAddress)
    }

    private var bookmarkChrome: ArticleBookmarkChromeProjection {
        if isBookmarked {
            ArticleBookmarkChromeProjection(
                toolbarSystemImage: "bookmark.fill",
                usesAccentColor: true,
                accessibilityLabel: "Remove bookmark",
                swipeTitle: "Remove",
                menuTitle: "Remove bookmark",
                actionSystemImage: "bookmark.slash"
            )
        } else {
            ArticleBookmarkChromeProjection(
                toolbarSystemImage: "bookmark",
                usesAccentColor: false,
                accessibilityLabel: "Bookmark article",
                swipeTitle: "Bookmark",
                menuTitle: "Bookmark",
                actionSystemImage: "bookmark"
            )
        }
    }

    /// Derives the curation-set menu items from the live kernel snapshot.
    /// SwiftUI re-evaluates this whenever `kernel.bookmarks` changes
    /// because `HighlighterAppKernel` is `@Observable`.
    private var curationItems: [CurationMenuItem] {
        guard let sets = kernel.bookmarks?.myCurationSets else { return [] }
        return sets.map { set in
            CurationMenuItem(
                id: "30004:\(set.pubkey):\(set.dTag)",
                title: set.title ?? "",
                isMember: set.articleAddresses.contains(articleAddress)
            )
        }
    }

    // MARK: - Actions

    private func toggleInCuration(_ item: CurationMenuItem) {
        // item.id is the full NIP-33 coordinate "30004:<pubkey>:<d>"
        if item.isMember {
            kernel.app.dispatch(.removeFromSet(setCoordinate: item.id, itemCoordinate: articleAddress))
        } else {
            kernel.app.dispatch(.addToSet(setCoordinate: item.id, itemCoordinate: articleAddress))
        }
    }

    private func createAndAdd(title: String) {
        kernel.app.dispatch(.createAndAddToSet(title: title, itemCoordinate: articleAddress))
    }
}

/// Tiny modal that prompts for a new collection title. Cancel discards;
/// Save invokes `onCreate(title)`. Title field is auto-focused so the
/// keyboard shows immediately.
struct NewCollectionSheet: View {
    var onCancel: () -> Void
    var onCreate: (String) -> Void

    @Environment(HighlighterStore.self) private var app
    @State private var title: String = ""
    @FocusState private var focused: Bool

    var body: some View {
        NavigationStack {
            VStack(alignment: .leading, spacing: 16) {
                Text("Group articles, podcasts, or notes you want to share or revisit. You can add to it from any artifact.")
                    .font(.footnote)
                    .foregroundStyle(Color.highlighterInkMuted)

                TextField("Collection name", text: $title)
                    .focused($focused)
                    .textFieldStyle(.roundedBorder)
                    .submitLabel(.done)
                    .onSubmit { commit() }

                Spacer(minLength: 0)
            }
            .padding(.horizontal, 20)
            .padding(.top, 16)
            .navigationTitle("New collection")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarLeading) {
                    Button("Cancel", action: onCancel)
                }
                ToolbarItem(placement: .topBarTrailing) {
                    Button("Save") { commit() }
                        .fontWeight(.semibold)
                        .disabled(!createProjection.canCreate)
                }
            }
            .onAppear { focused = true }
        }
    }

    private var createProjection: CurationSetCreateProjection {
        let submitTitle = title.trimmingCharacters(in: .whitespaces)
        return CurationSetCreateProjection(submitTitle: submitTitle, canCreate: !submitTitle.isEmpty)
    }

    private func commit() {
        let projection = createProjection
        guard projection.canCreate else { return }
        onCreate(projection.submitTitle)
    }
}
