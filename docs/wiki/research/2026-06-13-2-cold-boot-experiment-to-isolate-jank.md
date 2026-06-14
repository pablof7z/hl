---
type: research-record
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
source_lines: 1743-1800
agent_attribution: Cold-boot emulator + isolate jank cause subagent
has_preregistered_criteria: true
has_method: true
has_structured_report: true
characterization: "Cold-boot experiment to isolate jank cause (emulator exhaustion vs real app bug); concluded jank is a real app bug — navigation works on fresh boot but 72.56% janky frames persist"
captured_at: 2026-06-13T16:24:06Z
---

Cold-boot experiment to isolate jank cause (emulator exhaustion vs real app bug); concluded jank is a real app bug — navigation works on fresh boot but 72.56% janky frames persist

---

User: [Agent task result: Agent "Cold-boot emulator + isolate jank cause" completed]
Critical finding! Let me summarize the cold-boot test results:

**Cold-Boot Test Report**

**Step A: Cold Boot & Reinstall — SUCCESS**
- Killed old emulator, cold-booted fresh instance (no snapshot, no-boot-anim)
- Boot completed in ~30 seconds
- App reinstalled successfully
- App auto-logged in (or retained session from profile data)
- Feed loaded with highlights populated

**Step B: Responsiveness Test — MIXED (navigation works, but rendering is janky)**

1. **Feed responsiveness**: ✓ PASS
   - Feed populated with 140 highlights (hodlbod, c1a9282b7da2, etc.)
   - Initial gfxinfo reset showed: 77.78% janky frames, 99th percentile at 1550ms

2. **Rooms tab navigation**: ✓ PASS
   - Rooms tab responded immediately to tap
   - Displayed 24 rooms in "New & noteworthy" section
   - Room tiles rendered with "Open" and "Join" buttons

3. **Room detail screen**: ✓ PASS
   - "Open" button on first room navigated successfully
   - Room detail screen shows NAME placeholder and pill tabs (Home, Library, Discussions)
   - Discussions tab tap worked

4. **Tab-switch responsiveness**: ✓ PASS (registers all taps)
   - Tapped Highlights > Rooms > Search 3 cycles (9 taps)
   - All taps registered (no missed inputs)
   - Navigation transitions completed

5. **Frame statistics (over ~70 seconds of interactions)**:
   - Total frames rendered: 317
   - Janky frames: 230 (72.56%) — **CRITICAL JANK**
   - 50th percentile: 44ms
   - 90th percentile: 93ms
   - 99th percentile: 150ms (down from 1550ms initially)
   - High input latency: 183 / 317 (57.7%)
   - Slow UI thread: 102
   - Slow draw commands: 220
   - **Logcat shows**: "Skipped 49 frames", "Skipped 92 frames", "Skipped 57 frames", "Skipped 108 frames" — app main thread overload

**JUDGMENT: Real App Bug (NOT Emulator Exhaustion)**

On a fresh, cold-booted emulator with 4GB memory:
- Navigation buttons register and execute correctly
- Screen transitions work
- But rendering is **severely janky** (72.56% janky frames)
- Logcat shows repeated frame skips indicating main thread blocking
- The 183 high-input-latency events means touch responsiveness *feels* sluggish even though taps register

**Conclusion**: The unresponsiveness is a **real app performance issue**, not emulator degradation. The app's main thread is blocking on heavy work (likely network I/O, JSON parsing, or list composition) during navigation and rendering. This requires profiling (CPU profiler, ANR logs, compose recomposition tracing) to identify the bottleneck.

**Screenshots saved to**: ~/Builds/validation-cold/
- welcome.png, feed.png, rooms.png, room-detail.png, room-discussions.png, nav.png
- gfx.txt (frame stats + logcat)
