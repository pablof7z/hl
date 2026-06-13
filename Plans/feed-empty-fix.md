# Android Highlighter feed renders empty — root cause + fix spec

**Date:** 2026-06-13
**Author:** automated diagnosis (Opus 4.8, 1M)
**Scope:** Why the Android Highlights feed shows zero cards while iOS shows highlights/articles, and the exact Android changes to fix it. Diagnosis only — no app code changed by this document.

---

## TL;DR (one sentence)

The Android feed render path, UniFFI type mapping, enum/field names, state plumbing and core bridges are **byte-for-byte correct and symmetric with iOS** — so the "zero cards" is **not** a Compose/mapping bug (options 2/3/4 are ruled out by code); the cards render straight from `lead.quote` / `read.title` with **no async-hydration gate**, which means an empty list can only come from `state.homeFeed.items` being empty at the snapshot level (**option 1: data/sync starvation in the core process**), and the live emulator is currently **logged out** (no persisted credential), so the populated-feed case cannot be reproduced here without a sign-in.

**Confidence:** Code-correctness of the render path: **high** (verified line-by-line + binding). Root cause of the *empty snapshot when logged in*: **medium** — requires a logged-in runtime check to confirm `items` count; the prohibited-signup constraint blocks final confirmation on this emulator.

---

## What was verified (and therefore ruled out)

### Render path is correct — options 2, 3, 4 are NOT the cause

**Item-kind mapping (option 3) — correct.** Generated binding:
`android/app/build/generated/source/uniffi/main/kotlin/uniffi/highlighter_core/highlighter_core.kt`
- `HighlighterHomeFeedItemKind` (`:10766`) = exactly `HIGHLIGHTS, READ`.
- `HighlighterHomeFeedItem` (`:4958`): `kind`, `stableId`, `sortKey`, `highlights: List<HydratedHighlight>`, `highlightCount: ULong`, `read: HighlighterHomeReadItem?`.
- `HighlighterHomeFeedSnapshot` (`:5006`): `items`, `itemCount`, `isLoading`, `errorMessage?`.
- `HydratedHighlight` (`:6378`): `highlight: HighlightRecord`, `artifact: ArtifactRecord?`, …
- `HighlightRecord` (`:3754`): `quote`, `note`, `imageUrl`, `artifactAddress`, `externalReference`, `sourceUrl`, …

Android `HomeFeedRow` (`android/.../ui/home/HomeFeedPanel.kt:115`) does an **exhaustive** `when (item.kind)` over both enum arms, reads the exact field names above, and casts nothing that can silently fail. iOS does the identical switch (`HighlightsTabView.swift:114`). No mismatched names, no all-dropping filter.

**List is actually composed (option 4) — correct.** `MainScaffold.kt:298` mounts `HighlightsTab` on the HIGHLIGHTS tab; `HighlightsTab` (`:318-338`) dispatches `OpenHomeFeed` once in `DisposableEffect(Unit)` (`:323`) and renders `HomeFeedPanel(feed = state.homeFeed, …)` inside a `LazyColumn item {}` (`:335`). `state` is the live `StateFlow` from the reconciler. The populated branch `HomeFeedPanel.kt:98-105` iterates **all** items (no `take()` cap — the prior `take(8)` is already gone). Tab switching does not tear the feed down in a loop.

**State plumbing is correct.** iOS `homeFeed` is a pure pass-through `nmpState.homeFeed` (`HighlighterStore.swift:530`). Android is the same pass-through: `onState(state) { _state.value = state }` (`HighlighterViewModel.kt:201`) → `state.homeFeed`. No merge, no transform, no divergence.

**Core bridges are wired (the NMP wiring the user previously hit).** Android registers **both** required callbacks like iOS:
- `app.listenForUpdates(this)` (`HighlighterViewModel.kt:69`) — snapshot reconciler.
- `app.setCoreEventCallback(bridge)` (`HighlighterViewModel.kt:227`, via `registerEventBridge()` called in `bootstrap()` `:81`) — delta bridge (`EventBridge.kt`).
- Credential restore on bootstrap (`HighlighterViewModel.kt:88-107`).
iOS does the identical three (`HighlighterStore.swift:132`, `:1052`, `bootstrap()`). **No missing bridge.**

### The only true render asymmetry (does NOT cause empty, but is a real parity gap — option 2 partial)

iOS feed cards dispatch hydration; **Android dispatches none**:

| Hydration | iOS (dispatched from card) | Android |
|---|---|---|
| `requestProfile(pubkeyHex)` | `HighlightFeedCardView.swift:580`, `ReadingFeedCardView.swift:32,36` | **omitted** |
| `requestWebMetadata(url)` | `HighlightFeedCardView.swift:56` | **omitted** |
| `requestIsbnPreview(isbn)` | `HighlightFeedCardView.swift:566` | **omitted** |
| `article(pubkeyHex,dTag)` | `HighlightFeedCardView.swift:579` | **omitted** |

Crucially, **iOS does not gate the card body on this hydration** — it renders the resource header + pull-quote immediately from `items[0]` and merely *upgrades* author name / cover / title when hydration lands. Android likewise renders `lead.quote` (`HomeFeedPanel.kt:140`) and `read.title` (`:188`) **directly and immediately**. So the missing hydration makes Android cards look *plainer* (raw quote, no resolved author/cover/title, no web/isbn enrichment) — **it does not make them blank or zero-height.** This is a polish gap to close, not the empty-feed cause.

---

## Runtime evidence (live emulator-5554, `com.highlighter.app`)

Launched `am start -n com.highlighter.app/.MainActivity`; screenshot `/tmp/feed-diag/now.png`:

- **App is LOGGED OUT** — Welcome screen ("Highlighter / Save what moves you" + *Create account* / *Sign in*). Not the feed. The "authenticated What's New" an earlier agent saw was a prior session that has since been cleared.
- `shared_prefs/highlighter_session.xml` contains **only** the AndroidX-security keyset entries (`__androidx_security_crypto_encrypted_prefs_key_keyset__` / `…value_keyset__`) — **no `nsec` and no `bunker_uri` value.** So `SessionStore.storedCredential()` (`SessionStore.kt:26`) returns `null` → no auto-login → logged-out. (SessionStore persist/clear logic itself is correct: it writes under keys `nsec`/`bunker_uri`, `SessionStore.kt:36-56`.)
- `files/highlighter-core/highlighter_app_state.json` = `{"onboarding_complete": false, …}` (141 bytes). Even with a restored credential, RootScene gates on `isLoggedIn && onboarding.isComplete`.
- `files/highlighter-core/data.mdb` = 53 KB, last written **2026-06-12** — the nostrdb holds almost no events (a populated account would be far larger and freshly written).
- **Zero `highlighter-core` logs.** `initPlatformLogging()` (`HighlighterViewModel.kt:37`) routes Rust tracing to tag `highlighter-core`, yet a fresh `force-stop` + launch produced **no** core / relay / login / bootstrap log lines at all. The core is silent — so the only runtime instrument we have (item-count / relay-status tracing) is unavailable.

**Consequence:** a populated-feed repro requires a sign-in (a real `nsec1…`). The task prohibits signup, and there is no stored credential to restore, so item-count cannot be confirmed at runtime on this emulator right now.

---

## Confirmed root cause (best evidence)

With the render path proven correct and symmetric, **zero cards while logged-in can only be `state.homeFeed.items.isEmpty()` at the snapshot level — option 1 (empty snapshot / data starvation in the core process), not a Compose bug.** The candidates, ranked:

1. **The user was looking at the genuine empty/logged-out state** (as the emulator is right now). If the feed was viewed while logged out or with `onboarding_complete:false`, "zero cards" is correct behavior, masked by a copy/loading-state that reads as broken. *Most consistent with the live evidence.*
2. **Core delivered `items` but the account's follow-graph highlights had not synced** (relay/sync timing) — the core's `homeFeed` query returns empty until the social graph + highlights land in nostrdb. The 53 KB stale `data.mdb` + silent core are consistent with a core that never finished syncing. The fix here is **not** in `HomeFeedPanel.kt`; it is making the empty-vs-loading state honest and confirming `OpenHomeFeed` runs against a connected, synced core.

There is **no Android-side mapping/filter/cast defect** producing the empty render. Anyone re-investigating should not look again at `HomeFeedPanel.kt`'s mapping — it is correct.

---

## Loading-state question (explicit ask)

`HomeFeedPanel.kt:67-95` has three branches:
```
feed.isLoading && feed.items.isEmpty()  -> "Syncing highlights…" spinner   (:73)
feed.items.isEmpty()                    -> EmptyPanel("No highlights yet")  (:95)
else                                    -> render all items                 (:98)
```
This is **identical** to iOS `HighlightsTabView.swift:64-71` (`isLoading && items.isEmpty → ProgressView`, `items.isEmpty → emptyState`, else `feedList`). The "Syncing highlights…" branch is **original code shipped in the first commit** (`git log` shows `HomeFeedPanel.kt` has exactly one commit, `d33872b feat(android): ship native app shell`) — it was **not** a later band-aid, contrary to the brief's assumption.

**Verdict: KEEP it. It does NOT swallow the populated case.** The populated branch (`else`) is only skipped when `items.isEmpty()`, which is true precisely when there is nothing to render. The loading branch only shows when `isLoading && items.isEmpty()`. There is no ordering bug — a non-empty `items` always reaches the `else` branch regardless of `isLoading`. Removing it would only replace an honest "Syncing…" with a flash of "No highlights yet," which is *worse*. **No change required to the branch logic.**

(If the user "explicitly does not want a loading state masking an empty render": the current logic does not mask anything — it distinguishes the two correctly. The real masking risk is the opposite: showing "No highlights yet" while the core is still syncing. The existing `isLoading` gate already prevents that, provided the core sets `homeFeed.isLoading=true` during sync.)

---

## The fix spec

The empty-feed perception is a **data/auth/sync** problem, not a render problem. Implement in this order; only items A–B are code changes to the Android feed surface, C is the parity polish, D is the diagnostic that confirms which case you're in.

### A. Make the empty/loading distinction trustworthy + add a self-diagnosing log (small, high value)

**File:** `android/app/src/main/java/com/highlighter/app/ui/home/HomeFeedPanel.kt`

Keep the three-branch `when`. Add a lightweight log of the snapshot shape so the next runtime check needs no signup-guessing. Before the `when` at `:67`:

```kotlin
// before (:67)
when {
    feed.isLoading && feed.items.isEmpty() -> { … }

// after
android.util.Log.i(
    "highlighter-feed",
    "render items=${feed.items.size} count=${feed.itemCount} " +
        "loading=${feed.isLoading} err=${feed.errorMessage ?: "-"} " +
        "kinds=${feed.items.joinToString { it.kind.name }}",
)
when {
    feed.isLoading && feed.items.isEmpty() -> { … }
```
This is the one allowed diagnostic touch; it makes `adb logcat -s highlighter-feed` answer "did the core deliver items?" definitively. (Remove or downgrade to verbose before release.)

**Do NOT** change the branch logic. **Do NOT** re-add any `take()` cap.

### B. Guard the HIGHLIGHTS arm against a degenerate empty `highlights` list (defensive parity)

**File:** `HomeFeedPanel.kt:116-157`

iOS does `items[0]` unconditionally (it would crash on an empty `highlights`), relying on the core invariant that a HIGHLIGHTS item always carries ≥1 hydrated highlight. Android currently renders **nothing** (no Surface, zero height) when `lead == null` (`:117-119`). That is safe but invisible. Make it explicit so a future core regression that emits an empty `highlights` array is visible rather than a silent blank row:

```kotlin
// :117
val leadHydrated = item.highlights.firstOrNull()
val lead = leadHydrated?.highlight
if (lead != null) {
    … existing Surface …
}
// add an else that logs (degenerate item) — do not render a blank gap silently
```
Low priority; only matters if A's log shows `items > 0` but cards still missing.

### C. Add the missing card hydration (real iOS parity — closes the "plain cards" gap)

**File:** `HomeFeedPanel.kt`, both rows. Mirror iOS. Wire `dispatch` (already passed in) + obtain a profiles snapshot (iOS reads `app.profile(pubkeyHex:)`; on Android the resolved profiles live in `state.profiles` — pass it into `HomeFeedPanel`/`HomeFeedRow` or expose via the existing `LocalProfiles` provider used elsewhere in the app).

For the **HIGHLIGHTS** row (`:116`), on first composition of each lead, dispatch:
- `requestProfile(lead.pubkey)` — always.
- If `lead.artifactAddress` is a `30023:<pubkey>:<dTag>` → `OpenArticleReader` is the tap action already; for the header also call the async `app.article(pubkey, dTag)` equivalent + `requestProfile(pubkey)` (iOS `HighlightFeedCardView.swift:579-580`).
- If `lead.externalReference`/`artifactAddress` starts with `isbn:` → `RequestIsbnPreview(isbn)` (iOS `:566`).
- If the artifact kind is web (sourceUrl/url present, not article/isbn) → `RequestWebMetadata(url)` (iOS `:56`).

For the **READ** row (`:158`), on first composition dispatch:
- `RequestProfile(read.pubkey)` (iOS `ReadingFeedCardView.swift:32`).
- `RequestProfile(interactorPubkeys.first)` if present (iOS `:36`).

Use `LaunchedEffect(key)` keyed on the stable id / pubkey so each dispatch fires once, matching iOS `.task(id:)`. Then upgrade the rendered author name / cover / title from `state.profiles` / `state.isbnPreviews` / `state.webMetadata` (the snapshot list buffers named in the architecture §0) when present, falling back to the raw `quote`/`title` exactly as now. **Cards must still render the raw quote/title immediately — never gate the Surface on hydration arriving** (this is the iOS contract and is what keeps the feed non-empty).

This does **not** fix an empty feed; it makes a populated feed look like iOS (resolved author/avatar/cover, social badge, web/isbn enrichment).

### D. Confirm the actual empty cause at runtime (the decisive check — needs a sign-in)

Because the emulator is logged out with no stored credential and the core emits no logs, the empty-vs-starved question is unresolved. To confirm:
1. Sign in with a real `nsec1…` for an account that follows active highlighters (Plan §5.4).
2. Watch `adb logcat -s highlighter-feed highlighter-core` while opening Highlights.
3. Read the line from change **A**:
   - `items=0 loading=true` then later `items=N` → it was **sync latency**; the loading state is correct; no feed bug. Ensure relays reach "Connected" (top bar `MainScaffold.kt:209`).
   - `items=0 loading=false` persistently while iOS (same nsec) shows cards → genuine **core/relay starvation on Android** (the NMP host process not delivering events). Investigate the `org.nmp.android` host process wiring (seen as a separate task in logcat), not `HomeFeedPanel.kt`.
   - `items=N` but no cards on screen → only then is it a render bug (and B's log will show the degenerate item).

---

## Files to change (Android)
- `android/app/src/main/java/com/highlighter/app/ui/home/HomeFeedPanel.kt` — add snapshot-shape log (A), defensive HIGHLIGHTS guard (B), card hydration dispatch + profile/isbn/web upgrade (C).
- (C only) `android/app/src/main/java/com/highlighter/app/ui/MainScaffold.kt` — pass `state.profiles` / `state.isbnPreviews` / `state.webMetadata` (or `LocalProfiles`) into `HomeFeedPanel` so the rows can read resolved data.
- **No change** to `HighlighterViewModel.kt`, `EventBridge.kt`, `SessionStore.kt`, the binding, or the loading/empty branch logic — all verified correct.

## What would confirm the root cause
A single logged-in run reading the `highlighter-feed` log line (change A). Until that runs, the highest-probability explanation given all evidence is **#1 (the feed was viewed logged-out / pre-onboarding, which is the emulator's current state) or #2 (relay/sync had not delivered the follow-graph highlights into nostrdb)** — both upstream of the render code, which is correct.
