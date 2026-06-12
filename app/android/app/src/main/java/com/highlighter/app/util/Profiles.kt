package com.highlighter.app.util

import uniffi.highlighter_core.HighlighterAppState
import uniffi.highlighter_core.HighlighterProfile
import uniffi.highlighter_core.HighlighterWebMetadata
import uniffi.highlighter_core.ProfileMetadata
import uniffi.highlighter_core.WebMetadata

/**
 * Resolve a cached author profile for the given pubkey.
 *
 * Mirrors iOS `HighlighterStore.profile(pubkeyHex:)`: the lookup key is trimmed
 * and lowercased before comparison, because Rust stores `pubkey_hex` trimmed +
 * ASCII-lowercased (see `insert_profile_metadata` in `nmp_app.rs`). We match on
 * either the stored `pubkeyHex` or the metadata's own `pubkey`, matching the iOS
 * `$0.pubkeyHex == key || $0.metadata.pubkey == key` predicate.
 */
fun List<HighlighterProfile>.profileFor(pubkey: String): ProfileMetadata? {
    val key = pubkey.trim().lowercase()
    if (key.isEmpty()) return null
    return firstOrNull {
        it.pubkeyHex == key || it.metadata.pubkey.trim().lowercase() == key
    }?.metadata
}

/** Convenience overload reading from full app state's profile table. */
fun HighlighterAppState.profileFor(pubkey: String): ProfileMetadata? =
    profiles.profileFor(pubkey)

/**
 * Preferred display name for an author: displayName, then name, then a short
 * truncated pubkey fallback — matching how iOS author rows degrade gracefully.
 */
fun ProfileMetadata?.displayNameOr(pubkey: String): String {
    val display = this?.displayName?.trim().orEmpty()
    if (display.isNotEmpty()) return display
    val name = this?.name?.trim().orEmpty()
    if (name.isNotEmpty()) return name
    return pubkey.take(12)
}

/** Avatar picture URL for an author, or null when unset. */
fun ProfileMetadata?.avatarUrl(): String? =
    this?.picture?.trim()?.takeIf { it.isNotEmpty() }

/**
 * Resolve cached web-link metadata for a URL. Rust keys this cache by the exact
 * URL passed to `RequestWebMetadata`, so we match on the trimmed URL directly —
 * mirroring iOS `app.webMetadata(url:)`.
 */
fun List<HighlighterWebMetadata>.webMetadataFor(url: String): WebMetadata? {
    val key = url.trim()
    if (key.isEmpty()) return null
    return firstOrNull { it.url == key }?.metadata
}
