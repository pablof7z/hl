# iOS Article Reader — v2 Roadmap

Status: **deferred** — v1 shipping 2026-04-22. This file tracks the surface
work that v1 explicitly defers.

Architecture contract carries over from v1: **nostrdb is the single source of
truth.** Every item below reads and writes through the Rust core's ndb path;
Swift stores subscribe to deltas.

## Deferred features

### 1. Comments (NIP-22 `kind:1111`)

- Reader gets a "Comments" section (or lazy-loaded tab) rendering a threaded
  view of `kind:1111` events that reference this article's `a`-tag.
- Rust core needs `comments::query_for_article(ndb, address)` and a new
  `SubscriptionKind::ArticleComments` variant in `subscriptions.rs`.
- UI mirrors the web `ArticleView.svelte` comment tree (parent → children,
  reply-in-place textarea).
- Posting a comment: reuse `publish_discussion` if root, or add
  `publish_article_reply(parent_event_id, body)`.

### 2. Typography controls

- Top-right gear → sheet with: font size (S/M/L/XL), serif vs sans body face,
  paper vs cream vs sepia vs dark background.
- Persist choice in `UserDefaults` (client-only; not a nostr kind).
- `MarkdownRenderer` takes a `TypographyOptions` struct and parameterizes the
  emitted `NSAttributedString` (font family, base point size, paragraph
  spacing, link color).
- Dark mode variant needs a separate highlight-tint alpha to stay legible.

### 3. Reading progress

- `UITextView.contentOffset` → percentage of total body height.
- Thin progress bar at the top of the nav bar (0%–100%).
- "X min left" readout derived from remaining words ÷ 240 wpm.
- Optional: persist scroll position per `a`-tag in `UserDefaults` so reopening
  the article lands the reader where they left off.

### 4. Bookmarks (NIP-51 `kind:10003`)

- Nav bar toolbar item: bookmark icon (filled when this article's `a`-tag is
  in the user's latest `kind:10003` list).
- Rust core needs:
  - `lists::query_bookmark_list(ndb, user_pubkey)` returning the latest
    `kind:10003` event;
  - `lists::set_bookmark_address_presence(runtime, list_event, address, present)`
    mirroring `web/src/lib/ndk/lists.ts::setBookmarkAddressPresence`.
- The Bookmarks tab (new root-level destination) lists every article whose
  `a`-tag appears in the current bookmark list.

### 5. "Share highlight into a community" action

- From the highlight detail bottom sheet: a "Share to community" button opens
  a community picker (existing `joinedCommunities` list).
- On pick → call the existing Rust `publish_highlights_and_share` with the
  **already-published** highlight as a `HighlightDraft` reconstruction, OR
  add a new `share_existing_highlight(event_id, target_group_id)` that emits
  only the `kind:16` repost (no re-publishing of the 9802).
- Preferred: the latter — `highlights::share_to_community` already exists in
  Rust (`highlights.rs:87-127`) but isn't exposed over FFI. Just wire it up.

### 6. Offline caching of highlights

- v1 inherits ndb's article caching for free (bodies land in ndb via
  `getUserArticles`). Highlights don't cache pre-view — the reader's
  subscription is what fills ndb.
- v2 warmth pass: on app launch, seed highlights for the user's recently
  opened articles by spawning short-lived `{ kinds:[9802], '#a':[addr] }`
  subs for N recent `a`-tags (tracked in a client-side LRU).

### 7. Author-only editing

- Pencil icon visible only when `currentUser.pubkey == article.pubkey`.
- Tap → push a composer seeded with the current `content` + metadata tags.
- Publish: replaceable `kind:30023` with the same `d` tag supersedes.
- Not in v1 — composer is a substantial surface; reader comes first.

### 8. Zaps and reactions

- Nav bar: lightning button with total zap amount; reactions row under
  footnotes.
- Zap flow reuses the existing Lightning address resolution path (if it
  exists in the app — needs audit) or uses NIP-57 zap request.
- Reactions: `kind:7` events referencing the article's `a`-tag; tap an
  emoji chip to toggle your own reaction.

### 9. Improved highlight matching

- v1 uses strict `String.range(of:)` on the flattened body text; if the
  highlight's `content` doesn't match byte-for-byte, the overlay silently
  drops. Same fragility as the web app.
- v2: whitespace-normalized match (collapse runs of whitespace to single
  space, then `range(of:)`), falling back to a Levenshtein-bounded fuzzy
  match for content that differs by `<5%` (probably rendered vs. source
  whitespace).
- Open question: whether to add explicit NIP-84 position hints (proposed
  in some NIP discussions) when publishing our own highlights so matching
  doesn't depend on content fuzzing.

### 10. Deep links to highlights

- URL scheme: `highlighter://article/<naddr>?highlight=<event_id>` scrolls
  the reader to the highlight and flashes it.
- Would require adding the `naddr` bech32 into the external URL layer and
  wiring `App.swift`'s `onOpenURL` handler to the article destination.

## Intentionally out of scope

- Text-to-speech playback.
- PDF/print export.
- Annotating images or selecting ranges that span figures.
- Collaborative highlighting (cursors).
