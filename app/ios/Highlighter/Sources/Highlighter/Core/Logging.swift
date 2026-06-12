import os

/// Shared logger for the Highlighter app.
///
/// Follows the existing `Logger(subsystem: "com.highlighter.app", ...)`
/// convention already used in the Podcast feature. Use the shared
/// `Logger.highlighter` instance for app-wide diagnostics, or create a
/// category-specific logger via `Logger.highlighter(category:)` for a
/// subsystem that wants its own bucket in Console.app.
extension Logger {
    /// Default app-wide logger.
    static let highlighter = Logger(subsystem: "com.highlighter.app", category: "app")

    /// Category-scoped logger sharing the app subsystem.
    static func highlighter(category: String) -> Logger {
        Logger(subsystem: "com.highlighter.app", category: category)
    }
}
