---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: product
status: active
subjects:
  - android-icon
  - android-branding
supersedes: []
related_claims: []
source_lines:
  - 350-370
captured_at: 2026-06-12T08:31:54Z
---

# Episode: Branded adaptive launcher icon replaces default robot

## Prior State

Android app shipped with the default Android robot icon (no mipmap resources, empty directories).

## Trigger

Professionalization directive; the iOS app has a branded quote-marks icon.

## Decision

Created an Android adaptive icon (vector foreground + monochrome layer for themed icons) recreating the iOS opening-quotes mark, with proper ic_launcher.xml and ic_launcher_round.xml in mipmap-anydpi-v26.

## Consequences

- App now has a recognizable brand icon on device launchers
- Monochrome layer supports Android 13+ themed icons
- Vector drawable scales cleanly across all densities

## Open Tail

*(none)*

## Evidence

- transcript lines 350-370
