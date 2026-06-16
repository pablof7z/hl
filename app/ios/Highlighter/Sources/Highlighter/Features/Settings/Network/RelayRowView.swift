import Kingfisher
import SwiftUI

/// Single row inside `NetworkSettingsView`. Leads with the relay's NIP-11
/// icon (or a monogram fallback), displays its declared name above the URL,
/// and shows live state + role chips. Chips here are display-only; the
/// detail view makes them tappable.
struct RelayRowView: View {
    let projection: RelayRowProjection

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            RelayAvatar(projection: projection.avatar, size: 36)
            VStack(alignment: .leading, spacing: 6) {
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    stateDot(projection.statusTone)
                    Text(projection.primaryLabel)
                        .font(.subheadline.weight(.semibold))
                        .lineLimit(1)
                        .truncationMode(.tail)
                    Spacer()
                    if let rtt = projection.rttLabel {
                        Text(rtt)
                            .font(.caption.monospacedDigit())
                            .foregroundStyle(.secondary)
                    }
                }
                Text(projection.displayUrl)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                    .truncationMode(.middle)
                HStack(spacing: 6) {
                    roleChip("Read", isOn: projection.read)
                    roleChip("Write", isOn: projection.write)
                    roleChip("Rooms", isOn: projection.rooms)
                    roleChip("Indexer", isOn: projection.indexer)
                }
            }
        }
        .padding(.vertical, 4)
    }

    // MARK: - Pieces

    private func stateDot(_ tone: RelayStatusTone) -> some View {
        Circle()
            .fill(statusColor(tone))
            .frame(width: 8, height: 8)
    }

    private func roleChip(_ label: String, isOn: Bool) -> some View {
        Text(label)
            .font(.caption2.weight(.semibold))
            .padding(.horizontal, 8)
            .padding(.vertical, 3)
            .background(
                Capsule()
                    .fill(isOn ? Color.accentColor.opacity(0.18) : Color.secondary.opacity(0.12))
            )
            .foregroundStyle(isOn ? Color.accentColor : .secondary)
    }
}

/// Leading-edge avatar for a relay row. Loads `nip11.icon` via Kingfisher
/// (disk-cached like every other image in the app) with a monogram
/// fallback rendered on the relay's host as a deterministic hue. The
/// fallback also shows while the NIP-11 probe is in flight, so rows look
/// right from the first frame.
struct RelayAvatar: View {
    let projection: RelayAvatarProjection
    var size: CGFloat = 36

    var body: some View {
        Group {
            if let iconURL = projection.iconUrl.flatMap(URL.init(string:)) {
                KFImage(iconURL)
                    .resizable()
                    .placeholder { monogram }
                    .fade(duration: 0.2)
                    .cancelOnDisappear(true)
                    .scaledToFill()
            } else {
                monogram
            }
        }
        .frame(width: size, height: size)
        .clipShape(RoundedRectangle(cornerRadius: size / 4, style: .continuous))
    }

    private var monogram: some View {
        ZStack {
            RoundedRectangle(cornerRadius: size / 4, style: .continuous)
                .fill(Color(hue: projection.hue, saturation: 0.55, brightness: 0.65))
            Text(projection.initial)
                .font(.system(size: size * 0.45, weight: .semibold, design: .rounded))
                .foregroundStyle(.white)
        }
    }
}

func statusColor(_ tone: RelayStatusTone) -> Color {
    switch tone {
    case .connected: return .green
    case .connecting: return .yellow
    case .error: return .red
    case .unknown: return .gray
    }
}
