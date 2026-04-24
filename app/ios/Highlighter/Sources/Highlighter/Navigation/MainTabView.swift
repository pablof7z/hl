import SwiftUI

struct MainTabView: View {
    enum Section: Hashable {
        case highlights, reads, rooms, capture
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
            Tab("Capture", systemImage: "camera.viewfinder", value: Section.capture) {
                CaptureTabView()
            }
        }
    }
}
