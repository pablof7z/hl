# NIP-55 "Sign in with Amber" — Android E2E runbook + evidence

This documents the emulator end-to-end acceptance procedure for Highlighter's
NIP-55 external-signer (Amber) integration, and records the verified pass that
the original integration PRs (#3/#4) could not run ("no emulator available").

The Kotlin bridge (`app/src/main/java/com/highlighter/app/nip55/`) is vendored
from the NMP registry's `compose/login-block` and is byte-identical to NMP's
Stage-4 known-good source except the package line. The transport/intent-shape
contract is now guarded by
`app/src/test/java/com/highlighter/app/nip55/ExternalSignerCapabilityBridgeTest.kt`
(23 pure-Kotlin tests — run with `./gradlew :app:testDebugUnitTest`).

## Prerequisites

| Item | Requirement |
|------|-------------|
| Emulator | arm64-v8a AVD, API 31+ |
| Amber | v6.x installed (`com.greenart7c3.nostrsigner`) with a key imported |
| NDK | `26.1.10909125` (AGP 8.7 default); cargo-ndk + `aarch64-linux-android` target |
| JDK | 17 |

## Build + install

```sh
cd app/android
export JAVA_HOME=$(/usr/libexec/java_home -v 17)
export ANDROID_HOME=$HOME/Library/Android/sdk
./gradlew :app:assembleDebug          # builds Rust core via cargo-ndk + uniffi, then the APK
adb -s <serial> install -r app/build/outputs/apk/debug/app-debug.apk
```

The gradle pipeline cross-compiles `highlighter-core` (`cargoBuildArm64` →
`fixLibsodiumAndRelink` → `generateUniffiKotlin`) before the APK. A cold Rust
build is several minutes.

## E2E procedure

1. `adb -s <serial> logcat -c` then start capture.
2. Launch Highlighter → **Sign in** → **Sign in with Amber**.
   - The button only renders when `detectInstalledSigners` finds Amber, which
     requires the `<queries>` block in `AndroidManifest.xml` (both the
     `nostrsigner` scheme intent AND `<package com.greenart7c3.nostrsigner>`).
3. Amber's `SignerActivity` opens showing **Highlighter / com.highlighter.app**,
   the account npub, and permission options. Pick a trust level → **Connect**.
4. Control returns to Highlighter; the kernel installs the pubkey-only account
   and proceeds past login (onboarding / Highlights timeline).
5. Trigger any signing op (e.g. **Rooms → Create room**) → Amber auto-signs
   (full-trust) → the event lands on `wss://relay.highlighter.com`.

## Pass criteria

| Leg | Check | Pass |
|-----|-------|------|
| get_public_key | Amber opens | `SignerActivity` shows the app + correct npub (no "cannot open signer") |
| get_public_key | Pubkey returned | App advances past login; kernel logs `user relay config applied user=<hex>` |
| sign_event | Amber signs | `START ... dat=nostrsigner: pkg=com.greenart7c3.nostrsigner ... (has extras)` in logcat; SignerActivity opens/auto-returns |
| sign_event | Event verified | `nak verify` passes; `pubkey` == Amber-held key |

## Verified pass — 2026-06-13

Emulator `nip55_test_avd` (arm64-v8a, API 31), Amber 6.2.1, Highlighter debug
APK built from this branch.

- **get_public_key**: Amber's approval dialog rendered "Highlighter /
  com.highlighter.app" with account `npub1rvwkdn8x8ahmdsrevuqyeqf64dpvufjf6hhwx5xqm7az52ze0cms9nqmku`.
  After Connect the kernel logged
  `user relay config applied user=1b1d66cce63f6fb6c07967004c813aab42ce2649d5eee350c0dfba2a28597e37`
  (the hex of that npub — verified with `nak decode`) and the app advanced to
  onboarding. No "cannot open signer" at any point.
- **sign_event**: Creating a NIP-29 room fired
  `START u0 {act=android.intent.action.VIEW dat=nostrsigner: pkg=com.greenart7c3.nostrsigner cmp=.../.SignerActivity (has extras)}`;
  Amber auto-signed (full-trust, onCreate→onPause ≈140 ms, no user tap) and the
  `kind:9007` group event was fetched back from `wss://relay.highlighter.com`:

  ```json
  {"kind":9007,"id":"cfb295a07fbeb559ea81b8c0c7819d5fb27a77733590210153b115547ce7bef6",
   "pubkey":"1b1d66cce63f6fb6c07967004c813aab42ce2649d5eee350c0dfba2a28597e37",
   "created_at":1781346107,"tags":[["h","4088bea6a27d"]],"content":"",
   "sig":"af6a604d488bc5c57060eb05b1e0a7d898b3635c46ba3c973ffa6a7f079ba7a89baefb9bf71afe1d03d1fe1128fec7e698effb6811b7345ac0b97864725aef0c"}
  ```

  `nak verify` → exit 0 (id + schnorr signature valid); `pubkey` equals the
  Amber-held test key. The `h` tag `4088bea6a27d` matches the room the UI
  reported as "Created 4088bea6a27d".
