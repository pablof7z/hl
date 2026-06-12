package com.highlighter.app

import uniffi.highlighter_core.DataChangeType
import uniffi.highlighter_core.Delta
import uniffi.highlighter_core.EventCallback

/**
 * Routes app-scope native capability notifications from Rust deltas — the
 * Android counterpart to iOS's `EventBridge`.
 *
 * **nostrdb is the source of truth.** The Rust core writes every event to
 * nostrdb, then emits a [Delta] carrying the `subscriptionId` that installed
 * the pump. `0` is reserved for app-scope deltas (signer state,
 * joined-communities summary); any non-zero id routes to a view-scoped
 * subscriber (none on Android yet).
 *
 * Registering this callback is what makes the app *connect* and *stay*
 * logged in: the NIP-46 `nostrconnect://` handshake fires `SignerConnected`
 * from a background tokio task, and without a wired callback that delta is
 * dropped — leaving the UI stuck at the pre-login state. iOS registers the
 * bridge before any login attempt; Android must do the same.
 */
class EventBridge(
    private val onSignerConnected: () -> Unit,
) : EventCallback {
    override fun onDataChanged(delta: Delta) {
        if (delta.subscriptionId != 0uL) return
        when (delta.change) {
            // NIP-46 signer completed the handshake — finish the login by
            // refreshing app chrome (matches iOS `completeLogin`).
            is DataChangeType.SignerConnected -> onSignerConnected()
            // RelayStatusChanged, BookmarksUpdated, etc. flow into the core's
            // own state recompute; the reconciler's onState already re-renders.
            else -> Unit
        }
    }
}
