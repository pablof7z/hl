import SwiftUI
import UIKit

extension Color {
    /// Terracotta accent used for clip ranges, primary CTAs, and highlighted
    /// segments. Matches the webapp's `--color-highlighter-accent`. Dark
    /// variant is slightly brighter so it stays legible on the dark paper.
    static let highlighterAccent = Color(uiColor: UIColor { trait in
        trait.userInterfaceStyle == .dark
            ? UIColor(red: 0.88, green: 0.60, blue: 0.46, alpha: 1)
            : UIColor(red: 0.77, green: 0.49, blue: 0.37, alpha: 1)
    })

    /// Page background — warm ivory in light, deep warm ink in dark.
    static let highlighterPaper = Color(uiColor: UIColor { trait in
        trait.userInterfaceStyle == .dark
            ? UIColor(red: 0.082, green: 0.078, blue: 0.067, alpha: 1)
            : UIColor(red: 0.98, green: 0.98, blue: 0.97, alpha: 1)
    })

    /// Primary body type.
    static let highlighterInkStrong = Color(uiColor: UIColor { trait in
        trait.userInterfaceStyle == .dark
            ? UIColor(red: 0.957, green: 0.945, blue: 0.918, alpha: 1)
            : UIColor(red: 0.082, green: 0.075, blue: 0.059, alpha: 1)
    })

    /// Muted metadata / secondary type.
    static let highlighterInkMuted = Color(uiColor: UIColor { trait in
        trait.userInterfaceStyle == .dark
            ? UIColor(red: 0.678, green: 0.651, blue: 0.588, alpha: 1)
            : UIColor(red: 0.478, green: 0.455, blue: 0.408, alpha: 1)
    })

    /// Hairlines / dividers / separator rules.
    static let highlighterRule = Color(uiColor: UIColor { trait in
        trait.userInterfaceStyle == .dark
            ? UIColor(red: 0.212, green: 0.200, blue: 0.173, alpha: 1)
            : UIColor(red: 0.898, green: 0.878, blue: 0.816, alpha: 1)
    })

    /// Pale blue tint used behind subtle informational surfaces.
    static let highlighterTintPale = Color(uiColor: UIColor { trait in
        trait.userInterfaceStyle == .dark
            ? UIColor(red: 0.106, green: 0.145, blue: 0.192, alpha: 1)
            : UIColor(red: 0.91, green: 0.955, blue: 0.992, alpha: 1)
    })

    /// Book-lane surface — a warmer paper leaf that reads distinctly from
    /// `highlighterPaper` when stacked next to other lane atmospheres.
    static let laneBookPaper = Color(uiColor: UIColor { trait in
        trait.userInterfaceStyle == .dark
            ? UIColor(red: 0.110, green: 0.094, blue: 0.075, alpha: 1)
            : UIColor(red: 0.965, green: 0.945, blue: 0.902, alpha: 1)
    })

    /// Gutter rule shown down the margin of a book-lane pull-quote.
    static let laneBookGutter = Color(uiColor: UIColor { trait in
        trait.userInterfaceStyle == .dark
            ? UIColor(red: 0.35, green: 0.30, blue: 0.24, alpha: 1)
            : UIColor(red: 0.78, green: 0.74, blue: 0.65, alpha: 1)
    })

    /// Podcast-lane surface. Always dark regardless of system mode — the
    /// audio interior doesn't adapt. Warm near-black so the page doesn't
    /// feel clinical.
    static let laneAudioSurface = Color(red: 0.078, green: 0.070, blue: 0.058)

    /// Primary text on a podcast lane. Warm off-white (lamp-lit), never
    /// pure white.
    static let laneAudioInk = Color(red: 0.925, green: 0.898, blue: 0.835)

    /// Muted text on a podcast lane (timestamps, speaker kickers,
    /// attribution).
    static let laneAudioInkMuted = Color(red: 0.565, green: 0.530, blue: 0.470)

    /// Hairlines and dim timeline track on a podcast lane.
    static let laneAudioRule = Color(red: 0.180, green: 0.162, blue: 0.132)

    /// Article-lane surface. Cleaner than the warm book paper — reads as
    /// a magazine page next to the book lane's leaf.
    static let laneArticlePage = Color(uiColor: UIColor { trait in
        trait.userInterfaceStyle == .dark
            ? UIColor(red: 0.062, green: 0.060, blue: 0.055, alpha: 1)
            : UIColor(red: 0.995, green: 0.992, blue: 0.985, alpha: 1)
    })

    /// Highlighter-stroke underlay used behind an article lane's hero
    /// pull-quote. Accent at low opacity, scoped to the quote.
    static let laneArticleHighlightFill = Color(uiColor: UIColor { trait in
        trait.userInterfaceStyle == .dark
            ? UIColor(red: 0.88, green: 0.60, blue: 0.46, alpha: 0.22)
            : UIColor(red: 0.95, green: 0.78, blue: 0.42, alpha: 0.32)
    })
}
