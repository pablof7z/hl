---
type: episode-card
date: 2026-06-12
session: cd5f3967-ddef-43db-91ca-0d6b810bcfea
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/cd5f3967-ddef-43db-91ca-0d6b810bcfea.jsonl
salience: product
status: superseded
subjects:
  - highlight-feed-card
  - nostr-profile-rendering
  - nmp-ui-components
supersedes: []
related_claims: []
source_lines:
  - 1-3
  - 361-361
  - 365-378
  - 488-496
  - 544-562
  - 926-932
  - 988-997
  - 1013-1016
captured_at: 2026-06-12T19:13:26Z
---

# Episode: Replace custom AuthorAvatar with NMP registry UI components for profile rendering

## Prior State

Highlights feed used a custom `AuthorAvatar` SwiftUI view with manual profile resolution — calling `app.profile(pubkeyHex:)` to read and `app.requestProfile()` in `.task` modifiers to fetch. When profile data was missing or slow to resolve, the fallback showed truncated pubkey hex (e.g. 'a9434ee165 · 4d') and a gradient identicon. NMP's canonical SwiftUI components (`user-avatar`, `user-name`) existed in the registry but were not installed or used.

## Trigger

User reported that highlights were not showing profiles properly (some pubkeys rendered as hex truncations) and explicitly asked whether NMP UI components were being used. On investigation, confirmed they were not — only a hand-rolled `AuthorAvatar` existed. User then corrected the assistant's assumption that NMP had no UI layer: 'nmp DOES have UI components! installable via nmp cli!'

## Decision

Adopted the NMP registry components (`swiftui/user-avatar` → `NostrAvatar`, `swiftui/user-name` → `NostrProfileName`) as the canonical profile rendering primitives in the highlights feed. `HighlighterStore` was extended to conform to `NostrProfileHost` so components self-claim profile resolution on mount. Components were installed via `nmp add component` (not manual vendoring) to preserve lock-file tracking and future `nmp update component` capability. The custom `AuthorAvatar` and manual `.task { requestProfile }` calls in `HighlightFeedCardView` were replaced with `NostrAvatar` + `NostrProfileName`.

## Consequences

- Profile resolution lifecycle is now owned by the NMP components (claim-on-mount pattern) instead of per-view `.task` calls, eliminating duplicated or missed requestProfile calls
- Component source lives at the canonical `Components/NostrUser/` path dictated by the NMP registry manifest, with a `nmp.components.lock` file for future updates
- An initial attempt to manually vendor files into `Core/NMPUI/` was reverted — the `nmp add component` CLI is now the required installation method
- `App.swift` injects `.environment(\.nostrProfileHost, store)` so all NMP UI components in the view hierarchy can resolve profiles without explicit wiring
- The `AuthorAvatar` view is no longer referenced from the highlights feed (may still exist for other consumers)

## Open Tail

- Other views in the app (e.g. article reader, comments) may still use `AuthorAvatar` or manual profile resolution — same migration pattern should apply
- Need to verify that profile data actually populates correctly at runtime now that claim semantics changed
- The `NostrAvatar` was adapted from the registry version to use Kingfisher instead of AsyncImage — this local override may conflict on `nmp update component`

## Evidence

- transcript lines 1-3
- transcript lines 361-361
- transcript lines 365-378
- transcript lines 488-496
- transcript lines 544-562
- transcript lines 926-932
- transcript lines 988-997
- transcript lines 1013-1016

