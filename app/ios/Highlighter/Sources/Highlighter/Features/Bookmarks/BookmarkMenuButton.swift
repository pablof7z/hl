import SwiftUI

/// Toolbar button that bundles the simple bookmark toggle (kind:10003)
/// with a long-press menu that lets the user add the same artifact to
/// any of their kind:30004 curation sets — including a "New
/// collection…" entry that prompts for a title and adds the artifact in
/// one shot.
///
/// Uses SwiftUI's `Menu(primaryAction:)` so a tap stays one-tap-fast and
/// long-press surfaces the curation choices. Loads curations lazily on
/// the first appear; refreshes after every membership change so the
/// checkmark state is always accurate without a full BookmarkStore.
struct BookmarkMenuButton: View {
    /// NIP-33 a-tag value — `"30023:<pubkey>:<d>"`.
    let articleAddress: String

    @Environment(HighlighterStore.self) private var app

    @State private var curationItems: [CurationMenuItem] = []
    @State private var newCollectionPresented: Bool = false
    @State private var errorMessage: String?

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
        .task { await loadCurations() }
        .sheet(isPresented: $newCollectionPresented) {
            NewCollectionSheet(
                onCancel: { newCollectionPresented = false },
                onCreate: { title in
                    newCollectionPresented = false
                    Task { await createAndAdd(title: title) }
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
                        Task { await toggleInCuration(item) }
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

    // MARK: - Actions

    private func loadCurations() async {
        let outcome = await app.safeCore.getCurationMenuItems(address: articleAddress)
        if outcome.error.isEmpty {
            curationItems = outcome.values
        }
    }

    private func toggleInCuration(_ item: CurationMenuItem) async {
        let outcome = await app.safeCore.toggleAddressInCurationSet(
            dTag: item.id,
            address: articleAddress
        )
        guard outcome.error.isEmpty else {
            errorMessage = "Couldn't update collection — \(outcome.error)"
            return
        }
        await loadCurations()
    }

    private func createAndAdd(title: String) async {
        let createOutcome = await app.safeCore.createCurationSet(title: title)
        guard createOutcome.error.isEmpty, let newSet = createOutcome.value else {
            errorMessage = "Couldn't create collection — \(createOutcome.error)"
            return
        }
        let updateOutcome = await app.safeCore.setAddressInCurationSet(
            dTag: newSet.id,
            address: articleAddress,
            member: true
        )
        guard updateOutcome.error.isEmpty else {
            errorMessage = "Couldn't create collection — \(updateOutcome.error)"
            return
        }
        await loadCurations()
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
        app.safeCore.projectCurationSetCreate(
            input: CurationSetCreateProjectionInput(title: title)
        )
    }

    private func commit() {
        let projection = createProjection
        guard projection.canCreate else { return }
        onCreate(projection.submitTitle)
    }
}
