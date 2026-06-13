---
type: episode-card
date: 2026-06-12
session: cd5f3967-ddef-43db-91ca-0d6b810bcfea
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/cd5f3967-ddef-43db-91ca-0d6b810bcfea.jsonl
salience: architecture
status: active
subjects:
  - nmp-registry
  - nostr-avatar-component
  - image-loading
supersedes: []
related_claims: []
source_lines:
  - 1190-1348
captured_at: 2026-06-12T20:53:33Z
---

# Episode: NMP registry NostrAvatar standardized on Kingfisher instead of AsyncImage

## Prior State

The registry swiftui/user-avatar component used SwiftUI's AsyncImage with a comment instructing apps to 'replace AsyncImage with your own image cache (Kingfisher, Nuke, etc.)' — requiring per-app adaptation of every installed copy.

## Trigger

User called the per-app adaptation approach 'pretty dumb' and directed that Kingfisher should be the standard at the NMP registry level.

## Decision

Changed the registry source for NostrAvatar to import Kingfisher and use KFImage directly, removing the AsyncImage fallback pattern. Updated all installed copies (nmp-gallery, Highlighter) to match.

## Consequences

- NMP registry now assumes Kingfisher as the standard image cache — apps adopting NMP components must have Kingfisher as a dependency
- nmp-gallery will need Kingfisher added as a dependency when it builds
- The 'replace AsyncImage' instruction comment was removed from the registry source
- Future nmp update component calls will preserve this Kingfisher-based version

## Open Tail

- content-core's NostrContentRenderer still has a framework-agnostic imageLoader closure pattern — may need similar Kingfisher standardization
- NostrMediaGrid also references Kingfisher adaptation in its comments

## Evidence

- transcript lines 1190-1348

