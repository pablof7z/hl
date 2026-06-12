package com.highlighter.app

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LifecycleEventEffect
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import com.highlighter.app.nip55.ExternalSignerCapabilityBridge
import com.highlighter.app.ui.RootScene
import com.highlighter.app.ui.theme.HighlighterTheme
import com.highlighter.app.util.LocalDispatch
import com.highlighter.app.util.LocalProfiles
import com.highlighter.app.util.LocalWebMetadata

class MainActivity : ComponentActivity() {
    private val viewModel: HighlighterViewModel by viewModels()

    // NIP-55 external signer bridge. Registered in onCreate (before onStart),
    // unregistered in onDestroy. The drain thread polls nextSignerRequest() and
    // hands each JSON string to the bridge for Intent/ContentResolver dispatch.
    private lateinit var signerBridge: ExternalSignerCapabilityBridge

    @Volatile private var drainRunning = false
    private var drainThread: Thread? = null

    // Re-emitted whenever a new ACTION_SEND arrives (initial launch or while the
    // app is already running via singleTop -> onNewIntent), so Compose can pick
    // it up in a LaunchedEffect and open the share composer.
    private val shareIntents = MutableStateFlow<Intent?>(null)

    // Re-emitted whenever a VIEW intent for a highlight deep link arrives
    // (https://beta.highlighter.com/highlight/{nevent} or
    // highlighter://highlight/{nevent}). Compose drains this in a LaunchedEffect
    // and asks the ViewModel to decode + route it.
    private val highlightDeepLinks = MutableStateFlow<String?>(null)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        // NIP-55 bridge must be registered before the first onStart (Android
        // Activity Result API requirement). The bridge fires onResult on the
        // main thread; we forward it to Rust on the main thread too (safe
        // because deliverExternalSignerResponse posts onto the actor inbox).
        signerBridge = ExternalSignerCapabilityBridge(
            activity = this,
            onResult = { responseJson ->
                viewModel.deliverExternalSignerResponse(responseJson)
            },
        )
        signerBridge.register()

        // Drain thread: polls nextSignerRequest() in a tight spin with a short
        // park when the channel is empty. Rust's sync_channel has capacity 16,
        // so a 20 ms poll period adds at most ~20 ms of latency per request.
        // No-polling rule applies to production data paths; this is the
        // OS-IPC boundary which has no blocking-recv equivalent across the
        // UniFFI boundary. Park duration is bounded; it does not grow.
        drainRunning = true
        drainThread = Thread {
            while (drainRunning) {
                val requestJson = viewModel.nextSignerRequest()
                if (requestJson != null) {
                    runOnUiThread { signerBridge.handleJson(requestJson) }
                } else {
                    Thread.sleep(20)
                }
            }
        }.also { it.name = "nip55-drain"; it.isDaemon = true; it.start() }

        enableEdgeToEdge()
        handleShareIntent(intent)
        handleViewIntent(intent)
        setContent {
            HighlighterTheme {
                // Bootstrap once: register the event bridge, dispatch Bootstrap,
                // and restore any stored session credential. The whole startup
                // choreography now lives in the ViewModel (see bootstrap()).
                DisposableEffect(viewModel) {
                    viewModel.bootstrap()
                    onDispose { }
                }
                LifecycleEventEffect(Lifecycle.Event.ON_RESUME) {
                    viewModel.appForegrounded()
                }
                val pendingShareIntent by shareIntents.asStateFlow().collectAsState()
                LaunchedEffect(pendingShareIntent) {
                    val shareIntent = pendingShareIntent ?: return@LaunchedEffect
                    viewModel.openShare(
                        text = shareIntent.getStringExtra(Intent.EXTRA_TEXT),
                        subject = shareIntent.getStringExtra(Intent.EXTRA_SUBJECT),
                    )
                    // Consume so a config change / recomposition won't re-open it.
                    shareIntents.value = null
                }
                val pendingDeepLink by highlightDeepLinks.asStateFlow().collectAsState()
                LaunchedEffect(pendingDeepLink) {
                    val token = pendingDeepLink ?: return@LaunchedEffect
                    viewModel.openHighlightDeepLink(token)
                    // Consume so a config change / recomposition won't re-route it.
                    highlightDeepLinks.value = null
                }
                val state by viewModel.state.collectAsState()
                val pendingShare by viewModel.pendingShare.collectAsState()
                // Provide host-side lookup tables (profiles, web-link previews)
                // and the dispatcher so leaf rows (comments, chat, discussions)
                // can resolve avatars/names and request + render link previews
                // without threading state through every intermediate composable.
                CompositionLocalProvider(
                    LocalProfiles provides state.profiles,
                    LocalWebMetadata provides state.webMetadata,
                    LocalDispatch provides viewModel::dispatch,
                ) {
                    RootScene(
                        state = state,
                        pendingShare = pendingShare,
                        dispatch = viewModel::dispatch,
                        onDismissShare = viewModel::dismissShare,
                    )
                }
            }
        }
    }

    override fun onDestroy() {
        drainRunning = false
        drainThread?.interrupt()
        drainThread = null
        signerBridge.unregister()
        super.onDestroy()
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        // Keep the activity's intent current and route shares + deep links.
        setIntent(intent)
        handleShareIntent(intent)
        handleViewIntent(intent)
    }

    /** Surfaces an ACTION_SEND with text content to the Compose layer. */
    private fun handleShareIntent(intent: Intent?) {
        if (intent?.action != Intent.ACTION_SEND) return
        val type = intent.type ?: return
        if (!type.startsWith("text/")) return
        if (intent.getStringExtra(Intent.EXTRA_TEXT).isNullOrBlank()) return
        shareIntents.value = intent
    }

    /**
     * Surfaces an ACTION_VIEW highlight share link to the Compose layer.
     *
     * Handles both the verified App Link
     * (https://beta.highlighter.com/highlight/{nevent}) and the custom-scheme
     * fallback (highlighter://highlight/{nevent}). The highlighter://nip46 signer
     * callback is intentionally ignored here — like iOS, that pairing is handled
     * by the relay subscription, not a route.
     */
    private fun handleViewIntent(intent: Intent?) {
        if (intent?.action != Intent.ACTION_VIEW) return
        val data = intent.data ?: return
        val token = extractHighlightToken(data) ?: return
        highlightDeepLinks.value = token
    }

    private fun extractHighlightToken(uri: Uri): String? {
        // nip46 signer callback — not a highlight, leave to relay pairing.
        if (uri.scheme == "highlighter" && uri.host == "nip46") return null

        val segments = uri.pathSegments.orEmpty()
        val token = when {
            // .../highlight/{token} — the https App Link
            // (host = beta.highlighter.com) and any deep custom-scheme path.
            segments.size >= 2 && segments[segments.size - 2] == "highlight" ->
                segments.last()
            // highlighter://highlight/{token} — Android maps the authority to
            // host="highlight" and the token becomes the first path segment.
            uri.host == "highlight" && segments.isNotEmpty() ->
                segments.first()
            else -> null
        }?.trim().orEmpty()

        if (token.isEmpty()) {
            Log.d(TAG, "VIEW intent carried no highlight token: $uri")
            return null
        }
        return token
    }

    private companion object {
        const val TAG = "HighlighterDeepLink"
    }
}
