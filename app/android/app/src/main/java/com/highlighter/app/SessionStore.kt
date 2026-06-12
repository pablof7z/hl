package com.highlighter.app

import android.content.Context
import android.content.SharedPreferences
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKey
import uniffi.highlighter_core.HighlighterSessionCredential

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
class SessionStore(context: Context) {
    private val prefs: SharedPreferences = openEncryptedPrefs(context.applicationContext)

    fun storedCredential(): HighlighterSessionCredential? {
        prefs.getString(KEY_NSEC, null)?.takeIf { it.isNotBlank() }?.let {
            return HighlighterSessionCredential.Nsec(it)
        }
        prefs.getString(KEY_BUNKER, null)?.takeIf { it.isNotBlank() }?.let {
            return HighlighterSessionCredential.BunkerUri(it)
        }
        return null
    }

    fun persist(credential: HighlighterSessionCredential) {
        when (credential) {
            is HighlighterSessionCredential.Nsec ->
                prefs.edit()
                    .putString(KEY_NSEC, credential.nsec)
                    .remove(KEY_BUNKER)
                    .apply()
            is HighlighterSessionCredential.BunkerUri ->
                prefs.edit()
                    .putString(KEY_BUNKER, credential.uri)
                    .remove(KEY_NSEC)
                    .apply()
        }
    }

    fun clear() {
        prefs.edit()
            .remove(KEY_NSEC)
            .remove(KEY_BUNKER)
            .apply()
    }

    private companion object {
        const val PREFS_FILE = "highlighter_session"
        const val KEY_NSEC = "nsec"
        const val KEY_BUNKER = "bunker_uri"

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
