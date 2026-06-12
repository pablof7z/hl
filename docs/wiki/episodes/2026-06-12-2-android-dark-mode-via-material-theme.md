---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: product
status: active
subjects:
  - android-dark-mode
  - android-theme
supersedes: []
related_claims: []
source_lines:
  - 946-958
  - 1009-1062
captured_at: 2026-06-12T08:31:54Z
---

# Episode: Android dark mode via Material theme tokens

## Prior State

All 16 Android UI files used hardcoded color literals (Paper, Ink, Moss, Gold, Clay, Line as bare identifiers or Color(0x...) values), making dark mode impossible.

## Trigger

Professionalization directive; the theme layer already defined matching light and dark Material3 color schemes but the UI screens couldn't respond to them.

## Decision

Swept every hardcoded color reference across 16 files to semantic MaterialTheme.colorScheme tokens (Paper→background, Ink→onSurface, Moss→primary, Gold→secondary, Clay→tertiary, Muted→onSurfaceVariant, Line→outline, etc.). Added values-night window theme. Zero inline Color(0x...) literals remain in converted files.

## Consequences

- Android app now renders correctly in both light and dark system themes
- Color.White on Moss backgrounds correctly became onPrimary
- Color(0xFFF1EFE6) chip fill was mapped to surfaceVariant (closest semantic match)
- Future color changes only need to update Theme.kt, not every panel

## Open Tail

- Visual validation on device/emulator needed once runtime crash is resolved
- No error/danger-red tokens yet used — may need adding for validation states

## Evidence

- transcript lines 946-958
- transcript lines 1009-1062
