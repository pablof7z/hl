package com.highlighter.app.ui.podcast

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** In-memory [PositionBackingStore] so the store logic is testable off-device. */
private class FakeBackingStore : PositionBackingStore {
    val map = mutableMapOf<String, String>()
    override fun getString(key: String): String? = map[key]
    override fun putString(key: String, value: String) {
        map[key] = value
    }
}

class PodcastPositionStoreTest {

    private val now = 1_700_000_000_000L // fixed clock

    @Test
    fun `returns null when nothing persisted`() {
        val store = PodcastPositionStore(FakeBackingStore()) { now }
        assertNull(store.lastPosition("episode-1"))
    }

    @Test
    fun `persists and restores a position for the same artifact id`() {
        val store = PodcastPositionStore(FakeBackingStore()) { now }
        store.save("episode-1", positionSeconds = 123.4)
        assertEquals(123.4, store.lastPosition("episode-1")!!, 0.001)
    }

    @Test
    fun `positions are scoped per artifact id`() {
        val store = PodcastPositionStore(FakeBackingStore()) { now }
        store.save("episode-1", positionSeconds = 10.0)
        store.save("episode-2", positionSeconds = 99.0)
        assertEquals(10.0, store.lastPosition("episode-1")!!, 0.001)
        assertEquals(99.0, store.lastPosition("episode-2")!!, 0.001)
    }

    @Test
    fun `latest save for an id wins`() {
        val store = PodcastPositionStore(FakeBackingStore()) { now }
        store.save("episode-1", positionSeconds = 10.0)
        store.save("episode-1", positionSeconds = 42.0)
        assertEquals(42.0, store.lastPosition("episode-1")!!, 0.001)
    }

    @Test
    fun `stale positions older than seven days are dropped on read`() {
        val backing = FakeBackingStore()
        var clock = now
        val store = PodcastPositionStore(backing) { clock }
        store.save("episode-1", positionSeconds = 50.0)
        // Advance 8 days.
        clock = now + 8L * 24 * 60 * 60 * 1000
        assertNull(store.lastPosition("episode-1"))
    }

    @Test
    fun `positions within seven days are retained`() {
        val backing = FakeBackingStore()
        var clock = now
        val store = PodcastPositionStore(backing) { clock }
        store.save("episode-1", positionSeconds = 50.0)
        clock = now + 6L * 24 * 60 * 60 * 1000
        assertEquals(50.0, store.lastPosition("episode-1")!!, 0.001)
    }

    @Test
    fun `blank artifact id is ignored`() {
        val store = PodcastPositionStore(FakeBackingStore()) { now }
        store.save("", positionSeconds = 10.0)
        assertNull(store.lastPosition(""))
    }
}
