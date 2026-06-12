package com.highlighter.app.util

import org.junit.Assert.assertEquals
import org.junit.Test
import uniffi.highlighter_core.HighlighterConnectionState

class FormattersTest {

    @Test
    fun `bootstrapping wins over connection state`() {
        assertEquals("Syncing", HighlighterConnectionState.ONLINE.statusLabel(isBootstrapping = true))
        assertEquals("Syncing", HighlighterConnectionState.OFFLINE.statusLabel(isBootstrapping = true))
    }

    @Test
    fun `connection states map to labels`() {
        assertEquals("Connecting", HighlighterConnectionState.CONNECTING.statusLabel(isBootstrapping = false))
        assertEquals("Online", HighlighterConnectionState.ONLINE.statusLabel(isBootstrapping = false))
        assertEquals("Offline", HighlighterConnectionState.OFFLINE.statusLabel(isBootstrapping = false))
    }

    @Test
    fun `feed count label pluralizes`() {
        assertEquals("1 highlight", 1uL.feedCountLabel("highlight"))
        assertEquals("0 highlights", 0uL.feedCountLabel("highlight"))
        assertEquals("42 highlights", 42uL.feedCountLabel("highlight"))
    }
}
