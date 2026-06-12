package com.highlighter.app.nip55

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.net.Uri
import androidx.activity.ComponentActivity
import androidx.activity.result.ActivityResultLauncher
import androidx.activity.result.contract.ActivityResultContracts
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json

// Vendored from the NMP registry (ADR-0048 Stage 2) with the package line
// changed to com.highlighter.app.nip55.

private val bridgeJson = Json {
    ignoreUnknownKeys = true
    isLenient = true
    classDiscriminator = "kind"
}

/**
 * D7 host adapter for the `external_signer` capability namespace.
 *
 * Receives fully-built `ExternalSignerRequest` objects from Rust (via
 * `nextSignerRequest()`), fires the right OS IPC mechanism (Intent round-trip
 * or ContentResolver fast-path), and reports raw results back via `onResult`
 * to be delivered via `deliverExternalSignerResponse`.
 *
 * Register in `Activity.onCreate` (before first `onStart`) via [register].
 * Call [unregister] in `onDestroy`.
 *
 * @param activity The host activity.
 * @param onResult Called with serialised `ExternalSignerResponse` JSON.
 *   Route back to kernel via `app.deliverExternalSignerResponse(responseJson)`.
 */
class ExternalSignerCapabilityBridge(
    private val activity: ComponentActivity,
    private val onResult: (responseJson: String) -> Unit,
) {

    @Volatile private var pendingCorrelationId: String? = null
    @Volatile private var pendingMethod: String? = null

    private var launcher: ActivityResultLauncher<Intent>? = null

    /**
     * Register the Activity Result launcher. Call from `Activity.onCreate`
     * BEFORE first `onStart`. Safe to call multiple times; subsequent calls
     * are no-ops.
     */
    fun register() {
        if (launcher != null) return
        launcher = activity.registerForActivityResult(
            ActivityResultContracts.StartActivityForResult(),
        ) { result ->
            val correlationId = pendingCorrelationId ?: return@registerForActivityResult
            pendingCorrelationId = null
            val method = pendingMethod ?: "unknown"
            pendingMethod = null

            val response = if (result.resultCode == Activity.RESULT_OK) {
                val data = result.data
                val rawResult = selectAmberResultValue(
                    method = method,
                    eventExtra = data?.getStringExtra("event"),
                    resultExtra = data?.getStringExtra("result"),
                )
                val signerPackage = data?.getStringExtra("package")
                when {
                    data?.getBooleanExtra("rejected", false) == true ->
                        ExternalSignerResponse(
                            correlationId = correlationId,
                            outcome = ExternalSignerOutcome.Rejected(
                                reason = "signer rejected the request",
                            ),
                        )
                    rawResult != null ->
                        ExternalSignerResponse(
                            correlationId = correlationId,
                            outcome = ExternalSignerOutcome.Ok(result = rawResult),
                            signerPackage = signerPackage.takeIf {
                                method == "get_public_key" && it != null
                            },
                        )
                    else ->
                        ExternalSignerResponse(
                            correlationId = correlationId,
                            outcome = ExternalSignerOutcome.Unavailable(
                                reason = "signer returned no result",
                            ),
                        )
                }
            } else {
                ExternalSignerResponse(
                    correlationId = correlationId,
                    outcome = ExternalSignerOutcome.Rejected(reason = "user cancelled"),
                )
            }
            onResult(bridgeJson.encodeToString(response))
        }
    }

    /** Unregister the launcher. Call from `Activity.onDestroy`. */
    fun unregister() {
        launcher?.unregister()
        launcher = null
    }

    /**
     * Parse a raw `ExternalSignerRequest` JSON string and dispatch.
     * Called from the signer-request drain loop in MainActivity.
     *
     * D6: malformed JSON is silently dropped; it degrades to timeout on the
     * Rust side (the correlation_id sender is never resolved).
     */
    fun handleJson(requestJson: String) {
        val request = try {
            bridgeJson.decodeFromString<ExternalSignerRequest>(requestJson)
        } catch (_: Exception) {
            return
        }
        if (shouldUseContentResolver(request)) {
            dispatchContentResolver(request)
        } else {
            dispatchIntent(request)
        }
    }

    private fun dispatchIntent(request: ExternalSignerRequest) {
        val intent = buildAmberSignerIntent(request)
        pendingCorrelationId = request.correlationId
        pendingMethod = request.method
        val l = launcher
        if (l != null) {
            l.launch(intent)
        } else {
            pendingCorrelationId = null
            pendingMethod = null
            reportUnavailable(request.correlationId, "capability bridge not registered")
        }
    }

    private fun dispatchContentResolver(request: ExternalSignerRequest) {
        val pkg = request.signerPackage ?: run {
            reportUnavailable(request.correlationId, "signer package unknown for ContentResolver path")
            return
        }
        val method = request.method.toNostrSignerMethod()
        val authority = "$pkg.$method"
        val uri = Uri.parse("content://$authority")
        val selectionArgs = arrayOf(
            request.payload,
            request.counterparty ?: "",
            request.currentUser ?: "",
        )
        try {
            val cursor = activity.contentResolver.query(
                uri,
                null,
                null,
                selectionArgs,
                null,
            )
            cursor?.use { c ->
                if (c.moveToFirst()) {
                    val resultCol = c.getColumnIndex("result")
                    val rawResult = if (resultCol >= 0) c.getString(resultCol) else null
                    if (rawResult != null) {
                        onResult(bridgeJson.encodeToString(ExternalSignerResponse(
                            correlationId = request.correlationId,
                            outcome = ExternalSignerOutcome.Ok(result = rawResult),
                        )))
                    } else {
                        reportUnavailable(request.correlationId, "ContentResolver returned null result")
                    }
                } else {
                    reportUnavailable(request.correlationId, "ContentResolver returned empty cursor")
                }
            } ?: reportUnavailable(request.correlationId, "ContentResolver returned null cursor")
        } catch (e: Exception) {
            reportUnavailable(request.correlationId, "ContentResolver error: ${e.message}")
        }
    }

    private fun reportUnavailable(correlationId: String, reason: String) {
        onResult(bridgeJson.encodeToString(ExternalSignerResponse(
            correlationId = correlationId,
            outcome = ExternalSignerOutcome.Unavailable(reason = reason),
        )))
    }

    companion object {
        /** Detect installed Nostr signer apps. */
        fun detect(context: Context): List<NostrSignerInfo> =
            detectInstalledSigners(context.packageManager)
    }
}
