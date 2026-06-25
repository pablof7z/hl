package com.highlighter.app

import android.content.Context
import android.content.SharedPreferences
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKey
import uniffi.highlighter_core.HighlighterSessionCredential
import uniffi.highlighter_core.NmpKeyringHandler
import org.json.JSONObject

/**
 * Encrypted persistence for the active session credential — the Android
 * counterpart to iOS's `AppSessionStore` (which leans on the Keychain).
 *
 * Rust owns authentication policy; this store only loads, persists, and
 * clears the raw credential so the user stays signed in across launches.
 * Exactly one of the two variants is ever stored — persisting one clears
 * the other, mirroring `AppSessionStore.persist`.
 *
 * Backed by [EncryptedSharedPreferences] (AES-256, keys held in the
 * AndroidKeyStore). If the encrypted store cannot be opened (e.g. the
 * underlying master key was invalidated by a credential reset), the prefs
 * file is wiped once and recreated so a corrupt keystore never bricks login.
 */
class SessionStore(context: Context) : NmpKeyringHandler {
    private val prefs: SharedPreferences = openEncryptedPrefs(context.applicationContext)

    fun storedCredential(): HighlighterSessionCredential? {
        prefs.getString(KEY_NSEC, null)?.takeIf { it.isNotBlank() }?.let {
            return HighlighterSessionCredential.Nsec(it)
        }
        prefs.getString(KEY_BUNKER, null)?.takeIf { it.isNotBlank() }?.let {
            return HighlighterSessionCredential.BunkerUri(it)
        }
        prefs.getString(KEY_NIP55_PACKAGE, null)?.takeIf { it.isNotBlank() }?.let {
            return HighlighterSessionCredential.Nip55SignerPackage(it)
        }
        return null
    }

    fun persist(credential: HighlighterSessionCredential) {
        when (credential) {
            is HighlighterSessionCredential.Nsec ->
                prefs.edit()
                    .putString(KEY_NSEC, credential.nsec)
                    .remove(KEY_BUNKER)
                    .remove(KEY_NIP55_PACKAGE)
                    .apply()
            is HighlighterSessionCredential.BunkerUri ->
                prefs.edit()
                    .putString(KEY_BUNKER, credential.uri)
                    .remove(KEY_NSEC)
                    .remove(KEY_NIP55_PACKAGE)
                    .apply()
            is HighlighterSessionCredential.Nip55SignerPackage ->
                prefs.edit()
                    .putString(KEY_NIP55_PACKAGE, credential.signerPackage)
                    .remove(KEY_NSEC)
                    .remove(KEY_BUNKER)
                    .apply()
        }
    }

    fun clear() {
        prefs.edit()
            .remove(KEY_NSEC)
            .remove(KEY_BUNKER)
            .remove(KEY_NIP55_PACKAGE)
            .apply()
    }

    override fun handleKeyringRequest(requestJson: String): String {
        return runCatching {
            val request = JSONObject(requestJson)
            val namespace = request.optString("namespace", "nmp.keyring.capability")
            val correlationId = request.optString("correlation_id", "")
            val payload = JSONObject(request.getString("payload_json"))
            val accountId = payload.getString("account_id")
            val key = "$KEY_NMP_PREFIX$accountId"
            when (payload.getString("op")) {
                "store" -> {
                    prefs.edit().putString(key, payload.getString("secret")).apply()
                    nmpEnvelope(namespace, correlationId, nmpResult("ok"))
                }
                "retrieve" -> {
                    val secret = prefs.getString(key, null)
                    if (secret == null) {
                        nmpEnvelope(namespace, correlationId, nmpResult("not_found"))
                    } else {
                        nmpEnvelope(namespace, correlationId, nmpResult("ok", secret = secret))
                    }
                }
                "delete" -> {
                    prefs.edit().remove(key).apply()
                    nmpEnvelope(namespace, correlationId, nmpResult("ok"))
                }
                else -> nmpEnvelope(namespace, correlationId, nmpResult("error", osStatus = -50))
            }
        }.getOrElse {
            nmpEnvelope("nmp.keyring.capability", "", nmpResult("error", osStatus = -50))
        }
    }

    private companion object {
        const val PREFS_FILE = "highlighter_session"
        const val KEY_NSEC = "nsec"
        const val KEY_BUNKER = "bunker_uri"
        const val KEY_NIP55_PACKAGE = "nip55_signer_package"
        const val KEY_NMP_PREFIX = "nmp.keyring."

        fun nmpResult(status: String, secret: String? = null, osStatus: Int? = null): String {
            val result = JSONObject().put("status", status)
            if (secret != null) result.put("secret", secret)
            if (osStatus != null) result.put("os_status", osStatus)
            return result.toString()
        }

        fun nmpEnvelope(namespace: String, correlationId: String, resultJson: String): String =
            JSONObject()
                .put("namespace", namespace)
                .put("correlation_id", correlationId)
                .put("result_json", resultJson)
                .toString()

        fun openEncryptedPrefs(context: Context): SharedPreferences {
            val masterKey = MasterKey.Builder(context)
                .setKeyScheme(MasterKey.KeyScheme.AES256_GCM)
                .build()
            return runCatching {
                buildEncryptedPrefs(context, masterKey)
            }.getOrElse {
                // A rotated/invalidated master key makes the existing blob
                // undecryptable. Drop it and start fresh rather than crashing.
                context.deleteSharedPreferences(PREFS_FILE)
                buildEncryptedPrefs(context, masterKey)
            }
        }

        fun buildEncryptedPrefs(context: Context, masterKey: MasterKey): SharedPreferences =
            EncryptedSharedPreferences.create(
                context,
                PREFS_FILE,
                masterKey,
                EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
                EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM,
            )
    }
}
