package com.highlighter.app.ui.podcast

/**
 * Minimal key/value sink the [PodcastPositionStore] writes through. Backed by
 * SharedPreferences in production; by a plain map in tests so the
 * persist/restore + staleness logic can be verified off-device.
 */
internal interface PositionBackingStore {
    fun getString(key: String): String?
    fun putString(key: String, value: String)
}

/**
 * Per-episode "resume where you left off" persistence, keyed by artifact id
 * (the share event id). Mirrors iOS PodcastPlayerStore's PositionRecord: a
 * position plus a timestamp, where entries older than seven days are treated
 * as expired so we don't silently jump users deep into an episode they last
 * touched months ago.
 *
 * Stored value format is `"<positionSeconds>|<savedAtMillis>"` — deliberately
 * trivial so there is no serialization framework to break on.
 */
internal class PodcastPositionStore(
    private val backing: PositionBackingStore,
    private val clock: () -> Long = System::currentTimeMillis,
) {
    /** Persist [positionSeconds] for [artifactId]. No-ops on a blank id. */
    fun save(artifactId: String, positionSeconds: Double) {
        if (artifactId.isBlank()) return
        backing.putString(key(artifactId), "$positionSeconds|${clock()}")
    }

    /**
     * The last saved position for [artifactId], or null when nothing is stored,
     * the id is blank, the record is malformed, or it is older than seven days.
     */
    fun lastPosition(artifactId: String): Double? {
        if (artifactId.isBlank()) return null
        val raw = backing.getString(key(artifactId)) ?: return null
        val parts = raw.split('|')
        if (parts.size != 2) return null
        val position = parts[0].toDoubleOrNull() ?: return null
        val savedAt = parts[1].toLongOrNull() ?: return null
        if (clock() - savedAt > MAX_AGE_MILLIS) return null
        return position
    }

    private fun key(artifactId: String): String = "$KEY_PREFIX$artifactId"

    private companion object {
        const val KEY_PREFIX = "podcast.position."
        const val MAX_AGE_MILLIS = 7L * 24 * 60 * 60 * 1000
    }
}
