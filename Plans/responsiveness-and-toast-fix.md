# Responsiveness & "not found" toast — diagnosis and fix spec

Scope: Highlighter Android app. READ-ONLY diagnosis; no app code changed.
`ui/rooms/RoomDetailPanel.kt` is being edited by another agent and is deliberately
NOT a fix target here — the fixes live in the Android bridge/ViewModel and in the
Rust core toast emitters.

---

## Issue 1 — Main-thread jank / "Skipped 200-322 frames" / ANR

### Does the Android→core bridge block the MAIN/UI thread?

**No — `dispatch` does not block the UI thread, and the `block_on_local` work does
not run on the UI thread.** The chain:

- `MainActivity.kt:90` wires `dispatch = viewModel::dispatch`.
- `HighlighterViewModel.dispatch` — `HighlighterViewModel.kt:114-116` — calls
  `app.dispatch(action)` synchronously on the caller's thread (the Compose
  Main dispatcher for LaunchedEffect bodies).
- `HighlighterNmpApp::dispatch` — `app/core/src/nmp_app.rs:1812-1816` — only does
  `self.tx.try_send(KernelMsg::Action(...))`. That is a **non-blocking** send into a
  bounded `sync_channel(256)` (`ACTION_QUEUE_CAPACITY = 32`/`nmp_app.rs:1716`,
  capacity const at `nmp_app.rs:32` = 256). It never blocks and never runs the action body.
- The action body (including every `block_on_local(...)` — `nmp_app.rs:2043`,
  e.g. `refresh_home_feed`/`refresh_room_detail` at 2656, 3059) runs on a **dedicated
  background thread** named `highlighter-nmp-actor`, spawned at `nmp_app.rs:2513-2516`
  with its own current-thread tokio runtime. So the `block_on_local exceeded local
  budget site="delta.refresh_home_feed"` warnings are blocking the **actor thread**,
  not the UI thread.

So the per-card burst of `RequestProfile` / `RequestWebMetadata` / `RequestIsbnPreview`
(`HomeFeedPanel.kt:187,202,218,238,409,416`) is cheap on the UI side — each is just a
`try_send`. The `block_on_local` budget warnings are real but happen off-UI-thread; they
do not directly cause skipped frames.

### What actually causes the skipped frames / ANR

**Recomposition flood from full-state snapshots delivered on the actor thread.**

- `onState` — `HighlighterViewModel.kt:201-204` — assigns `_state.value = state`
  (a full `HighlighterAppState` clone). It is invoked from the core's `emit`
  (`nmp_app.rs:11093-11100`), which calls `reconciler.on_state(state.read().clone())`
  **synchronously on the actor thread**, once per call site (NOT coalesced at the
  `emit` layer; `emit_hz` is configured but `emit` fires per-mutation).
- Every resolved hydration op emits: `KernelMsg::OpResolved` → `apply_op_outcome` →
  `emit(&ctx.state, &ctx.reconciler)` (`nmp_app.rs:2555`). A burst of N visible cards
  hydrating produces N full-state snapshots pushed into the `StateFlow` back-to-back.
- `MainActivity.kt:75` collects `viewModel.state.collectAsState()`; each new snapshot
  is a new object, so Compose recomposes the whole tree (large feed/room lists) on the
  **Main thread**. N large recompositions in a tight window = "Skipped 200-322 frames"
  and, under enough load, the input-dispatch ANR.
- The per-card `LaunchedEffect`s dedupe only on their key (e.g. `lead.pubkey`), and do
  **not** check whether the datum is already in state. On scroll/recycle and on first
  load they re-dispatch for pubkeys/urls already present, amplifying the op→emit→recompose
  storm. The core de-dups web-metadata/isbn requests (`start_web_metadata_request`
  guards on `state_has_web_metadata`, `nmp_app.rs:8930`), but the request still costs a
  channel round-trip and `RequestProfile` re-runs `hydrate_profile` via `block_on_local`
  each time (`nmp_app.rs:9520-9523`).

### Comparison to iOS

iOS marshals state delivery onto the main actor and hops off the FFI callback thread:
`HighlighterAppStateReconciler.onState` wraps the apply in `Task { @MainActor in ... }`
(`HighlighterStore.swift:1135-1140`), and `applyNmpState` is `@MainActor`
(`HighlighterStore.swift:1090`). dispatch is fire-and-forget `nmpApp.dispatch(...)`
(non-blocking, same as Android). The relevant structural difference is only that iOS's
SwiftUI `@Observable` diffing is more granular than Android collecting one monolithic
`StateFlow`; both receive full snapshots.

### Thread-safety constraint of the core

`HighlighterAppReconciler: Send + Sync` (`nmp_app.rs:1673`) and the app is `Arc<Self>`
shared across threads. The core **serializes all mutations through the single actor
thread** (the channel + `spawn_actor` loop). `dispatch` is safe to call from any thread
because it only does `try_send`. So: **calls do not need to be serialized by the host**
— the channel already serializes them. There is no requirement (and no benefit) to call
`dispatch` from a particular thread.

### Recommended fix (Android-side)

Two independent, additive changes. Neither touches `RoomDetailPanel.kt`.

**Fix 1a — Move `onState` work off the actor thread / coalesce recomposition.**
`onState` is invoked on the core actor thread and directly writes the `StateFlow`. Keep
the write cheap (it already is — just a `value =`), but the real win is throttling the
**collection** so Compose does not recompose on every one of a burst of snapshots.

- File: `HighlighterViewModel.kt`. The `StateFlow` exposure is at lines 58-59:
  ```kotlin
  private val _state = MutableStateFlow(app.state())
  val state: StateFlow<HighlighterAppState> = _state.asStateFlow()
  ```
- Before/after sketch (debounce/conflate the UI-facing flow so a burst collapses to the
  latest snapshot per frame, instead of recomposing per emit):
  ```kotlin
  // after
  private val _state = MutableStateFlow(app.state())
  val state: StateFlow<HighlighterAppState> =
      _state
          .sample(16.milliseconds)            // coalesce bursts to ~1 per frame
          .stateIn(viewModelScope, SharingStarted.Eagerly, _state.value)
  ```
  (`sample`/`conflate` from kotlinx.coroutines.flow; `viewModelScope` already available on
  `AndroidViewModel`.) This bounds recomposition frequency regardless of how many ops
  resolve in a burst, directly addressing "Skipped N frames."
- `onState` itself (lines 201-204) needs no thread change — it writes a `MutableStateFlow`,
  which is thread-safe. Do **not** wrap it in `Dispatchers.Main` (that would serialize
  through the main thread and reintroduce jank). The collection-side `sample` is the lever.

**Fix 1b — Dedupe per-card hydration dispatches against current state.**
File: `HomeFeedPanel.kt` (and any other feed card composables; explicitly NOT
`RoomDetailPanel.kt`). Guard each `RequestProfile` / `RequestWebMetadata` /
`RequestIsbnPreview` on absence from the corresponding state map before dispatching, so
already-hydrated cards stop re-dispatching on recycle.

- Profile dispatch at `HomeFeedPanel.kt:187-191`:
  ```kotlin
  // before
  LaunchedEffect(lead.pubkey) {
      if (lead.pubkey.isNotBlank()) {
          dispatch(HighlighterAppAction.RequestProfile(lead.pubkey))
      }
  }
  // after — skip if the profile is already in state (LocalProfiles is already in scope, line 180)
  LaunchedEffect(lead.pubkey) {
      if (lead.pubkey.isNotBlank() && profiles[lead.pubkey] == null) {
          dispatch(HighlighterAppAction.RequestProfile(lead.pubkey))
      }
  }
  ```
  Apply the same `== null` / `!contains` guard to the article-author profile
  (`HomeFeedPanel.kt:218-223`), the read-card profiles (`409-419`), ISBN
  (`202-206`, guard on `isbnPreviews`), and web-metadata (`238-...`, guard on
  `webMetadataList`). `LocalProfiles`, `LocalIsbnPreviews`, `LocalWebMetadata` are
  already pulled in at lines 180-182.

This is **Android-side**. Fix 1a is the high-confidence lever for the ANR/skipped frames;
1b reduces the storm at the source.

### Optional core-side hardening (not required)
`emit` is uncoalesced. If desired later, an `emit_hz`-rate limiter could be applied at the
`emit` boundary in `nmp_app.rs:11093` so bursts of `OpResolved` coalesce before reaching
any host. Lower priority; the Android `sample` achieves the same UI outcome without a core
change.

**Confidence:** High that the UI bridge does NOT block the main thread and that the jank
is recomposition-driven by uncoalesced full-state snapshots. High that Fix 1a (sample/
conflate the collected flow) resolves the skipped-frames/ANR. Medium-high that 1b is also
needed to fully tame the burst.

---

## Issue 2 — Persistent "not found" green banner across Highlights / Rooms / Room detail

### Root cause (core-side emitter)

Benign per-card background hydration failures are surfaced as a **global error toast**
whose message is the raw `CoreError` Display string. `CoreError::NotFound`'s Display is
literally `"not found"` (`app/core/src/errors.rs:20-21`):
```rust
#[error("not found")]
NotFound,
```

Emitters that set `state.toast` from such a failure:

1. **Web metadata** — `handle_web_metadata_resolved`, `nmp_app.rs:8961-8979`:
   ```rust
   Err(message) => set_toast(state, Some(HighlighterToast {
       kind: HighlighterToastKind::Error,
       message,
   })),
   ```
   `message` comes from `err.to_string()` (`nmp_app.rs:8951`). A web URL on a card that
   returns a non-success HTTP status, an unparseable/non-HTML page, or a cached negative
   entry yields `CoreError::NotFound` (`web_metadata.rs:246, 269, 291`) → message `"not
   found"`.

2. **ISBN preview** — `handle_isbn_preview_resolved`, `nmp_app.rs:8898-8910`: same pattern,
   global error toast with the raw error string.

3. **Profile subscription** — `request_profile`, `nmp_app.rs:9536-9544`: on a
   `subscribe_user_profile` failure, also sets a global error toast from `err.to_string()`.

These ops are fired automatically, one per visible card, by the feed/room LaunchedEffects
(`HomeFeedPanel.kt:187-240, 409-419`, and the room-detail panel). So any card pointing at
a dead/blocked link, a missing ISBN, or an unresolvable profile pops a global "not found"
banner — which is why it appears identically on Highlights, Rooms, and Room detail.

### Why it is *persistent*

The Android `ToastBanner` has **no auto-expiry**. `RootScene.kt:169-186` renders
`state.toast` with a Dismiss button that dispatches `ClearToast`, but nothing clears it on
a timer. It stays until the user taps Dismiss — and any subsequent background hydration
failure simply re-sets `state.toast` to "not found" again. The component:
`ui/components/Common.kt:118-142` (`ToastBanner`) has no `LaunchedEffect`/delay.

Note the iOS comment claims the banner is "Cleared by the banner after a few seconds"
(`HighlighterStore.swift:16`), but iOS's `ShareToastBanner`
(`RootSceneView.swift:59-83`) also has **no timer** — so iOS surfaces the same toast and
it does not actually auto-expire either. The asymmetry is only visual (green capsule vs.
M3 surface). This confirms the emitter is core-side and shared.

### Recommended fix

This is fundamentally a **core-side** bug: a benign, expected, background per-card lookup
failure should not raise a user-facing global toast at all. Recommended primary fix
(core), with an Android-side safety net.

**Primary (core-side) — stop emitting a global toast for benign hydration failures.**
In all three emitters, drop the `set_toast` on failure (optionally record into a per-item
negative/empty state instead so the card just renders without a preview):

- `nmp_app.rs:8969-8978` (`handle_web_metadata_resolved`): on `Err`, do not `set_toast`.
  At most log (`tracing::debug!`) and/or insert a negative `WebMetadata::empty` marker so
  the card stops re-requesting.
- `nmp_app.rs:8898-8910` (`handle_isbn_preview_resolved`): same — on `Err`, do not
  `set_toast`; drop quietly.
- `nmp_app.rs:9536-9544` (`request_profile` subscribe failure): same — log instead of
  toasting; a missing profile subscription is not user-actionable.

Rationale: these are Class-A network lookups fired automatically for every visible card.
Failure is the expected steady-state for dead links / missing previews and must be silent.
Reserve `set_toast(Error)` for user-initiated actions (publish, join, sign-in, etc.), which
already have their own toasts. Because the emitter is shared, this also fixes iOS.

**Secondary (Android-side safety net) — auto-expire error toasts.**
Even with the core fix, give the banner a timeout so a transient toast can never persist.
In `RootScene.kt:169-186`, wrap the toast in a keyed `LaunchedEffect` that dispatches
`ClearToast` after a few seconds:
```kotlin
state.toast?.let { toast ->
    LaunchedEffect(toast) {
        delay(4_000)
        dispatch(HighlighterAppAction.ClearToast)
    }
    // ... existing Box { ToastBanner(...) }
}
```
This is defensive only; the core change is the real fix. (Do not put the auto-expire only
on Android and call it done — the core would still spam iOS.)

**Confidence:** High on the root cause and exact emitter lines (web-metadata /
isbn-preview / profile-subscribe `set_toast` with raw `CoreError::NotFound` Display
"not found", surfaced because background per-card lookups fail routinely and the banner
never auto-expires). High that removing the `set_toast` calls for these three benign
background paths is the correct fix; the Android auto-expire is a high-confidence
secondary guard.

---

## Summary of files to change

| Fix | File | Lines | Side |
|-----|------|-------|------|
| 1a coalesce recompositions | `android/.../HighlighterViewModel.kt` | 58-59 (`state` flow) | Android |
| 1b dedupe per-card hydration | `android/.../ui/home/HomeFeedPanel.kt` | 187, 202, 218, 238, 409, 416 | Android |
| 2 primary: stop benign toast | `app/core/src/nmp_app.rs` | 8898-8910, 8969-8978, 9536-9544 | Core (shared w/ iOS) |
| 2 secondary: auto-expire | `android/.../ui/RootScene.kt` | 169-186 | Android |

Explicitly excluded: `android/.../ui/rooms/RoomDetailPanel.kt` (owned by another agent).
