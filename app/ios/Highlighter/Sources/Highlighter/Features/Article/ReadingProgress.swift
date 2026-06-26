import CoreGraphics

/// Pure reading-progress calculator for the article reader's scroll overlay.
/// No state, no UIKit dependency — safe to unit-test without a view hierarchy.
enum ReadingProgress {

    /// Fraction of the article that has been scrolled past, in [0, 1].
    ///
    /// - Parameters:
    ///   - contentOffsetY: The scroll view's current vertical content offset.
    ///   - contentHeight: The scroll view's total content height.
    ///   - viewportHeight: The scroll view's visible frame height (container size).
    /// - Returns:
    ///   `0.0` when at the top (or when the content fits entirely in the
    ///   viewport), `1.0` when scrolled to the bottom. Rubber-band overscroll
    ///   in either direction is clamped.
    static func fraction(
        contentOffsetY: CGFloat,
        contentHeight: CGFloat,
        viewportHeight: CGFloat
    ) -> Double {
        let scrollable = contentHeight - viewportHeight
        guard scrollable > 0 else { return 0 }
        return min(1, max(0, Double(contentOffsetY / scrollable)))
    }
}
