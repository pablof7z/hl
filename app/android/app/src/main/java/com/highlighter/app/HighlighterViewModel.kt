package com.highlighter.app

import android.app.Application
import android.content.Context
import android.content.Intent
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import android.net.Uri
import android.util.Log
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import kotlin.time.Duration.Companion.milliseconds
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.sample
import kotlinx.coroutines.flow.stateIn
import com.highlighter.app.ui.share.firstUrlIn
import uniffi.highlighter_core.HighlighterAppAction
import uniffi.highlighter_core.HighlighterAppConfig
import uniffi.highlighter_core.HighlighterAppReconciler
import uniffi.highlighter_core.HighlighterAppState
import uniffi.highlighter_core.HighlighterNmpApp
import uniffi.highlighter_core.HighlighterSessionCredential
import uniffi.highlighter_core.NostrEntityRef
import uniffi.highlighter_core.initPlatformLogging
import java.io.File

class HighlighterViewModel(application: Application) :
    AndroidViewModel(application),
    HighlighterAppReconciler {
    private val connectivityManager =
        application.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
    private var networkCallback: ConnectivityManager.NetworkCallback? = null

    // Route the Rust core's tracing output to logcat (tag: "highlighter-core")
    // before constructing the app, so relay/login activity is visible from the
    // very first delta. Inspect with: adb logcat -s highlighter-core
    private val loggingInitialized = run { initPlatformLogging() }

    private val app = HighlighterNmpApp(
        HighlighterAppConfig(
            dataDir = File(application.filesDir, "highlighter-core").absolutePath,
            visibleLimit = 250u,
            emitHz = 30u,
        ),
    )

    /** Encrypted credential persistence — keeps the user signed in across launches. */
    private val sessionStore = SessionStore(application)

    /**
     * Bridges core deltas (signer connection, live data) back to the app.
     * Registered before login so the NIP-46 `SignerConnected` delta is never
     * dropped. Held so we can clear it on logout, matching iOS.
     */
    private var eventBridge: EventBridge? = null
    private var hasBootstrapped = false

    // Track the last wifiOnlyEnabled value so onState can skip the OS-level
    // ConnectivityManager registration call on every emit and only invoke it
    // when the setting actually changes.
    private var lastWifiOnly: Boolean? = null

    private val _state = MutableStateFlow(app.state())

    // Coalesce full-state snapshots so Compose recomposes at most once per
    // frame (~16 ms) instead of once per resolved hydration op. `sample` drops
    // intermediate emissions during a burst and delivers only the latest;
    // `stateIn(Eagerly)` keeps the hot StateFlow contract for `collectAsState`.
    @OptIn(kotlinx.coroutines.FlowPreview::class)
    val state: StateFlow<HighlighterAppState> by lazy {
        _state
            .sample(16.milliseconds)
            .stateIn(viewModelScope, SharingStarted.Eagerly, _state.value)
    }

    // Presentation-only state for an inbound ACTION_SEND. The Rust core exposes
    // no "composer is open" / "shared payload" field — its share composer
    // snapshot only tracks the publish lifecycle — so, like the iOS share flow
    // that keeps the incoming URL in its view, we hold the pending payload here.
    private val _pendingShare = MutableStateFlow<PendingShare?>(null)
    val pendingShare: StateFlow<PendingShare?> = _pendingShare.asStateFlow()

    init {
        app.listenForUpdates(this)
        val initialWifiOnly = _state.value.network.wifiOnlyEnabled
        lastWifiOnly = initialWifiOnly
        syncNetworkCallback(initialWifiOnly)
    }

    fun bootstrap() {
        if (hasBootstrapped) return
        hasBootstrapped = true

        // Register the EventBridge unconditionally, before any login attempt.
        // The NIP-46 nostrconnect:// flow fires `SignerConnected` from a
        // background tokio task; if no callback is wired by then, the delta is
        // dropped silently and the UI never transitions to logged-in.
        registerEventBridge()
        app.dispatch(HighlighterAppAction.Bootstrap)

        // Restore a previously saved credential and sign back in. Restore uses
        // persist=false (already stored) and clearStoredOnFailure=true so a
        // stale/revoked credential is dropped rather than retried forever —
        // mirrors iOS `dispatchStoredCredential`.
        sessionStore.storedCredential()?.let { credential ->
            when (credential) {
                is HighlighterSessionCredential.Nsec ->
                    app.dispatch(
                        HighlighterAppAction.SignInNsec(
                            nsec = credential.nsec,
                            persist = false,
                            clearStoredOnFailure = true,
                        ),
                    )
                is HighlighterSessionCredential.BunkerUri ->
                    app.dispatch(
                        HighlighterAppAction.PairBunker(
                            uri = credential.uri,
                            persist = false,
                            clearStoredOnFailure = true,
                        ),
                    )
            }
        }
    }

    fun appForegrounded() {
        app.dispatch(HighlighterAppAction.AppForegrounded)
    }

    fun dispatch(action: HighlighterAppAction) {
        app.dispatch(action)
    }

    /**
     * Open the share composer for an inbound ACTION_SEND. Extracts the first
     * link from the shared text (falling back to the raw text for display) and
     * pre-fills the note from EXTRA_SUBJECT when it is not itself the link.
     */
    fun openShare(text: String?, subject: String?) {
        val body = text?.trim().orEmpty()
        if (body.isEmpty()) return
        val url = firstUrlIn(body)
        val note = subject?.trim().orEmpty().takeIf { it.isNotEmpty() && it != url } ?: ""
        // Reset any stale publish lifecycle from a previous share before opening.
        app.dispatch(HighlighterAppAction.ClearShareComposerError)
        app.dispatch(HighlighterAppAction.ClearShareComposerResult)
        _pendingShare.value = PendingShare(text = body, url = url, note = note)
    }

    fun dismissShare() {
        _pendingShare.value = null
        app.dispatch(HighlighterAppAction.ClearShareComposerError)
        app.dispatch(HighlighterAppAction.ClearShareComposerResult)
    }

    /**
     * Route an inbound highlight share link (https://beta.highlighter.com/highlight/{nevent}
     * or highlighter://highlight/{nevent}). The [token] is the bech32 identifier
     * carried in the URL path.
     *
     * The core exposes `decodeNostrEntity` (NIP-19 → [NostrEntityRef]) on the app
     * facade. For an `nevent`/`note` we get the event id plus an optional kind
     * hint, which we route into the existing comments/detail overlay via
     * [HighlighterAppAction.OpenComments] using the "e" root tag — the same
     * affordance room highlights already use to surface their detail. iOS does
     * not (yet) route these inbound links itself (App.swift only handles the
     * share-extension handoff and the nip46 callback), so this is the first
     * shared-link consumer across the native shells.
     *
     * Profiles and addresses are accepted but currently have no dedicated route;
     * see the TODO below.
     */
    fun openHighlightDeepLink(token: String) {
        val trimmed = token.trim().removePrefix("nostr:")
        if (trimmed.isEmpty()) return
        val entity = runCatching { app.decodeNostrEntity(trimmed) }
            .getOrElse { error ->
                Log.w(TAG, "failed to decode deep-link entity '$trimmed'", error)
                null
            } ?: run {
                Log.w(TAG, "deep-link entity '$trimmed' was not a recognized NIP-19 identifier")
                return
            }
        when (entity) {
            is NostrEntityRef.Event -> {
                // Highlights are kind 9802; honor an explicit nevent kind hint
                // when present, otherwise default to the highlight kind.
                val kind = (entity.kindHint ?: HIGHLIGHT_EVENT_KIND).toUShort()
                app.dispatch(HighlighterAppAction.OpenComments("e", entity.eventIdHex, kind))
            }
            is NostrEntityRef.Profile -> {
                // A profile link could open the profile overlay; the share-link
                // surface only mints highlight links today, so route it anyway.
                app.dispatch(HighlighterAppAction.OpenProfile(entity.pubkeyHex))
            }
            is NostrEntityRef.Address -> {
                // TODO(deep-links): addressable events (naddr) have no single
                // dispatch target yet — articles open via OpenArticleReader
                // (pubkey + dTag), which we could map here once the share surface
                // starts minting naddr links. No-op for now.
                Log.i(TAG, "naddr deep link not yet routable: ${entity.pubkeyHex}")
            }
        }
    }

    /** Tear down the signer session and forget the stored credential. */
    fun logout() {
        app.clearCoreEventCallback()
        app.dispatch(HighlighterAppAction.Logout)
        eventBridge = null
        sessionStore.clear()
        // Re-arm the bridge so a subsequent sign-in on the same launch still
        // receives the SignerConnected delta.
        registerEventBridge()
    }

    override fun onState(state: HighlighterAppState) {
        _state.value = state
        // Guard against redundant OS-level ConnectivityManager calls: the core
        // emits full state snapshots on every resolved op, so onState runs at
        // high frequency on the actor thread. Only update the network callback
        // registration when wifiOnlyEnabled actually changes.
        val wifiOnly = state.network.wifiOnlyEnabled
        if (wifiOnly != lastWifiOnly) {
            lastWifiOnly = wifiOnly
            syncNetworkCallback(wifiOnly)
        }
    }

    override fun onPersistSessionCredential(credential: HighlighterSessionCredential) {
        sessionStore.persist(credential)
    }

    override fun onClearSessionCredentials() {
        sessionStore.clear()
    }

    override fun onOpenExternalUrl(url: String) {
        openExternalUrl(url)
    }

    private fun registerEventBridge() {
        val bridge = EventBridge(
            onSignerConnected = {
                // Finish the NIP-46 login: re-arm the bridge defensively, then
                // refresh app chrome to surface the new session (iOS completeLogin).
                if (eventBridge == null) registerEventBridge()
                app.dispatch(HighlighterAppAction.RefreshAppChrome)
            },
        )
        app.setCoreEventCallback(bridge)
        eventBridge = bridge
    }

    private fun openExternalUrl(url: String) {
        val intent = Intent(Intent.ACTION_VIEW, Uri.parse(url))
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        val context = getApplication<Application>()
        val accepted = intent.resolveActivity(context.packageManager) != null &&
            runCatching { context.startActivity(intent) }.isSuccess
        if (!accepted) {
            app.dispatch(HighlighterAppAction.ExternalUrlOpenFailed(url))
        }
    }

    override fun onCleared() {
        syncNetworkCallback(false)
        app.close()
    }

    private fun syncNetworkCallback(wifiOnlyEnabled: Boolean) {
        if (wifiOnlyEnabled) {
            if (networkCallback != null) return
            val callback = object : ConnectivityManager.NetworkCallback() {
                override fun onAvailable(network: Network) {
                    reportActiveNetworkWifi()
                }

                override fun onCapabilitiesChanged(
                    network: Network,
                    networkCapabilities: NetworkCapabilities,
                ) {
                    reportActiveNetworkWifi()
                }

                override fun onLost(network: Network) {
                    reportActiveNetworkWifi()
                }
            }
            connectivityManager.registerNetworkCallback(
                NetworkRequest.Builder()
                    .addCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
                    .build(),
                callback,
            )
            networkCallback = callback
            reportActiveNetworkWifi()
        } else {
            networkCallback?.let { callback ->
                runCatching { connectivityManager.unregisterNetworkCallback(callback) }
            }
            networkCallback = null
        }
    }

    private fun reportActiveNetworkWifi() {
        val active = connectivityManager.activeNetwork
        val caps = active?.let { connectivityManager.getNetworkCapabilities(it) }
        val isWifi = caps?.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) == true
        app.dispatch(HighlighterAppAction.NetworkPathChanged(isWifi))
    }

    private companion object {
        const val TAG = "HighlighterViewModel"

        // NIP-23/highlight kind minted by the core share-link surface
        // (see app/core/src/share_links.rs HIGHLIGHT_EVENT_KIND).
        val HIGHLIGHT_EVENT_KIND: UInt = 9802u
    }
}
