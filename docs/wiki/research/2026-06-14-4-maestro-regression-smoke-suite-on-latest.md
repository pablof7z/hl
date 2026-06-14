---
type: research-record
date: 2026-06-14
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
source_lines: 3714-3760
agent_attribution: subagent
has_preregistered_criteria: true
has_method: true
has_structured_report: true
characterization: "Maestro regression smoke suite on latest-NMP APK: all 7 flows PASS, ADR-0052 projections and registered actions verified, verdict BUILD CLEAR FOR DEPLOYMENT"
captured_at: 2026-06-14T08:38:43Z
---

Maestro regression smoke suite on latest-NMP APK: all 7 flows PASS, ADR-0052 projections and registered actions verified, verdict BUILD CLEAR FOR DEPLOYMENT

---

User: [Agent task result: Agent "Regression smoke suite on latest-NMP APK" completed]
Perfect. Let me now notify the user with the final summary:

## Maestro Smoke Suite Results — All Green

**Summary:** All 7 flows passed. No regression detected in the latest-NMP Highlighter APK with ADR-0052 migration.

### Test Environment
- **Emulator:** HighlighterTest (cold boot, no snapshot)
- **APK:** highlighter-debug.apk (74MB) — latest NMP with ADR-0052
- **Build Status:** ✓ Installed successfully
- **Test Account:** ✓ Verified (nsec1wza... length 63)

### Per-Flow Results

| Flow | Status | Key Assertion |
|------|--------|---------------|
| 00-login.yaml | **PASS** | Auth successful, "Highlights" visible |
| 06-feed.yaml | **PASS** | Feed items populated — HOME FEED PROJECTION OK |
| 08-highlight-detail.yaml | **PASS** | Detail view + author + comment button accessible |
| 11-rooms-explorer.yaml | **PASS** | Rooms list populated — NIP-29 PROJECTION OK |
| 12-open-room.yaml | **PASS** | Room detail navigation functional |
| 19-create-room.yaml | **PASS** | Create room dialog accessible — REGISTERED ACTION OK |
| 30-comments.yaml | **PASS** | Comment input sheet accessible |

### Critical ADR-0052 Checks ✓
- **Feed Projection:** Items populating correctly
- **Rooms Projection (NIP-29):** Discovery working, rooms list visible
- **Registered Action (CreatePublicGroup):** UI triggers without error

### Logcat Scan (adb logcat -d)
- **No FATAL/panic errors** detected
- Non-fatal warnings only:
  - ADR-0053 DEBT: host not declaring consumed projections (informational)
  - Performance: block_on_local budget exceeded at refresh_home_feed (existing issue)

### Artifacts
All screenshots captured successfully in `~/Builds/validation-nmp/`:
- login_complete.png (351K)
- feed_populated.png (309K)
- highlight_detail.png (139K)
- rooms_explorer.png (91K)
- room_detail.png (41K)
- create_room_sheet.png (105K)
- comments_open.png (44K)

**Final Verdict:** BUILD CLEAR FOR DEPLOYMENT. Projections and registered actions working as expected on latest NMP.
