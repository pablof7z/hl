# Premium NIP-22 Comments — Design

**Date:** 2026-04-26
**Scope:** A polished, generic NIP-22 (kind:1111) comments experience that attaches to **any artifact** in the iOS app — articles, podcasts, books, websites, highlights — regardless of whether the artifact was shared in a community.

## North Star

> Reading is sovereign. Conversation orbits the work. The toolbar is a small, unobtrusive presence; tapping it opens an AMAZING UX.

## Locked Decisions

### 1. Architecture: Liquid Glass morph + recursive thread push

A **Liquid Glass capsule** lives at the bottom of every reader (article, podcast, book, website, highlight). It shows comment count + an avatar trio. **Tapping the capsule** matched-geometry-morphs it into a **bottom sheet** that takes over the bottom half of the screen.

The sheet hosts a **NavigationStack**. Tapping any comment row pushes a **thread view** (the same comment, now centered, with its own sub-conversation below). Pushes can recurse arbitrarily — every comment can become its own room. **Drag-down on the sheet at any depth dismisses everything back to the artifact.** The artifact is always one swipe away.

### 2. Detents

Three detents:
- **Peek (140pt):** intermediate dismissal state — never the landing detent
- **Half (52%):** **default landing**. The artifact dims to ~60% behind, taps on it dismiss
- **Full (top safe-area + 8pt):** for deep reading + active typing

Tapping the toolbar lands at half. Pulling up promotes to full. Pulling down crosses peek to dismiss.

### 3. Toolbar — Liquid Glass capsule with scroll-shrink

- **At rest:** 56pt-tall capsule above safe-area bottom. Avatar trio (3 most-recent commenters, 24pt, 8pt overlap) + "**N comments**" in SF Pro Text 15/Semibold + accent send glyph. Glass: `.regularMaterial` with 1pt inner highlight.
- **Shrunk on scroll-down past ~120pt:** width collapses to ~96pt, height to 32pt, single 18pt avatar + count "**N**" in 12pt (text-xs). Re-anchors to the bottom-trailing corner. Spring response 0.38, damping 0.82.
- **Coexistence with mini player accessory:** when the TabView Liquid Glass accessory carries the mini player, the comments capsule mounts as the **left sibling** on the same accessory plate. Otherwise it floats freestanding. Never stacks vertically.
- **Pulse on new comment:** when sheet is closed and a new comment arrives via NostrDB delta, the capsule pulses (scale 1.0 → 1.04 → 1.0, 280ms) and the latest avatar slides into the trio from the right.

### 4. Composer rule — composer always replies to the *current thread's subject*

- **Sheet root composer** = new top-level comment on the artifact
- **Pushed thread composer** = reply to that comment (the comment is the parent)
- **No "Reply" button** anywhere in any cell. **Tap a row = push its thread.** The push *is* the reply gesture.
- 350ms ceremony before keyboard is the right amount of friction for considered replies.
- Composer is bottom-pinned, 56pt at rest, grows to ~140pt on focus. Mention autocomplete = horizontally-scrolling rail of glass chips above the composer when `@` is typed. Drafts are in-memory keyed by `(artifactRef, parentEventId ?? "root")`. (Persistent drafts deferred — YAGNI.)

### 5. Cell anatomy — whisper-quiet

- Avatar (44pt at depth 0 / 32pt at depth 1) + name + relative time
- Body rendered via `NostrRichText` (mentions reuse Phase 2 chip pills)
- Footer: a single 12pt heart glyph at 50% opacity, with count to its right *only when count > 0*
- Trailing: chevron + reply count when comment has replies
- **Tap row = push thread** (single tap target, the row is the gesture)
- **Double-tap row body = like** (Instagram pattern, optimistic kind:7, heart bursts from tap point)
- **Long-press row = action menu** (bookmark, share, copy text, view profile, mute author)
- **No swipe gestures** — they conflict with the system back-swipe in pushed views

### 6. Inline reply preview — depth 1

At the sheet root view, top-level comments render with **the most-recent reply** inline below them (one indent, 32pt avatar, vertical thread line in accent @ 30%). If there are more replies beyond that one, a "**› 6 more replies**" inline chip sits below. The inline reply is **fully interactive** — same heart, same double-tap, same long-press, same tap-to-push.

**Author signal:** if the inline reply is by the artifact's author (article author, podcaster, etc.), the thread line tints to **gold/accent-strong** instead of accent — free signal that the author engaged.

## Killer Detail

**The capsule never disappears during the morph — it *is* the sheet.** Drag the sheet down and it physically shrinks back into the toolbar capsule, glass-to-glass, with the comment count ticking through any new arrivals that landed while it was open. At the moment it re-docks, if a new comment arrived, the capsule does a single soft pulse and the latest avatar slides into the trio from the right. One continuous object, reader → thread → reader.

## Generic over artifacts

Comments scope is driven by NIP-22 root tags:

| Artifact | Root tag | Tag value | Root kind |
|---|---|---|---|
| NIP-23 article (kind:30023) | `A` | `30023:<pubkey>:<d>` | `30023` |
| Highlight (kind:9802) | `E` | `<event_id>` | `9802` |
| Podcast item (NIP-73) | `I` | `podcast:item:guid:<guid>` | `30023` (or per share) |
| External URL | `I` | `url:<href>` (or domain-specific) | (none / per share kind) |
| Book (NIP-73 ISBN) | `I` | `isbn:<isbn>` | (per share kind) |
| Comment-on-comment | inherits root, parent = comment-id | | |

Swift owns the mapping from in-app artifact types to (`rootTagName`, `rootTagValue`, `rootKind`). The Rust layer takes raw scope info.

## v1 Scope (this PR)

Included:
- Toolbar + sheet + thread navigation + recursive push
- Cell + composer + mention rendering reuse
- Likes (kind:7) and bookmarks (kind:10003) on comments
- New top-level threads + nested replies
- Wire into ArticleReaderView, WebReaderView, PodcastListeningView, HighlightDetailSheet

Deferred:
- **Span/timestamp/CFI anchoring** ("comment on this passage") — strong NIP-84 + `q`-tag layering possible later
- Persistent drafts (in-memory only for v1)
- Reaction emoji beyond `+` (NIP-25 supports any unicode; v1 = like/unlike only)
- Mute author follow-through (action menu surface, no muting plumbing yet)
- Web parity

## Implementation Slices

1. **Core (Rust):** extend `publish_comment` for generic root + parent (replies); add `reactions.rs` (kind:7); extend `bookmarks.rs` for e-tag bookmarks; UniFFI-bind.
2. **iOS primitives:** `ArtifactRef` value type, `CommentTreeBuilder` (flat → nested), `CommentsViewModel`.
3. **iOS UI primitives:** `CommentsToolbar`, `CommentsSheet`, `CommentRow`, `ThreadView`, `Composer`.
4. **Wire into surfaces:** Article, Web, Podcast, Highlight readers.
5. **Polish:** matched-geometry morph, pulse animations, live NostrDB updates.

Commit + build + relaunch after each visible slice (per `feedback_commit_and_relaunch.md`).
