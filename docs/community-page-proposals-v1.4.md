# Highlighter Community Front Page Proposals

**Content-Centric Refined | Version 1.4 (2026)**
**Status: Active design reference**

> **Context**: Communities in Highlighter are NIP-29 relay-based groups. See `product-spec-v2.0.md` for the full product model and `technical-architecture.md` for NIP-29 implementation details. Groups can be open/closed (access) and public/private (visibility) — these proposals cover the **group home page UI** and apply across all group types, with member/non-member views adapting based on the group's visibility setting.

These proposals lean into content-centric, magazine-quality design:

- The **full piece of content** (book cover, podcast artwork, article hero image, video thumbnail + title/author/source) — called an **artifact** — is the undeniable visual hero of every card/section.
- **Highlights** (member-created excerpts) exist as small, tasteful teasers ("what caught our eye") to spark curiosity and pull people into the full-content conversation.
- The ensuing **group discussion** is the living heart of the page.
- Design is intentionally premium, intimate, and "private intellectual club" — spacious, warm, high-signal, never cluttered or feed-like.

All versions mobile-first. For **public groups**: non-members get seductive, rich previews with FOMO + dead-simple "Join" (open groups) or "Request Invite" (closed groups) CTAs. For **private groups**: non-members see only the group name and member count with a join/request CTA. Members always see invite and cross-group sharing loops baked into every surface.

## Proposal A: "The Private Collection" (Most Recommended)

**Vibe**: Members-only library or private art gallery of ideas — calm, spacious, slightly luxurious.

### Member View
- Top hero: Large community name + subtle member avatar ring + elegant "Invite Friends" button (styled like a membership card)
- Main area: Spacious vertical "collection" of content cards with generous white space
  - Each card dominated by full-content visual (large book cover / podcast artwork / article image) + title + source + "Shared by [Member]"
  - Below: 1–2 tasteful highlight teasers (pull-quotes with faint highlight styling) labeled "What caught our eye"
  - Under that: Unioned discussion thread preview (2–3 richest comments) + "Join the full conversation (18 replies)" button
- Floating action: "Share new content to the collection"

### Non-Member View (Public Groups)
- Hero becomes "Preview of this collection • 14 members"
- Cards identical but with soft overlay and "Members only" on discussion area
- Every card ends with "This piece sparked a great discussion — **Join** (open) or **Request Invite** (closed) to join the collection"
- For open groups: one-tap join directly from the preview

### Non-Member View (Private Groups)
- Group name, description, member count only. No content preview.
- Single CTA: "This is a private group — **Request Invite** (closed) or **Join to see** (open + private)"

**Growth power**: Public group cards are extremely shareable. Open + public groups have the lowest-friction conversion. Closed + public groups create the strongest FOMO loop.

## Proposal B: "The Salon Wall"

**Vibe**: Warm, intimate digital salon wall — beautifully lit room where the best content is pinned and conversation flows naturally around each piece.

### Member View
- Hero bar: Community name + short tagline (editable) + "Invite to the Salon" button
- Main area: Elegant masonry-style wall of content "frames" (slightly overlapping, artistic layout)
  - Each frame anchored by dominant full content visual + title
  - Small elegant callout: "Highlights that started the conversation" with 1–2 short teasers
  - Bottom third: unioned discussion — live comment snippets + AI summary pill ("The group is converging on…")
- Quick actions on hover/tap: "Add your thoughts" or "Cross-post this entire discussion to another of my communities"

### Non-Member View (Public Groups)
- Same artistic wall layout with subtle "Salon Preview" watermark
- Teasers and comment snippets fully readable for strong FOMO
- Persistent top banner + per-card "Join This Salon" (open) or "Ask to Join" (closed) button

### Non-Member View (Private Groups)
- Artistic wall layout with group branding only — no content frames visible
- "Private salon • 14 members • Join to see what's on the wall"

**Growth power**: Artistic, overlapping "wall" layout makes screenshots and shares feel special. Open + public is the most viral; closed + public creates exclusivity FOMO.

## Proposal C: "The Featured Conversation Canvas"

**Vibe**: Modern high-end magazine meets private reading room — one strong featured piece at top, clean supporting collection below.

### Member View
- Top: Large, full-bleed featured content canvas (most discussed or most recently shared)
  - Dominated by content's hero image + title
  - Subtle highlight teasers as elegant overlaid pull-quotes
  - Below visual: Full unioned discussion thread + AI co-host summary at top
- Below hero: Clean grid-style supporting collection (smaller but still visually led by covers/artwork)
- "Invite Friends" pinned in hero area

### Non-Member View (Public Groups)
- Entire hero canvas fully visible (image + teasers + comment previews) for maximum FOMO
- Supporting grid limited to 4–6 items, readable except full replies
- Multiple CTAs: "Join" (open groups) or "Request Invite" (closed groups) — hero + bottom of each supporting card

### Non-Member View (Private Groups)
- Hero shows group branding and description only — no content canvas
- "Private group • Request Invite to see what's being discussed"

**Growth power**: Big featured canvas on public groups creates instant "I want to be in that discussion" pull. Open groups convert immediately; closed groups build waitlist energy.

## Quick Comparison

| Proposal | Vibe | Best Group Types | Growth Strength | Vision Alignment |
|---|---|---|---|---|
| A. The Private Collection | Calm private library/gallery | Closed + Public (curated showcase) | Highest shareability & retention | Very high |
| B. The Salon Wall | Warm, artistic salon | Closed + Private (exclusive feel) | Strongest "exclusive club" FOMO | Highest |
| C. The Featured Conversation Canvas | Modern magazine + depth | Open + Public (discovery engine) | Strongest single-piece conversion | High |

> **Note**: All proposals work for all four group types (open/closed × public/private). The "Best Group Types" column indicates where each design's strengths shine most. The non-member view automatically adapts based on group visibility (public = rich preview, private = metadata only) and access (open = instant join, closed = request invite).
