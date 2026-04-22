import SwiftUI

struct MainTabView: View {
    enum Section: Hashable {
        case communities, capture
    }

    @State private var selection: Section = .communities

    var body: some View {
        TabView(selection: $selection) {
            Tab("Communities", systemImage: "square.grid.2x2", value: Section.communities) {
                CommunitiesTabView()
            }
            Tab("Capture", systemImage: "camera.viewfinder", value: Section.capture) {
                CaptureTabView()
            }
        }
    }
}
