import SwiftUI

struct MainTabView: View {
    enum Section: Hashable {
        case highlights, reads, rooms, search
    }

    @State private var selection: Section = .highlights

    var body: some View {
        TabView(selection: $selection) {
            Tab("Highlights", systemImage: "text.quote", value: Section.highlights) {
                HighlightsTabView()
            }
            Tab("Reads", systemImage: "text.book.closed", value: Section.reads) {
                ReadsTabView()
            }
            Tab("Rooms", systemImage: "square.grid.2x2", value: Section.rooms) {
                RoomExplorerView()
            }
            // iOS 26 TabRole.search renders this as a distinct liquid-glass
            // capsule separated from the main tab bar.
            Tab(value: Section.search, role: .search) {
                SearchView()
            }
        }
    }
}
