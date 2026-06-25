package com.highlighter.app.util

import android.content.Context
import android.graphics.BitmapFactory
import android.net.Uri
import java.io.ByteArrayOutputStream
import java.io.InputStream

private const val MAX_PICKED_IMAGE_BYTES = 12 * 1024 * 1024

/**
 * A decoded image selected via [androidx.activity.result.contract.ActivityResultContracts.PickVisualMedia],
 * ready to hand to the Rust core for upload (bytes + mime + intrinsic size).
 */
internal data class PickedImage(
    val bytes: ByteArray,
    val mime: String,
    val width: UInt,
    val height: UInt,
) {
    // ByteArray breaks data-class equality; override so the model behaves sanely.
    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is PickedImage) return false
        return bytes.contentEquals(other.bytes) &&
            mime == other.mime &&
            width == other.width &&
            height == other.height
    }

    override fun hashCode(): Int {
        var result = bytes.contentHashCode()
        result = 31 * result + mime.hashCode()
        result = 31 * result + width.hashCode()
        result = 31 * result + height.hashCode()
        return result
    }
}

/**
 * Reads the picked [uri] into a [PickedImage], decoding only the bounds (no full
 * bitmap allocation) to recover intrinsic width/height. Returns null when the
 * stream can't be opened or decoded. Mirrors the capture flow's reader.
 */
internal fun readPickedImage(context: Context, uri: Uri): PickedImage? =
    runCatching {
        val bytes = context.contentResolver.openInputStream(uri)?.use {
            it.readBytesBounded(MAX_PICKED_IMAGE_BYTES)
        }
            ?: return null
        val options = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeByteArray(bytes, 0, bytes.size, options)
        PickedImage(
            bytes = bytes,
            mime = context.contentResolver.getType(uri) ?: "image/jpeg",
            width = options.outWidth.coerceAtLeast(0).toUInt(),
            height = options.outHeight.coerceAtLeast(0).toUInt(),
        )
    }.getOrNull()

private fun InputStream.readBytesBounded(maxBytes: Int): ByteArray {
    val buffer = ByteArray(DEFAULT_BUFFER_SIZE)
    val out = ByteArrayOutputStream()
    var total = 0
    while (true) {
        val read = read(buffer)
        if (read == -1) break
        total += read
        if (total > maxBytes) {
            throw IllegalArgumentException("Selected image is too large")
        }
        out.write(buffer, 0, read)
    }
    return out.toByteArray()
}
