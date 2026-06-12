package com.highlighter.app.util

import uniffi.highlighter_core.HighlighterConnectionState
import uniffi.highlighter_core.HighlighterProfileViewSnapshot
import uniffi.highlighter_core.RoomRecommendation

internal fun HighlighterConnectionState.statusLabel(isBootstrapping: Boolean): String =
    when {
        isBootstrapping -> "Syncing"
        this == HighlighterConnectionState.CONNECTING -> "Connecting"
        this == HighlighterConnectionState.ONLINE -> "Online"
        this == HighlighterConnectionState.OFFLINE -> "Offline"
        else -> "Ready"
    }

internal fun RoomRecommendation.signalLabel(): String =
    when (val count = reasonPubkeys.size) {
        0 -> summary.about
        1 -> "1 matching reader"
        else -> "$count matching readers"
    }

internal fun HighlighterProfileViewSnapshot.displayName(): String =
    profile?.displayName?.takeIf { it.isNotBlank() }
        ?: profile?.name?.takeIf { it.isNotBlank() }
        ?: pubkeyHex.take(12)

internal fun ULong.feedCountLabel(noun: String): String =
    when (this) {
        1uL -> "1 $noun"
        else -> "$this ${noun}s"
    }
