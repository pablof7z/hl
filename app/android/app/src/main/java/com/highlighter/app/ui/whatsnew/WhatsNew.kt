package com.highlighter.app.ui.whatsnew

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import uniffi.highlighter_core.HighlighterAppAction
import uniffi.highlighter_core.HighlighterWhatsNewEntry

@Composable
internal fun WhatsNewDialog(
    entries: List<HighlighterWhatsNewEntry>,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    AlertDialog(
        onDismissRequest = { dispatch(HighlighterAppAction.DismissWhatsNew) },
        confirmButton = {
            TextButton(onClick = { dispatch(HighlighterAppAction.DismissWhatsNew) }) {
                Text("Got it")
            }
        },
        title = { Text("What's new", fontWeight = FontWeight.Bold) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(
                    text = "SINCE YOU LAST OPENED HIGHLIGHTER",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    fontWeight = FontWeight.SemiBold,
                )
                entries.forEach { entry ->
                    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                        Text(
                            text = whatsNewDateline(entry.shippedAt),
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            fontWeight = FontWeight.SemiBold,
                        )
                        entry.lines.forEach { line ->
                            Row(
                                horizontalArrangement = Arrangement.spacedBy(8.dp),
                                verticalAlignment = Alignment.Top,
                            ) {
                                Text("*", color = MaterialTheme.colorScheme.secondary, style = MaterialTheme.typography.bodyMedium)
                                Text(
                                    text = line,
                                    style = MaterialTheme.typography.bodyMedium,
                                    color = MaterialTheme.colorScheme.onSurface,
                                )
                            }
                        }
                    }
                }
            }
        },
        containerColor = MaterialTheme.colorScheme.background,
    )
}

private fun whatsNewDateline(shippedAt: String): String {
    val month = shippedAt.substringOrNull(5, 7)?.toIntOrNull()
    val day = shippedAt.substringOrNull(8, 10)?.toIntOrNull()
    val time = shippedAt.substringOrNull(11, 16)
    val monthLabel = month?.let { monthLabels.getOrNull(it - 1) }
    return if (monthLabel != null && day != null && time != null) {
        "$monthLabel $day - $time"
    } else {
        shippedAt
    }
}

private val monthLabels = listOf(
    "JAN",
    "FEB",
    "MAR",
    "APR",
    "MAY",
    "JUN",
    "JUL",
    "AUG",
    "SEP",
    "OCT",
    "NOV",
    "DEC",
)

private fun String.substringOrNull(startIndex: Int, endIndex: Int): String? {
    return if (length >= endIndex) substring(startIndex, endIndex) else null
}
