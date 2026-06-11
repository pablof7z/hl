import SwiftUI

/// Toolbar button that bundles the simple bookmark toggle (kind:10003)
/// with a long-press menu that lets the user add the same artifact to
/// any of their kind:30004 curation sets — including a "New
/// collection…" entry that prompts for a title and adds the artifact in
/// one shot.
///
/// Uses SwiftUI's `Menu(primaryAction:)` so a tap stays one-tap-fast and
/// long-press surfaces the Rust-owned curation choices.
struct BookmarkMenuButton: View {
    /// NIP-33 a-tag value — `"30023:<pubkey>:<d>"`.
    let articleAddress: String

    @Environment(HighlighterStore.self) private var app

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
                .foregroundStyle(isBookmarked ? Color.highlighterAccent : Color.highlighterInkStrong)
        } primaryAction: {
            Task { await app.toggleBookmark(articleAddress: articleAddress) }
        }
        .accessibilityLabel(isBookmarked ? "Remove bookmark" : "Bookmark article")
        .task(id: articleAddress) {
            app.openCurationMenu(articleAddress: articleAddress)
        }
        .onDisappear {
            app.closeCurationMenu()
        }
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
        let curationSets = app.curationMenu.articleAddress == articleAddress ? app.curationMenu.curationSets : []

        if app.curationMenu.isLoading {
            Text("Loading collections")
                .font(.footnote)
        } else if let errorMessage = app.curationMenu.errorMessage, !errorMessage.isEmpty {
            Text(errorMessage)
                .font(.footnote)
        } else if curationSets.isEmpty {
            // Header-only section so the menu still reads as the
            // collection picker before any sets exist.
            Text("No collections yet")
                .font(.footnote)
        } else {
            Section("Add to collection") {
                ForEach(curationSets, id: \.id) { set in
                    Button {
                        toggleInCuration(set)
                    } label: {
                        if set.articleAddresses.contains(articleAddress) {
                            Label(displayTitle(set), systemImage: "checkmark")
                        } else {
                            Text(displayTitle(set))
                        }
                    }
                }
            }
        }
    }

    private var isBookmarked: Bool {
        app.isBookmarked(articleAddress: articleAddress)
    }

    private func displayTitle(_ set: BookmarkSetRecord) -> String {
        if !set.title.isEmpty { return set.title }
        if !set.id.isEmpty { return set.id }
        return "Untitled"
    }

    // MARK: - Actions

    private func toggleInCuration(_ set: BookmarkSetRecord) {
        let nowMember = !set.articleAddresses.contains(articleAddress)
        app.setAddressInCurationSet(
            dTag: set.id,
            address: articleAddress,
            member: nowMember
        )
    }

    private func createAndAdd(title: String) {
        app.createCurationSetAndAdd(title: title, address: articleAddress)
    }
}

/// Tiny modal that prompts for a new collection title. Cancel discards;
/// Save invokes `onCreate(title)`. Title field is auto-focused so the
/// keyboard shows immediately.
struct NewCollectionSheet: View {
    var onCancel: () -> Void
    var onCreate: (String) -> Void

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
                        .disabled(trimmed.isEmpty)
                }
            }
            .onAppear { focused = true }
        }
    }

    private var trimmed: String {
        title.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private func commit() {
        guard !trimmed.isEmpty else { return }
        onCreate(trimmed)
    }
}
