package com.highlighter.app.ui.settings

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.highlighter.app.ui.components.Panel
import com.highlighter.app.ui.components.SectionHeader
import uniffi.highlighter_core.HighlighterAppAction
import uniffi.highlighter_core.HighlighterMediaSettingsSnapshot
import uniffi.highlighter_core.HighlighterNetworkSnapshot
import uniffi.highlighter_core.RelayConfig
import uniffi.highlighter_core.RelayStatus

@Composable
internal fun MediaSettingsPanel(
    media: HighlighterMediaSettingsSnapshot,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    var serverUrl by remember { mutableStateOf("") }
    Panel {
        SectionHeader("Media", media.blossomServerCount.toString())
        Spacer(modifier = Modifier.height(8.dp))
        OutlinedTextField(
            value = serverUrl,
            onValueChange = { serverUrl = it },
            modifier = Modifier.fillMaxWidth(),
            singleLine = true,
            label = { Text("Blossom server") },
        )
        Spacer(modifier = Modifier.height(8.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(
                onClick = {
                    val url = serverUrl.trim()
                    if (url.isNotEmpty()) {
                        dispatch(HighlighterAppAction.AddBlossomServer(url))
                        serverUrl = ""
                    }
                },
                shape = RoundedCornerShape(8.dp),
                enabled = serverUrl.isNotBlank() && !media.isSaving,
            ) {
                Text(if (media.isSaving) "Saving" else "Add")
            }
            OutlinedButton(
                onClick = { dispatch(HighlighterAppAction.RefreshMediaSettings) },
                shape = RoundedCornerShape(8.dp),
            ) {
                Text(if (media.isLoading) "Loading" else "Refresh")
            }
        }
        media.errorMessage?.takeIf { it.isNotBlank() }?.let { message ->
            Spacer(modifier = Modifier.height(6.dp))
            Text(text = message, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.tertiary)
        }
        media.blossomServers.take(6).forEach { url ->
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    text = url,
                    modifier = Modifier.weight(1f),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                TextButton(onClick = { dispatch(HighlighterAppAction.RemoveBlossomServer(url)) }) {
                    Text("Remove")
                }
            }
        }
    }
}

@Composable
internal fun NetworkPanel(
    network: HighlighterNetworkSnapshot,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    val pathLabel = when (network.currentPathIsWifi) {
        true -> "Wi-Fi path active"
        false -> "Not on Wi-Fi"
        null -> "Path pending"
    }
    Panel {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = "Network",
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold,
                    color = MaterialTheme.colorScheme.onSurface,
                )
                Text(
                    text = if (network.wifiOnlyEnabled) pathLabel else "Relay connections allowed",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            Switch(
                checked = network.wifiOnlyEnabled,
                onCheckedChange = { dispatch(HighlighterAppAction.SetNetworkWifiOnly(it)) },
            )
        }
        Spacer(modifier = Modifier.height(10.dp))

        var relayUrl by remember { mutableStateOf("") }
        OutlinedTextField(
            value = relayUrl,
            onValueChange = { relayUrl = it },
            modifier = Modifier.fillMaxWidth(),
            singleLine = true,
            label = { Text("Relay URL (wss://…)") },
        )
        Spacer(modifier = Modifier.height(8.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(
                onClick = {
                    val url = relayUrl.trim()
                    if (url.isNotEmpty()) {
                        dispatch(
                            HighlighterAppAction.UpsertNetworkRelay(
                                RelayConfig(
                                    url = url,
                                    read = true,
                                    write = true,
                                    rooms = false,
                                    indexer = false,
                                ),
                            ),
                        )
                        relayUrl = ""
                    }
                },
                shape = RoundedCornerShape(8.dp),
                enabled = relayUrl.isNotBlank() && !network.isSaving,
            ) {
                Text(if (network.isSaving) "Saving" else "Add")
            }
            OutlinedButton(
                onClick = { dispatch(HighlighterAppAction.ReconnectNetwork) },
                shape = RoundedCornerShape(8.dp),
                border = BorderStroke(1.dp, MaterialTheme.colorScheme.outline),
            ) {
                Text("Reconnect All")
            }
        }
        network.errorMessage?.takeIf { it.isNotBlank() }?.let { message ->
            Spacer(modifier = Modifier.height(6.dp))
            Text(text = message, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.tertiary)
        }

        val diagnosticsByUrl = network.diagnostics.associateBy { it.url.trimEnd('/') }
        if (network.relays.isNotEmpty()) {
            Spacer(modifier = Modifier.height(10.dp))
            SectionHeader("Relays", network.relayCount.toString())
            network.relays.forEach { relay ->
                RelayRow(
                    relay = relay,
                    status = diagnosticsByUrl[relay.url.trimEnd('/')]?.state,
                    onRemove = { dispatch(HighlighterAppAction.RemoveNetworkRelay(relay.url)) },
                )
            }
        }
        if (network.autoConnectedRelays.isNotEmpty()) {
            Spacer(modifier = Modifier.height(10.dp))
            SectionHeader("Auto-connected", network.autoConnectedRelayCount.toString())
            network.autoConnectedRelays.forEach { relay ->
                RelayRow(
                    relay = relay,
                    status = diagnosticsByUrl[relay.url.trimEnd('/')]?.state,
                    onRemove = null,
                )
            }
        }
    }
}

@Composable
private fun RelayRow(
    relay: RelayConfig,
    status: RelayStatus?,
    onRemove: (() -> Unit)?,
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Spacer(
            modifier = Modifier
                .size(8.dp)
                .background(color = status.indicatorColor(), shape = CircleShape),
        )
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = relay.url,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurface,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                text = listOfNotNull(
                    status.statusLabel(),
                    relayRolesLabel(relay).takeIf { it.isNotBlank() },
                ).joinToString(" · "),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        if (onRemove != null) {
            TextButton(onClick = onRemove) {
                Text("Remove")
            }
        }
    }
}

@Composable
private fun RelayStatus?.indicatorColor(): Color = when (this) {
    RelayStatus.CONNECTED -> MaterialTheme.colorScheme.primary
    RelayStatus.CONNECTING -> MaterialTheme.colorScheme.secondary
    RelayStatus.DISCONNECTED, RelayStatus.TERMINATED, RelayStatus.BANNED ->
        MaterialTheme.colorScheme.tertiary
    null -> MaterialTheme.colorScheme.outline
}

private fun RelayStatus?.statusLabel(): String = when (this) {
    RelayStatus.CONNECTED -> "Connected"
    RelayStatus.CONNECTING -> "Connecting"
    RelayStatus.DISCONNECTED -> "Disconnected"
    RelayStatus.TERMINATED -> "Terminated"
    RelayStatus.BANNED -> "Banned"
    null -> "No status"
}

private fun relayRolesLabel(relay: RelayConfig): String =
    listOfNotNull(
        "read".takeIf { relay.read },
        "write".takeIf { relay.write },
        "rooms".takeIf { relay.rooms },
        "indexer".takeIf { relay.indexer },
    ).joinToString("/")
