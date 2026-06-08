import SwiftUI

/// Adds the app-wide trailing toolbar chrome — current-user avatar with
/// an account menu — to any tab root. Each tab hosts its own copy so the
/// items sit in the NavigationStack context where toolbars actually render.
///
/// Apply inside the NavigationStack content. The avatar Menu replaces the
/// old separate gear icon; settings, bookmarks, and profile are all
/// accessible from it.
private struct GlobalUserToolbar: ViewModifier {
    @Environment(HighlighterStore.self) private var appStore
    @State private var showSettings = false
    @State private var showBookmarks = false
    @State private var showLogoutConfirm = false
    @State private var navigateToProfile = false

    func body(content: Content) -> some View {
        content
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    if let me = appStore.currentUser {
                        Menu {
                            Button {
                                navigateToProfile = true
                            } label: {
                                Label("Profile", systemImage: "person.crop.circle")
                            }

                            Button {
                                showBookmarks = true
                            } label: {
                                Label("Bookmarks", systemImage: "bookmark")
                            }

                            Button {
                                showSettings = true
                            } label: {
                                Label("Settings", systemImage: "gearshape")
                            }

                            Divider()

                            Button(role: .destructive) {
                                showLogoutConfirm = true
                            } label: {
                                Label("Log Out", systemImage: "rectangle.portrait.and.arrow.right")
                            }
                        } label: {
                            let projection = profileDisplay(for: me)
                            AuthorAvatar(
                                pubkey: me.pubkey,
                                pictureURL: projection.pictureUrl,
                                displayInitial: projection.displayInitial,
                                size: 30
                            )
                        }
                        .accessibilityLabel("Account menu")
                    }
                }
            }
            .navigationDestination(isPresented: $navigateToProfile) {
                if let me = appStore.currentUser {
                    ProfileView(pubkey: me.pubkey)
                }
            }
            .sheet(isPresented: $showSettings) {
                SettingsView()
                    .environment(appStore)
            }
            .sheet(isPresented: $showBookmarks) {
                BookmarksView()
                    .environment(appStore)
            }
            .confirmationDialog(
                "Log out of Highlighter?",
                isPresented: $showLogoutConfirm,
                titleVisibility: .visible
            ) {
                Button("Log Out", role: .destructive) {
                    appStore.logout()
                }
                Button("Cancel", role: .cancel) {}
            } message: {
                Text("Make sure you've saved your private key. Without it, you can't sign back in.")
            }
    }

    private func profileDisplay(for user: CurrentUser) -> ProfileDisplayProjection {
        appStore.safeCore.projectProfileDisplay(
            input: ProfileDisplayProjectionInput(
                pubkey: user.pubkey,
                profile: appStore.currentUserProfile
            )
        )
    }
}

extension View {
    /// Attach the app-wide user account menu toolbar to a tab's
    /// NavigationStack content.
    func globalUserToolbar() -> some View {
        modifier(GlobalUserToolbar())
    }
}
