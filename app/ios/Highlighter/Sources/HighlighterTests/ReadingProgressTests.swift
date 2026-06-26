import Foundation
import Testing
@testable import Highlighter

/// Unit tests for `ReadingProgress.fraction` — the pure scroll-fraction
/// calculator. All cases are synchronous; no UI involved.
struct ReadingProgressTests {

    @Test func zeroAtTop() {
        let f = ReadingProgress.fraction(contentOffsetY: 0, contentHeight: 1000, viewportHeight: 100)
        #expect(f == 0.0)
    }

    @Test func oneAtBottom() {
        // scrollable = 1000 - 100 = 900; offset 900 → exactly 1.0.
        let f = ReadingProgress.fraction(contentOffsetY: 900, contentHeight: 1000, viewportHeight: 100)
        #expect(f == 1.0)
    }

    @Test func midpointIsHalf() {
        // scrollable = 900; offset 450 → 450/900 = 0.5.
        let f = ReadingProgress.fraction(contentOffsetY: 450, contentHeight: 1000, viewportHeight: 100)
        #expect(f == 0.5)
    }

    @Test func clampsNegativeOffsetToZero() {
        // Rubber-band overscroll above top.
        let f = ReadingProgress.fraction(contentOffsetY: -50, contentHeight: 1000, viewportHeight: 100)
        #expect(f == 0.0)
    }

    @Test func clampsOverscrollToOne() {
        // Rubber-band overscroll below bottom.
        let f = ReadingProgress.fraction(contentOffsetY: 1000, contentHeight: 1000, viewportHeight: 100)
        #expect(f == 1.0)
    }

    @Test func contentShorterThanViewportReturnsZero() {
        // scrollable = 50 - 100 = -50 → not scrollable → 0.
        let f = ReadingProgress.fraction(contentOffsetY: 0, contentHeight: 50, viewportHeight: 100)
        #expect(f == 0.0)
    }
}
