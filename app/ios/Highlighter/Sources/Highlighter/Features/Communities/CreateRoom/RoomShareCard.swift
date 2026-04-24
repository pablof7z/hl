import CoreImage.CIFilterBuiltins
import Kingfisher
import SwiftUI

/// "Whoever shows up" half of the invite screen. A paper-styled card with
/// the room's cover, name, an invite URL, a Copy button, and an
/// expandable QR. Mirrors the existing room-card aesthetic — accent
/// gradient fallback, ink-on-paper QR rather than black-on-white.
///
/// The shareable URL is a plain universal link (`https://highlighter.com/r/<id>`).
/// Open rooms join via the existing preview sheet on tap; closed rooms
/// land on the same preview but require approval. No auth tokens in v1 —
/// admins can always directly add specific people from the search field
/// below.
struct RoomShareCard: View {
    let groupId: String
    let room: CommunitySummary?

    @State private var qrShown = false
    @State private var copied = false

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            heroBackdrop

            VStack(alignment: .leading, spacing: 14) {
                if let room {
                    Text(room.name)
                        .font(.system(.title3, design: .serif).weight(.semibold))
                        .foregroundStyle(Color.highlighterInkStrong)
                        .lineLimit(1)
                } else {
                    Text("New room")
                        .font(.system(.title3, design: .serif).weight(.semibold))
                        .foregroundStyle(Color.highlighterInkStrong)
                }

                HStack(spacing: 8) {
                    Image(systemName: "link")
                        .font(.caption.weight(.semibold))
                        .foregroundStyle(Color.highlighterInkMuted)
                    Text(shareURL)
                        .font(.system(.caption, design: .monospaced))
                        .foregroundStyle(Color.highlighterInkMuted)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }

                HStack(spacing: 10) {
                    Button(action: copy) {
                        Label(copied ? "Copied" : "Copy link",
                              systemImage: copied ? "checkmark" : "doc.on.doc")
                            .font(.subheadline.weight(.medium))
                            .foregroundStyle(Color.highlighterInkStrong)
                            .padding(.horizontal, 14)
                            .padding(.vertical, 9)
                            .background(
                                Capsule().fill(Color.highlighterTintPale)
                            )
                    }
                    .buttonStyle(.plain)

                    ShareLink(item: URL(string: shareURL) ?? URL(string: "https://highlighter.com")!) {
                        Label("Share", systemImage: "square.and.arrow.up")
                            .font(.subheadline.weight(.medium))
                            .foregroundStyle(Color.highlighterInkStrong)
                            .padding(.horizontal, 14)
                            .padding(.vertical, 9)
                            .background(
                                Capsule().fill(Color.highlighterTintPale)
                            )
                    }

                    Spacer(minLength: 0)

                    Button {
                        withAnimation(.easeInOut(duration: 0.25)) { qrShown.toggle() }
                    } label: {
                        Image(systemName: qrShown ? "qrcode.viewfinder" : "qrcode")
                            .font(.title3)
                            .foregroundStyle(Color.highlighterAccent)
                            .padding(9)
                            .background(
                                Circle().fill(Color.highlighterTintPale)
                            )
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(qrShown ? "Hide QR" : "Show QR")
                }

                if qrShown {
                    qrView
                        .transition(.opacity.combined(with: .move(edge: .top)))
                }
            }
            .padding(18)
        }
        .background(Color.highlighterPaper)
        .overlay(
            RoundedRectangle(cornerRadius: 18)
                .stroke(Color.highlighterRule, lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: 18))
    }

    private var shareURL: String {
        "https://highlighter.com/r/\(groupId)"
    }

    private func copy() {
        UIPasteboard.general.string = shareURL
        UISelectionFeedbackGenerator().selectionChanged()
        copied = true
        Task {
            try? await Task.sleep(for: .seconds(2))
            await MainActor.run { copied = false }
        }
    }

    @ViewBuilder
    private var heroBackdrop: some View {
        ZStack {
            if let url = URL(string: room?.picture ?? ""),
               !(room?.picture ?? "").isEmpty {
                KFImage(url)
                    .resizable()
                    .scaledToFill()
            } else {
                LinearGradient(
                    colors: [
                        Color.highlighterAccent.opacity(0.72),
                        Color.highlighterAccent.opacity(0.36),
                    ],
                    startPoint: .topLeading,
                    endPoint: .bottomTrailing
                )
            }
        }
        .frame(height: 110)
        .frame(maxWidth: .infinity)
        .clipped()
    }

    @ViewBuilder
    private var qrView: some View {
        if let image = QRCodeGenerator.image(for: shareURL) {
            HStack {
                Spacer()
                VStack(spacing: 8) {
                    Image(uiImage: image)
                        .interpolation(.none)
                        .resizable()
                        .scaledToFit()
                        .frame(width: 200, height: 200)
                        .padding(12)
                        .background(Color.highlighterPaper)
                        .overlay(
                            RoundedRectangle(cornerRadius: 12)
                                .stroke(Color.highlighterRule, lineWidth: 1)
                        )
                    Text("Scan to join")
                        .font(.caption)
                        .foregroundStyle(Color.highlighterInkMuted)
                }
                Spacer()
            }
            .padding(.top, 6)
        }
    }
}

private enum QRCodeGenerator {
    static func image(for string: String) -> UIImage? {
        let context = CIContext()
        let filter = CIFilter.qrCodeGenerator()
        filter.message = Data(string.utf8)
        filter.correctionLevel = "M"
        guard let output = filter.outputImage else { return nil }

        // Tint to ink-on-paper instead of black-on-white. We feed the QR
        // through `CIFalseColor` so the dark bits become highlighter ink and
        // the light bits become paper — matches the rest of the app and
        // doesn't look like every other generic share QR.
        let inkComponents = UIColor(Color.highlighterInkStrong).cgColor.components ?? [0, 0, 0, 1]
        let paperComponents = UIColor(Color.highlighterPaper).cgColor.components ?? [1, 1, 1, 1]
        let ink = CIColor(
            red: inkComponents[safe: 0] ?? 0,
            green: inkComponents[safe: 1] ?? 0,
            blue: inkComponents[safe: 2] ?? 0
        )
        let paper = CIColor(
            red: paperComponents[safe: 0] ?? 1,
            green: paperComponents[safe: 1] ?? 1,
            blue: paperComponents[safe: 2] ?? 1
        )
        let colored = output.applyingFilter("CIFalseColor", parameters: [
            "inputColor0": ink,
            "inputColor1": paper,
        ])

        let scale: CGFloat = 10
        let scaled = colored.transformed(by: CGAffineTransform(scaleX: scale, y: scale))
        guard let cg = context.createCGImage(scaled, from: scaled.extent) else { return nil }
        return UIImage(cgImage: cg)
    }
}

private extension Array where Element == CGFloat {
    subscript(safe index: Int) -> CGFloat? {
        indices.contains(index) ? self[index] : nil
    }
}
