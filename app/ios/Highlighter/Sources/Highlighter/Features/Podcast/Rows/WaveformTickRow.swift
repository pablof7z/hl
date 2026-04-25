import SwiftUI

struct WaveformTickRow: View {
    let t: Double
    let isSpeech: Bool
    let state: TimelineRowState
    let onSeek: (Double) -> Void

    var body: some View {
        Button {
            onSeek(t)
        } label: {
            HStack(alignment: .center, spacing: 14) {
                Text(formatTimestamp(t))
                    .font(.system(.caption, design: .monospaced))
                    .foregroundStyle(Color.laneAudioInkMuted)
                    .frame(width: 48, alignment: .leading)

                HStack(spacing: 6) {
                    Canvas { context, size in
                        let stripeWidth: CGFloat = 6
                        let gapWidth: CGFloat = 3
                        let totalPattern = stripeWidth + gapWidth
                        var x: CGFloat = 0
                        while x < size.width {
                            let rect = CGRect(x: x, y: 0, width: stripeWidth, height: size.height)
                            context.fill(Path(rect), with: .color(Color.laneAudioRule))
                            x += totalPattern
                        }
                    }
                    .frame(maxWidth: .infinity)
                    .frame(height: 8)
                    .clipShape(RoundedRectangle(cornerRadius: 2))

                    Text(isSpeech ? "speech" : "silence")
                        .font(.system(.caption2, design: .monospaced))
                        .foregroundStyle(Color.laneAudioInkMuted)
                        .frame(width: 48, alignment: .trailing)
                }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 6)
            .frame(maxWidth: .infinity, alignment: .leading)
            .opacity(state == .future ? 0.55 : 1.0)
        }
        .buttonStyle(.plain)
    }
}

private func formatTimestamp(_ seconds: Double) -> String {
    guard seconds.isFinite, seconds >= 0 else { return "0:00" }
    let total = Int(seconds)
    let h = total / 3600
    let m = (total % 3600) / 60
    let s = total % 60
    if h > 0 { return String(format: "%d:%02d:%02d", h, m, s) }
    return String(format: "%d:%02d", m, s)
}
