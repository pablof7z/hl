---
type: episode-card
date: 2026-06-13
session: 847487cd-e15b-4222-85ee-4a5a2b6f590b
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/847487cd-e15b-4222-85ee-4a5a2b6f590b.jsonl
salience: product
status: active
subjects:
  - comments-threading
  - comments-panel
supersedes: []
related_claims: []
source_lines:
  - 1985-2019
captured_at: 2026-06-13T17:33:54Z
---

# Episode: Comment threading mirrors iOS tree model

## Prior State

Android CommentsPanel displayed comments as a flat list without threading or reply affordance.

## Trigger

iOS parity gap — iOS CommentTreeBuilder builds a tree from topLevelEventIds + childLinks and renders inline reply previews with a reply composer keyed by parentEventId.

## Decision

Built buildCommentTree() mirroring iOS CommentTreeBuilder: CommentNode data class, recursive assembly from topLevelEventIds and childLinks, replyingToEventId state tracking, ReplyContextBanner ("Replying to @name" with cancel), depth-1 indented display with thread rail, "View N more replies" chip, and SetCommentDraft/PublishComment carrying optional parentEventId.

## Consequences

- Rust core owns all ordering and orphan promotion; Kotlin only assembles view nodes
- CommentRecord does not carry parentEventId directly (uses parentTagName/parentTagValue for NIP-22 e/q tags); tree builder uses topLevelEventIds + childLinks from the snapshot
- Each comment row dispatches RequestProfile(pubkey) for avatar/name hydration (deduped by Rust)
- Threaded replies reachable only via highlight-detail screen or room detail (not directly from flat feed cards)

## Open Tail

*(none)*

## Evidence

- transcript lines 1985-2019

