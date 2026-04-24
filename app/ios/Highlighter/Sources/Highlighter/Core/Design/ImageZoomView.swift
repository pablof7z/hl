import Kingfisher
import SwiftUI

struct ImageZoomView: View {
    let url: URL?
    let onDismiss: () -> Void

    @State private var scale: CGFloat = 1
    @State private var lastScale: CGFloat = 1

    var body: some View {
        ZStack {
            Color.black.ignoresSafeArea()
            if let url {
                KFImage(url)
                    .placeholder { ProgressView().tint(.white) }
                    .resizable()
                    .scaledToFit()
                    .scaleEffect(scale)
                    .gesture(
                        MagnificationGesture()
                            .onChanged { value in
                                scale = max(1, min(5, lastScale * value))
                            }
                            .onEnded { _ in lastScale = scale }
                    )
            }
            VStack {
                HStack {
                    Spacer()
                    Button(action: onDismiss) {
                        Image(systemName: "xmark")
                            .font(.body.weight(.semibold))
                            .foregroundStyle(.white)
                            .padding(12)
                            .background(.ultraThinMaterial, in: Circle())
                    }
                    .padding()
                }
                Spacer()
            }
        }
    }
}
