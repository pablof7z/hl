package com.highlighter.app.nip55

import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * Regression guard for the NIP-55 `<queries>` manifest block.
 *
 * ## Why this exists
 *
 * On Android 11+ (API 30+), `PackageManager.queryIntentActivities` returns an
 * empty list — even when Amber (`com.greenart7c3.nostrsigner`) is installed —
 * unless the app's manifest declares matching `<queries>`. Without it,
 * `detectInstalledSigners` finds nothing, the "Sign in with Amber" button
 * either never renders or its Intent fails to resolve, and the user sees
 * **"cannot open signer"**.
 *
 * This exact block was once silently DELETED from a local working tree (an
 * uncommitted clobber, same class as a regenerated iOS scheme), producing the
 * "cannot open signer" report on a physical phone while `origin/main` was fine.
 * The original integration's emulator E2E was skipped, so nothing caught it.
 *
 * This test asserts the SOURCE manifest still contains both required entries.
 * It is a pure-JVM unit test (runs in `./gradlew :app:testDebugUnitTest`, no
 * device/emulator), so the guard runs on every CI build and fails loudly the
 * moment the block goes missing.
 */
class AndroidManifestNip55QueriesTest {

    private fun locateSourceManifest(): File {
        // JVM unit tests run with the working dir at the module root
        // (app/android/app), but don't depend on that — walk up from user.dir
        // looking for src/main/AndroidManifest.xml so the guard is robust to
        // however the test runner sets the working directory.
        val candidates = mutableListOf<File>()
        val userDir = System.getProperty("user.dir") ?: "."
        var dir: File? = File(userDir).absoluteFile
        repeat(6) {
            val d = dir ?: return@repeat
            candidates += File(d, "src/main/AndroidManifest.xml")
            candidates += File(d, "app/src/main/AndroidManifest.xml")
            dir = d.parentFile
        }
        return candidates.firstOrNull { it.isFile }
            ?: error(
                "Could not locate app/src/main/AndroidManifest.xml from user.dir=" +
                    "${System.getProperty("user.dir")} (searched: " +
                    candidates.joinToString { it.path } + ")",
            )
    }

    @Test
    fun manifestDeclaresNostrsignerQueriesBlock() {
        val manifest = locateSourceManifest()
        // Normalise whitespace so single/double quotes and line wrapping don't
        // matter — we only care that the two semantic entries are present.
        val xml = manifest.readText()

        assertTrue(
            "AndroidManifest.xml is missing the <queries> element required for " +
                "NIP-55 signer detection on API 30+. Restore the nostrsigner " +
                "<queries> block — without it Amber is invisible to " +
                "PackageManager and 'Sign in with Amber' shows 'cannot open signer'.",
            xml.contains("<queries"),
        )

        // (a) the nostrsigner scheme intent — lets PackageManager resolve the
        // VIEW/nostrsigner Intent the bridge dispatches.
        val hasSchemeData = Regex(
            """<data\s+android:scheme\s*=\s*["']nostrsigner["']""",
        ).containsMatchIn(xml)
        assertTrue(
            "AndroidManifest.xml <queries> is missing " +
                "<data android:scheme=\"nostrsigner\"/>. Amber's Intent will not " +
                "resolve on API 30+ without it.",
            hasSchemeData,
        )

        // (b) the Amber package visibility — required so getPackage()/setPackage()
        // routing and ContentResolver fast-path can see the Amber APK.
        val hasAmberPackage = Regex(
            """<package\s+android:name\s*=\s*["']com\.greenart7c3\.nostrsigner["']""",
        ).containsMatchIn(xml)
        assertTrue(
            "AndroidManifest.xml <queries> is missing " +
                "<package android:name=\"com.greenart7c3.nostrsigner\"/>. This MUST " +
                "match KNOWN_NOSTR_SIGNERS in ExternalSignerWire.kt; without it " +
                "Amber is invisible to PackageManager on API 30+.",
            hasAmberPackage,
        )
    }
}
