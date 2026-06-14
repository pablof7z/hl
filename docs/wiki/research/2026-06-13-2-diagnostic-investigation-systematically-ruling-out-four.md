---
type: research-record
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
source_lines: 602-621
agent_attribution: a1a74b484b5df94e5
has_preregistered_criteria: true
has_method: true
has_structured_report: true
characterization: "Diagnostic investigation systematically ruling out four hypotheses for empty Android feed (mapping bug, list composition, blank cards, bridge wiring), finding render path correct and pointing to logged-out state as root cause — verdict: high confidence render-path correct, medium confidence on root cause"
captured_at: 2026-06-13T12:08:00Z
---

Diagnostic investigation systematically ruling out four hypotheses for empty Android feed (mapping bug, list composition, blank cards, bridge wiring), finding render path correct and pointing to logged-out state as root cause — verdict: high confidence render-path correct, medium confidence on root cause

---

User: [Agent task result: Agent "Diagnose empty Android feed root cause" completed]
## Summary

**One-sentence root cause:** The Android feed's render path, UniFFI type mapping, enum/field names, state plumbing, and core bridges are **byte-for-byte correct and symmetric with iOS** — so "zero cards" is **not** a Compose/mapping bug (the cards render the raw `quote`/`title` immediately with no hydration gate); an empty list can therefore only mean `state.homeFeed.items` is empty at the snapshot level (**option 1: data/auth/sync starvation in the core**), and the live emulator is in fact **logged out**, so a populated-feed repro was impossible without the prohibited sign-in.

**What I verified (and ruled out):**
- **Option 3 (mapping)** — ruled out. Binding confirmed: `HighlighterHomeFeedItemKind = {HIGHLIGHTS, READ}`, fields `highlights: List<HydratedHighlight>`, `read: HighlighterHomeReadItem?`, etc. Android's `when(item.kind)` is exhaustive and reads exact field names. Identical to iOS.
- **Option 4 (list not composed)** — ruled out. `OpenHomeFeed` dispatched once, `LazyColumn` renders `HomeFeedPanel(state.homeFeed)`, all items (the `take(8)` cap is already gone), no teardown loop.
- **Option 2 (blank cards)** — ruled out as the *empty* cause. Both platforms render `quote`/`title` immediately. But there IS a real parity gap: **Android dispatches none** of `requestProfile`/`requestWebMetadata`/`requestIsbnPreview`/`article` that iOS fires from the cards — so Android cards look plain (raw quote, no resolved author/cover), not blank.
- **Bridges** — both `listenForUpdates` AND `setCoreEventCallback` are wired on Android (`HighlighterViewModel.kt:69,227`), matching iOS. Not the missing-NMP-bridge issue.

**Runtime evidence (emulator-5554):** App is on the logged-out Welcome screen. `highlighter_session.xml` holds only the crypto keyset — **no stored credential**. `app_state.json` shows `onboarding_complete:false`. `data.mdb` is 53KB, stale (yesterday). Core emits **zero** `highlighter-core` logs.

**Loading state verdict:** **KEEP it** — the "Syncing highlights…" branch is original (single commit), identical to iOS, and does NOT swallow the populated case (the `else` branch fires whenever `items` is non-empty regardless of `isLoading`). It is not masking anything.

**Files to change:** `ui/home/HomeFeedPanel.kt` (add a one-line snapshot-shape log so the next logged-in run self-diagnoses; add card hydration dispatches for iOS parity) and `ui/MainScaffold.kt` (pass `state.profiles`/`isbnPreviews`/`webMetadata` into the panel). **No change** to the VM, bridges, SessionStore, or the loading/empty logic — all correct.

**Confidence:** Render-path correctness **high**; empty-snapshot root cause **medium** — the decisive confirmation (the `highlighter-feed` item-count log under a real `nsec` sign-in) is blocked by the no-signup constraint and the absence of a stored credential.

Full spec: `/Users/pablofernandez/Work/hl/Plans/feed-empty-fix.md`. Screenshot: `/tmp/feed-diag/now.png`.
