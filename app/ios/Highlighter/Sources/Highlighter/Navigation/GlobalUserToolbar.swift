import SwiftUI

/// Adds the app-wide trailing toolbar chrome — settings gear + current-user
/// avatar — to any tab root. Each tab hosts its own copy so the items sit in
/// the NavigationStack context where toolbars actually render.
///
/// Apply inside the NavigationStack content. Tabs with their own trailing
/// items (e.g. Highlights' `+`) keep them; the settings/avatar pair appears
/// after them on the trailing edge.
private struct GlobalUserToolbar: ViewModifier {
    @Environment(HighlighterStore.self) private var appStore
    @State private var showSettings = false

    func body(content: Content) -> some View {
        content
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button { showSettings = true } label: {
                        Image(systemName: "gearshape")
                    }
                    .accessibilityLabel("Settings")
                }
                ToolbarItem(placement: .topBarTrailing) {
                    if let me = appStore.currentUser {
                        NavigationLink(value: ProfileDestination.pubkey(me.pubkey)) {
                            AuthorAvatar(
                                pubkey: me.pubkey,
                                pictureURL: appStore.currentUserProfile?.picture ?? "",
                                displayInitial: preferredInitial(for: me),
                                size: 30
                            )
                        }
                        .accessibilityLabel("Your profile")
                    }
                }
            }
            .sheet(isPresented: $showSettings) {
                SettingsView()
                    .environment(appStore)
            }
    }

    private func preferredInitial(for user: CurrentUser) -> String {
        if let profile = appStore.currentUserProfile {
            if let ch = profile.displayName.first { return String(ch) }
            if let ch = profile.name.first { return String(ch) }
        }
        return String(user.pubkey.prefix(1))
    }
}

extension View {
    /// Attach the app-wide settings + user-avatar toolbar to a tab's
    /// NavigationStack content.
    func globalUserToolbar() -> some View {
        modifier(GlobalUserToolbar())
    }
}
