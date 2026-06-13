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
import uniffi.highlighter_core.HighlighterSignerRequestDrain
import com.highlighter.app.ui.RootScene
import com.highlighter.app.ui.theme.HighlighterTheme
import com.highlighter.app.util.LocalDispatch
import com.highlighter.app.util.LocalProfiles
import com.highlighter.app.util.LocalWebMetadata

class MainActivity : ComponentActivity() {
    private val viewModel: HighlighterViewModel by viewModels()

    // NIP-55 external signer bridge. Registered in onCreate (before onStart),
    // unregistered in onDestroy. The drain thread loops on the BLOCKING
    // nextSignerRequest() drain and hands each JSON payload to the bridge for
    // Intent/ContentResolver dispatch.
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

        // Force ViewModel creation HERE, on the main thread, before the drain
        // thread can touch the lazy `by viewModels()` delegate: ViewModelStore
        // access is main-thread-only, and a background-thread first access
        // throws IllegalStateException — which would silently kill the drain
        // loop (observed: Amber Intent never fired because the drain thread
        // lost the init race and died on its first call).
        val vm = viewModel

        // NIP-55 bridge must be registered before the first onStart (Android
        // Activity Result API requirement). The bridge fires onResult on the
        // main thread; we forward it to Rust on the main thread too (safe
        // because deliverExternalSignerResponse posts onto the actor inbox).
        signerBridge = ExternalSignerCapabilityBridge(
            activity = this,
            onResult = { responseJson ->
                vm.deliverExternalSignerResponse(responseJson)
            },
        )
        signerBridge.register()

        // Drain thread: BLOCKING timed drain (D8 — no polling). Each
        // nextSignerRequest() call parks INSIDE the Rust channel's
        // recv_timeout (≤250 ms tick); a request arriving while parked wakes
        // the thread immediately, and the Idle tick exists only so the
        // drainRunning flag is observed with bounded latency on activity
        // teardown. Closed (channel sender gone = session teardown) and an
        // app handle destroyed mid-call both terminate the loop.
        drainRunning = true
        drainThread = Thread {
            drain@ while (drainRunning) {
                val drained = try {
                    vm.nextSignerRequest()
                } catch (_: IllegalStateException) {
                    // UniFFI object destroyed (ViewModel cleared) — stop.
                    break@drain
                }
                when (drained) {
                    is HighlighterSignerRequestDrain.Request ->
                        runOnUiThread { signerBridge.handleJson(drained.requestJson) }
                    is HighlighterSignerRequestDrain.Idle -> Unit // parked in channel wait
                    is HighlighterSignerRequestDrain.Closed -> break@drain
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
        // The drain thread observes the flag on its next Idle tick (≤250 ms;
        // it is parked inside the Rust recv_timeout, which Java interrupt
        // cannot unblock — the bounded tick IS the teardown latency).
        drainRunning = false
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
