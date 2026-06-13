package com.highlighter.app.nip55

import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * ADR-0048 — D7 contract + Stage-4 intent-shape regression tests for the
 * vendored NIP-55 [ExternalSignerCapabilityBridge] / [AmberIntentCodec].
 *
 * Vendored from the NMP gallery suite
 * (`apps/nmp-gallery/.../bridge/ExternalSignerCapabilityBridgeTest.kt`) with
 * the package line changed to `com.highlighter.app.nip55` and the Primal
 * known-signer assertion dropped (Highlighter only lists Amber).
 *
 * Why this exists: the original Android NIP-55 PRs (#3/#4) vendored the
 * bridge but its emulator E2E was skipped ("no emulator available"), so the
 * intent-shape contract that Amber depends on was never guarded. These pure
 * Kotlin tests pin the exact Stage-4 encoding (payload in the data URI;
 * `type`/`permissions` as extras; `{"type","kind"}` permission JSON;
 * `event` extra preference on sign_event replies) so a future vendor-drift
 * cannot silently re-break "Sign in with Amber". The OS seams themselves
 * (Intent dispatch, PackageManager visibility, the round-trip) are covered by
 * the emulator E2E recorded in the PR that adds this file.
 *
 * Pure Kotlin — no Activity, no PackageManager, no ContentProvider.
 */
class ExternalSignerCapabilityBridgeTest {

    private val json = Json {
        ignoreUnknownKeys = true
        isLenient = true
        classDiscriminator = "kind"
    }

    // ── ExternalSignerRequest round-trip ──────────────────────────────────

    @Test
    fun signEventRequestDeserialises() {
        val raw = """
            {
                "correlation_id": "abc123",
                "method": "sign_event",
                "payload": "{\"kind\":1,\"content\":\"hello\"}",
                "current_user": "deadbeef",
                "counterparty": null,
                "permissions": [],
                "signer_package": "com.greenart7c3.nostrsigner",
                "force_interactive": false
            }
        """.trimIndent()

        val req = json.decodeFromString<ExternalSignerRequest>(raw)
        assertEquals("abc123", req.correlationId)
        assertEquals("sign_event", req.method)
        assertEquals("deadbeef", req.currentUser)
        assertNull(req.counterparty)
        assertEquals("com.greenart7c3.nostrsigner", req.signerPackage)
        assertTrue(req.permissions.isEmpty())
    }

    @Test
    fun getPublicKeyRequestWithPermissionsDeserialises() {
        val raw = """
            {
                "correlation_id": "perm-req-1",
                "method": "get_public_key",
                "payload": "",
                "current_user": null,
                "permissions": [
                    {"kind": "sign_event:1"},
                    {"kind": "nip44_encrypt"},
                    {"kind": "nip44_decrypt"}
                ],
                "signer_package": null,
                "force_interactive": false
            }
        """.trimIndent()

        val req = json.decodeFromString<ExternalSignerRequest>(raw)
        assertEquals("get_public_key", req.method)
        assertNull(req.currentUser)
        assertNull(req.signerPackage)
        assertEquals(3, req.permissions.size)
        assertEquals("sign_event:1", req.permissions[0].kind)
        assertEquals("nip44_encrypt", req.permissions[1].kind)
        assertEquals("nip44_decrypt", req.permissions[2].kind)
    }

    @Test
    fun forceInteractiveDefaultsFalse() {
        val raw = """{"correlation_id":"x","method":"sign_event","payload":"{}"}"""
        val req = json.decodeFromString<ExternalSignerRequest>(raw)
        assertEquals(false, req.forceInteractive)
    }

    // ── ExternalSignerResponse round-trip ─────────────────────────────────

    @Test
    fun okResponseSerialises() {
        val resp = ExternalSignerResponse(
            correlationId = "abc123",
            outcome = ExternalSignerOutcome.Ok(result = "signedEventJsonHere"),
            signerPackage = null,
        )
        val encoded = json.encodeToString(ExternalSignerResponse.serializer(), resp)
        assertTrue(encoded.contains("\"ok\"") || encoded.contains("\"kind\":\"ok\""))
        assertTrue(encoded.contains("abc123"))
        assertTrue(encoded.contains("signedEventJsonHere"))
    }

    @Test
    fun rejectedResponseSerialises() {
        val resp = ExternalSignerResponse(
            correlationId = "abc123",
            outcome = ExternalSignerOutcome.Rejected(reason = "user cancelled"),
        )
        val encoded = json.encodeToString(ExternalSignerResponse.serializer(), resp)
        assertTrue(encoded.contains("user cancelled"))
    }

    @Test
    fun unavailableResponseSerialises() {
        val resp = ExternalSignerResponse(
            correlationId = "no-pkg",
            outcome = ExternalSignerOutcome.Unavailable(reason = "signer not installed"),
        )
        val encoded = json.encodeToString(ExternalSignerResponse.serializer(), resp)
        assertTrue(encoded.contains("signer not installed"))
    }

    @Test
    fun signerPackagePopulatedOnGetPublicKeyReply() {
        val resp = ExternalSignerResponse(
            correlationId = "gpk-1",
            outcome = ExternalSignerOutcome.Ok(result = "aabbccdd"),
            signerPackage = "com.greenart7c3.nostrsigner",
        )
        val encoded = json.encodeToString(ExternalSignerResponse.serializer(), resp)
        assertTrue(encoded.contains("com.greenart7c3.nostrsigner"))
    }

    // ── Transport-path selection logic ────────────────────────────────────
    //
    // These tests exercise the PRODUCTION `shouldUseContentResolver`
    // predicate — the exact function `handleJson()` branches on. Rule (D7,
    // mechanical): ContentResolver iff NOT forceInteractive AND
    // signerPackage != null AND the method's permission kind is in the batch.

    @Test
    fun contentResolverSelectedWhenPermissionGrantedAndNotForced() {
        val req = ExternalSignerRequest(
            correlationId = "cr-1",
            method = "nip44_encrypt",
            payload = "plaintext",
            currentUser = "pubkeyhex",
            signerPackage = "com.greenart7c3.nostrsigner",
            permissions = listOf(Nip55Permission("nip44_encrypt")),
            forceInteractive = false,
        )
        assertTrue(shouldUseContentResolver(req))
    }

    @Test
    fun intentSelectedWhenForceInteractive() {
        val req = ExternalSignerRequest(
            correlationId = "intent-1",
            method = "nip44_encrypt",
            payload = "plaintext",
            signerPackage = "com.greenart7c3.nostrsigner",
            permissions = listOf(Nip55Permission("nip44_encrypt")),
            forceInteractive = true,
        )
        assertTrue(!shouldUseContentResolver(req))
    }

    @Test
    fun intentSelectedWhenSignerPackageUnknown() {
        val req = ExternalSignerRequest(
            correlationId = "intent-2",
            method = "nip44_encrypt",
            payload = "plaintext",
            signerPackage = null, // unknown
            permissions = listOf(Nip55Permission("nip44_encrypt")),
            forceInteractive = false,
        )
        assertTrue(!shouldUseContentResolver(req))
    }

    @Test
    fun intentSelectedWhenPermissionNotGranted() {
        val req = ExternalSignerRequest(
            correlationId = "intent-3",
            method = "nip44_decrypt",
            payload = "ciphertext",
            signerPackage = "com.greenart7c3.nostrsigner",
            permissions = emptyList(), // no permissions granted
            forceInteractive = false,
        )
        assertTrue(!shouldUseContentResolver(req))
    }

    @Test
    fun contentResolverSelectedForSignEventWhenKindPermissionGranted() {
        // sign_event:1 grants sign_event for kind:1. The prefix-match should
        // recognise "sign_event:" as the permission kind for "sign_event".
        val req = ExternalSignerRequest(
            correlationId = "cr-sign-1",
            method = "sign_event",
            payload = "{\"kind\":1}",
            currentUser = "pubkeyhex",
            signerPackage = "com.greenart7c3.nostrsigner",
            permissions = listOf(Nip55Permission("sign_event:1")),
            forceInteractive = false,
        )
        assertTrue(shouldUseContentResolver(req))
    }

    // ── buildAmberPermissionsJsonInternal — Stage-4 regression ───────────
    //
    // Amber expects Intent extras with `[{"type":"sign_event","kind":1}]`,
    // not our internal `[{"kind":"sign_event:1"}]` format. These pin the
    // corrected encoding.

    @Test
    fun buildAmberPermissionsJson_signEvent_kindSplit() {
        val result = buildAmberPermissionsJsonInternal(listOf(Nip55Permission("sign_event:1")))
        assertEquals("""[{"type":"sign_event","kind":1}]""", result)
    }

    @Test
    fun buildAmberPermissionsJson_noColonMethod() {
        val result = buildAmberPermissionsJsonInternal(listOf(Nip55Permission("nip44_encrypt")))
        assertEquals("""[{"type":"nip44_encrypt"}]""", result)
    }

    @Test
    fun buildAmberPermissionsJson_multiplePermissions() {
        val perms = listOf(
            Nip55Permission("sign_event:1"),
            Nip55Permission("nip44_encrypt"),
            Nip55Permission("nip44_decrypt"),
        )
        val result = buildAmberPermissionsJsonInternal(perms)
        assertEquals(
            """[{"type":"sign_event","kind":1},{"type":"nip44_encrypt"},{"type":"nip44_decrypt"}]""",
            result,
        )
    }

    @Test
    fun buildAmberPermissionsJson_emptyList() {
        val result = buildAmberPermissionsJsonInternal(emptyList())
        assertEquals("[]", result)
    }

    @Test
    fun buildAmberPermissionsJson_getPublicKey() {
        val result = buildAmberPermissionsJsonInternal(listOf(Nip55Permission("get_public_key")))
        assertEquals("""[{"type":"get_public_key"}]""", result)
    }

    // ── selectAmberResultValue — Stage-4 sign_event regression ───────────
    //
    // Amber's RESULT_OK reply for `sign_event` carries the signature hex in
    // `result` and the FULL signed-event JSON in `event`. Rust verifies the
    // complete event (id + schnorr sig), so the bridge must hand back the
    // `event` extra for sign_event and `result` for everything else.

    @Test
    fun signEventPrefersEventExtra() {
        val signedJson = """{"id":"abc","pubkey":"def","sig":"012"}"""
        assertEquals(
            signedJson,
            selectAmberResultValue("sign_event", eventExtra = signedJson, resultExtra = "sighex"),
        )
    }

    @Test
    fun signEventFallsBackToResultWhenEventBlank() {
        assertEquals(
            "sighex",
            selectAmberResultValue("sign_event", eventExtra = "", resultExtra = "sighex"),
        )
        assertEquals(
            "sighex",
            selectAmberResultValue("sign_event", eventExtra = null, resultExtra = "sighex"),
        )
    }

    @Test
    fun getPublicKeyUsesResultExtra() {
        assertEquals(
            "pubkeyhex",
            selectAmberResultValue("get_public_key", eventExtra = "pubkeyhex", resultExtra = "pubkeyhex"),
        )
        assertEquals(
            "pubkeyhex",
            selectAmberResultValue("get_public_key", eventExtra = null, resultExtra = "pubkeyhex"),
        )
    }

    @Test
    fun encryptUsesResultExtra() {
        assertEquals(
            "ciphertext",
            selectAmberResultValue("nip44_encrypt", eventExtra = null, resultExtra = "ciphertext"),
        )
    }

    @Test
    fun missingExtrasYieldNull() {
        assertNull(selectAmberResultValue("sign_event", eventExtra = null, resultExtra = null))
        assertNull(selectAmberResultValue("get_public_key", eventExtra = "x", resultExtra = null))
    }

    // ── KNOWN_NOSTR_SIGNERS contract ──────────────────────────────────────

    @Test
    fun amberIsInKnownSigners() {
        val amber = KNOWN_NOSTR_SIGNERS.firstOrNull { it.intentScheme == "nostrsigner" }
        assertNotNull("Amber must be in KNOWN_NOSTR_SIGNERS", amber)
        assertEquals("com.greenart7c3.nostrsigner", amber!!.contentAuthority)
        // packageName must be set explicitly — the signer_package wire field
        // carries the APK identifier, not the ContentProvider authority. This
        // value MUST match the <package> entry in AndroidManifest.xml <queries>.
        assertEquals("com.greenart7c3.nostrsigner", amber.packageName)
    }
}
