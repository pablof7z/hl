package com.highlighter.app.ui.auth

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import uniffi.highlighter_core.HighlighterAppAction
import uniffi.highlighter_core.HighlighterCreateAccountSnapshot
import uniffi.highlighter_core.HighlighterUsernameStatus

/**
 * Full-screen account creation: display name + optional Nostr username with
 * live availability status. Reuses the exact `CreateAccountSnapshot` rules the
 * legacy `AuthPanels` form used (canSubmit / usernameStatus / isCreating).
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun CreateAccountScreen(
    createAccount: HighlighterCreateAccountSnapshot,
    onBack: () -> Unit,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    Scaffold(
        containerColor = MaterialTheme.colorScheme.background,
        topBar = {
            TopAppBar(
                title = { Text("Create account") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(
                            imageVector = Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = "Back",
                        )
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.background,
                    titleContentColor = MaterialTheme.colorScheme.onBackground,
                    navigationIconContentColor = MaterialTheme.colorScheme.onBackground,
                ),
            )
        },
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 24.dp, vertical = 16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text(
                text = "Pick a name to get started. A Nostr key is generated for you.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Spacer(modifier = Modifier.height(4.dp))
            OutlinedTextField(
                value = createAccount.displayName,
                onValueChange = { dispatch(HighlighterAppAction.SetCreateAccountDisplayName(it)) },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
                label = { Text("Display name") },
                keyboardOptions = KeyboardOptions(
                    capitalization = KeyboardCapitalization.Words,
                    keyboardType = KeyboardType.Text,
                    imeAction = ImeAction.Next,
                ),
            )
            OutlinedTextField(
                value = createAccount.username,
                onValueChange = { dispatch(HighlighterAppAction.SetCreateAccountUsername(it)) },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
                label = { Text("Username") },
                supportingText = { CreateAccountUsernameStatus(createAccount) },
                isError = createAccount.usernameStatus == HighlighterUsernameStatus.TAKEN ||
                    createAccount.usernameStatus == HighlighterUsernameStatus.INVALID ||
                    createAccount.usernameStatus == HighlighterUsernameStatus.ERROR,
                keyboardOptions = KeyboardOptions(
                    capitalization = KeyboardCapitalization.None,
                    keyboardType = KeyboardType.Ascii,
                    imeAction = ImeAction.Done,
                ),
                keyboardActions = KeyboardActions(
                    onDone = {
                        if (createAccount.canSubmit && !createAccount.isCreating) {
                            dispatch(HighlighterAppAction.SubmitCreateAccount)
                        }
                    },
                ),
            )
            createAccount.errorMessage?.let { message ->
                Text(
                    text = message,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.tertiary,
                )
            }
            Spacer(modifier = Modifier.height(4.dp))
            Button(
                onClick = { dispatch(HighlighterAppAction.SubmitCreateAccount) },
                modifier = Modifier.fillMaxWidth(),
                shape = RoundedCornerShape(8.dp),
                enabled = createAccount.canSubmit && !createAccount.isCreating &&
                    createAccount.usernameStatus != HighlighterUsernameStatus.CHECKING,
            ) {
                Text(if (createAccount.isCreating) "Creating..." else "Create account")
            }
        }
    }
}

@Composable
private fun CreateAccountUsernameStatus(createAccount: HighlighterCreateAccountSnapshot) {
    val text = when (createAccount.usernameStatus) {
        HighlighterUsernameStatus.CHECKING -> "Checking availability"
        HighlighterUsernameStatus.AVAILABLE -> createAccount.usernameIdentifier
        HighlighterUsernameStatus.TAKEN -> "Already taken"
        HighlighterUsernameStatus.INVALID -> "Only letters, numbers, - and _"
        HighlighterUsernameStatus.ERROR -> createAccount.errorMessage ?: "Could not check username"
        HighlighterUsernameStatus.IDLE -> if (createAccount.usernameIdentifier.isNotBlank()) {
            createAccount.usernameIdentifier
        } else {
            "Optional Nostr username"
        }
    }
    Text(text = text, color = MaterialTheme.colorScheme.onSurfaceVariant)
}
