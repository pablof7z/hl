---
type: research-record
date: 2026-06-14
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
source_lines: 3804-3859
agent_attribution: subagent
has_preregistered_criteria: true
has_method: true
has_structured_report: true
characterization: "Extended Maestro coverage: 3 PASS, 4 FAIL; real bug found (card author taps open reader not profile), 2 Maestro flow syntax errors identified"
captured_at: 2026-06-14T08:38:43Z
---

Extended Maestro coverage: 3 PASS, 4 FAIL; real bug found (card author taps open reader not profile), 2 Maestro flow syntax errors identified

---

User: [Agent task result: Agent "Run extended Maestro flows, triage failures" completed]
Excellent. All artifacts are in place. Let me provide the final summary.

---

## Summary: Maestro Validation Coverage Report

**Build Status**: ✅ SUCCESS (incremental — testTags only, all tasks UP-TO-DATE)

**Test Results**: 3 PASS / 4 FAIL (7 flows total)

| Flow | Result | Finding |
|------|--------|---------|
| 00-login | ✅ PASS | Session established |
| 10-article-reader | ✅ PASS | Feed card → article reader navigation works |
| 27-profile | ❌ FAIL | Card author tap opens article reader instead of profile |
| 28-follow | ❌ FAIL | Card author tap opens article reader instead of profile |
| 29-edit-profile | ✅ PASS | Own profile avatar → edit profile works |
| 23-bookmarks | ❌ FAIL | Invalid YAML syntax: `description` property not valid for `tapOn` |
| 34-network-settings | ❌ FAIL | Invalid YAML syntax: `description` property not valid for `tapOn` |

### Real App Bug (27-profile, 28-follow)

**Issue**: Tapping `id: card_author` on feed highlight cards opens the **article reader** instead of the **profile screen**. The UIAutomator dump at failure shows `article_reader` node present with article content, but `profile_screen` is completely absent.

**Screenshot**: `/Users/pablofernandez/Builds/validation-coverage/27-profile-fail.png` — Article reader displays with title "Reader", author "hodlbod", and full article content visible.

**Root Cause** (a): Navigation dispatcher is wired incorrectly. The card author clickable area is dispatching `OpenArticleReader` instead of `OpenProfile`. This is specific to the feed highlight card's author element — own profile avatar (29-edit-profile) routes correctly, so the bug is isolated to the "other user from feed" path.

### Flow Syntax Errors (23-bookmarks, 34-network-settings)

**Issue**: Both flows fail at parse time with `Unknown Property: description`.

Lines affected:
- **23-bookmarks.yaml:20** → `- tapOn: { description: "Settings" }`
- **34-network-settings.yaml:19** → `- tapOn: { description: "Settings" }`

**Root Cause** (b): The Maestro `tapOn` action does not accept a `description` property. Correct syntax is either:
- `- tapOn: { id: settings_button }` (if the gear icon has a testTag)
- `- tapOn: "Settings"` (string matching visible text)

These flows are attempting to reference an accessibility description as a Maestro selector, which is not a supported property.

### Artifacts

All test artifacts saved to `/Users/pablofernandez/Builds/validation-coverage/`:
- `27-profile-fail.png` — Screenshot of app state at failure (article reader visible)
- `27-profile-ui-dump.xml` — UIAutomator hierarchy (contains `article_reader` node, no `profile_screen`)
- `28-follow-ui-dump.xml` — UIAutomator hierarchy at 28-follow failure
- `article_reader_opened.png` — Baseline successful 10-article-reader
- `edit_profile_screen.png` — Baseline successful 29-edit-profile
- `login_complete.png` — Baseline successful 00-login

---

**Next Action**: The card author navigation bug in flows 27/28 is a real finding requiring app code fix. The syntax errors in flows 23/34 are flow definition issues (not app bugs).
