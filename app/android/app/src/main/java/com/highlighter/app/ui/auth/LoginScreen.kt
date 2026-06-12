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
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import com.highlighter.app.nip55.ExternalSignerCapabilityBridge
import uniffi.highlighter_core.HighlighterAppAction
import uniffi.highlighter_core.HighlighterAuthSnapshot

/** NIP-46 callback URL — unchanged from the legacy sign-in panel. */
private const val NOSTR_CONNECT_CALLBACK = "highlighter://nip46"

/**
 * Full-screen sign-in: nsec field (with paste), "Continue with signer"
 * (NIP-46 `StartNostrConnect`), "Sign in with Amber" (NIP-55), and a path
 * back to the welcome screen. The Amber button only renders when Amber (or
 * another NIP-55 signer) is detected via PackageManager.
 * Loading state comes from [auth]; sign-in errors surface via the global
 * toast host on the root scene.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun LoginScreen(
    auth: HighlighterAuthSnapshot,
    onBack: () -> Unit,
    dispatch: (HighlighterAppAction) -> Unit,
) {
    var nsec by remember { mutableStateOf("") }
    val clipboard = LocalClipboardManager.current
    val context = LocalContext.current

    // Detect installed NIP-55 signers once per composition. The list is
    // stable for the life of the screen (the user can't install apps while
    // this screen is visible); no LaunchedEffect needed.
    val installedSigners = remember {
        ExternalSignerCapabilityBridge.detect(context)
    }

    Scaffold(
        containerColor = MaterialTheme.colorScheme.background,
        topBar = {
            TopAppBar(
                title = { Text("Sign in") },
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
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Text(
                text = "Sign in with your Nostr key or pair a remote signer.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Spacer(modifier = Modifier.height(4.dp))

            // NIP-55: one button per detected signer app (typically just Amber).
            // The button dispatches SignInNip55 with the signer's package name so
            // Rust can route subsequent signing requests to the right app.
            // persist=true: this is a fresh interactive login.
            installedSigners.forEach { signer ->
                OutlinedButton(
                    onClick = {
                        dispatch(
                            HighlighterAppAction.SignInNip55(
                                signerPackage = signer.packageName,
                                persist = true,
                                clearStoredOnFailure = false,
                            ),
                        )
                    },
                    modifier = Modifier.fillMaxWidth(),
                    shape = RoundedCornerShape(8.dp),
                    enabled = !auth.isSigningIn,
                ) {
                    Text("Sign in with ${signer.displayName}")
                }
            }

            OutlinedButton(
                onClick = { dispatch(HighlighterAppAction.StartNostrConnect(NOSTR_CONNECT_CALLBACK)) },
                modifier = Modifier.fillMaxWidth(),
                shape = RoundedCornerShape(8.dp),
                enabled = !auth.isSigningIn,
            ) {
                Text("Continue with signer")
            }
            Text(
                text = "or",
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                fontWeight = FontWeight.Medium,
            )
            OutlinedTextField(
                value = nsec,
                onValueChange = { nsec = it.trim() },
                modifier = Modifier.fillMaxWidth(),
                singleLine = true,
                label = { Text("nsec") },
                visualTransformation = PasswordVisualTransformation(),
                keyboardOptions = KeyboardOptions(
                    capitalization = KeyboardCapitalization.None,
                    keyboardType = KeyboardType.Password,
                ),
                trailingIcon = {
                    TextButton(onClick = {
                        clipboard.getText()?.text?.trim()?.let { pasted ->
                            if (pasted.isNotEmpty()) nsec = pasted
                        }
                    }) {
                        Text("Paste")
                    }
                },
            )
            Button(
                onClick = {
                    val value = nsec.trim()
                    if (value.isNotEmpty()) {
                        // Fresh UI login: persist=true so the session survives
                        // relaunch; clearStoredOnFailure=false (nothing stored yet).
                        dispatch(HighlighterAppAction.SignInNsec(value, true, false))
                        nsec = ""
                    }
                },
                modifier = Modifier.fillMaxWidth(),
                shape = RoundedCornerShape(8.dp),
                enabled = !auth.isSigningIn && nsec.isNotBlank(),
            ) {
                Text(if (auth.isSigningIn) "Signing in..." else "Sign in")
            }
        }
    }
}
