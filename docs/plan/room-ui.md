# Room UI — Implementation Plan

**Version 1.0 · 2026-04-19**
**Status: active · supersedes the existing room implementation entirely**

---

## 0. Purpose

This plan specifies the full replacement of the room surfaces in the web app. It covers the room landing page, the four-tab pinned-artifact view inside it, and the two full-viewport artifact views (article and podcast) that open from the room.

The plan is grounded in three design mocks produced during our 2026-04 exploration. They are the canonical source of truth for every layout, palette, and interaction decision in this document. Where this document and the mocks disagree, the mocks win until this plan is updated.

| Mock | File | Role |
|---|---|---|
| Room landing | `docs/landing-proposals/room-signal-vs-noise.html` | Definitive room surface — use as the reference for every decision about layout, spacing, colour, and copy. |
| Article view | `docs/landing-proposals/artifact-article.html` | Full-viewport reading surface for essays and articles. |
| Podcast view | `docs/landing-proposals/artifact-podcast.html` | Full-viewport listening surface for podcasts and other timeline-based audio. |

When the mocks are updated (they will be), this document is updated to match in the same pull request.

---

## 1. Scope

### 1.1 In scope

1. The room landing page at `/room/[roomId]` with all six sections (Pinned, This week, The shelf, Highlights, Discussions, Lately) and the sidebar (Members, Up next, Capture).
2. The pinned-artifact view embedded in the room, with four tabs: Discussions (default), Highlights, Notes, Members.
3. The **article** artifact view at `/room/[roomId]/artifact/[artifactId]` when the artifact is an essay / article / long-form text.
4. The **podcast** artifact view at the same route when the artifact is a podcast or other timeline-based audio.
5. A shared design-token layer (CSS custom properties) feeding both DaisyUI theme config and a Svelte component library.
6. A shared Svelte component library for every reusable piece (MemberDot, ArtifactTile, HighlightCard, DiscussionRow, etc. — inventory in §7).
7. Client-side interaction behaviour: tab switching on the pinned artifact, scroll-spy for the secondary room nav, member-filter state for highlights / discussions / shelf.

### 1.2 Explicitly out of scope for this replacement

- Landing / marketing pages (tracked separately; see `docs/landing-proposals/03-annotation.html`)
- The vault (`/me/**`) surfaces
- The discover page (`/discover`)
- Room creation, invite flows, moderation UI
- The notification layer and push
- Mobile / desktop native clients (they share the Nostr data model but have their own UI layers per `product-surfaces-v3.md`)
- Search
- Real-time presence (see §2.5 — explicitly rejected as a design direction)

### 1.3 Code and artifacts being replaced

| Path | Action | Notes |
|---|---|---|
| `web/src/routes/community/[id]/+page.svelte` | Remove | Replaced by `/room/[roomId]/+page.svelte` |
| `web/src/routes/community/[id]/content/[contentId]/+page.svelte` | Remove | Replaced by `/room/[roomId]/artifact/[artifactId]/+page.svelte` |
| `web/src/routes/community/[id]/content/[contentId]/discussion/+page.svelte` | Remove | Discussions are now a tab inside the artifact view, not a separate route |
| `web/src/lib/features/community/**` | Remove | Replaced by `web/src/lib/features/room/**` |
| `web/src/lib/features/artifacts/**` (room-facing components) | Replace | Kept only where NDK event helpers can be reused |
| Prior highlight-card and passage-mock styling | Remove | Replaced by components listed in §7 |

A redirect from `/community/[id]` to `/room/[roomId]` is installed for one release cycle, then removed.

### 1.4 Terminology

This plan uses **room** as the user-facing term for what NIP-29 calls a group. The word `community` is retired from user-facing copy and URLs. The word `artifact` is retained internally (event schemas, prop names, developer docs) but never shown to users — in the UI it becomes *a book, a podcast, an essay,* etc. (see §11 decision D-09).

---

## 2. Design language — locked decisions

The design language below is locked for v1 of this rebuild. No member of the implementation team should adjust palette, typography rules, or spacing values without an explicit review cycle.

### 2.1 Palette

Exactly six structural values + six member tints + one pop colour. No other colours enter production.

| Role | Token | Hex | Usage |
|---|---|---|---|
| Page background | `--bg` | `#FAFAF7` | The default body colour — warm near-white, barely tinted. **This is not cream.** |
| Surface | `--surface` | `#FFFFFF` | All cards, panels, and contained content. Pure white. |
| Surface warm | `--surface-warm` | `#F5EFE0` | Reserved for *one* accent moment per surface: the discussion thread panel, a highlight callout, a note card background. Never the default surface. |
| Surface muted | `--surface-muted` | `#F3F2EE` | Neutral greyed tint, used for progress-bar tracks and filter-pill backgrounds. |
| Rule | `--rule` | `#E5E0D0` | Default border and divider. |
| Rule soft | `--rule-soft` | `#EFEAD9` | Softer divider, used between repeating rows. |
| Ink | `--ink` | `#15130F` | Primary text. |
| Ink soft | `--ink-soft` | `#3A362E` | Body text. |
| Ink fade | `--ink-fade` | `#7A7468` | Labels, metadata. |
| **Accent** | `--brand-accent` | `#C24D2C` | Terracotta. The only pop colour. Used for hover states, em-accents in section headings, the currently-active state, link colour. |
| Marker | `--marker` | `#F5D896` | Honey-amber used for the signature reader highlight. |
| Marker strong | `--marker-strong` | `#E8B96A` | Slightly stronger amber used for highlight borders. |

**Member tints** — six desaturated tones used consistently across every surface to identify a member's highlights, avatars, and colour-coded activity.

| Member | Token | Hex |
|---|---|---|
| DK | `--h-amber` / `--marker` | `#F5D896` |
| Pablo F | `--h-sage` | `#C8D4B5` |
| Miljan | `--h-blue` | `#BCD0E0` |
| Bob S | `--h-rose` | `#EAC6C8` |
| Steve L | `--h-lilac` | `#D0C4E0` |
| Max W | `--h-amber-l` | `#F5E6A8` |

In production, the colour assigned to each member is determined by the first six members in the room in join order; `member-dot` component takes a `colorIndex: 1..6` prop. Additional members beyond the sixth cycle back through the palette.

### 2.2 Typography

Two families plus a mono for labels. **The rule is absolute**: serif is for content, sans is for chrome.

| Family | Tokens | Purpose |
|---|---|---|
| `Inter` | `--font-sans` | Everything that is not content. Navigation, labels, section headings, card titles, metadata, buttons, form fields, table cells, filter pills. Weight 400–700 as needed. |
| `Fraunces` | `--font-serif` | **Content only.** The room title itself, book passages, highlighted quotes, note bodies, inline italic emphasis (`em` within content), personal voice status lines, and typography on artwork covers. |
| `JetBrains Mono` | `--font-mono` | Labels with a technical tone: timestamps, kickers, key counts, monospace codes. |

**Forbidden**: Fraunces for any h2 / h3 / section heading / button / form field / placeholder / status pill / footer chrome. If it appears in a "chrome" context in any future ticket, it is a bug.

**Size scale** (exact values used in the mocks — do not deviate):

| Role | Size / weight / tracking |
|---|---|
| Room title | Fraunces 400, clamp(44px, 6vw, 68px), tracking -0.025em, line-height 1.02 |
| Section head h2 | Inter 700, 19px, tracking -0.018em, line-height 1.15 |
| Section head `em` accent | Inter 700, colour `--brand-accent`, same size as h2 (not italic) |
| Artifact title (card) | Inter 600, 17px, tracking -0.005em |
| Body content (Fraunces) | Fraunces 400, 17px (notes) / 18–22px (passages / article body), line-height 1.55–1.72 |
| Label / kicker (mono) | JetBrains Mono 400, 10–11px, tracking 0.1–0.22em, uppercase |
| Highlight quote | Fraunces italic, 16–22px depending on surface |
| Member status | Fraunces italic 13px, line-height 1.4 |

### 2.3 Spacing and grid

Base unit: **4px**. All spacing is a multiple (4, 8, 12, 14, 16, 18, 20, 22, 24, 28, 32, 36, 40, 44, 48, 56, 64, 72, 80, 96).

**Page container**:
- Max-width: 1440px
- Horizontal padding: 40px desktop, 20px mobile

**Main grid on the room page**:
- `grid-template-columns: minmax(0, 1fr) 380px`
- `gap: 44px`
- Collapses to single column below 1060px width

**Block spacing** between sections: 44px margin-bottom on each `.block`

**Scroll margin** on anchored blocks (for the secondary nav jumps): 120px top

**Border radius**: 4px on all cards and surfaces. 2px on highlighted-phrase backgrounds. 999px on filter pills. No other radii.

### 2.4 Motion

Minimal, purposeful, deliberate.

| Interaction | Specification |
|---|---|
| Generic transition | 200ms ease-out on `border-color`, `transform`, `background` |
| Card hover | `translateY(-2px)` and border-colour shift to `--brand-accent` |
| Tab switch | Instant (display:none ↔ display:block). No fade, no slide. |
| Scroll-spy active nav update | Instant |
| Filter pill toggle | Instant |

**Explicitly forbidden**: bouncy springs, slide-ins on load, parallax, any hover animation longer than 200ms, any load-spinner animation that draws the eye.

### 2.5 Presence anti-patterns (strict — these are hard no's)

Real-time presence was considered and **rejected**. The room must not surface live-activity pressure.

**Do not build**:

- A pulsing "N active now" dot anywhere on the page
- Green "online" overlay dots on member avatars
- "X is listening at timestamp Y" real-time indicators
- "Someone is reading this right now" banners
- Typing indicators
- "Last seen N minutes ago" per-member timestamps on any surface that isn't explicitly an admin view
- Any UI that updates without the user interacting and demands attention

**Acceptable** presence signals (past-tense, not live):

- "Last here Tue 9:00" style timestamps in the Members tab of the pinned artifact (past activity, not live)
- Cumulative progress bars ("Member got to Ch. 5") showing how far each person has read — these are state, not real-time
- "Bob & Steve haven't started" on a specific artifact — room state, not live
- Thread status ("Active") on discussions — thread state, not person state

The test is: *does this UI update when another member does something, while the current user is looking at it, in a way that draws attention?* If yes, don't build it.

---

## 3. Room landing page

### 3.1 Route

`/room/[roomId]` — where `[roomId]` is the NIP-29 group id (`kind:39000.d`).

Server loader (`+page.server.ts`) hydrates public-safe metadata only (`kind:39000`). All member-gated content (artifacts, highlights, threads, notes) loads client-side after NIP-42 auth completes.

### 3.2 Vertical structure

```
┌─ Top nav (sticky) ────────────────────────────────────┐
│  [logo]  [Your rooms] [Discover] [Vault]   [⌕] [✎] [👤] │
└───────────────────────────────────────────────────────┘
┌─ Room header band (in page flow, scrolls) ────────────┐
│  Signal vs Noise                                       │
│  [6 member avatars, overlapping]                      │
└───────────────────────────────────────────────────────┘
┌─ Secondary room nav (sticky under top nav) ───────────┐
│  Pinned · This week · The shelf 24 · Highlights 412  │
│  · Discussions 38 · Lately                            │
└───────────────────────────────────────────────────────┘
┌─ Main grid ───────────────────────────────────────────┐
│ LEFT COLUMN (flex 1)          │ SIDEBAR (380px fixed) │
│                               │                        │
│ #pinned — featured artifact   │  Members (6 rows)      │
│    with 4-tab view            │                        │
│                               │  Up next — voting      │
│ #this-week — 2 cards          │                        │
│                               │  Capture CTA           │
│ #shelf — filter row + grid    │                        │
│                               │                        │
│ #highlights — filter row      │                        │
│    + card grid                │                        │
│                               │                        │
│ #discussions — filter row     │                        │
│    + list of rows             │                        │
│                               │                        │
│ #lately — activity feed       │                        │
└───────────────────────────────┴────────────────────────┘
┌─ Footer ──────────────────────────────────────────────┐
│  Highlighter.                            room meta    │
└───────────────────────────────────────────────────────┘
```

At viewport < 1060px, the sidebar flows under the main column. At < 760px, the top nav's desktop links disappear (hamburger replacement is out of scope for this plan — acceptable compromise for v1).

### 3.3 Top nav (same across all room and artifact views)

Sticky, full-width white with `border-bottom: 1px solid --rule`. Height 62px.

- Left: Logo "Highlighter." (Inter 600 17px, terracotta full-stop) + 3 nav links (`Your rooms` active, `Discover`, `Vault`)
- Right: Search icon button (⌕), `Share to a room` primary button (dark ink bg), user avatar (member dot in user's colour)

**No green online dot on the user avatar.** See §2.5.

### 3.4 Room header band

Purpose: identify the room. Nothing else. No real-time state, no tagline, no meta row.

Contents, top to bottom:
1. Room title — Fraunces 400 clamp(44–68px), margin-bottom 32px
2. Row of overlapping member avatars (36px each, 2.5px border in bg colour)

**No**: room kicker ("— Room · reading together since…"), tagline, meta row, "N active today" pulse, "X people are reading right now" note, any presence signal.

Padding: `56px 0 36px`. Bottom border: 1px `--rule`.

### 3.5 Secondary room nav

Sticky directly under the top nav (top: 62px). Height ~44px.

Contents: six anchor links to the six sections below.

- `Pinned`
- `This week`
- `The shelf` with count (e.g. "24")
- `Highlights` with count (e.g. "412")
- `Discussions` with count (e.g. "38")
- `Lately`

Counts come from live data (see §8). Active link is determined by scroll position — a simple scroll-spy that highlights the link whose section is currently under the top threshold (140px).

Styling: Inter 500 13px, ink-fade colour, 2px bottom border on active (terracotta), counts in JetBrains Mono.

No hamburger. Horizontal scroll on narrow viewports.

### 3.6 Section: `#pinned` (Currently pinned)

Heading: `Currently pinned.` (Inter 700 19px, "pinned." in terracotta)

No meta sub-heading (no "week 2 · 31 highlights · 5 threads" — the tabs convey this).

The pinned block is a **white card with border** (`--rule`) containing:

1. **Artifact header row** (`.pinned-top`):
   - Cover visual (140px wide, 2:3 aspect for books, 1:1 for podcasts, vertical essay for longform web) — typographic, no image dependency
   - Title (Inter 600 26px) + subtitle (Inter italic 14px) + stats row (4–5 counts in Inter small)
   - Reader avatars — a row of `member-dot`s showing who's in vs who hasn't started (with a quiet Inter italic note like "Bob & Steve haven't started" — past-state, not live)
   - Actions: `Open artifact` (secondary) + `Continue reading` (primary dark-ink filled)

2. **Tab strip** with four tabs. See §4 for each tab's spec.

3. **Active tab panel** — rendered inline based on the active tab.

Box shadow: `0 18px 40px -22px rgba(21, 19, 15, 0.12)` — a deep shadow used only on this most-important card.

### 3.7 Section: `#this-week` (Also this week)

Heading: `Also this week.`

Content: a grid of 2 columns (single col below 760px) of secondary artifact cards. Each card is a `<a>` — clicking takes the user to the full artifact view.

Card contents:
- Kicker line: `[Type] · shared by [member] · [when]`
- Header: square artwork (58px) + title + source (italic)
- Highlighted excerpt block: warm-cream accent background, 3px amber left border, timestamp or "a line worth re-reading" label, italic serif quote, two reaction rows (member dot + name bold + text)
- Foot row: avatar stack (who engaged) + `Open episode / Open essay →` arrow

### 3.8 Section: `#shelf` (The shelf — all past material)

Heading: `The shelf.`

**Filter row** — horizontally scrollable pills (sans 500):
- `Everything [count]`
- `Books [count]`
- `Podcasts [count]`
- `Essays [count]`
- `Papers [count]`
- `Archive [count]`
- Right-aligned: sort dropdown ("Recent ↓")

**Grid**: `repeat(auto-fill, minmax(180px, 1fr))`, gap 14px. Each tile is a `<a>` linking to the artifact view.

Tile structure:
- Typographic cover (aspect-ratio 4:5) with:
  - Top-left: type chip (JetBrains Mono 9px, faint bg)
  - Top-right: optional status badge (`Reading` green, `This week` terracotta, `Re-read` terracotta)
  - Bottom content: title + author inside the cover art
  - Colour treatment per medium (see §7 for `shelf-cover` variants)
- Meta block below cover: title (Inter 600 13px) + author (Inter italic 12px)
- Foot: row of member dots (who engaged) + count string (`N hl · N thr · month`)

**See all 24** link at the bottom — dashed border, terracotta text, fills on hover.

### 3.9 Section: `#highlights` (The room's highlights)

Heading: `The room's highlights.`

**Filter row**:
- `All [count]`
- One pill per member with their colour dot and per-member highlight count
- Right-aligned sort ("Most-replied ↓")

**Grid**: `repeat(auto-fill, minmax(320px, 1fr))`, gap 14px.

Each card (`HighlightCard`):
- Italic serif quote in Fraunces, with smart quote marks in terracotta
- Amber-gradient underline highlight on the quote text itself
- Foot row (flex-end aligned):
  - Left: `<source-label>` (bold title line + italic sub line "ch. 6 · Currently reading")
  - Right (flex column): avatar stack, then reply count ("**3** replies"), then date (mono 10px)

**See all 412** link at bottom.

### 3.10 Section: `#discussions` (Every discussion)

Heading: `Every discussion.`

**Filter row**: All, Active, Unread, Books, Podcasts, Essays, sort ("Most recent ↓")

**List** (single column, dense rows):

Each row is a `<a>` with three columns:
- `auto` width: stack of 2–3 participant avatars
- `1fr`: status pill (`Active` green / `Closed · month` grey), thread title (Inter 600 14.5px), source line ("on **Book** · ch. 6 · started by [member]")
- `auto`: replies count ("● **3** replies" with terracotta hot dot for active) + last-activity time

Hover: subtle background shift to `--bg`.

**See all 38** link at bottom.

### 3.11 Section: `#lately` (Lately in the room)

Heading: `Lately in the room.`

Contents: a single white card containing an activity feed.

Each row: `member-dot` + activity sentence (inline-styled action verb in mono terracotta: `marked`, `replied`, `shared`, `voted`, `proposed`, `started`, `opened`) + right-aligned relative time (mono 10px).

The feed is powered by an NDK subscription (`ndk.$subscribe`) that streams room events in real time. No polling, no refresh button needed — events arrive automatically within the 48-hour window filter. This is past-tense activity, not live-presence signalling — see §2.5.

### 3.12 Sidebar

Fixed 380px wide on desktop, flows below main on tablet.

**Sidebar becomes sticky** at `top: 112px` (below top nav + room nav). Max-height is `100vh − 140px` with `overflow-y: auto`. Own scroll context.

Three cards, stacked 24px gap:

**Members card**: 6 rows. Each row:
- 32px member dot (no online green)
- Name (Inter 600 13.5px) + handle (mono 11px)
- Italic Fraunces status line (one short quoted sentence — personal voice, e.g., "Highlighting at 3am again.")
- **No "active N ago" timestamp.** See §2.5.

**Up next card**: 4 voting rows. Each row: position number (mono) + title/source + vote tally (honey-amber dots matching vote count + number). The card is fully functional — members can cast votes via the relay. Vote event kind: `kind:999` with `h` tag = room id, `a` tag = candidate artifact id, `vote` tag = upvote. Multiple votes by the same member on the same artifact are replaced (idempotent). Bottom: "Voting closes Sunday, 9pm." + `cast yours →` button.

**Capture CTA**: full-width dark-ink block. Icon (Fraunces italic ✎) + heading (Inter 600) + subtitle (Inter 13px). Hovers to terracotta.

---

## 4. The pinned-artifact tabs

Four tabs on the pinned card. Default active: **Discussions** (the most social / most alive surface for an actively-read book). Tabs switch instantly — no transition.

### 4.1 Tab: Discussions

Active by default.

**Content**:
- "Most-discussed passage" label (JetBrains Mono small + chapter reference + reply count)
- The passage itself, rendered as Fraunces 22px, with 2–4 overlapping member-color highlight spans on specific phrases. Each `.hl` span shows a tooltip of who marked it on hover (pure CSS `::after`).
- Warm-cream thread panel (`--surface-warm`) containing:
  - "Thread · **N messages** · started by [member] · [when]" title
  - Messages in a grid (avatar | body), alternating indent for root vs reply
  - Each message: avatar + name bold + handle mono + time mono + body text (Inter 14.5px, italics render as Fraunces)
  - Reply box at bottom: placeholder "Reply in the thread…" + Send button

### 4.2 Tab: Highlights

Purpose: a chronological index of every highlight in this artifact, by all members, in book/content order.

**Panel head** (padding 18px 32px):
- Filter row: `All` pill + one pill per member (with colour dot + count per member). Sort indicator: "By position ↓"

**List** (padding 0 32px):
- Each entry (`HighlightEntry`) has:
  - Meta row (mono small): member dot (22px) + location chip ("Ch. 1 · pg. 8", terracotta) + spacer + date (mono, right)
  - Quote block: Fraunces italic 18px, 2px amber-strong left border, 14px left padding
  - Optional foot row: "● N replies →" link in terracotta sans 12px

**See all N highlights** dashed link at bottom.

Sort: default by book position (chapter + page). Alternatives (future): by member, by replies, by recency.

### 4.3 Tab: Notes

Purpose: longer-form member reflections about the artifact as a whole. Distinct from highlights (which attach to a specific passage) and discussions (which attach to a highlight or the artifact).

**Panel head**:
- Left: "Longer-form reflections from the room. N so far." (Inter 500 12px ink-fade)
- Right: `✎ Write a note` button (outlined `panel-btn`)

**List** (padding 24px 32px, gap 20px between notes):

Each note (`NoteEntry`) is a warm-cream card (`--surface-warm`) with:
- Head row: 36px member dot + (name bold Inter 14 + handle mono 11) stacked with (date mono 10 + reply count)
- Title: Inter 700 18px (not serif — this is chrome, the *body* is content)
- Body: Fraunces 17px paragraphs, line-height 1.65. Multiple paragraphs, no length limit.
- Foot: dashed-top divider + "View N replies →" terracotta link

### 4.4 Tab: Members

Purpose: reading status per member for *this artifact* (different from the sidebar members card which is whole-room).

**Table layout**: 4 columns (`1.5fr 2fr 2fr 90px`).

Header row (JetBrains Mono 10px uppercase ink-fade):
- Member · Progress · Contribution · Last here

Each row (sorted by progress descending):
- **Member**: 30px dot + (name bold Inter 13 + handle mono 10)
- **Progress**: thin 4px progress bar with fill colour (terracotta in progress, `--online` green when finished, transparent when not started) + label line ("**Ch. 6** · pg. 86" or "*not started* — 'will get to it'")
- **Contribution**: three inline counts separated by 14px — "**N** highlights · **N** messages · **N** note" or `—` where absent
- **Last here**: mono 10.5px timestamp right-aligned (this is past-state activity, acceptable per §2.5)

Below 760px: table collapses to stacked rows (grid: 1fr).

---

## 5. Article view — full-viewport reading

### 5.1 Route

`/room/[roomId]/artifact/[artifactId]` — when the artifact's `kind:11` share-thread tags identify it as an article / essay (external URL resolving to a text document, or `a` tag pointing to a `kind:30023`).

### 5.2 Vertical structure

```
┌─ Thin top bar (sticky, 54px) ─────────────────────────────┐
│ ← Signal vs Noise    Essay · [title] · [author]   [filter] [Share] [✎ Highlight] │
└───────────────────────────────────────────────────────────┘
┌─ Hero (max 720px centred) ────────────────────────────────┐
│ Kicker: [An essay · shared by PF · [when] · week 1]       │
│ Title (Fraunces 40–64px clamp)                            │
│ — Author, on source (Fraunces italic 20px)               │
│ [rule] Stats: 12 min · Apr 2023 · 14 highlights · 9 replies │
└───────────────────────────────────────────────────────────┘
┌─ Body + margin (max 1280px) ──────────────────────────────┐
│ ARTICLE BODY (max 720px)    │   MARGIN COLUMN (300px)    │
│ Fraunces 20px, line 1.72    │   Filter by member        │
│ First paragraph: drop cap    │   [pill per member]       │
│ Drop-cap lead-in sub-drop    │                            │
│ ...multi-paragraph prose...  │   Highlights · 14         │
│ <mark> spans in member       │   [8 inline annotation     │
│ colours, tooltip on hover    │    cards sorted by book    │
│ Annot-inline cards between   │    position]              │
│ paragraphs (full-width in    │                            │
│ the content column)          │                            │
└───────────────────────────────────────────────────────────┘
┌─ Article footer card (max 720px centred) ─────────────────┐
│ Kicker: "room stats · this essay"                         │
│ Title: "All six members have read the Dergigi."           │
│ Per-member stats row (avatars + count)                    │
│ ← Back to [room]    Open the full thread →               │
└───────────────────────────────────────────────────────────┘
┌─ Footer ──────────────────────────────────────────────────┐
```

At < 1060px, the margin column flows to stacked inline cards between paragraphs. At < 640px, hero and body pad tightens to 20px.

### 5.3 Article body specifics

- Max column width 720px, centred, but the body + margin grid is laid out inside a max-1280px centred shell.
- First paragraph: first letter (`::first-letter`) is a 68px Fraunces 500 drop cap in terracotta, floated left.
- A non-drop-cap "drop" paragraph can open the piece — italic Fraunces 17px, bottom rule, margin-bottom 32px.
- Highlights are inline `.hl` spans with member-color backgrounds (6 variants matching the room's member palette + one `hl-all` diagonal gradient for everyone-marked-this).
- Tooltip on hover: member name appears above the span (`::after` with `data-by` attribute).
- `annot-inline` cards between paragraphs: full-width (within content column), `--surface` bg, `--rule` border, 3px amber left border, member dot + name + timestamp + reaction text (Inter 13.5px).

### 5.4 Margin column

Two stacked regions:
1. **Filter** (`.margin-filter`) — warm-cream accent card with 6 member pills showing per-member highlight counts. The one design place where cream does real work.
2. **Highlights list** — one `.margin-card` per highlight in order. Each card: head row (avatar, name, time), echoed quote (italic Fraunces 13px with amber gradient underline), commentary (Inter 13px, ink).

Position: sticky at `top: 78px` on desktop, max-height `100vh − 100px`, own scroll.

---

## 6. Podcast view — full-viewport listening

### 6.1 Route

Same URL structure as article view — the view is selected by the artifact's media type (derived from the `i`/`k` tags on the share thread or the linked Nostr event kind).

### 6.2 Vertical structure

```
┌─ Thin top bar ────────────────────────────────────────────┐
│ ← Signal vs Noise   Podcast · [title] · [show #]   [Transcript] [Share] [✎ Mark] │
└───────────────────────────────────────────────────────────┘
┌─ Hero (1200px centred, grid) ─────────────────────────────┐
│ [200px square artwork]    │   Kicker line                │
│ (typographic, no image)   │   Episode title (Fraunces)   │
│                            │   — Host & Guest, on Show   │
│                            │   [rule] 1h 24m · released · │
│                            │   N of 6 listened · N hl    │
└───────────────────────────────────────────────────────────┘
┌─ Player card (1200px centred, white card) ────────────────┐
│  [▶]  [scrubber with highlighted spans]  [15:04 / 1:24:00]│
│  ────────────────────────────────────────────────────────│
│  [←30s] [1.0×] [30s→]                                    │
└───────────────────────────────────────────────────────────┘
┌─ Main grid (1200px centred) ──────────────────────────────┐
│ TIMELINE (1fr)              │   SIDEBAR (320px)           │
│                              │                              │
│ Heading: "Marked moments."   │   Chapters · 6              │
│                              │   [list with timestamps     │
│ Vertical timeline with node  │    and highlight counts]    │
│ connector, chronological     │                              │
│ order:                        │   Everyone's listen · 4/6  │
│                              │   [per-member progress     │
│ Per timestamp: timestamp     │    bars — past state only] │
│ label + card:                 │                              │
│   - head (member dots)       │   Mark a moment CTA        │
│   - transcript quote         │                              │
│   - threaded replies         │                              │
│   - optional mark-a-moment   │                              │
│     prompt                    │                              │
└───────────────────────────────┴─────────────────────────────┘
```

### 6.3 Player

- 48px round dark-ink play button (hover terracotta)
- Scrub track: 48px tall, 6px progress track. Progress fill dark-ink. Scrubber head 14px round dark-ink with soft shadow.
- Highlighted spans overlaid at absolute positions matching timestamps. Height 18px. Hover shows tooltip `MM:SS · Name` (pure CSS).
- Right: current time / total (mono, bold current).
- Ctrl row: 3 pill buttons (skip back 30s, speed, skip forward 30s). **No member-presence display on the right.** See §2.5.

### 6.4 Timeline of marked moments

- Padding-left 40px with a 1px `--rule` vertical line at left 15px.
- Each row: a 14px circle marker (surface bg + 2px terracotta border) at the rule line, positioned at top 16px of the row.
- Timestamp label (mono 11px terracotta 500) above the card.
- Card (`.stamp-card`): white, bordered, subtle shadow, 20–24px padding.
  - Head: member dot(s) + "by **X**" or "**X** + **Y** marked this"
  - Quote: Fraunces italic 18px with 3px amber-strong left border (multi-colour gradient when multiple members marked same span)
  - Replies: dashed-top-bordered section, one `.stamp-reply` per comment (avatar + body + time)
  - Optional prompt at bottom: warm-cream pill with "Mark a passage…" + `✎ Mark a moment` button

### 6.5 Sidebar

- **Chapters card**: 6 rows. Each: timestamp mono + chapter title + hl count. Active chapter highlighted.
- **Everyone's listen card**: 6 rows (all members, including those who haven't listened). Member dot + name + 4px progress bar + position mono. Lower opacity for not-yet-listened members. This shows state, not presence.
- **Mark-a-moment CTA**: same pattern as room sidebar capture CTA.

---

## 7. Component inventory

Every reusable piece built as a Svelte 5 component in `web/src/lib/features/room/components/`. Components take props, never reach into stores directly unless noted.

| Component | Props | Used in |
|---|---|---|
| `TopNav.svelte` | `user`, `activeRoute` | All room and artifact views |
| `RoomHeader.svelte` | `room`, `members[]` | Room landing only |
| `RoomNav.svelte` | `sections[]` with counts; wires scroll-spy internally | Room landing |
| `Block.svelte` | `id`, `title`, `accent` (for em-part); slot for filters + content | All room sections |
| `FilterRow.svelte` | `pills[]`, `activePill`, `sort` | Shelf, Highlights, Discussions, Members panel |
| `FilterPill.svelte` | `label`, `count`, `dotColour`, `on` | Filter row |
| `MemberDot.svelte` | `initials`, `colourIndex` 1..6, `size` (18–36), `title` | Everywhere |
| `MemberStack.svelte` | `members[]`, `size`, `max` | Headers, tiles, sidebar |
| `PinnedArtifact.svelte` | `artifact`, `activeTab` | Room landing |
| `PinTabs.svelte` | `tabs[]`, `activeTab`, `onSwitch` | Pinned artifact |
| `ArtifactHeader.svelte` | `artifact`, `readers[]`, `actions[]` | Pinned artifact top |
| `BookCover.svelte` | `title`, `author`, `subtitle`, `variant` (dark/red/blue/green/plum) | Pinned artifact, shelf, article hero |
| `PodcastArtwork.svelte` | `show`, `title`, `episode`, `variant` (purple/orange) | Shelf, also-card, podcast hero |
| `ArtifactCard.svelte` | `artifact`, `highlight`, `reactions[]`, `engaged[]` | This-week section |
| `ShelfTile.svelte` | `artifact`, `status` (open/week/re-read/none), `engaged[]`, `stats` | Shelf grid |
| `HighlightCard.svelte` | `quote`, `source`, `marks[]`, `replies`, `date` | Highlights reel |
| `HighlightEntry.svelte` | `quote`, `location`, `member`, `date`, `replies` | Highlights tab in pinned artifact, article margin |
| `DiscussionRow.svelte` | `thread`, `participants[]`, `status` (active/closed), `replies`, `lastAt` | Discussions section |
| `NoteEntry.svelte` | `author`, `title`, `body` (slotted paragraphs), `date`, `replies` | Notes tab |
| `MembersTable.svelte` | `rows[]` with progress/contribution/last | Members tab |
| `ActivityFeed.svelte` | `rows[]` with member/action/ref/time | Lately section |
| `Passage.svelte` | `text`, `highlights[]` — renders inline member-color marks with tooltips | Pinned discussions panel, article body |
| `Thread.svelte` | `messages[]`, `onReply` | Pinned discussions panel, stamp-card replies |
| `Player.svelte` | `duration`, `position`, `spans[]`, `onSeek`, `onPlayPause` | Podcast view |
| `TimelineStamp.svelte` | `timestamp`, `quote`, `markers[]`, `replies[]` | Podcast view |
| `MarginCard.svelte` | `highlight`, `echo`, `commentary` | Article margin |
| `CaptureCta.svelte` | `title`, `subtitle`, `href` | Room sidebar, podcast sidebar |
| `SeeAllLink.svelte` | `label`, `href` | End of each scrollable section |
| `Footer.svelte` | none | All pages |

**Implementation note**: no component should directly fetch Nostr events. Data comes in via props, sourced from a feature-level orchestrator (`room/loaders/`) that uses NDK. Components stay presentational.

---

## 8. Data model — NIP mapping

The room page aggregates events from the room's relay (Croissant-based, NIP-29) and the user's personal relays (for their own highlights).

| UI element | Nostr source |
|---|---|
| Room title, member count | `kind:39000` (group metadata) |
| Member list | `kind:39002` (member list) + fallback: tally `kind:9000`/`9001`/`9021`/`9022` |
| Member avatars, display names, bios | `kind:0` profile metadata, per-pubkey |
| Member status line (italic in sidebar) | Derive from most recent `kind:0` about or a dedicated short-status event (decision D-04 below) |
| Pinned artifact | `kind:999` — `h` tag = NIP-29 group id, `e` tag = the pinned `kind:11` share thread. Most recent `kind:999` event for the room is the pinned artifact. Only admins can write. See D-01. |
| Artifact title, author, source | Tags on the `kind:11` share thread (`title`, `source`, `a`/`e`/`i`+`k`) |
| Highlights on the pinned artifact | `kind:9802` events referencing the artifact's `a`/`e`/`i` tag, filtered by membership (authored by room members) |
| "Marked by" attribution on a highlight | `.pubkey` of the `kind:9802` event → member dot |
| Discussions on a highlight | `kind:1111` events (`e` tag → highlight event id, uppercase `E`/`K` tags → root scope) |
| Notes on an artifact | A `kind:11` thread with `type: note` tag — NOT a new kind, keeps no-custom-kinds principle. Body lives in the thread's content + subsequent `kind:1111` comments. Fully implemented in v1 — members can write new notes. See D-05. |
| Activity feed | NDK subscription (`ndk.$subscribe`) streaming room events within a 48-hour window — `kind:9802`, `kind:1111`, `kind:11` shares, `kind:7` reactions, vote events. Events arrive in real time as they occur; no polling, no refresh button. See D-06. |
| Up-next votes | `kind:999` events — `h` tag = room id, `a` tag = candidate artifact id, `vote` tag = upvote. Multiple votes by the same member on the same artifact are replaced (idempotent). See D-16. |
| Everyone's listen progress (podcast) | Decision D-07: for v1 this reads from a local-per-device cumulative "how far I got" record + publishes a low-frequency `kind:30078` app-specific store ("parameterised replaceable") with `d` tag = artifact id and tag `position: <seconds>`. Other room members read each other's `kind:30078` for that artifact to render progress bars. **This is past-state publish, not real-time broadcast.** |
| Per-member contribution counts (Members tab) | Aggregate on client from the events the client already holds |

**Not used, not shown**:

- No `kind:30078` "currently playing" or "currently reading" presence events
- No ephemeral `kind:20000+` events for presence
- No typing indicators

### 8.1 Load sequence on the room page

1. SSR loader fetches `kind:39000` + a public-safe count summary. Renders shell with title + member count placeholders.
2. Client-side hydration:
   a. NDK connects, NIP-42 authenticates.
   b. Subscribe to `kind:39002` for the room → populate member dots & sidebar.
   c. Subscribe to `kind:0` for all member pubkeys → hydrate avatars, names, status lines.
   d. Fetch the pinned artifact's `kind:11` thread + its `kind:9802` highlights + first-level `kind:1111` replies.
   e. Fetch "also this week" artifacts: the room's 2–3 most recent `kind:11` threads not flagged as pinned.
   f. Lazy-load the remaining shelf, full highlights list, full discussions list as the user scrolls or clicks filters.
3. No subscription listens for presence / ephemeral events.

Pagination and caching are per the NDK patterns already established in the app (see `web/src/lib/ndk/client.ts`). No new caching strategy introduced here.

---

## 9. State management

Svelte 5 runes-based state, per existing conventions.

| Store / reactive source | What it holds | Where |
|---|---|---|
| `roomStore(roomId)` | Room metadata, members, current user's role | `web/src/lib/ndk/rooms.ts` |
| `artifactStore(roomId, artifactId)` | A single artifact's metadata, highlights, threads, notes | `web/src/lib/ndk/artifacts.ts` |
| `pinnedTab` | `'discussions' \| 'highlights' \| 'notes' \| 'members'` — component-local | `PinnedArtifact.svelte` |
| `filterState` | Per-section filter + sort selection | Component-local to each `Block` |
| `roomNavActive` | Current section from scroll-spy | `RoomNav.svelte` |

**No store** carries "who's currently online" or "who's currently listening". If such a store is ever introduced, this plan is violated (see §2.5).

---

## 10. Route and file layout

```
web/src/routes/
├── room/
│   ├── [roomId]/
│   │   ├── +page.svelte            ← room landing
│   │   ├── +page.server.ts          ← SSR: public room metadata
│   │   └── artifact/
│   │       └── [artifactId]/
│   │           ├── +page.svelte    ← dispatches to Article or Podcast view based on media type
│   │           ├── +page.server.ts ← SSR artifact public metadata
│   │           └── _views/
│   │               ├── ArticleView.svelte
│   │               └── PodcastView.svelte
│   └── +layout.svelte              ← shared top nav across all room views

web/src/lib/features/room/
├── components/                      ← every component in §7
├── loaders/                          ← NDK-backed data orchestration
│   ├── roomLoader.ts
│   ├── artifactLoader.ts
│   ├── highlightsLoader.ts
│   ├── threadsLoader.ts
│   └── shelfLoader.ts
├── styles/
│   ├── tokens.css                    ← CSS custom properties (palette, typography, spacing)
│   └── components.css                ← shared component styles pulled from the mocks
└── index.ts                          ← public API exports

web/src/lib/ndk/
├── rooms.ts                          ← group ops + subscriptions
├── artifacts.ts                      ← kind:11 share thread helpers
├── highlights.ts                      ← kind:9802 + kind:16 repost helpers
├── threads.ts                         ← kind:1111 helpers + uppercase/lowercase tag builder
└── progress.ts                        ← kind:30078 reading/listening progress helpers
```

---

## 11. Decisions log

Non-obvious choices captured here so future revisitors have the reasoning.

**D-01 · Pinned artifact mechanism**
`kind:999` — a made-up event kind that h-tags the NIP-29 group and e-tags the pinned post. The most recent `kind:999` event for the group is the authoritative pinned artifact. Only admins can write `kind:999` events; the relay enforces this.
*Why*: simplest possible pin — no custom list kind, no tagging convention on the share thread. The newest one wins, which maps naturally to "set the pinned artifact" semantics.

**D-02 · Four tabs on the pinned artifact — Discussions as default**
The four-tab set (Highlights, Discussions, Notes, Members) with Discussions as the default active tab is a decision, not a guess. Discussions is the liveliest surface for an actively-read piece and answers "what is my room *saying* right now?" immediately. Highlights shows accumulated marginalia; Notes is longer-form reflection; Members is reader-progress state.
*Why*: in testing the room mock, the tab a reader wants first is whatever is most social, which is the thread.

**D-03 · No presence / real-time anxiety indicators**
Explicit, strict, covered in §2.5.
*Why*: multiple user sessions surfaced that "X is doing Y right now" creates a constant-update anxiety that degrades the product's core value prop (slow, deep reading together).

**D-04 · Member status line source**
The italic one-sentence status in the sidebar ("Highlighting at 3am again.") is personal voice, not system-derived. Source: most recent `kind:0` `about` field, OR a dedicated short-status event (e.g., `kind:0`'s `status` extension). Decision for v1: read from `kind:0.about` (one line max, truncate after 80 chars). A dedicated status-event can come later.
*Why*: avoids a new kind; gives members control over their own copy; degrades gracefully if they don't set one.

**D-05 · Notes are fully functional — kind:11 with type:note tag**
Consistent with the "no custom event kinds" rule from `product-surfaces-v3.md`. A note is a thread rooted in the artifact, flagged as `note` rather than `share`. Replies to a note are normal `kind:1111` comments. The "Write a note" button in the Notes tab is live — members can post new notes. Reading and writing are both implemented in v1.
*Why*: a reading group needs the ability to publish reflections; read-only surface is a dead end that would need to be rebuilt.

**D-06 · Activity feed is event-based via NDK subscriptions**
"Lately in the room" is powered by an NDK subscription (`ndk.$subscribe`) that streams events in real time as they arrive. No polling, no refresh button needed — events surface automatically. The 48-hour window is the subscription filter, not a batch-fetch.
*Why*: consistent with §2.5 (no live-updates that draw attention), but the feed is still alive when the user is actively on the page. Events arrive in the background without aggressive notification.

**D-07 · Listening/reading progress is published, not broadcast**
Per-member progress bars (podcast sidebar, members tab) are driven by `kind:30078` events with artifact-id `d` tags + `position` tags, published at low frequency (on pause, on chapter change, on exit — never more than once per 30 seconds). They appear as past state, not "currently listening."
*Why*: a room needs to know who's read what; a room does not need to know who's listening right this second.

**D-08 · Colour-per-member assignment is join-order modulo 6**
The first six members in the room get colours 1–6 in join order. Members 7+ cycle back. Colours are deterministic per member's position in the member list; they do not change when the list re-sorts. Client caches the assignment keyed to the room.
*Why*: members recognise their own colour; stability matters.

**D-09 · "Artifact" is a dev term, not a UX term**
UI copy says "a book", "a podcast", "an essay", "this week's listen", etc. — never "artifact". The word stays in code, prop names, event schemas, and docs.
*Why*: "artifact" is abstract and unhelpful to users.

**D-10 · "Room" replaces "community" as user-facing term**
UI copy, navigation, and URLs use `room`. The word `community` is retired. Protocol-level terminology (NIP-29 "group") stays in code and docs.
*Why*: "room" is concrete, physical, small-feeling. "Community" is generic and has accumulated SaaS baggage.

**D-11 · Cream is an accent, not a default**
Body bg is `#FAFAF7` (barely warm off-white). Warm cream `#F5EFE0` is reserved for specific accent uses: thread panel, highlight call-out block, note cards, margin filter, capture-CTA subtitle area. Anywhere else — white card on near-white page.
*Why*: earlier iterations over-applied cream and read as one continuous sepia wash; restricting cream to "one accent moment per surface" restored visual hierarchy.

**D-12 · Serif is for content only**
Room title, book passages, quoted highlights, note bodies, typographic artwork covers — these are content. Everything else — navigation, section headings, labels, card titles, buttons, table cells — is sans (Inter). Fraunces does not appear on chrome. Ever.
*Why*: earlier iterations leaned literary to the point of degrading scan-ability; sans chrome lets the serif moments actually land.

**D-13 · Tabs switch instantly, not with animation**
The four tabs on the pinned artifact switch with `display:none ↔ display:block`. No fade, slide, or height-animate.
*Why*: tab content is variable-height; animating it stutters. Instant switch reads as responsive.

**D-14 · Scroll-spy on the secondary nav uses a 140px top threshold**
Because the top nav (62px) + room nav (~44px) = ~106px sticky UI, the active-section check reads from `window.scrollY + 140` to give the currently-centred section the spy focus.
*Why*: the reader should see the nav highlight the section that's actually in view, not the one whose top just crossed the nav.

**D-15 · Sidebar sticks at 112px top**
Matches the combined height of top nav + room nav (62 + ~44 + a few px breathing room). Scrolls independently with `max-height: 100vh − 140px`.
*Why*: sidebar should accompany the reader through the page, not scroll off.

**D-16 · Up-next voting uses kind:999**
`kind:999` is the same made-up kind used for pinned artifacts (D-01). For voting, it carries: `h` tag = room id, `a` tag = candidate artifact id, `vote` tag = upvote. All non-pinned `kind:999` events for the room are aggregated by `a` tag to produce the vote tally. Multiple votes by the same pubkey on the same artifact replace each other (last-write-wins, replaceable). The same event kind reused across both use cases keeps the surface small.
*Why*: consistent with D-01; avoids a second custom kind.

---

## 12. Migration and rollout

### 12.1 Migration

1. Ship the new routes under `/room/**` while leaving `/community/**` running.
2. Add a server redirect (301) from `/community/[id]` → `/room/[id]` and `/community/[id]/content/[contentId]` → `/room/[id]/artifact/[contentId]`.
3. Update internal links (email invites, sharing URLs, deep links) to the new paths in the same release.
4. Leave the old routes and redirect in place for one release.
5. In the next release, remove the old route handlers entirely.

### 12.2 Feature flag

Ship behind a user-level feature flag (`FEATURE_ROOM_V2`) for the first release. Enable for internal and beta users first. Remove flag when the `/community/**` routes are deleted.

### 12.3 Beta members

Pre-enroll DK, Pablo, Miljan, Bob S, Steve L, Max W as the first six members of a "Signal vs Noise" seed room on the production relay. Their real highlights, notes, and threads become the reference content for the room's home surface.

---

## 13. Implementation milestones

Each milestone ships behind the feature flag, in this order.

### M1 · Foundation

- Design tokens file (`room/styles/tokens.css`) matching §2
- Base component: `MemberDot`, `MemberStack`, `FilterRow`, `FilterPill`, `Block`
- Typography + palette verified visually against the mocks

**Success**: token-verification page renders all primitives identically to the mock swatches.

### M2 · Room shell

- Route `/room/[roomId]` with SSR metadata load
- `TopNav`, `RoomHeader`, `RoomNav` components
- Scroll-spy wired up, six sections empty stubs

**Success**: the room page renders with the room title, member avatars, and a working secondary nav that highlights on scroll. No content yet.

### M3 · Pinned artifact (Discussions tab only)

- `PinnedArtifact`, `PinTabs`, `ArtifactHeader`, `BookCover`
- `Passage` component with inline member-colour highlights and tooltips
- `Thread` component with alternating root / reply indentation
- Tab strip renders all four labels; only Discussions is implemented

**Success**: a reader can see the pinned book, the featured passage with multi-member highlights, and the ongoing thread.

### M4 · Remaining three tabs

- Highlights tab: `HighlightEntry` list with filter row
- Notes tab: `NoteEntry` list with writer CTA stub
- Members tab: `MembersTable`
- Tab switcher JS, instant switch

**Success**: all four tabs switch, each renders its content for the seed room.

### M5 · This-week, Shelf, Highlights reel, Discussions, Lately

- `ArtifactCard` for this-week
- `ShelfTile` variants for book, podcast, essay, paper, archive
- `HighlightCard` for the reel
- `DiscussionRow` for the list
- `ActivityFeed` for lately
- Filter rows and sort controls on shelf / highlights / discussions

**Success**: the room page is fully populated and navigable. Filters work client-side. See-all links route to archival views (stubbed for v1).

### M6 · Sidebar

- `MembersSidebar` card
- `UpNextVoting` card
- `CaptureCta` card
- Sticky behaviour + independent scroll

**Success**: sidebar behaves correctly across breakpoints.

### M7 · Article view

- Route dispatcher that chooses the view based on artifact media type
- `ArticleView` — hero, body with inline member highlights, margin column with filter + cards
- `annot-inline` cards between paragraphs
- Article footer card

**Success**: opening the Dergigi essay from the "also this week" lane renders the full-viewport reader.

### M8 · Podcast view

- `PodcastView` — hero, player, timeline, sidebar
- `Player` with scrub highlights and tooltips
- `TimelineStamp` card per marked moment
- Chapters list + per-member progress card
- `kind:30078` progress event read/write

**Success**: opening the TFTC episode from the "also this week" lane renders the full-viewport podcast view.

### M9 · Migration and polish

- Redirects from `/community/**`
- Feature flag
- Responsive audit at 375px, 768px, 1060px, 1440px
- Keyboard navigation audit on the tab strip and the secondary nav
- Accessibility pass: ARIA labels on buttons, roles on table, contrast checks

**Success**: full QA sign-off. Ready to enable flag for beta users.

---

## 14. Verification

The rebuild is verified when every one of these passes:

```bash
cd web
npm run check    # zero TypeScript / Svelte type errors
npm run build    # clean production build, no warnings
npm run preview  # production build serves locally
```

Plus a manual walkthrough:

1. Land on `/room/signal-vs-noise` — page renders with title "Signal vs Noise" and six member avatars. No kicker line, no tagline, no meta row, no live indicators.
2. The secondary nav highlights "Pinned" by default and updates as I scroll through Shelf, Highlights, Discussions, Lately.
3. On the pinned artifact, I can switch between Discussions → Highlights → Notes → Members instantly. Each tab has real content rendered from the seed room.
4. Click the Lyn Alden podcast card in "Also this week" → the podcast view opens with the player scrubber showing six member-colour highlight spans, the timeline of six marked moments rendered chronologically, and the chapters list + everyone's-listen sidebar.
5. Back to the room, click the Dergigi essay card → the article view opens with six-member inline highlights, a drop cap on the first paragraph, annotation cards between paragraphs, and a filterable margin column.
6. At no point does any UI claim "X is doing Y right now" or show a pulsing live indicator.
7. At no point does the word "artifact" or "community" appear in user-facing copy.
8. At 375px viewport width, the room page stacks, the sidebar flows under the main column, and the members table collapses. All content remains readable.
9. Lighthouse accessibility ≥ 95 on all three surfaces.

When all nine pass on the beta flag, the `/community/**` routes are removed and the flag is retired.

---

*This plan is versioned; update it when the mocks change or when an implementation decision diverges from what's written here. Divergence without plan update = technical debt.*
