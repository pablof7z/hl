package com.highlighter.app.ui.podcast

/**
 * Pure formatting / selection helpers for the podcast player. Kept free of
 * Android types so they are unit-testable on the JVM (mirrors the small format
 * helpers used by iOS's PodcastListeningView).
 */

/** `m:ss` under an hour, `h:mm:ss` at/above. Clamps NaN/∞/negative to `0:00`. */
internal fun formatPlaybackTime(seconds: Double): String {
    val safe = if (!seconds.isFinite() || seconds < 0) 0.0 else seconds
    val total = safe.toInt()
    val h = total / 3600
    val m = (total % 3600) / 60
    val s = total % 60
    return if (h > 0) {
        "%d:%02d:%02d".format(h, m, s)
    } else {
        "%d:%02d".format(m, s)
    }
}

/**
 * Coarse "1h 2m" / "5m" duration label. Prefers the artifact's metadata
 * [durationSeconds]; falls back to the live player [fallbackSeconds]. Returns
 * null when neither is positive (so callers can omit the line entirely).
 */
internal fun durationLabel(durationSeconds: Long?, fallbackSeconds: Double): String? {
    val total: Int = when {
        durationSeconds != null && durationSeconds > 0 -> durationSeconds.toInt()
        fallbackSeconds > 0 -> fallbackSeconds.toInt()
        else -> return null
    }
    val h = total / 3600
    val m = (total % 3600) / 60
    return if (h > 0) "${h}h ${m}m" else "${m}m"
}

/**
 * Chooses the URL to feed the player: the full [audioUrl] when present,
 * otherwise the [audioPreviewUrl]. Returns null when neither is usable —
 * matching iOS's `audioUrl.isEmpty ? audioPreviewUrl : audioUrl` guard.
 */
internal fun selectAudioUrl(audioUrl: String, audioPreviewUrl: String): String? {
    audioUrl.trim().takeIf { it.isNotEmpty() }?.let { return it }
    audioPreviewUrl.trim().takeIf { it.isNotEmpty() }?.let { return it }
    return null
}
