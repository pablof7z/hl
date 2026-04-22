import SwiftUI

/// Placeholder — Phase 4 will bring camera + OCR + book picker + publish.
struct CaptureTabView: View {
    var body: some View {
        NavigationStack {
            ContentUnavailableView(
                "Capture coming in Phase 4",
                systemImage: "camera.viewfinder",
                description: Text("Take a photo of a book page, OCR with Vision, pick excerpts, attach to a book from your communities, and publish.")
            )
            .navigationTitle("Capture")
        }
    }
}
