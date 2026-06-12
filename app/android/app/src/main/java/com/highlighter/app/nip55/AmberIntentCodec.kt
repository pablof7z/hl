package com.highlighter.app.nip55

import android.content.Intent
import android.net.Uri

// ── Amber / NIP-55 encoding + decoding ────────────────────────────────────────
//
// Vendored from the NMP registry (ADR-0048 Stage 2) with the package line
// changed to com.highlighter.app.nip55.
// Pure top-level functions — no test-side copies needed.

/**
 * THE transport-selection rule (ADR-0048 D2) — mechanical consequence of
 * fields Rust set on the request, never host policy (D7).
 */
internal fun shouldUseContentResolver(request: ExternalSignerRequest): Boolean =
    !request.forceInteractive &&
        request.signerPackage != null &&
        request.permissions.any { p -> p.kind.startsWith(request.method.toPermissionKind()) }

/**
 * Select the reply value from an Amber `RESULT_OK` Intent.
 * `sign_event` must return the full event JSON (`event` extra);
 * everything else uses `result`.
 */
internal fun selectAmberResultValue(
    method: String,
    eventExtra: String?,
    resultExtra: String?,
): String? = if (method == "sign_event") {
    eventExtra.takeUnless { it.isNullOrBlank() } ?: resultExtra
} else {
    resultExtra
}

/**
 * Build the Amber-specific permissions JSON array from a `Nip55Permission` list.
 * Internal format `kind` = combined string like `"sign_event:1"`, `"nip44_encrypt"`.
 * Amber expects `[{"type":"sign_event","kind":1},{"type":"nip44_encrypt"}]`.
 */
internal fun buildAmberPermissionsJsonInternal(permissions: List<Nip55Permission>): String {
    val sb = StringBuilder("[")
    permissions.forEachIndexed { idx, perm ->
        if (idx > 0) sb.append(",")
        val combined = perm.kind
        val colonIdx = combined.indexOf(':')
        if (colonIdx >= 0) {
            val typePart = combined.substring(0, colonIdx)
            val kindPart = combined.substring(colonIdx + 1).toIntOrNull()
            if (kindPart != null) {
                sb.append("""{"type":"$typePart","kind":$kindPart}""")
            } else {
                sb.append("""{"type":"$typePart"}""")
            }
        } else {
            sb.append("""{"type":"$combined"}""")
        }
    }
    sb.append("]")
    return sb.toString()
}

/**
 * Build the NIP-55 Intent for one `ExternalSignerRequest`.
 *
 * Amber (v6.x):
 *   intent.data          = nostrsigner:<Uri.encode(payload)>
 *   extras["type"]       = method tag string
 *   extras["id"]         = caller request id
 *   extras["returnType"] = "signature" | "event"
 *   extras["current_user"] = current user pubkey hex (if known)
 *   extras["pubkey"]     = counterparty pubkey hex (encrypt/decrypt)
 *   extras["permissions"] = JSON array string (first call only)
 */
internal fun buildAmberSignerIntent(request: ExternalSignerRequest): Intent {
    val methodTag = request.method.toNostrSignerMethod()
    val uriPayload = if (request.payload.isNotEmpty()) Uri.encode(request.payload) else ""
    val intent = Intent(Intent.ACTION_VIEW, Uri.parse("nostrsigner:$uriPayload"))
    intent.putExtra("type", methodTag)
    intent.putExtra("id", request.correlationId)
    intent.putExtra("returnType", "signature")
    if (request.currentUser != null) {
        intent.putExtra("current_user", request.currentUser)
    }
    if (request.counterparty != null) {
        intent.putExtra("pubkey", request.counterparty)
    }
    if (request.permissions.isNotEmpty()) {
        intent.putExtra("permissions", buildAmberPermissionsJsonInternal(request.permissions))
    }
    request.signerPackage?.let { pkg -> intent.setPackage(pkg) }
    intent.putExtra("nmp_correlation_id", request.correlationId)
    return intent
}

// ── Method mapping helpers ────────────────────────────────────────────────────

internal fun String.toNostrSignerMethod(): String = when (this) {
    "get_public_key" -> "get_public_key"
    "sign_event" -> "sign_event"
    "nip44_encrypt" -> "nip44_encrypt"
    "nip44_decrypt" -> "nip44_decrypt"
    "nip04_encrypt" -> "nip04_encrypt"
    "nip04_decrypt" -> "nip04_decrypt"
    else -> this
}

internal fun String.toPermissionKind(): String = when (this) {
    "sign_event" -> "sign_event:"
    "nip44_encrypt" -> "nip44_encrypt"
    "nip44_decrypt" -> "nip44_decrypt"
    "nip04_encrypt" -> "nip04_encrypt"
    "nip04_decrypt" -> "nip04_decrypt"
    else -> this
}
