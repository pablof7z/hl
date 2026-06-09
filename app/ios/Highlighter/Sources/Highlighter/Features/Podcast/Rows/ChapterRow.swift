import SwiftUI

struct ChapterRow: View {
    let t: Double
    let timestampLabel: String
    let title: String
    let state: TimelineRowState
    let onSeek: (Double) -> Void

    var body: some View {
        Button {
            onSeek(t)
        } label: {
            HStack(alignment: .center, spacing: 14) {
                Text(timestampLabel)
                    .font(.caption.weight(.semibold).monospacedDigit())
                    .foregroundStyle(.secondary)
                    .frame(width: 48, alignment: .leading)

                VStack(alignment: .leading, spacing: 0) {
                    Rectangle()
                        .fill(Color(.separator))
                        .frame(maxWidth: .infinity, maxHeight: 2)
                    Text(title)
                        .font(.caption.weight(.semibold))
                        .foregroundStyle(state == .future ? Color.secondary : Color.primary)
                        .padding(.top, 6)
                        .lineLimit(1)
                }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 10)
            .frame(maxWidth: .infinity, alignment: .leading)
            .opacity(state == .future ? 0.55 : 1.0)
        }
        .buttonStyle(.plain)
    }
}
