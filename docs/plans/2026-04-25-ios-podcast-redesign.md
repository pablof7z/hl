# iOS Podcast Redesign — Plan (2026-04-25)

A first-principles redo of the iOS podcast experience: kill the desktop-grade "Mark In / Mark Out" full-screen player, replace it with a **persistent Liquid Glass MiniPlayer + a hybrid Listening Room view** that mixes Proposal A (audio-time river of member clips) and Proposal C (structured track of chapters / clips / transcript layers).

Wireframe references: `docs/podcast-mocks/` (deployed at `https://highlighter-podcast-mocks.vercel.app/`).

---

## Goals

1. **MiniPlayer that never goes away.** Above the tab bar. Apple Music–style Liquid Glass capsule. Persists across tab switches. Tap to expand into the full Listening Room. Long-press for quick actions.
2. **Hybrid Listening Room** (Proposal A × C). One vertical timeline anchored to **audio time**. Layers compose by availability:
   - Always: chapters (when present) + member clips + your own marks.
   - When transcript is available: transcript paragraphs interleave inline (A's "river of voices" + speaker tracking).
   - When transcript is absent: synthetic waveform/silence ticks fill the gaps so the timeline never collapses.
3. **Time-based clipping.** A clip is `(start_seconds, end_seconds)` attached to the NIP-73 episode GUID. Transcript text — if any — is decoration. The same publish path serves a transcript-rich Spotify episode and a transcript-less RSS feed.
4. **Inline clip threads.** Tap a member's clip card → it expands inline with NIP-22 (kind:1111) replies + a reply box. No separate "Discussion" tab.
5. **Resume-everywhere.** Position persisted continuously; reopening the MiniPlayer = resume.

## Non-goals (this pass)

- Background audio session refinements (lock-screen artwork, AirPlay handoff) — separate work.
- Apple Music transcript fetcher — we don't ship a transcript scraper. Transcripts come only when the source provides one.
- Cross-episode queue / playlist UX. The MiniPlayer holds *one* current episode for now.

---

## Architecture

### Single global playback store

Add `var podcastPlayer = PodcastPlayerStore()` to `HighlighterStore` (`Core/HighlighterStore.swift`). Already `@Observable` and environment-injected everywhere, so any view can read `app.podcastPlayer` reactively. No new env key needed.

`PodcastPlayerStore` keeps everything it has today (AVPlayer + KVO + transport methods + clip Mark In/Out + publish) and adds:

- `currentArtifact: ArtifactRecord?` — what's loaded.
- `episodePosition: Double` — the seek position; persisted via `UserDefaults` keyed by `podcastItemGuid` so resuming on relaunch works.
- `transcriptSegments: [TranscriptSegment]` — moved from the per-view state.
- `transcriptAvailability: .loading | .available | .unavailable` — drives layer toggles and the no-transcript fallback.
- `comments: [String: [CommentRecord]]` — keyed by clip event id; lazy-loaded when a clip card expands.

### MiniPlayer (Liquid Glass)

Mounted via `.safeAreaInset(edge: .bottom)` in `Navigation/RootSceneView.swift`, around the existing `MainTabView()`. Visible iff `app.podcastPlayer.currentArtifact != nil`.

```
┌──────────────────────────────────────────────┐
│ ▶  Tucker Carlson · Tucker Debates...   ✕    │
│ ▔▔▔▔▔▔▔▔▔▔▔▔━━━━━━━━━━━━━━━━━━━━━━━━━━━     │  <- progress sliver
└──────────────────────────────────────────────┘
                                                  <- safe-area inset on tab bar
                  [tab bar]
```

Layout:
- 56pt tall capsule, full-width minus 12pt margins.
- Liquid Glass: `.glassEffect(.regular, in: Capsule())` on iOS 26 (deployment target is 26.0, no shim needed).
- Show artwork (40pt rounded square) · episode title (one line, fades right) · play/pause button · close (X) on the right.
- Hairline progress at the bottom of the capsule (the same playhead-fraction the full view uses).
- Tap → expands the `PodcastListeningView` as a `.sheet` with `.presentationDetents([.large])`.
- Long-press → context menu: Skip 30s, Mark Clip, Stop, Open Show.

`matchedTransitionSource` ties the MiniPlayer artwork to the Listening Room's hero artwork so the expand morphs (not a push, not a fade) — true to Apple Music's pattern.

### Listening Room view

Replaces the current `PodcastPlayerView`. Lives at `Features/Podcast/PodcastListeningView.swift`. Presented as a sheet, full detent.

```
┌─────────────────────────────────┐
│  ← Listening · Curious Minds  ↑ │  <- thin top bar
├─────────────────────────────────┤
│ [art][Show · Episode title]      │
│      1h 02m · 5 clips · 31 heard │
│                                  │
│ [Layers: Transcript · Clips · Ch]│  <- toggles
├─────────────────────────────────┤
│                                  │
│  0:00 ── COLD OPEN ───           │  <- chapter row
│                                  │
│  0:25  TUCKER                    │  <- transcript paragraph (if avail)
│        Welcome back...           │
│                                  │
│  1:45  ┃ AK · "The honest        │  <- member clip card
│        ┃   version of that..."   │
│        ┃   12 ♥ · 8 replies      │
│                                  │
│  2:25  GUEST                     │
│        Eugenics is the word...   │
│                                  │
│  ─── (no transcript here) ───   │  <- waveform tick fallback
│  3:10  speech                    │
│                                  │
│  ...                             │
│                                  │
├─────────────────────────────────┤
│       ▶ 4:08  ▰▰▰▰━━━━━  1:02:22│  <- audio pill (Liquid Glass)
└─────────────────────────────────┘
```

The rail is one `LazyVStack` of typed rows. Row builder takes:
- Chapters from `chapter` tags on the kind:11 podcast share (or `<podcast:chapters>` from the RSS feed when available).
- Clips: `getHighlightsForArtifact(reference)` already exists.
- Transcript paragraphs (when loaded).
- Synthetic "waveform" bands every 30s as fallback rows that render a thin ▰▰▰ pattern; only emitted when no transcript is loaded.

Rows are **sorted by start time**. Active row is the one whose timestamp is the latest ≤ playhead. Playing autoscrolls to keep active row pinned ~100pt from the top, with a 1.5s manual-scroll grace window (port the gesture from the web mock).

### Time-based clipping

Two entry points:

1. **Long-press the rail** (300ms) at any point. Computes the timestamp from the touch's y-position, opens the composer with a 90s window centered on it.
2. **MiniPlayer long-press → Mark Clip** when watching from another tab: drops a 60s clip ending at the current position and opens the composer immediately.
3. **Mark In + Mark Out** is gone as a primary affordance. The "tap two rows" pattern from Proposal C lives behind a toolbar overflow ("Pick a range").

The composer is a `.sheet(.medium)`:
- Range: `4:08 — 5:36 · 1m 28s` with `−` / `+` 5s buttons on each handle.
- Excerpt slot: shows the transcript fragment if any rows in range have transcript text; otherwise "Time-only clip · X seconds. Add a note."
- Note field (optional).
- Room picker (already exists in capture flow — reuse `CommunityPicker`).
- Publish → `core.publishHighlightsAndShare(artifact:drafts:targetGroupId:)`. `quote` may be `""`. Existing Rust path supports this; verified in audit.

### Inline clip thread

Each member clip card in the rail is collapsed by default (avatar + name + range + excerpt + actions row). Tap → expand:
- Existing `LaneCommentsSection` handles rendering.
- Lazy-loads via `getCommentsForReference(tagName: "e", tagValue: clip.eventId)` on first expand. Cache in `podcastPlayer.comments`.
- Reply composer at the bottom of the expanded card → `core.publishComment(...)` (already exists).

### Resume-on-relaunch

`PodcastPlayerStore` writes `(podcastItemGuid, position, lastPlayedAt)` to `UserDefaults` every 5s while playing. On `HighlighterStore` init, if a record exists from the last 7 days, the MiniPlayer rehydrates with it (paused). Tapping play resumes.

---

## Subagent dispatch

Foreground audit done (in this conversation). Now dispatch four agents.

### Agent 1 — Foundations (Sonnet)

**Scope.** Move `PodcastPlayerStore` into `HighlighterStore`, add the new state surface (currentArtifact, transcriptAvailability, comments cache, position persistence). Build `MiniPlayerView` as a Liquid Glass capsule with progress sliver, play/pause, artwork, title. Wire it via `.safeAreaInset(edge: .bottom)` in `RootSceneView`. Wire long-press menu. Hide when `currentArtifact == nil`. Verify `cargo check` and a build for iPhone target.

### Agent 2 — Listening Room (Sonnet)

**Scope.** Build `PodcastListeningView.swift`. Single-rail timeline with typed rows: `ChapterRow`, `MemberClipRow`, `TranscriptRow`, `WaveformTickRow`. Layer toggles (transcript / chapters / clips) at the top. Auto-scroll to active row with manual-scroll grace. Bottom audio pill with Liquid Glass. Hero artwork with `matchedTransitionSource` for the MiniPlayer morph. Long-press the rail → opens composer placeholder (composer wired by Agent 3).

Depends on Agent 1.

### Agent 3 — Time-based clip composer (Sonnet)

**Scope.** Build `ClipComposerSheet.swift`. Range header with ±5s nudges. Transcript-fragment slot (with the no-transcript fallback). Note field. Room picker (reuse). Calls `core.publishHighlightsAndShare(...)` with empty `quote` allowed. Plumbs the existing FAB / long-press gestures from Listening Room.

Depends on Agent 2.

### Agent 4 — Inline clip thread (Sonnet, parallel with 3)

**Scope.** Make `MemberClipRow` expandable. On expand, lazy-load comments via `getCommentsForReference(tagName: "e", tagValue: clip.eventId)`, render via reused `LaneCommentsSection`, add a reply composer at the bottom that calls `core.publishComment(...)`. Cache results in `podcastPlayer.comments`.

Depends on Agent 2.

### Final verification (me)

Build for `iPhone 17 Pro Max (3C438D9B-2021-5A30-93DB-910F7754F9A2)` once codesigning is unblocked. Run through the smoke flow: open a podcast share → tap play → switch to Highlights tab (MiniPlayer persists) → tap MiniPlayer (expands into Listening Room with morph) → scroll the rail → long-press a row → composer opens with right time range → publish → see the clip card appear on the rail.

---

## File touch list (estimated)

**New files:**
- `Features/Podcast/PodcastListeningView.swift`
- `Features/Podcast/MiniPlayerView.swift`
- `Features/Podcast/Rows/ChapterRow.swift`
- `Features/Podcast/Rows/MemberClipRow.swift`
- `Features/Podcast/Rows/TranscriptRow.swift`
- `Features/Podcast/Rows/WaveformTickRow.swift`
- `Features/Podcast/ClipComposerSheet.swift`
- `Features/Podcast/Layout/AudioPill.swift`

**Modified:**
- `Core/HighlighterStore.swift` — add `podcastPlayer`.
- `Features/Podcast/PodcastPlayerStore.swift` — promote to global, add `currentArtifact`, `transcriptAvailability`, `comments`, position persistence.
- `Navigation/RootSceneView.swift` — `.safeAreaInset` for MiniPlayer + sheet hosting for Listening Room.
- `Features/Communities/RoomHomeView.swift` — replace `NavigationLink` to `PodcastPlayerView` with `app.podcastPlayer.load(artifact:)` + present sheet.
- Any other call site that pushes `PodcastPlayerView`.

**Deleted (eventually):**
- `Features/Podcast/PodcastPlayerView.swift` — replaced by `PodcastListeningView`. Keep until parity is verified.

---

## Risk register

- **Liquid Glass `matchedTransitionSource` on iOS 26.0.** First use in this codebase. Needs an iPhone for visual verification — can't validate from build alone.
- **AVPlayer in a singleton.** When the MiniPlayer outlives multiple sheets, KVO and periodic time observers must be torn down on `clear()` to avoid leaks. Existing store already handles this; just verify after the move.
- **Synthetic waveform.** Real waveform extraction needs decoding the audio. For this pass we ship the simple 30s speech/silence stand-in and gate real-waveform on a follow-up.
- **Codesigning.** Currently blocked (no valid certs in keychain). User must restore an `iOS Development` cert before final build/install.
