---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: superseded
subjects:
  - android-ui-chrome
  - toast-banner
supersedes: []
related_claims: []
source_lines:
  - 1002-1007
captured_at: 2026-06-13T12:58:25Z
---

# Episode: Toast banner repositioned below header under edge-to-edge

## Prior State

ToastBanner positioned at Alignment.TopCenter with 16dp padding, overlapping the TopAppBar and status bar under enableEdgeToEdge()

## Trigger

Validation screenshots showed 'Welcome to Highlighter / Dismiss' banner occluding the header, title, avatar, and settings gear

## Decision

Changed ToastBanner container padding to WindowInsets.statusBars + 64.dp (TopAppBar height) + 8.dp gap, matching iOS ShareToastBanner's overlay positioning below the navigation bar

## Consequences

- Header elements (title, connection status, profile button, settings) fully visible and unobstructed
- Toast banners now appear below the app bar, consistent with iOS

## Open Tail

*(none)*

## Evidence

- transcript lines 1002-1007

