package com.highlighter.app.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import com.highlighter.app.ui.components.MetricRow
import com.highlighter.app.ui.components.Panel
import com.highlighter.app.ui.components.SectionHeader
import com.highlighter.app.ui.settings.MediaSettingsPanel
import com.highlighter.app.ui.settings.NetworkPanel
import uniffi.highlighter_core.HighlighterAppAction
import uniffi.highlighter_core.HighlighterAppState

/**
 * Settings as a real screen (was a stack of panels in the old single scroll).
 * Houses the diagnostics MetricRow, network + media settings, entries into
 * Bookmarks and Feedback sub-screens, a What's New note, and a destructive
 * Sign out at the bottom. Dispatches Open/Close for the settings slices it
 * shows, mirroring the iOS settings views' `.task` lifecycle.
 */
@Composable
internal fun SettingsScreen(
    state: HighlighterAppState,
    onBack: () -> Unit,
    onOpenBookmarks: () -> Unit,
    onOpenFeedback: () -> Unit,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    DisposableEffect(Unit) {
        dispatch(HighlighterAppAction.OpenMediaSettings)
        dispatch(HighlighterAppAction.OpenNetworkSettings)
        onDispose {
            dispatch(HighlighterAppAction.CloseMediaSettings)
            dispatch(HighlighterAppAction.CloseNetworkSettings)
        }
    }
    DestinationScaffold(title = "Settings", onBack = onBack) { _ ->
        LazyColumn(
            modifier = Modifier.fillMaxSize(),
            verticalArrangement = Arrangement.spacedBy(14.dp),
            contentPadding = PaddingValues(18.dp),
        ) {
            item { MetricRow(chrome = state.chrome) }
            item { NetworkPanel(network = state.network, dispatch = dispatch) }
            item { MediaSettingsPanel(media = state.mediaSettings, dispatch = dispatch) }
            item {
                Panel {
                    SectionHeader("Library", state.chrome.bookmarkedArticleAddressCount.toString())
                    OutlinedButton(
                        onClick = onOpenBookmarks,
                        modifier = Modifier
                            .fillMaxWidth()
                            .testTag("library_bookmarks_button"),
                        shape = RoundedCornerShape(8.dp),
                    ) {
                        Text("Bookmarks")
                    }
                    OutlinedButton(
                        onClick = onOpenFeedback,
                        modifier = Modifier.fillMaxWidth(),
                        shape = RoundedCornerShape(8.dp),
                    ) {
                        Text("Send feedback")
                    }
                }
            }
            item {
                Panel {
                    SectionHeader("What's new", "")
                    Text(
                        text = "Release notes appear automatically the first time you open a new build.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
            item {
                OutlinedButton(
                    onClick = { dispatch(HighlighterAppAction.Logout) },
                    modifier = Modifier.fillMaxWidth(),
                    shape = RoundedCornerShape(8.dp),
                ) {
                    Text("Sign out", color = MaterialTheme.colorScheme.error)
                }
            }
        }
    }
}
