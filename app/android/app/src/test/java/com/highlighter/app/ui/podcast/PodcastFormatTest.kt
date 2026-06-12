package com.highlighter.app.ui.podcast

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class PodcastFormatTest {

    @Test
    fun `formats sub-hour timestamps as m colon ss`() {
        assertEquals("0:00", formatPlaybackTime(0.0))
        assertEquals("0:05", formatPlaybackTime(5.0))
        assertEquals("1:02", formatPlaybackTime(62.0))
        assertEquals("59:59", formatPlaybackTime(3599.0))
    }

    @Test
    fun `formats hour-plus timestamps as h colon mm colon ss`() {
        assertEquals("1:00:00", formatPlaybackTime(3600.0))
        assertEquals("1:02:03", formatPlaybackTime(3723.0))
    }

    @Test
    fun `clamps negative and non-finite times to zero`() {
        assertEquals("0:00", formatPlaybackTime(-10.0))
        assertEquals("0:00", formatPlaybackTime(Double.NaN))
        assertEquals("0:00", formatPlaybackTime(Double.POSITIVE_INFINITY))
    }

    @Test
    fun `duration label is null when no positive duration available`() {
        assertNull(durationLabel(durationSeconds = null, fallbackSeconds = 0.0))
        assertNull(durationLabel(durationSeconds = 0L, fallbackSeconds = 0.0))
    }

    @Test
    fun `duration label prefers metadata then falls back to player duration`() {
        assertEquals("1h 1m", durationLabel(durationSeconds = 3660L, fallbackSeconds = 10.0))
        assertEquals("5m", durationLabel(durationSeconds = null, fallbackSeconds = 300.0))
    }

    @Test
    fun `audio url selection prefers full url over preview`() {
        assertEquals("https://full.mp3", selectAudioUrl(audioUrl = "https://full.mp3", audioPreviewUrl = "https://preview.mp3"))
        assertEquals("https://preview.mp3", selectAudioUrl(audioUrl = "  ", audioPreviewUrl = "https://preview.mp3"))
        assertNull(selectAudioUrl(audioUrl = "", audioPreviewUrl = ""))
    }
}
