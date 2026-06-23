import SwiftUI

/// Toolbar button that bundles the simple bookmark toggle (kind:10003)
/// with a long-press menu that lets the user add the same artifact to
/// any of their kind:30004 curation sets — including a "New
/// collection…" entry that prompts for a title and adds the artifact in
/// one shot.
///
/// Uses SwiftUI's `Menu(primaryAction:)` so a tap stays one-tap-fast and
/// long-press surfaces the curation choices.
///
/// The curation-set list is read directly from `kernel.bookmarks.myCurationSets`
/// (kernel sole writer after #1653). Membership is computed locally by checking
/// whether `articleAddress` is present in each set's `articleAddresses`.
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
            Image(systemName: isBookmarked ? "bookmark.fill" : "bookmark")
                .foregroundStyle(
                    isBookmarked ? Color.highlighterAccent : Color.highlighterInkStrong
                )
        } primaryAction: {
            Task { await app.toggleBookmark(articleAddress: articleAddress) }
        }
        .accessibilityLabel(isBookmarked ? "Remove bookmark" : "Bookmark article")
        .sheet(isPresented: $newCollectionPresented) {
            NewCollectionSheet(
                onCancel: { newCollectionPresented = false },
                onCreate: { title in
                    newCollectionPresented = false
                    kernel.app.dispatch(.createAndAddToSet(title: title, itemCoordinate: articleAddress))
                }
            )
            .presentationDetents([.medium])
        }
    }

    @ViewBuilder
    private var curationsSection: some View {
        let items = curationItems
        if items.isEmpty {
            // Header-only section so the menu still reads as the
            // collection picker before any sets exist.
            Text("No collections yet")
                .font(.footnote)
        } else {
            Section("Add to collection") {
                ForEach(items, id: \.id) { item in
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

    /// Compute curation menu items from the kernel snapshot. The kernel pushes
    /// fresh snapshots whenever sets are updated, so no explicit reload is needed.
    private var curationItems: [CurationMenuItem] {
        let sets = kernel.bookmarks?.myCurationSets ?? []
        return sets.map { set in
            let isMember = set.articleAddresses.contains(articleAddress)
            let displayTitle: String
            if let t = set.title, !t.isEmpty {
                displayTitle = t
            } else if !set.dTag.isEmpty {
                displayTitle = set.dTag
            } else {
                displayTitle = "Untitled"
            }
            return CurationMenuItem(
                id: set.dTag,
                title: displayTitle,
                isMember: isMember
            )
        }
    }

    private var isBookmarked: Bool {
        app.isBookmarked(articleAddress: articleAddress)
    }

    // MARK: - Actions

    private func toggleInCuration(_ item: CurationMenuItem) {
        // Resolve the EXACT set this row represents and build its full
        // coordinate (kind:pubkey:d) from that set — not the first set's pubkey
        // (#1653 NIT #8). Targets the right set even across kinds/authors.
        guard let set = kernel.bookmarks?.myCurationSets.first(where: { $0.dTag == item.id })
        else { return }
        let setCoordinate = "\(set.kind):\(set.pubkey):\(set.dTag)"
        if item.isMember {
            kernel.app.dispatch(.removeFromSet(setCoordinate: setCoordinate, itemCoordinate: articleAddress))
        } else {
            kernel.app.dispatch(.addToSet(setCoordinate: setCoordinate, itemCoordinate: articleAddress))
        }
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
                        .disabled(!canCreateCollection)
                }
            }
            .onAppear { focused = true }
        }
    }

    private var canCreateCollection: Bool {
        !title.trimmingCharacters(in: .whitespaces).isEmpty
    }

    private func commit() {
        let trimmed = title.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty else { return }
        onCreate(trimmed)
    }
}
