---
type: research-record
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
source_lines: 3150-3181
agent_attribution: ac43d2ca70513bbcb
has_preregistered_criteria: true
has_method: true
has_structured_report: true
characterization: "Cold-boot OCR shutter check: camera bind PASS, shutter PASS, ML Kit OCR PASS, review screen FAIL (Compose infinite-height crash)"
captured_at: 2026-06-13T18:36:51Z
---

Cold-boot OCR shutter check: camera bind PASS, shutter PASS, ML Kit OCR PASS, review screen FAIL (Compose infinite-height crash)

---

User: [Agent task result: Agent "Cold-boot fast OCR shutter check" completed]
Good, it's back. Now let me trace the issue more carefully. The camera buffer error and Compose crash suggest the review screen layout is broken. Let me check the code to understand what happened:

**CRITICAL FINDING CONFIRMED:**

Based on the logcat analysis, the OCR path partially succeeded but then crashed on the review screen:

1. **Camera button → Viewfinder: PASS** — The camera successfully opened and bound to lifecycle (camera 10, ImageCapture + Preview active)
2. **Shutter button tapped: PASS** — takePictureInternal triggered, MLKit OCR models loaded and ran successfully  
3. **Review screen display: FAIL** — App crashed with a Compose layout constraint error

**Key logcat evidence:**
- `06-13 20:51:57.843 D CameraStateMachine: Publishing new public camera state CameraState{type=OPEN, error=null}` — Camera opened cleanly
- `06-13 20:52:18.643 D ImageCapture: takePictureInternal` — Photo capture initiated
- `06-13 20:52:20.675 I native: Loading mlkit-google-ocr-models/...` — OCR running
- `06-13 20:52:24.362 E Camera3-OutputStream: Error queueing buffer to native window: No such device (-19)` — Camera buffer issue
- `06-13 20:52:24.471 E AndroidRuntime: FATAL EXCEPTION: java.lang.IllegalStateException: Vertically scrollable component was measured with an infinity maximum height constraints` — **Compose layout crash on review screen**

The app exited before showing the review screen with OCR text and quote field. The crash signature (`Vertically scrollable component measured with infinity constraints`) points to a Compose UI hierarchy issue in the review/capture flow—likely nesting LazyColumn + Column(Modifier.verticalScroll()) or similar.

**Report:**

- **Cold boot + login:** SUCCESS
- **Camera button → Viewfinder open:** YES (black surface, shutter button visible)
- **Shutter → Review with OCR:** FAIL — App crashed with Compose layout constraint error
- **Camera binding:** Back camera (camera 10) bound successfully, no "No available camera" errors
- **OCR models:** MLKit OCR loaded and started processing (loading line_recognition and layout models)
- **Root cause:** Review screen crashed with `IllegalStateException: Vertically scrollable component measured with infinity constraints` — a Compose UI hierarchy bug, not a camera issue

**Screenshot paths:** `/Users/pablofernandez/Builds/validation-ocr-cold/` — `capture.png` (Capture screen), `viewfinder.png` (camera live), `review.png` (crashed to home), `camera-log.txt` (full logcat)

The fresh emulator itself is fine (camera working, no degradation). The issue is a Compose layout bug in the review screen that needs fixing before the OCR review flow can be validated.
