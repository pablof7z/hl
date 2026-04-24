import SwiftUI

/// Single row inside `NetworkSettingsView`. Shows URL + state dot + latency
/// + role chips. Chips are display-only here; the detail view is where they
/// become tappable.
struct RelayRowView: View {
    let config: RelayConfig
    let diagnostic: RelayDiagnostic?

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 8) {
                stateDot
                Text(displayURL(config.url))
                    .font(.subheadline)
                    .lineLimit(1)
                    .truncationMode(.middle)
                Spacer()
                if let rtt = diagnostic?.rttMs {
                    Text("\(rtt) ms")
                        .font(.caption.monospacedDigit())
                        .foregroundStyle(.secondary)
                }
            }
            HStack(spacing: 6) {
                roleChip("Read", isOn: config.read)
                roleChip("Write", isOn: config.write)
                roleChip("Rooms", isOn: config.rooms)
                roleChip("Indexer", isOn: config.indexer)
            }
        }
        .padding(.vertical, 2)
    }

    // MARK: - Pieces

    @ViewBuilder
    private var stateDot: some View {
        let color: Color = {
            switch diagnostic?.state {
            case .connected: return .green
            case .connecting: return .yellow
            case .disconnected, .terminated, .banned: return .red
            case .none: return .gray
            }
        }()
        Circle()
            .fill(color)
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

    private func displayURL(_ raw: String) -> String {
        // Strip the common `wss://` prefix to give room for the host on
        // narrow iPhones; keep `ws://` visible since it's a security signal.
        if raw.hasPrefix("wss://") { return String(raw.dropFirst(6)) }
        return raw
    }
}
