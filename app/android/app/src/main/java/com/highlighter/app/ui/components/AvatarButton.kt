package com.highlighter.app.ui.components

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import uniffi.highlighter_core.ProfileMetadata

/**
 * Top-bar account button: the current user's [AvatarImage], tappable to open
 * their profile. Falls back to a monogram derived from the display name (or
 * the supplied [fallbackName], e.g. npub) when no picture is available.
 */
@Composable
internal fun AvatarButton(
    profile: ProfileMetadata?,
    fallbackName: String,
    onClick: () -> Unit,
) {
    val name = profile?.displayName?.takeIf { it.isNotBlank() }
        ?: profile?.name?.takeIf { it.isNotBlank() }
        ?: fallbackName
    AvatarImage(
        url = profile?.picture,
        name = name,
        size = 32.dp,
        modifier = Modifier
            .padding(end = 4.dp)
            .clickable(onClick = onClick),
    )
}
