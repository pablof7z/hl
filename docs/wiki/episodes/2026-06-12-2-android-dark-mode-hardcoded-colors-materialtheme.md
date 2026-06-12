---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: product
status: active
subjects:
  - android-theme
  - dark-mode
  - compose-ui
supersedes: []
related_claims: []
source_lines:
  - 1009-1060
captured_at: 2026-06-12T08:57:33Z
---

# Episode: Android dark mode: hardcoded colors → MaterialTheme tokens across 16 files

## Prior State

All 16 Android UI files used hardcoded color literals (Color.White, Color(0xFFFFFCF5), palette constants like Paper/Ink/Moss) that could not respond to dark mode, despite Theme.kt defining matching light/dark color schemes

## Trigger

Professionalization directive to make Android a real app; systematic inventory found zero dark mode support

## Decision

Sweep all 16 UI files to use MaterialTheme.colorScheme tokens (Paper→background, Ink→onSurface, Muted→onSurfaceVariant, Line→outline, Moss→primary, Gold→secondary, Clay→tertiary, plus semantic mappings for inline literals). Color(0xFFF1EFE6)→surfaceVariant as closest semantic match.

## Consequences

- Dark mode now works end-to-end on Android
- Zero hardcoded color literals remain in converted files
- Color.Transparent retained as theme-neutral
- values-night window theme added

## Open Tail

- Visual validation on emulator/device still needed once runtime crash is fixed

## Evidence

- transcript lines 1009-1060
