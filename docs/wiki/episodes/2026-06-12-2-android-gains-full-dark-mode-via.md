---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: product
status: active
subjects:
  - android-dark-mode
  - theme-tokens
  - material3
supersedes: []
related_claims: []
source_lines:
  - 846-1062
captured_at: 2026-06-12T08:49:34Z
---

# Episode: Android gains full dark mode via Material3 theme token sweep

## Prior State

Android app had no dark mode support — all 16 UI files used hardcoded color literals (e.g. Color(0xFFF8F7F2), bare palette identifiers like Paper, Ink) that could not respond to system dark theme

## Trigger

Professionalization directive required real-app behavior; the theme layer (Theme.kt) already defined matching light and dark color schemes but no UI file referenced them

## Decision

Replaced every hardcoded color across 16 UI files with Material3 theme tokens (Paper→background, Ink→onSurface, Muted→onSurfaceVariant, Line→outline, Moss→primary, Gold→secondary, Clay→tertiary, plus semantic mappings for inline literals like 0xFFFFFCF5→surface). Added values-night window theme XML. Color.White mapped to onPrimary only on primary/Moss backgrounds

## Consequences

- App now responds to system dark/light theme end-to-end
- Zero hardcoded Color(0x...) literals remain outside the theme definition files
- Theme.kt defines both lightColorScheme and darkColorScheme with brand palette tokens
- All 16 files compile-verified with no orphan imports

## Open Tail

- Visual once-over on emulator/device recommended once runtime crash fix is confirmed

## Evidence

- transcript lines 846-1062
