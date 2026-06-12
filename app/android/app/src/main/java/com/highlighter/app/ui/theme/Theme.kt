package com.highlighter.app.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

private val LightColors = lightColorScheme(
    primary = Moss,
    onPrimary = Color.White,
    secondary = Gold,
    tertiary = Clay,
    background = Paper,
    surface = Cream,
    onBackground = Ink,
    onSurface = Ink,
    onSurfaceVariant = Muted,
    outline = Line,
    surfaceVariant = Color(0xFFEAF2EE),
    outlineVariant = Color(0xFFD2E2DA),
    // Container roles — without these, Material components (NavigationBar
    // indicator, FAB, chips) fall back to the baseline purple palette.
    primaryContainer = Color(0xFFD7E7DF),
    onPrimaryContainer = Color(0xFF12281F),
    secondaryContainer = Color(0xFFF2E4C8),
    onSecondaryContainer = Color(0xFF463311),
    tertiaryContainer = Color(0xFFF0DAD2),
    onTertiaryContainer = Color(0xFF3C2018),
    surfaceContainer = Color(0xFFF1EFE8),
    surfaceContainerHigh = Color(0xFFECE9E0),
    surfaceContainerHighest = Color(0xFFE6E3DA),
    surfaceContainerLow = Color(0xFFF5F3ED),
    surfaceContainerLowest = Color(0xFFFFFCF5),
    inverseSurface = Ink,
    inverseOnSurface = Paper,
    inversePrimary = MossDark,
)

private val DarkColors = darkColorScheme(
    primary = MossDark,
    onPrimary = Color(0xFF0C1410),
    secondary = GoldDark,
    tertiary = ClayDark,
    background = PaperDark,
    surface = CreamDark,
    onBackground = InkDark,
    onSurface = InkDark,
    onSurfaceVariant = MutedDark,
    outline = LineDark,
    surfaceVariant = Color(0xFF22332C),
    outlineVariant = Color(0xFF2E423A),
    primaryContainer = Color(0xFF24443A),
    onPrimaryContainer = Color(0xFFCBE3D8),
    secondaryContainer = Color(0xFF4A3A1C),
    onSecondaryContainer = Color(0xFFEFDFBC),
    tertiaryContainer = Color(0xFF4A2C22),
    onTertiaryContainer = Color(0xFFEBD2C8),
    surfaceContainer = Color(0xFF1C2620),
    surfaceContainerHigh = Color(0xFF222D27),
    surfaceContainerHighest = Color(0xFF28342D),
    surfaceContainerLow = Color(0xFF161F1A),
    surfaceContainerLowest = Color(0xFF101813),
    inverseSurface = InkDark,
    inverseOnSurface = PaperDark,
    inversePrimary = Moss,
)

@Composable
internal fun HighlighterTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    content: @Composable () -> Unit,
) {
    MaterialTheme(
        colorScheme = if (darkTheme) DarkColors else LightColors,
        typography = MaterialTheme.typography,
        content = content,
    )
}
