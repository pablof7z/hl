---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: active
subjects:
  - toast-banner
  - root-scene
  - edge-to-edge
supersedes:
  - 2026-06-13-6-toast-banner-repositioned-below-header-under
related_claims: []
source_lines:
  - 1001-1008
captured_at: 2026-06-13T13:15:31Z
---

# Episode: Toast banner repositioned for edge-to-edge display

## Prior State

ToastBanner in RootScene.kt used padding(16.dp) + Alignment.TopCenter, placing it 16dp from the physical screen top. With enableEdgeToEdge(), this squarely overlapped the TopAppBar and status bar.

## Trigger

Visual validation showed the 'Welcome to Highlighter / Dismiss' banner (and any 'not found' toast) occluding the header — status bar, title, and avatar were half-hidden behind it.

## Decision

Changed toast container padding to WindowInsets.statusBars + 64.dp (standard M3 TopAppBar height) + 8.dp visual gap, matching iOS's ShareToastBanner which is positioned below the navigation bar via .overlay(alignment: .top) + .padding(.top, 8).

## Consequences

- Header (title, connection status, avatar, settings gear) fully visible and unobstructed
- Toast banner appears below the header with comfortable visual spacing
- Matches iOS positioning convention

## Open Tail

*(none)*

## Evidence

- transcript lines 1001-1008

