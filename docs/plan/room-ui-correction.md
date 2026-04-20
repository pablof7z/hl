# Room UI — Correction Plan

**Version 1.0 · 2026-04-20**
**Status: active · supersedes the "done" status of M1–M9 in `room-ui.md`**

---

## 0. Why this document exists

`docs/plan/room-ui.md` lists milestones M1–M9 as shipped. Side-by-side comparison of the live page at `/room/[slug]` against the mock at `docs/landing-proposals/room-signal-vs-noise.html` shows the room landing page does **not** implement the layout the plan specifies.

What shipped: a pile of components in `web/src/lib/features/room/components/` plus a `/room/[slug]/+page.svelte` that assembles only a small subset of them in a structure that the mock does not contain.

This document inventories the gaps precisely and sequences the corrective work. It does not re-open design decisions — the mock wins.

---

## 1. Audit — what's there vs what's required

### 1.1 Page-level (`web/src/routes/room/[slug]/+page.svelte`)

| Element from mock | Status |
|---|---|
| `TopNav` (sticky 62px with logo / links / search / Share-to-a-room / avatar) | **Missing** |
| `RoomHeader` band — room title only + 6-overlap avatar row, 56px top / 36px bottom padding, bottom border | **Missing** |
| `RoomNav` — sticky secondary nav under top nav, 6 anchored links with counts and scroll-spy | **Missing** |
| `.room-main` two-column grid `minmax(0, 1fr) 380px` with 44px gap | **Wrong** — current layout puts sidebar **left** at `var(--grid-sidebar) 1fr`, sidebar orders first on mobile |
| `#pinned` anchored block with header + big white card | **Partially** — a `PinnedArtifact` component exists but its shape is wrong (see 1.2) |
| `#this-week` block with 2-col `.also-grid` and 2 `.also-card`s | **Missing** (replaced by an unplanned "In This Room" list) |
| `#shelf` block with filter row + `.shelf-grid` auto-fill minmax(180px) | **Missing** as a room-page block (tiles exist as `ShelfTile.svelte`, unused) |
| `#highlights` block with filter row + `.hl-reel` auto-fill minmax(320px) | **Missing** as a room-page block (`HighlightCard.svelte` exists, unused on this page) |
| `#discussions` block with filter row + `.disc-list` of discussion rows | **Missing** as a room-page block (`DiscussionRow.svelte` exists, only used inside the pinned-artifact Discussions tab which is wrong) |
| `#lately` block with activity feed card | **Missing** as a room-page block (`ActivityFeed.svelte` exists, unused) |
| Right-column sidebar with sticky `top:112px`, Members card, Up-next voting card, Capture CTA | **Wrong side & wrong shape** — on desktop it sits at `top:24px` on the left; `MembersSidebar` does not render the mock's mem-rows with italic status quotes |
| `Footer` (border-top, brand "Highlighter." + room meta line) | **Missing** |
| Scroll-spy JS updating `.roomnav a.active` based on section offset | **Missing** |
| A route-level `+layout.svelte` that owns the `TopNav` so artifact views share it | **Missing** (`web/src/routes/room/[slug]/+layout.svelte` does not exist) |

### 1.2 `PinnedArtifact.svelte`

The mock's pinned card is a *single* white, shadowed card containing three vertically stacked sections:

1. `.pinned-top` — 3-col grid `140px 1fr auto`
   - Typographic book cover (`.book-cover-lg`, Fraunces, dark gradient, red bookmark)
   - Meta column: `<h3>` title + italic subtitle + `.pin-stats` row (4 stat pills "4 of 6 reading" etc.) + `.pin-readers` row (4 avatars + `Bob & Steve haven't started` note)
   - Actions column: `Open artifact` (outline) + `Continue reading` (dark ink filled)
2. `.pin-tabs` — 4 tabs: Highlights (count) · **Discussions** (count, default active) · Notes (count) · Members (count)
3. `.pin-panel.active` — content of the currently selected tab, rendered inline

The current `PinnedArtifact.svelte` renders a flex row with cover + title + author + one italic quote in a yellow box + a `Discussions 12` label. It contains none of the header structure, and the tabs are implemented in the parent page **outside** of this card. Needs a ground-up rewrite.

### 1.3 The four pinned-artifact tab panels

| Tab | Current | Required |
|---|---|---|
| Discussions | `DiscussionsTab.svelte` renders filter pills + a list of `DiscussionRow`s (this is the mock's `#discussions` room-level block, not the mock's *pinned tab* content) | `.passage-wrap` with a "Most-discussed passage" label + colored-highlight Fraunces passage (`Passage` component) + `.thread` (warm-cream panel with alternating root/reply messages + reply box) |
| Highlights | `HighlightsTab.svelte` renders `HighlightEntry` list (good match for mock) | Keep — add panel-head with per-member filter pills matching mock's §4.2, fix padding (18px 32px head, 0 32px list), add "See all N highlights" dashed link |
| Notes | `NotesTab.svelte` renders `NoteEntry` list | Keep — add panel-head with count + `✎ Write a note` button per §4.3 |
| Members | `MembersTable.svelte` renders 4-col table | Keep — verify column widths `1.5fr 2fr 2fr 90px` and mobile stacking |

### 1.4 Components inventory

Reusable as-is or with minor tweaks:
- `MemberDot.svelte`, `MemberStack.svelte`, `FilterPill.svelte`, `FilterRow.svelte`, `TabStrip.svelte` (semantics good; used in correct place once pinned card is rewritten)
- `HighlightEntry.svelte`, `NoteEntry.svelte`, `DiscussionRow.svelte`, `MembersTable.svelte`
- `HighlightCard.svelte`, `ShelfTile.svelte`, `ActivityFeed.svelte`
- `UpNextVoting.svelte`, `CaptureCta.svelte`
- `ArticleView.svelte`, `PodcastView.svelte` (separately audited against `artifact-article.html` / `artifact-podcast.html` in M8 below)

Need new:
- `TopNav.svelte`
- `RoomHeader.svelte` (title + avatar row only — no tagline, no meta, no pulse)
- `RoomNav.svelte` (sticky, scroll-spy, 6 anchors with counts)
- `PinnedArtifact.svelte` (full rewrite — see §1.2)
- `PinnedHeader.svelte` (the `.pinned-top` 3-col row)
- `BookCoverLg.svelte` (typographic 140px cover — Fraunces, dark gradient, red bookmark)
- `Passage.svelte` (Fraunces 22px with inline `.hl.hl-{member}` spans, `data-by` tooltip via `::after`)
- `Thread.svelte` (warm-cream panel, alternating root/reply messages, reply box)
- `SeeAllLink.svelte` (dashed terracotta block link)
- `AlsoCard.svelte` (this-week card with highlight excerpt + reactions)
- `Footer.svelte`

Need rewrite (or large shape change):
- `MembersSidebar.svelte` → `MembersSidebarCard.svelte` — `.sb-card` with `.sb-head` + 6 `.mem-row`s (dot + name + handle + italic Fraunces status quote). The status quote comes from `kind:0.about` per D-04; for now accept static prop.
- `DiscussionsTab.svelte` → rewrite as a *passage + thread* surface (its current body belongs on the standalone `#discussions` block)

Extraneous (remove or leave unused until needed):
- `ArtifactCard.svelte` (was the "In This Room" list — not in mock)
- The `handleArtifactClick` flow in `+page.svelte` that swaps the whole page to an ArticleView / PodcastView — mock links to `/room/[slug]/artifact/[artifactId]` instead; existing route dispatcher under `artifact/[artifactId]` already handles that, this in-page swap should be removed.

### 1.5 Routing

The mock links to `artifact-article.html` / `artifact-podcast.html` from tiles in `#this-week`, `#shelf`, `#highlights`, and `#discussions`. The app should link those tiles to `/room/[slug]/artifact/[artifactId]`. The in-page `activeView = 'article' | 'podcast'` state swap in `+page.svelte` needs to go — it hides the URL and is not what the plan or mock specifies.

---

## 2. Milestones for the correction

Each milestone must end with a visual diff against the mock before the next begins. No "milestone done" claim is made unless the mock and live view are visually aligned at the relevant breakpoints (375, 760, 1060, 1440).

### C1 · Shared shell (TopNav, RoomHeader, RoomNav, Footer, +layout.svelte)

**Files:**
- `web/src/routes/room/[slug]/+layout.svelte` — new, hosts `TopNav` and `Footer`
- `web/src/lib/features/room/components/TopNav.svelte` — new
- `web/src/lib/features/room/components/RoomHeader.svelte` — new
- `web/src/lib/features/room/components/RoomNav.svelte` — new, includes scroll-spy effect (`$effect` + passive scroll listener, 140px threshold per §3.5/D-14)
- `web/src/lib/features/room/components/Footer.svelte` — new
- `web/src/routes/room/[slug]/+page.svelte` — remove the `view-container` full-page swap; wire to layout

**Success:** Page background is `--bg`. Sticky top nav renders at 62px. Under it, a non-sticky header band shows the room title and the 6-avatar row. Under that, a sticky secondary nav with 6 anchor links and counts. Clicking a link scrolls smoothly to the anchor; scroll-spy highlights the correct link as the page scrolls.

### C2 · Main grid and block shell

**Files:**
- `web/src/routes/room/[slug]/+page.svelte` — rebuild body as 2-col `minmax(0,1fr) 380px` with `<main>` on the left containing 6 `<section class="block" id="...">` elements and `<aside class="sidebar">` on the right
- `web/src/lib/features/room/components/Block.svelte` — verify it matches the `.block` + `.block-head` pattern (h2 with `<em>` accent, 44px bottom margin, 120px scroll-margin-top)
- `web/src/lib/features/room/components/SeeAllLink.svelte` — new

**Success:** Page structure renders with empty blocks in order Pinned → This week → Shelf → Highlights → Discussions → Lately. Sidebar is on the right on desktop ≥1060px, flows below on narrower viewports. Each block has the mock's heading, block-head bottom border, and a placeholder.

### C3 · Pinned artifact (card shape + default Discussions tab)

**Files:**
- `web/src/lib/features/room/components/PinnedArtifact.svelte` — **full rewrite** to match §1.2
- `web/src/lib/features/room/components/PinnedHeader.svelte` — new (`.pinned-top` 3-col grid)
- `web/src/lib/features/room/components/BookCoverLg.svelte` — new (140px Fraunces typographic cover, dark gradient variants: dark/red/blue/green/plum)
- `web/src/lib/features/room/components/Passage.svelte` — new (Fraunces 22px with `.hl.hl-{1..6}` inline spans + tooltip)
- `web/src/lib/features/room/components/Thread.svelte` — new (warm-cream panel, messages grid with root/reply variants, reply box)
- `web/src/lib/features/room/components/DiscussionsTab.svelte` — **rewrite** to compose `Passage` + `Thread`

**Success:** Pinned card shows the full mock structure: cover + stats/readers/actions header, 4-tab strip, and the passage + thread panel under the Discussions tab. Tabs switch instantly (no animation) per D-13.

### C4 · Remaining three pinned tabs (panel chrome + see-all)

**Files:**
- `web/src/lib/features/room/components/HighlightsTab.svelte` — add panel-head (filter pills per member + "By position ↓" sort), fix paddings per §4.2, add `SeeAllLink` at bottom
- `web/src/lib/features/room/components/NotesTab.svelte` — add panel-head (count line + `✎ Write a note` button) per §4.3
- `web/src/lib/features/room/components/MembersTable.svelte` — verify 4-col grid `1.5fr 2fr 2fr 90px`, dotted row borders, mobile stack

**Success:** All four tabs switch inside the pinned card. Each tab panel matches the mock for chrome, padding, and empty/footer states.

### C5 · `#this-week` + `#shelf`

**Files:**
- `web/src/lib/features/room/components/AlsoCard.svelte` — new (kicker + artwork + title/source + highlighted excerpt + reactions + foot)
- `web/src/lib/features/room/components/ShelfTile.svelte` — verify cover variants (book dark/red/blue/green/plum, podcast purple/orange, essay warm, paper lined, archive) and status badges (open / week / re-read)
- `+page.svelte` — populate `#this-week` and `#shelf` blocks

**Success:** This-week block renders 2 cards in 2-col (1-col <760px). Shelf renders filter row (Everything / Books / Podcasts / Essays / Papers / Archive + sort) + auto-fill 180px grid + see-all link.

### C6 · `#highlights` + `#discussions` + `#lately`

**Files:**
- `web/src/lib/features/room/components/HighlightCard.svelte` — verify amber gradient underline on quote, terracotta smart quotes, foot row layout
- `web/src/lib/features/room/components/DiscussionRow.svelte` — verify 3-col grid `auto 1fr auto`, status pill, hot-reply dot
- `web/src/lib/features/room/components/ActivityFeed.svelte` — verify row layout `28px 1fr auto`, action verb in mono terracotta
- `+page.svelte` — populate these three blocks with filter rows and see-all links

**Success:** Highlights reel is auto-fill 320px grid with member filter pills. Discussions list shows status pills with hot dots for active threads. Lately feed renders activity rows with mono action verbs and right-aligned timestamps.

### C7 · Sidebar (correct shape + correct side)

**Files:**
- `web/src/lib/features/room/components/MembersSidebarCard.svelte` — rewrite the current `MembersSidebar.svelte`. `.sb-card` with `.sb-head` (count + `invite another →`) and 6 `.mem-row`s (32px dot + name w/ mono handle + italic Fraunces status quote from `kind:0.about`)
- `web/src/lib/features/room/components/UpNextVoting.svelte` — verify vote-row layout (position mono | title/source | honey-amber dots + num), `vote-close` footer with `cast yours →`
- `web/src/lib/features/room/components/CaptureCta.svelte` — verify dark ink block, terracotta hover, icon + heading + subtitle
- `+page.svelte` — place the aside on the right, sticky at `top:112px`, max-height `calc(100vh - 140px)` with own scroll per D-15

**Success:** Sidebar matches the mock at desktop widths: right column, sticky, independent scroll, three stacked cards with correct internal structure. Mobile flows under main.

### C8 · Artifact view routing + teardown

**Files:**
- `web/src/routes/room/[slug]/+page.svelte` — remove `activeView` state swap and in-page ArticleView/PodcastView render. Tiles link via `<a href="/room/{slug}/artifact/{artifactId}">`
- `web/src/routes/room/[slug]/artifact/[artifactId]/+page.svelte` — verify route dispatcher chooses `ArticleView` / `PodcastView` based on media type (existing — validate only)
- Audit `ArticleView.svelte` against `docs/landing-proposals/artifact-article.html` and `PodcastView.svelte` against `artifact-podcast.html`. Any drift files a follow-up (this plan scopes the drift as "verify; if divergent, escalate to a separate correction document")

**Success:** Clicking a tile navigates to `/room/[slug]/artifact/[artifactId]` with URL change. Back button returns. Article and Podcast views render with their own TopNav (via shared layout).

### C9 · Responsive, a11y, cleanup

- Visual audit at 375px, 760px, 1060px, 1440px. Document any deltas vs mock breakpoints.
- Keyboard nav through TopNav → RoomNav → Pinned tabs → Filter pills → See-all links.
- Delete: unused `ArtifactCard.svelte` if nothing references it; `view-container` CSS; any other code orphaned by the correction.
- Remove seed data fallbacks that were standing in for unused paths (trace what's actually read by `data.room`).

**Success:** `npm run check` clean. Lighthouse accessibility ≥ 95 on the room page. Live view matches the mock at all four breakpoints.

---

## 3. Verification (same bar as the original plan §14, with the honest test)

The rebuild is verified when a fresh observer, comparing `docs/landing-proposals/room-signal-vs-noise.html` and `/room/signal-over-noise` side-by-side, cannot point to a structural difference beyond content.

Structural means: top nav present and shaped correctly, header band with title+avatars only, sticky secondary nav with scroll-spy, two-column grid with sidebar right, six anchored blocks in order, pinned card with integrated 4 tabs and the passage-in-thread Discussions panel, highlights reel / shelf grid / discussion list / activity feed blocks present with filter rows and see-all links, sidebar with three stacked cards sticky at 112px, footer at the bottom.

Content differences are expected (real data vs seed copy). Structural differences are bugs.

---

## 4. Decisions log (addenda, not replacements)

**CD-01 · The previous M1–M9 milestones are not "done"**
They built components but not the page. This correction plan replaces their "done" status with a fresh C1–C9 sequence. The original `room-ui.md` stays authoritative for *what* to build (design language, event mapping, decisions D-01…D-16). This document governs the *order and completeness* of the page assembly.

**CD-02 · Verification is visual, against the mock, before marking a milestone complete**
Commit log was previously trusted. That failed. Going forward, no milestone is marked complete without (a) the dev server running, (b) the mock open in another tab, (c) at least one agreed breakpoint compared, (d) a short note recording the comparison in the milestone's PR description.

**CD-03 · No new "In This Room" list, no in-page view swap**
The `ArtifactCard`-based "In This Room" list and the `activeView` page-swap flow were both invented during implementation. Neither is in the mock; both are removed in C8. The mock uses tiles within `#this-week` / `#shelf` / `#highlights` / `#discussions` that link via real URLs.

---

*When the live view matches the mock at all four breakpoints, this document is closed and the original `room-ui.md` can reclaim the status of single source of truth.*
