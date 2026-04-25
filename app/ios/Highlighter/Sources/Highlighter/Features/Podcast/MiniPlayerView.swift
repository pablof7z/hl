import Kingfisher
import SwiftUI

struct MiniPlayerView: View {
    @Environment(HighlighterStore.self) private var app
    @Namespace private var heroNamespace
    @State private var playerSheetPresented = false

    private var player: PodcastPlayerStore { app.podcastPlayer }

    var body: some View {
        guard let artifact = player.currentArtifact else { return AnyView(EmptyView()) }
        return AnyView(capsule(artifact: artifact))
    }

    @ViewBuilder
    private func capsule(artifact: ArtifactRecord) -> some View {
        ZStack(alignment: .bottom) {
            HStack(spacing: 12) {
                artwork(artifact: artifact)
                    .matchedTransitionSource(id: "podcast-mini-art", in: heroNamespace)

                VStack(alignment: .leading, spacing: 2) {
                    let showTitle = artifact.preview.podcastShowTitle.isEmpty
                        ? artifact.preview.author
                        : artifact.preview.podcastShowTitle
                    if !showTitle.isEmpty {
                        Text(showTitle)
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                    }
                    Text(artifact.preview.title.isEmpty ? "Untitled episode" : artifact.preview.title)
                        .font(.subheadline.weight(.semibold))
                        .lineLimit(1)
                }
                .frame(maxWidth: .infinity, alignment: .leading)

                Button {
                    player.toggle()
                } label: {
                    ZStack {
                        if player.isBuffering {
                            ProgressView()
                                .controlSize(.small)
                                .frame(width: 36, height: 36)
                        } else {
                            Image(systemName: player.isPlaying ? "pause.fill" : "play.fill")
                                .font(.system(size: 16, weight: .semibold))
                                .frame(width: 36, height: 36)
                        }
                    }
                }
                .buttonStyle(.plain)

                Button {
                    player.clear()
                } label: {
                    Image(systemName: "xmark")
                        .font(.system(size: 12, weight: .semibold))
                        .frame(width: 28, height: 28)
                }
                .buttonStyle(.plain)
                .foregroundStyle(.secondary)
            }
            .padding(.horizontal, 12)
            .frame(height: 56)
            .overlay(alignment: .bottom) {
                progressBar
                    .padding(.horizontal, 4)
                    .padding(.bottom, 3)
            }
        }
        .glassEffect(.regular, in: .capsule)
        .contentShape(.capsule)
        .onTapGesture {
            playerSheetPresented = true
        }
        .contextMenu {
            Button {
                player.skip(by: 30)
            } label: {
                Label("Skip 30 seconds", systemImage: "goforward.30")
            }

            Button {
                let end = player.currentTime
                let start = max(0, end - 60)
                player.setClipStart(start)
                player.setClipEnd(end)
            } label: {
                Label("Mark clip", systemImage: "scissors")
            }

            Button(role: .destructive) {
                player.clear()
            } label: {
                Label("Stop", systemImage: "stop.fill")
            }
        }
        .sheet(isPresented: $playerSheetPresented) {
            PodcastListeningView(heroSourceID: "podcast-mini-art", heroNamespace: heroNamespace)
                .environment(app)
                .presentationDetents([.large])
                .navigationTransition(.zoom(sourceID: "podcast-mini-art", in: heroNamespace))
        }
    }

    @ViewBuilder
    private func artwork(artifact: ArtifactRecord) -> some View {
        let imageUrl = artifact.preview.image
        Group {
            if !imageUrl.isEmpty, let url = URL(string: imageUrl) {
                KFImage(url)
                    .placeholder { artworkPlaceholder }
                    .fade(duration: 0.15)
                    .resizable()
                    .scaledToFill()
            } else {
                artworkPlaceholder
            }
        }
        .frame(width: 40, height: 40)
        .clipShape(RoundedRectangle(cornerRadius: 6, style: .continuous))
    }

    private var artworkPlaceholder: some View {
        ZStack {
            Color.secondary.opacity(0.2)
            Image(systemName: "waveform")
                .font(.footnote)
                .foregroundStyle(.secondary)
        }
    }

    private var progressBar: some View {
        GeometryReader { geo in
            let fraction: Double = player.duration > 0
                ? min(1, max(0, player.currentTime / player.duration))
                : 0
            Rectangle()
                .fill(.primary.opacity(0.6))
                .frame(width: geo.size.width * fraction, height: 2)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
        .frame(height: 2)
    }
}
