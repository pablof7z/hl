---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: root-cause
status: superseded
subjects:
  - home-feed
  - home-feed-panel
  - highlighter-viewmodel
supersedes:
  - 2026-06-13-4-home-feed-display-fixed-and-render
related_claims: []
source_lines:
  - 602-621
  - 712-758
captured_at: 2026-06-13T12:25:09Z
---

# Episode: Home feed emptiness root cause — render path correct; empty snapshot = logged-out state

## Prior State

Home feed appeared completely empty; believed to be a Compose rendering bug (silent take(8) truncation + missing loading state)

## Trigger

After removing take(8) and adding loading state, feed still showed zero cards. Opus diagnosis compared Android vs iOS render paths line-by-line and found them byte-for-byte symmetric. Live emulator was confirmed logged out with zero highlighter-core activity and no stored credential. Seeded test account validated 143 highlights populating correctly.

## Decision

take(8) removal and distinct loading/empty states kept (genuine improvements). Root cause of emptiness is data/auth starvation in the core (no account, no follows, no synced data) — NOT a Compose/mapping/enum bug. No changes to VM, bridges, SessionStore, or loading/empty logic.

## Consequences

- Feed populates correctly with real data (143 highlights confirmed with seeded account)
- Identified a real parity gap: Android cards don't dispatch requestProfile/requestWebMetadata/requestIsbnPreview/article hydration that iOS fires — cards show bare quote without resolved author/cover/title
- Session persistence also flagged: app reverted to logged-out on back-key press (possible SessionStore or navigation-state issue)
- A snapshot-shape log was recommended for future self-diagnosis of empty feeds

## Open Tail

- Card hydration dispatches (requestProfile, requestWebMetadata, requestIsbnPreview, article) needed for iOS parity
- Session persistence investigation after back-key press

## Evidence

- transcript lines 602-621
- transcript lines 712-758

