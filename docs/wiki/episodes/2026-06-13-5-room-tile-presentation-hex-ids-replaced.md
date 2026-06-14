---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: superseded
subjects:
  - room-tile-display
  - rooms-explorer
supersedes: []
related_claims: []
source_lines:
  - 869-877
  - 982-1015
captured_at: 2026-06-13T12:42:52Z
---

# Episode: Room tile presentation — hex IDs replaced by short fallbacks + member counts + cover images

## Prior State

RoomTile displayed the full 64-char hex ID (room.id) when name or about were empty strings (pre-hydration), and used AvatarImage for the cover. Result: rooms list showed raw hex identifiers instead of names and cover art.

## Trigger

Validation screenshots on the logged-in build showed room tiles rendering raw hex IDs (e.g. '6f732c8bd027') and bare presentation, identified as the same hydration gap affecting the feed (lines 869-877).

## Decision

Changed name fallback from room.id to room.id.take(8)+… (short hex, never the full string). Added memberSubtitle computed from room.memberCount (e.g. '12 members', 'Open room') mirroring iOS RoomCoverCard.memberSubtitle. Subtitle display priority: subtitle signal label → room.about → memberSubtitle — never raw hex. Switched cover from AvatarImage to RemoteImage(48×48dp, CoverShape) matching iOS KFImage usage on room.picture.

## Consequences

- Room tiles show short hex when name isn't hydrated yet (instead of full 64-char ID).
- Member counts and about text provide meaningful content even before full hydration.
- Cover images use the same RemoteImage+CoverShape component as feed cards.
- Test tags added: room_tile_cover, room_tile_name.

## Open Tail

- Visual validation on rebuilt APK pending at session end.

## Evidence

- transcript lines 869-877
- transcript lines 982-1015

