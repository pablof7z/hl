---
type: episode-card
date: 2026-06-12
session: 0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/0c7b6c09-7d1f-4cb2-b178-1adf69cd09ef.jsonl
salience: reversal
status: active
subjects:
  - android-architecture
  - android-navigation
  - nmp-android
supersedes: []
related_claims: []
source_lines:
  - 1854-1858
  - 1120-1140
captured_at: 2026-06-12T08:57:33Z
---

# Episode: Android app: single-column debug harness → real navigation architecture

## Prior State

Android app was a single 3,317-line MainActivity.kt with a LazyColumn dumping every panel (including auth) into one scroll, no tab navigation, no proper screen flow — a debug harness, not a real app

## Trigger

User explicitly called it out: 'it's all just a long view… no tabs for settings, it doesn't even connect to relays… basically missing everything… it's just a dumping ground of things… a total piece of shit'

## Decision

Restructure into proper app architecture: auth gate → onboarding flow → bottom tabs → dedicated screens/sheets, matching iOS navigation patterns. Split monolith into 23 cohesive files with per-feature packages.

## Consequences

- The 70-lambda HighlighterAppScreen signature collapsed to (state, dispatch) — each panel constructs its own HighlighterAppAction
- Android now needs real bottom-tab navigation, proper auth flow, and per-screen composables — work still in progress
- Even with file split, the app still lacks proper navigation flow (the restructure was structural only; the user's complaint about flow remains partially open)

## Open Tail

- Relay connection on Android still failing (Rust core never initializes Android logger, app shows 'Process system not responding')
- Podcast mini-player UI missing on Android
- Strings not externalized to strings.xml

## Evidence

- transcript lines 1854-1858
- transcript lines 1120-1140
