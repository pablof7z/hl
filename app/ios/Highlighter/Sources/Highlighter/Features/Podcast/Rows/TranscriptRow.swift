import SwiftUI

struct TranscriptRow: View {
    let segment: TranscriptSegment
    let timestampLabel: String
    let state: TimelineRowState
    let onSeek: (Double) -> Void

    var body: some View {
        Button {
            onSeek(segment.start)
        } label: {
            HStack(alignment: .top, spacing: 14) {
                Text(timestampLabel)
                    .font(.caption.monospacedDigit())
                    .foregroundStyle(.secondary)
                    .frame(width: 48, alignment: .leading)
                    .padding(.top, 1)

                VStack(alignment: .leading, spacing: 4) {
                    if !segment.speaker.isEmpty {
                        Text(segment.speaker)
                            .font(.caption2.weight(.semibold))
                            .foregroundStyle(.secondary)
                    }
                    Text(segment.text)
                        .font(.system(size: 15))
                        .lineSpacing(15 * 0.55)
                        .foregroundStyle(.primary)
                        .multilineTextAlignment(.leading)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 8)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(
                state == .active
                    ? Color(.separator).opacity(0.3)
                    : Color.clear
            )
            .opacity(state == .future ? 0.55 : 1.0)
        }
        .buttonStyle(.plain)
    }
}
