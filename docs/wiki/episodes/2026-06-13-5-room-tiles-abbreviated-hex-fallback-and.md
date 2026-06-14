---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: superseded
subjects:
  - android-rooms-tile
  - room-presentation
supersedes:
  - 2026-06-13-5-room-tile-presentation-hex-ids-replaced
related_claims: []
source_lines:
  - 989-999
captured_at: 2026-06-13T12:58:25Z
---

# Episode: Room tiles — abbreviated hex fallback and member subtitles

## Prior State

Room tiles displayed full 64-character hex IDs when room name or about was empty; used AvatarImage instead of cover-art image component

## Trigger

Validation screenshots showed raw hex like '6f732c8bd027' on room tiles where names had not yet hydrated

## Decision

Changed name fallback from room.id to room.id.take(8) + '…'; added memberSubtitle computed from memberCount (e.g. '12 members', 'Open room'); subtitle display priority: signal label → room.about → memberSubtitle (never raw hex); replaced AvatarImage with RemoteImage (48dp, CoverShape) for cover art matching iOS KFImage usage

## Consequences

- Named rooms display properly; unnamed rooms show abbreviated 8-char hex rather than full 64-char
- Cover images render via RemoteImage with CoverShape
- Named rooms still show hex when CommunitySummary.name is empty string (data hydration timing, not code)

## Open Tail

*(none)*

## Evidence

- transcript lines 989-999

