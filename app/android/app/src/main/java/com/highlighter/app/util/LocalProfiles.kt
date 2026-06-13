package com.highlighter.app.util

import androidx.compose.runtime.compositionLocalOf
import uniffi.highlighter_core.HighlighterAppAction
import uniffi.highlighter_core.HighlighterIsbnPreview
import uniffi.highlighter_core.HighlighterProfile
import uniffi.highlighter_core.HighlighterWebMetadata

/**
 * Host-side author profile lookup table, threaded through the Compose tree so
 * leaf rows (comments, chat, discussions) can resolve avatars + display names
 * without every intermediate composable having to pass the full app state.
 *
 * Provided once near the root in [com.highlighter.app.MainActivity]; defaults to
 * an empty list so any composable rendered outside a provider degrades to the
 * existing truncated-pubkey fallback rather than crashing.
 */
val LocalProfiles = compositionLocalOf<List<HighlighterProfile>> { emptyList() }

/**
 * Host-side web-link-preview cache (Rust-owned `state.webMetadata`). Threaded the
 * same way as [LocalProfiles] so leaf rows can render a link card for URLs found
 * in their text. Defaults to empty: no provider == no previews, never a crash.
 */
val LocalWebMetadata = compositionLocalOf<List<HighlighterWebMetadata>> { emptyList() }

/**
 * Host-side ISBN preview cache (Rust-owned `state.isbnPreviews`). Threaded the
 * same way as [LocalProfiles] so feed cards can resolve book cover + title
 * without additional prop drilling. Defaults to empty.
 */
val LocalIsbnPreviews = compositionLocalOf<List<HighlighterIsbnPreview>> { emptyList() }

/**
 * Action dispatcher, exposed to leaf rows that need to request enrichment (e.g.
 * [HighlighterAppAction.RequestWebMetadata]) without plumbing `dispatch` through
 * every intermediate composable. Defaults to a no-op.
 */
val LocalDispatch = compositionLocalOf<(HighlighterAppAction) -> Unit> { {} }
