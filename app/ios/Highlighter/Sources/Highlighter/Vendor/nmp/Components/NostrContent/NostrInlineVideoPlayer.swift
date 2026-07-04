import AVKit
import SwiftUI

/// Inline video playback for a `.video` media node, wrapping `AVKit.VideoPlayer`.
///
/// The `AVPlayer` is created exactly once per view identity, in `@State`'s
/// `initialValue` — NOT inline in `body`. `mediaGroup(urls:kind:)` in
/// `NostrContentView` runs on every SwiftUI re-render of the containing note
/// (scrolling, unrelated state changes, live timestamp refresh, etc.); a
/// player constructed directly in `body` is torn down and rebuilt — with a
/// full `AVPlayerViewController` KVO observer-registration churn — on every
/// single one of those re-renders, not just when the video URL actually
/// changed. Left unfixed, this saturates the main thread and can make the
/// app unresponsive for minutes on a feed containing video content (observed
/// in a sibling NMP app).
///
/// Second, independent guard: once `AVPlayerItem.status` reaches `.failed`
/// for this URL, playback stops being retried — `failed` latches and a
/// static fallback renders instead. Without this, an unloadable video URL
/// (dead link, wrong format, network issue) can have its `AVURLAsset`
/// recreated in a tight retry loop indefinitely by AVFoundation. This guard
/// bounds the cost to one failed load per real video regardless of how often
/// the containing view gets recreated.
struct NostrInlineVideoPlayer: View {
    let url: URL
    @State private var player: AVPlayer
    @State private var failed = false

    init(url: URL) {
        self.url = url
        _player = State(initialValue: AVPlayer(url: url))
    }

    var body: some View {
        Group {
            if failed {
                fallback
            } else {
                VideoPlayer(player: player)
                    .aspectRatio(16.0 / 9.0, contentMode: .fit)
                    .clipShape(RoundedRectangle(cornerRadius: 10))
            }
        }
        .onChange(of: url) { _, newUrl in
            failed = false
            player = AVPlayer(url: newUrl)
        }
        .task(id: url) {
            guard !failed, let item = player.currentItem else { return }
            for await status in item.publisher(for: \.status).values {
                if status == .failed {
                    player.pause()
                    failed = true
                    return
                }
                if status == .readyToPlay {
                    return
                }
            }
        }
    }

    private var fallback: some View {
        ZStack {
            RoundedRectangle(cornerRadius: 10)
                .fill(Color.gray.opacity(0.15))
            Image(systemName: "video.slash")
                .foregroundStyle(.secondary)
        }
        .aspectRatio(16.0 / 9.0, contentMode: .fit)
    }
}
