---
type: episode-card
date: 2026-06-12
session: cd5f3967-ddef-43db-91ca-0d6b810bcfea
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/cd5f3967-ddef-43db-91ca-0d6b810bcfea.jsonl
salience: architecture
status: active
subjects:
  - highlight-feed-card
  - nmp-ui-components
  - profile-resolution
supersedes:
  - 2026-06-12-1-replace-custom-authoravatar-with-nmp-registry
related_claims: []
source_lines:
  - 1-1388
captured_at: 2026-06-12T20:53:33Z
---

# Episode: Highlights feed migrated from custom AuthorAvatar to NMP registry components

## Prior State

Highlights feed used a custom AuthorAvatar view with manual app.profile() reads and app.requestProfile() calls in .task modifiers. NMP's canonical UI components (user-avatar, user-name) existed in the registry but were not installed in the Highlighter app. Profiles that hadn't resolved yet showed raw pubkey hex prefixes.

## Trigger

User reported highlights not showing profiles correctly (some showing fallback like 'a9434ee165 · 4d'). User corrected the assistant's assumption that NMP was FFI-only, confirming NMP has installable UI components via CLI.

## Decision

Replace AuthorAvatar + manual profile lifecycle with NMP registry components (NostrAvatar, NostrProfileName) installed via `nmp add component`. HighlighterStore conforms to NostrProfileHost protocol, and the environment key is injected in App.swift so components self-claim profile resolution on mount.

## Consequences

- NostrAvatar owns its own claimProfile lifecycle — no more manual .task { app.requestProfile() } calls scattered in views
- Components installed to standard Components/NostrUser/ path with nmp.components.lock for future updates
- HighlighterStore+NostrProfileHost adapter bridges Rust-backed ProfileMetadata to the ProfileWire type the registry expects
- AuthorAvatar is no longer used in HighlightFeedCardView — replaced by the canonical component
- The initial wrong approach (manual vendoring into Core/NMPUI/ with no lock file) was reverted before commit

## Open Tail

- Other views (ArticleReaderView, etc.) may still use AuthorAvatar and should be migrated similarly
- ProfileWire.npub fields fall back to hex prefix since Rust kernel doesn't produce npub yet

## Evidence

- transcript lines 1-1388

