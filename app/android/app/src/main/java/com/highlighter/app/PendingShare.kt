package com.highlighter.app

/**
 * Host-held payload for an inbound `ACTION_SEND` while the share composer is
 * open. The Rust core's share-composer snapshot only models the publish
 * lifecycle, so the shared content lives here (mirrors iOS keeping the incoming
 * URL in its share view).
 *
 * @param text the raw shared text (EXTRA_TEXT), used for preview/fallback.
 * @param url the first link detected in [text], or null if none was found.
 * @param note initial note text, seeded from EXTRA_SUBJECT when meaningful.
 */
data class PendingShare(
    val text: String,
    val url: String?,
    val note: String = "",
)
