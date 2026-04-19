# Highlighter — History & Vision

*A report on where Highlighter came from and what David King's trajectory — plus the Nostr continuation — reveal about where this rebuild is going.*

**Version 2.0 | April 2026**

*Version 1.0 of this document was written without external research and has been superseded. This version is grounded in primary and secondary sources about David King, Highlighter Inc., and the Nostr incarnation of the product; the sources are listed at the end.*

---

## 0. What this document is — and isn't

This is not a product spec; the specs in this directory already define the *what*. This is an attempt to read Highlighter's actual history — the 2018 company, the 2024 shutdown, the 2023 Nostr reboot, and the 2026 revamp in this repo — and draw out the vision, aesthetic, and product-care that persist across those phases.

**What I could verify from primary sources:** David King's public biography, Highlighter, Inc.'s company history, the shutdown announcement, the town-hall series (with documented guests), the 2023 Nostr client built by Pablo Fernandez, and the implemented design system in this repo.

**What I had to rely on secondary summaries for:** The full text of David King's March 2024 shutdown letter (posted on X; the platform's auth wall prevented direct retrieval). Multiple independent summaries of that letter converge on the same facts, and those are used here, but no line is quoted verbatim from the letter without that caveat.

**What is labeled as interpretation:** Section 7 ("Extrapolating vision") is explicitly synthesis across the sources, not a restatement of David's or Pablo's own words.

---

## 1. David King — who he actually is

David King (X: [@dksf](https://x.com/dksf), blog: [curiousdk.com](https://www.curiousdk.com/about)) is a San Francisco-based founder, angel investor, and community builder with, by his own account on his about page, "over 2 decades of experience in Silicon Valley building relationships, technologies, and products to improve peoples' lives."

**Relevant prior career:**

- Joined **Google** early; led a team working on "machine learning for advertising products."
- Created a social gaming network that reached 45 million players, **sold to The Walt Disney Company in 2010** (this was the Green Patch acquisition, per public Disney reporting).
- **Angel investor for more than a decade**: first-round investor in companies including Clubhouse, Quora, and Opendoor. Runs a podcast/interview series called *Founders You Should Know*.
- Self-description: "I love to organize communities, interview people, and write what I'm learning as I follow new interests" — specifically naming "nostr, bitcoin, and startups I think are interesting" as current focus areas.

Two threads from this biography matter for Highlighter:

1. **Organizing communities and interviewing people** is a stated core interest, independent of the product he chose to build. It's what *he does* when he's not building — and, as we'll see, it's what the original Highlighter's best-working feature turned out to be.
2. **Angel-investor pattern recognition** — sitting in rounds with Clubhouse and others — gave him access to direct advice from operators like Paul Davison and Naval Ravikant, which (by his own later account) he didn't fully follow.

---

## 2. Highlighter, Inc. (2018–2024) — what actually happened

This is the factual timeline of the original company, pieced together from David's shutdown letter (as summarized by multiple sources), Crunchbase/Tracxn profile data, and archived third-party posts.

| Date | Event |
|---|---|
| **March 2018** | Founded as **Curious Labs** by David King and Josh Mullineaux. Initial mandate: "explore new consumer social software concepts." |
| **2019** | Narrowed focus to "building tools and communities to help people gather around the most interesting ideas from books — the highlights." Company rebranded as Highlighter. |
| **2020** | Virtual **town halls** with guest authors become a recurring community event. Documented participants: Nadia Eghbal (*Working in Public*, August 11, 2020) and Jason Crawford (*Roots of Progress*, September 25, 2020). |
| **2020–2022** | Expanded highlighting beyond books: web articles, podcasts, video. Built a Chrome extension. None of the new surfaces achieved substantial traction. |
| **Late 2022** | Ran out of initial external capital. David King contributed **$100k+ of personal capital** to fund further experiments, **laid off half the small team**. |
| **March 2024** | After six years, David King publicly announced that **Highlighter, Inc. was shutting down**. The note was shared first with investors, friends, and supporters, then publicly on X as a long thread (*We're shutting down Highlighter, Inc.* — status 1764700524820259108, dated March 4, 2024). |
| **2024–present** | The domain `highlighter.com` continues to host the legacy client (e.g., David's own highlights page at `highlighter.com/@dk`), but the company is dissolved. |

### 2.1 The retrospective — David's own diagnosis

The shutdown letter (per multiple independent summaries — see sources) contains specific, falsifiable self-criticism. The key points the summaries converge on:

- The original approach was **too broad** — a general-purpose highlighting platform for "anyone reading anything."
- The format couldn't generate enough specific, time-bound reason for people to show up on the same day for the same thing.
- **His retrospective prescription**: if he were doing it again, he'd "pick a single book to focus on each week or two and create a community around the specific content they were reading, instead of starting as a broad platform for anyone reading anything."
- **The general principle he drew out**: rather than building a network, start by building "the most important channel of the potential future network."
- **The advice he didn't fully heed**: this "narrow first" prescription had been given early by investors and friends, explicitly naming **Paul Davison** (Clubhouse) and **Naval Ravikant**.
- **His forward-looking claim**: highlighting as a concept will become *more* important, not less, in the LLM era — because the quantity of public information will explode while average quality will fall, and trusted excerpting becomes a scarce service.

### 2.2 What actually worked

Read against the retrospective, the one consistent bright spot in the public record of old Highlighter is the **town hall format**:

- An author with an engaged readership (Nadia Eghbal's *Working in Public*, Jason Crawford's *Roots of Progress* writing) would appear for a scheduled session.
- The readership showed up because the session was *about a specific thing*, at a specific time, with a specific person.
- The product's social layer (highlights, comments, discussion) suddenly had a reason to cluster around one artifact — and did.

This is the shape of the "narrow channel" David later said they should have started from. The town hall wasn't the whole company; it was the part that worked because it accidentally satisfied the "single book, single community" prescription the rest of the product failed to impose.

The specs in this repo (`product-spec-v1.2.md` and `v2.0.md`) name this lesson as the founding premise of the 2026 revamp. What v1.2 calls "the magic commenters loved in David King's original town-halls" is a direct reference to this documented feature — and the only feature of the old company that anything in this rebuild is trying to preserve.

---

## 3. The Nostr continuation (2023) — Pablo's first rebuild

The story doesn't go directly from "2024 shutdown" to "2026 revamp." There's a middle chapter that matters.

**Pablo Fernandez** (Nostr: [pablof7z](https://github.com/pablof7z)) — who owns this repo and is the current author of the 2026 rebuild — had already begun building a **Nostr-native Highlighter** well before the original company was shut down.

| Date | Event |
|---|---|
| **~Early 2023** | Pablo begins building a Nostr client that surfaces reading, notes, and highlights. Repo: `github.com/pablof7z/highlighter`. |
| **April 25, 2023** | First notable third-party writeup by "Tony" on [Habla](https://habla.news/tony/highlighter): *Highlighter — Share wisdom and stack sats*. Pablo's framing quoted in that post: the tool is "a way of keeping the words you find valuable running on Nostr." |
| **April 28, 2023** | **David King himself blogs about Pablo's Nostr Highlighter** on his personal site (`curiousdk.com/p/nostr-based-highlighter-by-pablof7z`). The original founder publicly acknowledges a Nostr reimplementation of his product concept, built by someone else, while his own company is still alive. |
| **~2023** | **NIP-84** — the highlight event kind (`kind:9802`) — is formalized in the Nostr spec. The kind used for every highlight in the 2026 rebuild comes from this lineage. |
| **October 26, 2023** | Pablo announces **Highlighter 2.0, release name "Sig"** on Nostr. The announcement describes it as "essentially an entirely new nostr client." Features include data-vending-machine extraction of text from podcasts/video, zap-splits for highlight value attribution, article curations, and subscriptions (NIP-88). |

So by the time David King's company was formally dissolved in March 2024, a Nostr-native incarnation of the product concept was already live, open-source, and feature-rich — built on protocols (NIP-84 for highlights, NIP-29 for groups, NIP-88 for subscriptions) that guaranteed nothing was locked in. The 2023 README of the Nostr Highlighter repo states the principle explicitly: *"Nothing built on highlighter should be custom or 'lock' users into highlighter."*

This is the direct line from the dissolved company to the code in this repo:

```
Curious Labs (2018)  →  Highlighter Inc. (2019–2024, books → web/podcast/video)
                                ↓
                        (company shuts down, March 2024)
                                ↓
Pablo's Nostr Highlighter (2023, kind:9802 + NIP-29 + NIP-88) — already running
                                ↓
2026 revamp (this repo) — community-first, NIP-29 groups, SvelteKit + NDK + Croissant fork
```

The 2026 revamp isn't a greenfield tribute to a shuttered product. It's the *third* iteration of the same idea by someone who has been carrying it forward continuously for three years, now reshaped around the narrow/deep lesson David explicitly named in his own postmortem.

---

## 4. The spec lineage in this repo — how the vision sharpened

Inside the 2026 work itself, the docs show four eras, each superseding the last cleanly. This sequence is the vision becoming concrete.

| Doc | Status | What it locks in |
|---|---|---|
| `product-spec-v1.2.md` (Grok draft, April 2026) | **Superseded** | The pivot itself: community-first, invite-only, cross-community discussion union, read-only public for discovery. Explicitly "fully incorporating proposed Community-First model + David King's 2024 shutdown lessons." |
| `product-spec-v2.0.md` | **Active** | Nostr-native as foundation (not Phase 2). NIP-29 groups. Four group types from a 2×2 (access × visibility). Growth as a design principle, not a section. |
| `product-surfaces-v3.md` | **Active** | The stack decisions: custom fork of croissant (not relay29), SvelteKit + NDK + Vercel for web, Rust core + native UIs for mobile/desktop. **No custom event kinds** — NIP-84 for highlights, NIP-73 for artifact identity, standard `kind:11` for share threads. |
| `client-spec-v1.0.md` | **Active, locked** | "Quiet confidence." 3-tab nav (Communities / Discover / Me). For Later as a bridge. Photo-OCR capture as a first-class mobile feature. Design system **locked for v1, no changes without explicit approval.** |

A few moments are worth reading closely against the historical context from §2 and §3.

**v1.2 → v2.0 isn't a redesign, it's a commitment.** v1.2 still hedged on Nostr. v2.0 puts it on page one: *user control is non-negotiable, the identity is a Nostr keypair, the group is a relay-side NIP-29 artifact, and portability is the foundation.* Given that Pablo has been building on Nostr since 2023 and that the old Highlighter died in part because it was a siloed SaaS, that re-prioritization reads as lessons from both the shutdown *and* the three-year Nostr track record.

**v3's "no custom event kinds" decision is a tell.** Highlighter has every excuse to mint a private kind. It doesn't. Artifacts are identified by tags on a plain `kind:11`; highlights use NIP-84 `9802`; comments use NIP-22 `1111`. The stated rationale is interop; the deeper signal is a refusal to privatize domain logic into a silo — which was a structural flaw of the 2018–2024 product.

**The client spec is locked.** "Locked for v1. No changes without explicit approval." Palette, typography, card shape, motion — all frozen. That's not design rigidity; it's a deliberate decision, after a previous version of the product spread itself across too many surfaces, to hold one aesthetic firm.

---

## 5. The build itself — what the commits say

72 commits take the 2026 project from the first scaffold to the current main. The arc has a clear shape.

### 5.1 Phases

- **Foundation (early commits):** scaffold, delete prototypes, install DaisyUI + Tailwind v4, create route stubs. `Delete all prototype/wireframe HTML pages` marks the end of the exploratory phase.
- **Feature breadth (middle):** community browsing, highlights loading, artifact pages, discussions, podcast artifacts, search. Most commits are additive.
- **Consolidation (recent ~15 commits):** `Remove old prototypes, wireframes, and landing page proposals` · `Unify article rendering into a single ArticleView component` · `Refactor Share UI: replace inline ArtifactForm with modal dialog` · `Single-row navbar with DaisyUI, slide-in search animation` · `Refactor login modal to daisyUI components` · `Simplify Save for Later vault feature` (most recent).

The direction is unmistakable: **ship breadth first, then compress**. Almost every recent commit is *simplify*, *unify*, *remove*, or *refactor to X*, not a feature add. That signature — ship, see it, compress — matches a builder who has been around the same idea three times and knows how quickly it wants to sprawl.

### 5.2 Removal as a design act

A striking number of commits are pure subtractions:

- `remove highlight eyebrow label`
- `remove discover summary cards`
- `Remove placeholder eyebrow metadata`
- `remove community fallback` (search)
- `Remove old prototypes, wireframes, and landing page proposals`
- `Delete all prototype/wireframe HTML pages`

Each is a choice to do less. The negative space is being designed as deliberately as the positive.

### 5.3 Changing your mind in public

The Vercel-config commits contradict each other over days:

- `Fix vercel.json: use rootDirectory for proper SvelteKit subdirectory build`
- `Fix Vercel config: remove outputDirectory, let adapter-vercel handle it`
- `Add back outputDirectory to vercel.json`
- `Fix Vercel output: copy build to root .vercel/output`

No shame, no history rewriting. The working style is *try, ship, look, adjust* — not *plan everything up front*.

---

## 6. Aesthetics — "Quiet Confidence"

The design system in `client-spec-v1.0.md` and the actual implementation in `web/src/app.css` line up almost exactly.

### 6.1 The palette (as actually implemented)

From `web/src/app.css`:

| Role | Hex | What it is |
|---|---|---|
| Canvas | `#F8F5F0` | Off-white. Paper, not screen. |
| Surface soft | `#EDE4D8` | Warm beige. |
| Border | `#E2D9CD` | Beige one shade darker. |
| Text | `#2f3437` | Near-black, slightly warm. Not `#000`. |
| Muted | `#787774` | A deliberate warm gray. |
| **Accent** | **`#C47E5E`** | **Soft terracotta — the only chromatic pop.** |

One chromatic note; everything else paper-and-ink. This is a product that has decided, before the first pixel shipped, to be warm rather than cool, analog rather than digital, library rather than app.

### 6.2 The terracotta 2px left border

Every highlight teaser gets a 2-pixel terracotta left border. The plan calls it out as a named requirement: *"All highlight teasers use a 2px left border at `--accent` color with a small left padding — this is a recurring visual motif throughout the product."* The signature gesture of the product is also its most restrained one — a printer's rule, not a quote box.

### 6.3 Typography split

Inter for UI chrome; Source Serif 4 the moment you're *reading*. Sans when the software is speaking, serif when the content is. Most reading apps pick one; Highlighter draws the line exactly at the boundary of attention.

### 6.4 Motion

> "All transitions 200ms ease-out. Card tap: subtle scale 0.98 → 1.0. No bouncy or playful animations — everything is calm and deliberate."

*Deliberate* is doing the work. The product does not want to delight you with animation; it wants to not interrupt you.

### 6.5 Iconography

Lucide/Feather, 1.5px stroke, muted gray, **never emoji**, **never filled** (except the floating-action button — the one place the product is allowed to be bold). The "never emoji" rule in 2026 is notable; nearly every other social product is leaning harder into emoji reactions, sticker packs, and mixed visual vocabularies. This one declines that entire vocabulary on purpose.

### 6.6 The metaphors the docs reach for

Across the spec and proposals, Highlighter gets described as:

- "a beautifully designed private library"
- "a members-only gallery of ideas"
- "a warm, intimate digital salon wall"
- "a modern high-end magazine meets private reading room"
- "the private intellectual club"

Not a feed. Not a timeline. Physical rooms where reading-with-other-people already works, translated into software.

---

## 7. Extrapolating vision (synthesis, clearly marked as such)

Everything up to this point is grounded in primary or secondary sources. This section is explicitly my interpretation — what I think David's trajectory plus Pablo's three-year Nostr track record plus the specs and code *collectively* imply about the direction. Treat it as synthesis, not fact.

### 7.1 The idea that keeps coming back

From 2018 through 2026, across three distinct codebases, two founders, and one company closure, the underlying idea has been remarkably stable:

> **Make the kitchen-table kind of conversation about books, articles, and podcasts a first-class thing the internet can host.**

What has *changed* is the theory of how to get there:

| Era | Theory of the product |
|---|---|
| **Curious Labs → Highlighter (2018–2024)** | Be the platform. Let anyone highlight anything. Grow the network, then the magic emerges. |
| **David's retrospective (March 2024)** | That was backwards. Start with the *channel* — a single book, a single community, a single reason to show up — and let the network come out of a stack of well-chosen channels. |
| **Pablo's Nostr Highlighter (2023)** | Don't own the network at all. Put the highlight on an open protocol; let anyone else build on top. |
| **2026 revamp (this repo)** | Combine both lessons. Invite-only NIP-29 *groups* as the narrow channel. Nostr as the non-captive substrate. Growth as a consequence of product quality, not a department. |

The 2026 product reads as a synthesis of David's shutdown postmortem and Pablo's 2023 protocol-first bet. Neither would be right on its own — a "narrow channel" inside another silo still has a lock-in problem; a portable-highlights protocol without a social container is Readwise on Nostr. The interesting move is doing both at once.

### 7.2 What they won't do

The docs in this repo carry a set of load-bearing refusals:

- **No feed.** The daily experience is your communities; the front page is aha-moment discovery, not a "For You" scroll.
- **No emoji, stickers, or playful motion.** The atmosphere is a library, not a playground.
- **No public-broadcast default.** Public view is explicitly *read-only*. You can lurk, but participation requires a room.
- **No AI substituting for human taste.** AI helps extract, summarize, facilitate — never decides what's good.
- **No lock-in.** Users own their keys. Groups can move. Highlights are portable. The moat is quality, not switching cost.
- **No custom event kinds.** Interop with the broader Nostr ecosystem is treated as a feature, not a constraint.
- **No proof-of-consumption gate.** A member can share an artifact as a proposal, before having read it. The product refuses to police who has "earned" participation.

Each of these is a place a pragmatic PM would say "just ship, we can fix it later." Each has been held — and several of them directly mirror failure modes of the original public-platform era.

### 7.3 The product-care signature

Reading commits, specs, palette, and refusals as a single body of evidence, a distinctive operating signature emerges:

1. **Start from a human image, not a feature list** — eight friends at a kitchen table, a book club that actually works, a town hall with an author who showed up.
2. **Pick a medium that respects the image** — Nostr because identity and portability matter more than platform convenience.
3. **Design a room, not a feed** — warm palette, serif at the reading moment, terracotta margin rule, no emoji, no algorithmic scroll.
4. **Make the signature gesture tiny and unmistakable** — two pixels of terracotta on every highlight.
5. **Ship wide, then compress** — the last 15 commits are mostly *simplify/unify/remove*.
6. **Refuse the shortcuts that make products worse** — no custom kinds, no lock-in, no AI replacing taste, no broadcast default.
7. **Treat the design system and the spec as covenants** — locked v1, superseded-by pointers, decisions logged with rationale.
8. **Let growth be a consequence of care**, not a department.

### 7.4 What David King's vision, extrapolated, looks like

The version the evidence supports — not the version my first draft fabricated — is something like this:

- **Deep reading deserves social infrastructure that respects the reading.** The original company proved there's demand; the shutdown proved the shape has to be *narrow channel first, network later*.
- **Community is the actual product.** The one thing that demonstrably worked in 2018–2024 — the author town halls — was a community format, not a tool. Everything else (the Chrome extension, the podcast clipper, the web highlighter) was infrastructure looking for the community that would have given it meaning.
- **The commons matters more than the moat.** David's own blog about Pablo's 2023 Nostr build, while his own closed-source company was still alive, is the clearest public signal that he'd rather the *idea* survive on open protocols than the *company* survive on lock-in. The 2026 revamp's "no custom event kinds" decision is the same value applied to architecture.
- **Highlighting gets more important, not less, as AI floods the commons.** His forward-looking claim in the shutdown letter. The 2026 product's explicit market-research positioning ("anti-TikTok," "high-signal scrolling," "the scroll that leaves you smarter, not emptier") is the commercial expression of that bet.
- **Restraint is the brand.** The palette, the motion, the typography, the "no emoji" rule, the locked design system. If the content is what you came for, the product's job is to not interrupt.

Put together: **Highlighter 2026 is a long bet that taste, restraint, and user control will compound where virality, engagement, and lock-in have stopped working.** The bet is being placed with deliberate knowledge of why it didn't compound the first time — and with six years of hindsight from the person who lost capital and time proving that.

---

## 8. One-paragraph summary

Highlighter, Inc. was founded by David King and Josh Mullineaux as Curious Labs in March 2018 and narrowed to a book-highlights platform by 2019. It ran virtual town halls with authors (Nadia Eghbal, Jason Crawford, and others) that worked, built Chrome-extension, podcast, and video highlighters that didn't, exhausted its seed capital by late 2022, survived for another year on $100k+ of David's personal money and a half-team layoff, and was formally shut down in March 2024. In his shutdown letter, David's central self-criticism was that the company started too broad: he would have picked a single book every week or two and built a specific community around it — "build the most important channel of the potential future network," advice Paul Davison and Naval Ravikant had given early. Meanwhile, since early 2023, Pablo Fernandez had been building a Nostr-native Highlighter on open protocols (NIP-84 for highlights, NIP-29 for groups), with the explicit principle that nothing should lock users into the product; David publicly blogged about it while his own company was still alive. The 2026 rebuild in this repo is the synthesis: invite-only NIP-29 communities as the narrow channel David said should have come first, on top of the non-captive substrate Pablo has been building for three years, delivered through a deliberately quiet "library not feed" aesthetic that refuses the UI shortcuts the original product never quite refused. Read as a single arc, the vision underneath is the same as it was in 2018 — make small-group, content-centric conversation a first-class thing the internet can host — carried by someone who has now had six years of failure data and three years of protocol-level experience to sharpen it.

---

## Sources

**Primary-source, fetched directly:**

- David King's public about page — [curiousdk.com/about](https://www.curiousdk.com/about) (biography, self-description, stated focus on Nostr/Bitcoin/startups)
- Pablo's Highlighter repo on GitHub — [github.com/pablof7z/highlighter](https://github.com/pablof7z/highlighter) (README, open-source-first philosophy, tech stack)
- Tony's writeup on Habla — [habla.news/tony/highlighter](https://habla.news/tony/highlighter) (April 25, 2023 — Pablo's early framing of the Nostr-native build)
- *Video: Highlighter town hall with David King* — [blog.rootsofprogress.org/video-highlighter-town-hall-with-david-king](https://blog.rootsofprogress.org/video-highlighter-town-hall-with-david-king) (September 2020, documenting the town-hall format with Jason Crawford)

**Primary source referenced via multiple independent summaries (the X platform's auth wall prevented direct retrieval of the full text):**

- David King's shutdown announcement — [x.com/dksf/status/1764700524820259108](https://x.com/dksf/status/1764700524820259108) (March 2024). The company history, capital contribution, layoffs, retrospective self-criticism, and LLM-era forward-looking claim in §2 are all drawn from convergent summaries of this thread rather than verbatim quotation.

**Secondary sources (biographical and company facts):**

- [David King on Crunchbase](https://www.crunchbase.com/person/david-king-5) — career history including Google, Green Patch (Disney 2010), Curious Endeavors, Highlighter
- [Highlighter founders on Tracxn](https://tracxn.com/d/companies/highlighter/__q0_F94RLlOefIs2OvIrRMV1JJIBRPH95OhUG_bELzh4/founders-and-board-of-directors) — confirming David King and Josh Mullineaux as co-founders
- Announcement of Highlighter 2.0 "Sig" (October 26, 2023) — surfaced via [nostrapps.com](https://nostrapps.com/) and Nostr indexers

**Where the evidence is this repo itself:**

- `docs/product-spec-v1.2.md`, `docs/product-spec-v2.0.md`, `docs/product-surfaces-v3.md`, `docs/client-spec-v1.0.md`, `docs/plan.md`, `docs/market-research-2026.md`, `docs/community-page-proposals-v1.4.md`, `docs/AGENTS.md`
- `web/src/app.css` (implemented design tokens)
- Git history: 72 commits from `3cc5c0e Add Grok's product spec v1.2 draft for review` through `c3a3e77 Simplify Save for Later vault feature` (April 2026)

---

*This document was generated using external research (web search + document fetches) combined with reading the repository state as of April 2026. Where primary sources were available, they're cited. Where only summaries were available, that is flagged. Section 7 is explicitly interpretation. A previous (v1.0) draft of this document was written without external research and has been discarded as unreliable.*
