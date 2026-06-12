package com.highlighter.app.ui.auth

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import uniffi.highlighter_core.HighlighterAppAction
import uniffi.highlighter_core.HighlighterOnboardingInterest
import uniffi.highlighter_core.HighlighterOnboardingSnapshot

/**
 * Full-screen interest picker shown to a logged-in user who has not yet
 * completed onboarding (RootScene gate). Continue enables once the core's
 * `canFinish` rule (>= minimumSelectionCount) is met. Replaces the legacy
 * inline `OnboardingInterestsPanel`.
 */
@Composable
@OptIn(ExperimentalLayoutApi::class)
internal fun OnboardingInterestsScreen(
    onboarding: HighlighterOnboardingSnapshot,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    Surface(
        modifier = Modifier.fillMaxSize(),
        color = MaterialTheme.colorScheme.background,
    ) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 24.dp, vertical = 32.dp),
        ) {
            Text(
                text = "What do you read?",
                style = MaterialTheme.typography.headlineSmall,
                fontWeight = FontWeight.Bold,
                color = MaterialTheme.colorScheme.onBackground,
            )
            Spacer(modifier = Modifier.height(8.dp))
            Text(
                text = "Pick at least ${onboarding.minimumSelectionCount} to pre-fill your feed with highlights from readers like you.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Spacer(modifier = Modifier.height(20.dp))
            FlowRow(
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                onboarding.interests.forEach { interest ->
                    OnboardingInterestChip(interest = interest, dispatch = dispatch)
                }
            }
            Spacer(modifier = Modifier.height(20.dp))
            if (onboarding.remainingSelectionCount > 0u) {
                Text(
                    text = "Choose ${onboarding.remainingSelectionCount} more",
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Spacer(modifier = Modifier.height(8.dp))
            }
            Button(
                onClick = { dispatch(HighlighterAppAction.CompleteOnboarding) },
                enabled = onboarding.canFinish && !onboarding.isFinishing,
                modifier = Modifier.fillMaxWidth(),
                shape = RoundedCornerShape(10.dp),
            ) {
                Text(if (onboarding.isFinishing) "Finishing..." else "Start exploring")
            }
        }
    }
}

@Composable
private fun OnboardingInterestChip(
    interest: HighlighterOnboardingInterest,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    OutlinedButton(
        onClick = { dispatch(HighlighterAppAction.ToggleOnboardingInterest(interest.id)) },
        shape = RoundedCornerShape(8.dp),
        border = BorderStroke(
            1.dp,
            if (interest.selected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outline,
        ),
        colors = ButtonDefaults.outlinedButtonColors(
            containerColor = if (interest.selected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.surface,
            contentColor = if (interest.selected) MaterialTheme.colorScheme.onPrimary else MaterialTheme.colorScheme.onSurface,
        ),
    ) {
        Text(
            text = "${interest.emoji} ${interest.label}",
            style = MaterialTheme.typography.labelLarge,
            fontWeight = if (interest.selected) FontWeight.SemiBold else FontWeight.Normal,
        )
    }
}
