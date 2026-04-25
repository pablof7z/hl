# Web ⇆ iOS Feature Parity — Plan (2026-04-24)

The iOS app pulled ahead in the last sprint. This plan brings the web app back to parity on the user-facing surface, with priority on shared NDK adapters that unblock multiple features at once.

Web stack: SvelteKit 2 + Svelte 5 runes, DaisyUI 5 + Tailwind 4, NDK 4 (`@nostr-dev-kit/svelte`), Blossom 8.

## Gap summary (vs. iOS, last 7 days)

iOS shipped, web missing or partial:

- **Reading:** podcast clip-timeline player, book reader, NIP-22 comments on every artifact (web has it room-scoped), share-to-room beyond Nostr-articles.
- **Rooms:** chat tab (kind:9), stacked lanes per room, featured rooms via NIP-51 kind:10009, member-only CTAs, paywall gate.
- **Capture:** dedicated capture entry, share-to-room, book OCR/ISBN (skip web — camera).
- **Discovery:** NIP-50 search across all four buckets (articles/rooms/profiles/highlights), nostr: URI rendering on non-article surfaces (comments, notes).
- **Network:** NIP-65 outbox routing (NDK has it, web disabled it), NIP-11 probe, NIP-78 app-data RelayConfig, NIP-77 negentropy, full Network Settings UI.
- **Profile:** follow/unfollow/mute actions, profile sheet.
- **Settings:** no app-level settings page, no Blossom server management UI.

Web is *ahead* on: NIP-22 comment publish (fully wired to NDK), NIP-29 admin (invite mint, member add/remove, edit metadata), bookmarks (kind:10003 add/remove via `lib/ndk/lists.ts`), nostr: URI rich rendering in `MarkdownEventContent` (just not used everywhere yet).

## Shipping order

### Tier 1 — NDK adapter foundations

These are small and unblock multiple features. Build them in `web/src/lib/ndk/` per the subtree's AGENTS.md.

1. **Outbox routing on** — `client.ts` `enableOutboxModel: true`. NDK 4 has the planner built-in, web just disabled it. Re-enables correct fan-out for following feeds.
2. **`relay-probe.ts` (NIP-11)** — `probeRelayNip11(url)` → `{name, description, icon, supported_nips, software, version, fees}`. Used by Network Settings and `/relay/[hostname]`.
3. **`app-data.ts` (NIP-78 kind:30078)** — `readAppData(d)` / `publishAppData(d, payload)` keyed by d-tag, scoped per pubkey. First consumer: relay roles (rooms-host, indexer, search) mirroring iOS `RelayConfig`.
4. **`search.ts` (NIP-50)** — `readSearchRelayList(ndk, pubkey)` (kind:10007), `buildSearchFilter({kinds, query, limit})` emitting NIP-50 `search` field, `fetchSearch(ndk, kinds, query)` routed via the user's search relays (default fallback when empty).
5. **`lists.ts` extension (NIP-51 kind:10009)** — friends'-rooms list reader (parse `group` tags), `fetchFeaturedRooms(ndk, host)` reading Highlighter's curated featured-rooms list.
6. **Render nostr: URIs in comments + room notes** — wrap `CommentCard` / room `NotesTab` content in `MarkdownEventContent` so mentions and event refs render as inline chips/cards. Phase-1 parity with iOS `NostrRichText`.

### Tier 2 — visible UX

7. **`/search` 4 buckets** — articles (kind:30023), rooms (kind:39000 / NIP-29 metadata), profiles (kind:0), highlights (kind:9802). Tabbed page. Server-side `+server.ts` calls the search adapter.
8. **Rooms explorer** — replace bare `/rooms` grid with iOS-parity layout: hero card, "Friends are here" shelf (kind:10009 reads of friends), "Featured rooms" shelf, then "All rooms" grid below.
9. **NIP-29 chat tab** — add `Chat` tab to `/r/[slug]`. Live subscription on kind:9 with `#h=[groupId]`, optimistic compose, NIP-10 reply markers, content rendered via `MarkdownEventContent`.
10. **`/settings`** — app-level settings page with Network (NIP-11 + role chips backed by NIP-65 kind:10002 + NIP-78 kind:30078) and Media (Blossom servers via `NDKBlossomList`).
11. **Profile actions** — follow/unfollow + mute buttons on `/profile/[identifier]` header, optimistic updates against NDK session.

### Tier 3 — bigger lifts (not this pass)

- Stacked community lanes (article/podcast/book) — needs design work.
- Podcast clip player UI parity (timeline scrubbing, mark-in/mark-out).
- NIP-77 negentropy sync (NDK supports — wire it as separate work).
- Capture route + share-to-room.

## Source-of-truth references

- iOS feature surface: `app/ios/Highlighter/Sources/Highlighter/Features/`
- Rust core: `app/core/src/{events.rs,subscriptions.rs,relays.rs,search.rs,bookmarks.rs,outbox.rs,blossom.rs,pictures.rs}`
- Web NDK adapter layer: `web/src/lib/ndk/` (own AGENTS.md — keep adapters here)
