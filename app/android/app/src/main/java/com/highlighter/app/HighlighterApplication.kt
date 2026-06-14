package com.highlighter.app

import android.app.Application
import coil3.ImageLoader
import coil3.SingletonImageLoader
import coil3.bitmapFactoryMaxParallelism
import coil3.disk.DiskCache
import coil3.disk.directory
import coil3.memory.MemoryCache
import coil3.request.allowHardware
import coil3.request.crossfade

/**
 * Application class that configures a shared, process-wide Coil [ImageLoader]
 * tuned so per-card image decoding is cheap and stays off the UI critical path.
 *
 * Decisions, and why each one reduces steady-state scroll jank:
 *
 *  - **Memory cache (~25 % of app memory).** The dominant cost in the feed is
 *    re-decoding the same avatar / cover bitmap every time a card is recomposed
 *    or re-enters the viewport during a scroll. A generous strong-reference LRU
 *    means a bitmap is decoded once and then served straight from RAM.
 *
 *  - **Disk cache (100 MB).** Remote images survive process death and skip the
 *    network round-trip on subsequent launches.
 *
 *  - **Full-quality bitmaps (ARGB_8888, the default).** RGB_565 was considered
 *    but reverted: halved colour precision causes visible banding on cover art
 *    and the byte savings are already captured by decoding at slot size
 *    (see targetSize / AvatarImage). Coil's default ARGB_8888 is retained.
 *
 *  - **Hardware bitmaps left enabled (`allowHardware(true)`, the default).**
 *    Hardware bitmaps live in graphic memory and skip the extra CPU→GPU upload
 *    copy, keeping decoded frames off the Java heap. Stated explicitly here so
 *    it can never be silently disabled.
 *
 *  - **Higher BitmapFactory parallelism.** During a fast fling many cards ask
 *    for a decode at once; raising the parallel-decode ceiling stops those
 *    decodes from serialising behind one another and arriving late.
 *
 *  - **Global crossfade enabled.** Images fade in when they arrive from the
 *    network or disk, preserving the visual polish of the previous behaviour.
 */
class HighlighterApplication : Application(), SingletonImageLoader.Factory {

    override fun newImageLoader(context: android.content.Context): ImageLoader =
        ImageLoader.Builder(context)
            .memoryCache {
                MemoryCache.Builder()
                    // Use 25 % of the runtime's available memory for the
                    // strong-reference (LRU) image cache.
                    .maxSizePercent(context, 0.25)
                    .build()
            }
            .diskCache {
                DiskCache.Builder()
                    // Resolve to the app's private cache directory so the OS
                    // can evict entries under storage pressure.
                    .directory(context.cacheDir.resolve("image_cache"))
                    .maxSizeBytes(100L * 1024 * 1024) // 100 MB
                    .build()
            }
            // Keep hardware bitmaps (the default) so decoded frames skip the
            // CPU→GPU upload copy and stay off the Java heap.
            .allowHardware(true)
            // Restore the fade-in transition so images appear with the same
            // visual polish as before; Coil handles the animation off the
            // critical decode path.
            .crossfade(true)
            // Let several BitmapFactory decodes run concurrently so a fast
            // fling doesn't serialise them behind a single worker.
            .bitmapFactoryMaxParallelism(8)
            .build()
}
