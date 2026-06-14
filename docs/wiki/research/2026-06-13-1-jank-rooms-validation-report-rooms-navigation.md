---
type: research-record
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
source_lines: 1687-1725
agent_attribution: ac2a0a3c17a970812
has_preregistered_criteria: true
has_method: true
has_structured_report: true
characterization: "Jank/rooms validation report: Rooms navigation FAIL (unresponsive per 2-tap rule), frame performance SEVERE JANK (80% janky, 150ms p50, 224-frame skips), build SUCCESS"
captured_at: 2026-06-13T16:48:21Z
---

Jank/rooms validation report: Rooms navigation FAIL (unresponsive per 2-tap rule), frame performance SEVERE JANK (80% janky, 150ms p50, 224-frame skips), build SUCCESS

---

User: [Agent task result: Agent "Rebuild + tight jank/rooms validation" completed]
No ANRs, but plenty of frame skips and network jank. Now let me generate the final report with all findings and screenshots.

---

## REPORT

**Build/Install:** ✓ SUCCESS
- APK built in 5s (gradle assembleDebug)
- Installation succeeded
- App launched and automatically logged in (test account already seeded)

**Rooms Navigation:** ✗ UNRESPONSIVE
- Tapped Rooms tab twice (positions 640,525 and 640,565)
- After 6s wait post-tap both times: still on Highlights feed
- No nav occurred; marked as unresponsive per 2-tap rule
- Status bar now shows "Connecting" (network activity)

**Room Detail:** NOT ATTEMPTED
- Rooms nav failed, so cannot validate room detail screen, tabs, or discussions FAB

**Frame Performance:** SEVERE JANK (worsened vs baseline)
- **Janky frames: 80%** (4 of 5 frames janky in current gfxinfo window)
- **50th percentile: 150ms** (frame time; target ~16ms)
- **90th percentile: 200ms**
- **Major spikes:** 224, 225, 207, 183, 171, 123, 118 frames skipped in recent logcat
- **Previous baseline:** 222-379 frame skips; current still in that range (**not improved**)
- App is doing heavy work on main thread (Choreographer logs)
- GPU percentile shows outlier (4950ms at 90th/99th)

**Discussions FAB:** NOT VALIDATED (nav failed)

**Screenshots:**
- `~/Builds/validation-wave3/startup.png` — logged-in, on Highlights feed
- `~/Builds/validation-wave3/rooms.png` — tap 1, still Highlights
- `~/Builds/validation-wave3/rooms-attempt2.png` — tap 2, still Highlights (Connecting status)
- `~/Builds/validation-wave3/gfx.txt` — frame stats and Choreographer logs

**KEY ISSUE:** Rooms tab navigation is completely unresponsive. Combined with persistent jank (225-frame skips), this suggests **main-thread blocking on network I/O**. The "Connecting" status hint at relay negotiation overhead. This is a **critical responsiveness regression** blocking user navigation.
