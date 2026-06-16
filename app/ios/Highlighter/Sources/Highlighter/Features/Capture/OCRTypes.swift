import CoreGraphics
import Foundation

typealias OCRWord = OcrWord
typealias OCRLine = OcrLine

extension OcrRect {
    init(_ rect: CGRect) {
        self.init(
            x: Double(rect.minX),
            y: Double(rect.minY),
            w: Double(rect.width),
            h: Double(rect.height)
        )
    }

    var cgRect: CGRect {
        CGRect(x: CGFloat(x), y: CGFloat(y), width: CGFloat(w), height: CGFloat(h))
    }

    var minX: CGFloat { CGFloat(x) }
    var minY: CGFloat { CGFloat(y) }
    var width: CGFloat { CGFloat(w) }
    var height: CGFloat { CGFloat(h) }
    var maxX: CGFloat { minX + width }
    var maxY: CGFloat { minY + height }
    var midX: CGFloat { minX + width / 2 }
    var midY: CGFloat { minY + height / 2 }
    var isNull: Bool { cgRect.isNull }
    var isEmpty: Bool { cgRect.isEmpty }
}
