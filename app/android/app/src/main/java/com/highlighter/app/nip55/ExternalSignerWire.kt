package com.highlighter.app.nip55

import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

// ── Wire types mirroring nmp-signer-iface ExternalSignerRequest/Response ──────
//
// Vendored from the NMP registry (ADR-0048 Stage 2) with the package line
// changed to com.highlighter.app.nip55.

/**
 * Mirror of `ExternalSignerRequest` from `nmp-signer-iface`.
 *
 * Rust builds this and serialises it as `CapabilityRequest.payload_json`.
 * The Kotlin host fires it and reports the raw result — it decides nothing (D7).
 */
@Serializable
data class ExternalSignerRequest(
    @SerialName("correlation_id") val correlationId: String,
    val method: String,
    val payload: String,
    @SerialName("current_user") val currentUser: String? = null,
    val counterparty: String? = null,
    val permissions: List<Nip55Permission> = emptyList(),
    @SerialName("signer_package") val signerPackage: String? = null,
    @SerialName("force_interactive") val forceInteractive: Boolean = false,
)

/** Mirror of `Nip55Permission` from `nmp-signer-iface`. */
@Serializable
data class Nip55Permission(val kind: String)

/**
 * Mirror of `ExternalSignerResponse` from `nmp-signer-iface`.
 *
 * The host fills this and hands it back to Rust via `deliverResponse`.
 * D7: raw results only, no interpretation.
 */
@Serializable
data class ExternalSignerResponse(
    @SerialName("correlation_id") val correlationId: String,
    val outcome: ExternalSignerOutcome,
    @SerialName("signer_package") val signerPackage: String? = null,
)

/** Wire shape for `ExternalSignerOutcome` (tagged by `kind`). */
@Serializable
sealed class ExternalSignerOutcome {
    @Serializable
    @SerialName("ok")
    data class Ok(val result: String) : ExternalSignerOutcome()

    @Serializable
    @SerialName("rejected")
    data class Rejected(val reason: String) : ExternalSignerOutcome()

    @Serializable
    @SerialName("unavailable")
    data class Unavailable(val reason: String) : ExternalSignerOutcome()

    @Serializable
    @SerialName("signer_error")
    data class SignerError(val reason: String) : ExternalSignerOutcome()
}

// ── Known signer descriptors ──────────────────────────────────────────────────

/**
 * Describes one locally-detectable Nostr signer app.
 *
 * All package names listed here MUST also appear in the app's `<queries>` block
 * in `AndroidManifest.xml` — Android 11+ (API 30+) returns an empty result
 * even when the app is installed without it.
 */
data class NostrSignerInfo(
    val displayName: String,
    val intentScheme: String,
    val contentAuthority: String? = null,
    val packageName: String? = null,
    val installHint: String = "Install $displayName for one-tap sign-in",
)

/**
 * Ordered list of signers this detector knows about.
 * All `intentScheme` values MUST be mirrored in `<queries>` in AndroidManifest.xml.
 */
val KNOWN_NOSTR_SIGNERS: List<NostrSignerInfo> = listOf(
    NostrSignerInfo(
        displayName = "Amber",
        intentScheme = "nostrsigner",
        contentAuthority = "com.greenart7c3.nostrsigner",
        packageName = "com.greenart7c3.nostrsigner",
        installHint = "Install Amber for one-tap sign-in",
    ),
)

/**
 * Probes `PackageManager` for installed Nostr signer apps.
 * MUST be called on the main thread.
 */
fun detectInstalledSigners(packageManager: PackageManager): List<NostrSignerInfo> {
    return KNOWN_NOSTR_SIGNERS.filter { signer ->
        val probe = Intent(Intent.ACTION_VIEW, Uri.parse("${signer.intentScheme}://"))
        @Suppress("DEPRECATION")
        val handlers = packageManager.queryIntentActivities(probe, PackageManager.MATCH_DEFAULT_ONLY)
        handlers.isNotEmpty()
    }
}
