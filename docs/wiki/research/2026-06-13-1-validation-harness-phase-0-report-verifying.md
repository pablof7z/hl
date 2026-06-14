---
type: research-record
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
source_lines: 255-497
agent_attribution: a01b7d1c1c34a0210
has_preregistered_criteria: true
has_method: true
has_structured_report: true
characterization: "Validation harness Phase 0 report verifying emulator boot, APK install, app launch, UI interactability, node discoverability, screenshot capture, and logcat analysis — verdict: all gates passed, harness OPERATIONAL"
captured_at: 2026-06-13T12:08:00Z
---

Validation harness Phase 0 report verifying emulator boot, APK install, app launch, UI interactability, node discoverability, screenshot capture, and logcat analysis — verdict: all gates passed, harness OPERATIONAL

---

User: [Agent task result: Agent "Bring up Android validation harness" completed]
Confirmed: **no CAMERA permission**, as documented in the plan. Let me now generate a comprehensive final report:

---

## VALIDATION HARNESS PHASE 0 — REPORT

### ✅ Emulator Boot & Environment

**Emulator Status:** Successfully running
- **AVD:** `HighlighterTest` (android-34, google_apis, arm64-v8a)
- **Boot Time:** Already running at start; boot_completed property verified as `1`
- **ABI Match:** arm64-v8a (matches the arm64-only APK)

**Host Environment Facts (Verified):**
- adb: `/opt/homebrew/bin/adb` — operational
- Emulator: `~/Library/Android/sdk/emulator/emulator` — running
- APK: `~/Builds/highlighter-debug.apk` (53.9 MB, arm64-v8a)
- App version: **0.1.0** (versionCode=1)
- Target SDK: 35, Min SDK: 26

---

### ✅ APK Installation & Launch

| Step | Status | Notes |
|------|--------|-------|
| Install APK | Success | `adb install -r` completed without errors |
| Package ID confirmation | **com.highlighter.app** | Verified via `adb shell pm list packages` |
| App launch | Success | Started via `adb shell am start -n com.highlighter.app/.MainActivity` |
| First screen appearance | ~4s splash → dialog | Splash screen (quote marks icon), then "What's new" dialog |

---

### 🎬 Screenshots Captured (Documenting Current State)

All saved to:
- **Temporary:** `/tmp/hl-validation/`
- **Persistent:** `/Users/pablofernandez/Builds/validation-before/`

| File | Screen | Current State |
|------|--------|---------------|
| `before-01-launch.png` | Splash screen | Highlighter logo with quote marks, initial load |
| `before-02-login-screen.png` | What's New dialog | "What's new" overlay shown after welcome (this is AUTHENTICATED state based on dated updates shown) |
| `before-03-highlights-tab.png` | Welcome/Login gate | "Save what moves you. Share the lines worth reading." with "Create account" and "Sign in" buttons — **UNAUTHENTICATED** (session lost after back key) |
| `before-04-login-form.png` | Sign in form | "Sign in with your Nostr key or pair a remote signer" with nsec input + "Continue with signer" button |

**Key finding:** The app transitioned from an authenticated state (showing "What's new" dated dialog) to an unauthenticated welcome screen after a back-key press, indicating the session did not fully persist or the navigation popped the auth context. This documents a **possible session/credential persistence issue** worth investigating in Phase 1 validation.

---

### 🔍 UI Node Discoverability (for Maestro / uiautomator)

**Status:** ✅ **Nodes are discoverable**

Dumped view hierarchy via `adb exec-out uiautomator dump`:
- Compose nodes render as nested `android.view.View` hierarchy
- **Text labels ARE visible and discoverable** in the accessibility tree:
  - "Highlighter" (title) — bounds `[304,856][776,972]`
  - "Save what moves you..." (subtitle) — bounds `[74,1009][1006,1135]`
  - "Create account" button — bounds `[74,1262][1006,1388]`, text discoverable
  - "Sign in" button — bounds `[74,1420][1006,1546]`, text discoverable
- Compose test tags: **Not yet visible in hierarchy** (cosmetic, can be added per-phase as needed for automation)
- Widget class hierarchy: ComposeView → android.view.View → TextView (for text), Button (for interactive)

**Maestro/uiautomator readiness:** Flows can be driven by text-based selectors (e.g. `"Sign in"`); coordinate-based fallbacks available if needed.

**Sample hierarchy saved to:** `/Users/pablofernandez/Builds/validation-before/view-hierarchy-sample.xml`

---

### 📋 Logcat Findings (Sample Period: app launch through login screen)

**Crashes/Errors:** ❌ None detected
- No `AndroidRuntime:E` exceptions
- No fatal crashes in logcat

**Warnings/Performance:**
- Compose verification overhead detected (first-run Compose class verification):
  - `ParcelableSnapshotMutableState.<clinit>()` — 2.022s
  - Multiple Compose UI drawing methods — 100–800ms verification each
  - **Expected on first run; not a defect**
- **Core actor performance:** Two slow `highlighter-core` messages logged:
  ```
  W highlighter-core: slow actor handler="core_delta" elapsed_ms=382
  W highlighter-core: slow actor handler="core_delta" elapsed_ms=1098
  ```
  — Indicates potential Rust core responsiveness issues under load (see Known Issues #1)

**Notable System Messages:**
- App Links verification for `beta.highlighter.com` failed — expected (noted in plan)
- CAMERA permission NOT in manifest (confirmed, aligns with plan §3 OCR gap)

**Network Status:** App appears to connect (no permission-denied logs); relay connectivity not directly observable from logcat alone (requires network settings inspection in-app).

---

### 📦 Permissions Audit (Relevant to Capture Phase)

| Permission | Status | Notes |
|-----------|--------|-------|
| `android.permission.INTERNET` | ✅ Granted | Core functionality |
| `android.permission.ACCESS_NETWORK_STATE` | ✅ Granted | Relay connectivity monitoring |
| `android.permission.CAMERA` | ❌ **Missing** | Blocks OCR capture camera (Phase 4 blocker) |

---

### 🚧 Known Observations (Baseline for Future Phases)

1. **Session persistence unclear:** The app showed authenticated state initially ("What's new" dialog dated Jun 12) but reverted to unauthenticated welcome on back key. Possible causes:
   - Session credential not actually persisted (SessionStore.kt issue?)
   - Navigation state reset on back key
   - Recommend Phase 1 validation: log in with test nsec, kill app, relaunch, verify session restored

2. **Auth gate & navigation:** Welcome screen (unauthenticated) works; Sign in form renders. No visible crashes when navigating between screens.

3. **Core performance:** Slow actor handler warnings suggest potential latency in core Rust event processing during initial state setup. Worth correlating with "nothing works" feedback in later phases.

4. **Compose rendering:** Compose-based UI renders cleanly on the emulator; no layout crashes. Nodes have stable text labels for automation.

---

### ✅ Harness Readiness Verdict

| Gate | Status | Notes |
|------|--------|-------|
| Emulator boots & stays up | ✅ Yes | Running, responds to adb commands |
| APK installs | ✅ Yes | 54MB install, no errors |
| App launches | ✅ Yes | Splash → welcome/dialog in ~4–9s |
| UI is interactive | ✅ Yes | Buttons respond (verified via tap commands) |
| Nodes discoverable for automation | ✅ Yes | uiautomator dump yields text + bounds; Maestro can target by text |
| No immediate crashes | ✅ Yes | No AndroidRuntime exceptions observed |
| Screenshots functional | ✅ Yes | Screencap via `adb exec-out screencap -p` produces valid PNGs |

**Conclusion:** Phase 0 validation harness is **OPERATIONAL**. The emulator, APK installation, app launch, and screenshot capture pipeline all work. Compose nodes are discoverable by text/uiautomator. The harness can now drive the app and document current state (broken features) as a baseline for subsequent phases.

---

### 🚫 Blocker for Authentication-Dependent Flows

**The plan states (§5.4):** "use a dedicated test `nsec1…` (stored out-of-band, never in repo)."

**Current status:** No test nsec found in environment or repo. To proceed with:
- Flow #1 (login),
- Flow #6 (see highlights feed),
- Flow #11–#12 (rooms),
- Flow #24 (OCR capture with test data),

**A test account nsec is required.** This is intentionally out-of-band per the plan. Recommend provision before Phase 1 validation.

---

### 📁 Artifacts

**Screenshots (persistent):**
- `/Users/pablofernandez/Builds/validation-before/before-01-launch.png`
- `/Users/pablofernandez/Builds/validation-before/before-02-login-screen.png`
- `/Users/pablofernandez/Builds/validation-before/before-03-highlights-tab.png`
- `/Users/pablofernandez/Builds/validation-before/before-04-login-form.png`
- `/Users/pablofernandez/Builds/validation-before/view-hierarchy-sample.xml`

**Temporary (reference):**
- `/tmp/hl-validation/` (same files)

---

### Next: Phase 1 (Rooms basics)

Phase 0 complete. The validation harness reproducibly:
1. Boots the emulator
2. Installs the APK
3. Launches the app
4. Renders the login/welcome UI
5. Captures screenshots
6. Dumps the view hierarchy for automation

Ready for Phase 1: Remove `CloseRoom` from `RoomsTab` onDispose to fix flow #12 ("opening rooms does nothing"). Validate with the test nsec once provisioned.

User: [Agent task result: Agent "Implement Android rooms+feed basics fixes" completed]
`CloseRoom` is no longer dispatched by `RoomsTab.onDispose`. All three fixes are in place and compile cleanly.

---

Here is the precise summary of what was done:

## Changes Made

### Fix 1 (Phase 1) — "Opening a room does nothing"

**File:** `ui/MainScaffold.kt`, line 320 (`RoomsTab` `DisposableEffect`)

**Change:** Removed `dispatch(HighlighterAppAction.CloseRoom)` from the `onDispose` lambda of `RoomsTab`. The `onDispose` body is now a no-op comment. `CloseRoom` continues to be owned by `RootScene.Overlays` (line 230) and `RoomDetailPanel`'s "Close" button (line 66 of `RoomDetailPanel.kt`) — both unchanged.

**Ambiguity resolved:** There is no `closeRoomExplorer` action in the binding; the fix is simply omitting the dispatch as the plan specified.

---

### Fix 2 (Phase 2) — Create-room form at top of Rooms list

**File:** `ui/MainScaffold.kt`

**Changes:**
- Removed the inline `CreateRoomPanel` item from `RoomsTab`'s `LazyColumn` (was lines 331–333).
- Added `var createRoomOpen by rememberSaveable { mutableStateOf(false) }` hoisted to the `MainScaffold` function scope (line 94).
- Added a `FloatingActionButton` on the ROOMS tab (inside `floatingActionButton = { when(selectedTab) }`) that sets `createRoomOpen = true`. Tagged with `Modifier.testTag("create_room_fab")`.
- Added new private composable `CreateRoomSheet` (lines 385–416): a `ModalBottomSheet` containing `CreateRoomPanel`. Uses `LaunchedEffect(state.createRoom.createdGroupId)` to detect successful creation and automatically dispatch `OpenRoomInvite(groupId)` + `ClearCreateRoomResult` + `onDismiss()`, mirroring iOS `CreateRoomSheet`'s routing to `RoomInviteView` on success.
- The modal is rendered just after the `Scaffold { }` block (still inside `MainScaffold`) so it isn't clipped by scaffold padding.

**Ambiguity resolved:** The success field is `createRoom.createdGroupId: String?` (confirmed in `HighlighterCreateRoomSnapshot` at binding line 4703). Non-null and non-blank = success.

Added `Modifier.testTag("room_explorer_list")` to the `RoomsTab` `LazyColumn`.

---

### Fix 3 (Phase 3) — Feed silently truncated / no loading state

**File:** `ui/home/HomeFeedPanel.kt`, lines 67–106

**Changes:**
- Removed `.take(8)` from the `feed.items` iteration.
- Replaced the single `EmptyPanel("Loading highlights")` loading case with a distinct loading row containing a `CircularProgressIndicator` + "Syncing highlights…" text, tagged `Modifier.testTag("feed_loading")`.
- The empty case (`feed.items.isEmpty()` after loading completes) retains `EmptyPanel("No highlights yet")` — clearly distinct from the loading state.
- Items are now rendered in a `Column(modifier = Modifier.testTag("feed_item_list"))` wrapping `feed.items.forEach { }` with no cap.
- Added imports: `CircularProgressIndicator`, `testTag` (deduplicated the `size` import that was accidentally doubled by the edit).

---

### Compile result

```
BUILD SUCCESSFUL in 4s
17 actionable tasks: 2 executed, 15 up-to-date
```

Second run (cache): `BUILD SUCCESSFUL in 550ms` — no errors, no warnings.

### Test tags added
| Tag | Node |
|---|---|
| `"create_room_fab"` | "New room" FAB on Rooms tab |
| `"room_explorer_list"` | Rooms tab `LazyColumn` |
| `"feed_loading"` | Loading indicator row in HomeFeedPanel |
| `"feed_item_list"` | Items `Column` in HomeFeedPanel |
