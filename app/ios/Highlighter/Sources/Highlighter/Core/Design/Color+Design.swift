import SwiftUI

extension Color {
    /// Terracotta accent used for clip ranges, primary CTAs, and highlighted
    /// segments. Matches the webapp's `--color-highlighter-accent`.
    static let highlighterAccent = Color(red: 0.77, green: 0.49, blue: 0.37)

    /// Editorial neutrals matching the web profile palette. Used on the
    /// iOS profile screen so light-mode typography lands with the same
    /// contrast and warmth as `/profile/[identifier]` on the web.
    static let highlighterPaper = Color(red: 0.98, green: 0.98, blue: 0.97)
    static let highlighterInkStrong = Color(red: 0.082, green: 0.075, blue: 0.059)
    static let highlighterInkMuted = Color(red: 0.478, green: 0.455, blue: 0.408)
    static let highlighterRule = Color(red: 0.898, green: 0.878, blue: 0.816)
    static let highlighterTintPale = Color(red: 0.91, green: 0.955, blue: 0.992)
}
