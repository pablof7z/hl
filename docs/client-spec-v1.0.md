# Client Product Specification: Highlighter
## Version 1.0 | April 2026

---

## 0. Decisions Log

| Decision | Choice | Rationale |
|---|---|---|
| Navigation model | 3-tab bottom nav: Communities (default), Discover, Me | Minimal, content-centric. Communities-first reflects the core value prop. |
| Discover tab purpose | Public discovery — find new communities and content | NOT a personalized feed. Purely for discovering things outside your existing communities. |
| "For Later" shelf | Private personal queue under Me tab | Bridge between capture and community sharing. Nothing gets lost. |
| OCR photo capture | First-class mobile feature via universal capture button | Physical book readers can snap → OCR → highlights in one flow. |
| Highlights = teasers | Same concept, used interchangeably | Highlights ARE teasers — excerpts that tease the full content. |
| Community highlight sharing | Nostr `kind:16` repost with `h` tag | Sharing a highlight to a community = publishing a `kind:16` repost of the `kind:9802` highlight event, `h`-tagging the target community's group ID. |
| Style guide scope | All surfaces — webapp + iOS + Android | Same design language everywhere. "Quiet confidence." |
| Style guide status | **Locked for v1** | No changes without explicit approval. |

---

## 1. Relationship to Other Specs

This document defines the **client-side experience**: screens, navigation, components, interactions, and visual language. It does NOT redefine core concepts or technical infrastructure.

| Topic | Authoritative Document |
|---|---|
| Core concepts (groups, artifacts, highlights, discussions) | `product-spec-v2.0.md` |
| Product surfaces (relay, webapp, mobile, desktop stacks) | `product-surfaces-v3.md` |
| NIP-29 group model, event schemas, relay architecture | `technical-architecture.md` |
| Growth loops, virality mechanics | `product-spec-v2.0.md` §5 |
| Market research, competitive landscape | `market-research-2026.md` |

**Terminology mapping:**
- "Community" (user-facing) = NIP-29 group (protocol-level)
- "Highlight" = "Teaser" = NIP-84 `kind:9802` event (an excerpt from an artifact)
- "Artifact" = external content identified by source-reference tags on a community `kind:11` share thread (`a`, `e`, or `i`/`k`)
- "For Later" = private personal queue (no Nostr event kind — local/private storage)

---

## 2. Design System (Locked for v1)

### 2.1 Design Philosophy

**Quiet confidence.**

Every screen feels like a beautifully designed private library: spacious, warm, thoughtful, focused entirely on the content and the conversation. Never loud, never cluttered, never "social-app busy."

### 2.2 Color Palette

**Primary (Backgrounds & Base)**

| Name | Hex | Usage |
|---|---|---|
| Off-White | `#F8F5F0` | Light mode default background |
| Soft Charcoal | `#1F1F1F` | Dark mode default background |

**Accents & Text**

| Name | Hex | Usage |
|---|---|---|
| Warm Beige | `#EDE4D8` | Subtle card backgrounds, dividers |
| Deep Neutral | `#2C2C2C` | Primary text (light mode) |
| Muted Gray | `#6B6B6B` | Secondary text, labels |

**Highlight Accent** (the only "pop" color)

| Name | Hex | Usage |
|---|---|---|
| Soft Terracotta | `#C47E5E` | Highlight underline, "WHAT CAUGHT OUR EYE" label, primary text buttons, floating action button fill |

**Semantic Colors** (used sparingly)

| Name | Hex | Usage |
|---|---|---|
| Muted Green | `#8A9A7F` | Success / Active states |
| Muted Red | `#A36A6A` | Error / Destructive states |

**Dark Mode:** All colors invert naturally — background → `#1F1F1F`, text → `#F8F5F0`. Terracotta accent stays the same. 100% parity with light mode (no design debt).

### 2.3 Typography

**Font Family:** Inter (system sans-serif fallback: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif)

| Role | Size | Weight | Line Height | Notes |
|---|---|---|---|---|
| Title (content piece) | 22–24 px | Medium | 1.4 | |
| Body / Discussion text | 17 px | Regular | 1.55 | |
| Small labels / "WHAT CAUGHT OUR EYE" | 12 px | Medium | — | Uppercase, tracking +0.5px |
| Metadata (source, date) | 14 px | Regular | — | Muted color |

**Rules:**
- Never more than two type sizes on a single card
- Generous line height everywhere — readability above all
- No bold for emphasis except on titles

### 2.4 Spacing & Layout Grid

**Base unit:** 8 px

| Element | Spacing |
|---|---|
| Card padding | 24 px |
| Vertical rhythm between elements | 16 px or 24 px |
| Section gutters | 32 px |
| Screen edge margin | 20 px (mobile) |

### 2.5 Card Style (Used Everywhere)

The content card is the most important component in the system. It appears on the Community Front Page, For Later, Discover, Content Preview, and everywhere content is displayed.

| Property | Value |
|---|---|
| Rounded corners | 20 px |
| Background | Off-White (`#F8F5F0`) light / Soft Charcoal dark |
| Shadow | Very subtle inner shadow on hero images only (0.5 px blur). No heavy drop shadows. |
| Hero visual | Full-width, 4:3 ratio, rounded 16 px top corners |
| Teaser area | Terracotta left border (2 px) |
| Discussion preview | Indented 8 px, smaller type |

### 2.6 Buttons

| Type | Style |
|---|---|
| Primary | Text only, terracotta color, medium weight |
| Secondary | Muted gray text |
| Floating action | Soft terracotta fill, rounded 999 px (the only filled button) |

### 2.7 Input Fields

- Subtle bottom border only (no boxes)
- Focus state: terracotta underline + soft glow

### 2.8 Iconography

- **Icon set:** Lucide or Feather (thin stroke, 24 px)
- Stroke weight: 1.5 px
- Never use emoji or filled icons
- Color: always muted gray unless floating action button

### 2.9 Motion & Micro-interactions

| Interaction | Specification |
|---|---|
| All transitions | 200 ms ease-out |
| Card tap | Subtle scale 0.98 → 1.0 |
| Content appearing in feed | Gentle fade-up 120 ms |
| General | No bouncy or playful animations — everything is calm and deliberate |

### 2.10 Accessibility

- Minimum contrast ratio: 4.5:1 (AA)
- All text scalable up to 200%
- Tap targets: minimum 48 px
- No color-only information (always paired with label)
- Dark mode: 100% parity

---

## 3. Navigation Architecture

### 3.1 Bottom Navigation (3 Tabs)

```
┌─────────────────────────────────────────────────────────┐
│                                                          │
│                    [Active Screen]                        │
│                                                          │
├──────────────┬──────────────────┬────────────────────────┤
│ Communities  │     Discover     │          Me            │
│  (default)   │                  │                        │
└──────────────┴──────────────────┴────────────────────────┘
```

| Tab | Purpose | Default State |
|---|---|---|
| **Communities** | Your communities. The daily experience. | List of communities you belong to |
| **Discover** | Find new communities and content (public discovery) | Browse/search public communities |
| **Me** | Profile, vault, personal queue, settings | Profile header + sub-tabs |

### 3.2 Universal Content Capture Button

A floating + header button available anywhere in the app. Opens a clean bottom sheet:

| Option | Description |
|---|---|
| **Paste URL / import link** | Standard digital content capture |
| **Take Photo** (mobile only) | Physical book OCR capture (see §5) |
| **Share-sheet imports** | Content shared from other apps |
| **Browser extension trigger** | If extension installed (webapp) |

### 3.3 Route Mapping

How the 3-tab navigation maps to the webapp routes defined in `product-surfaces-v3.md`:

| Tab / Screen | Webapp Route |
|---|---|
| Communities (list) | `/` (default landing) |
| Community Front Page | `/community/[id]` |
| Content Preview / Overview | `/community/[id]/content/[contentId]` |
| Discussion Screen | `/community/[id]/content/[contentId]/discussion` |
| Create Community | `/community/create` |
| Discover | `/discover` |
| Me (profile) | `/me` |
| My Highlights | `/me/highlights` |
| For Later | `/me/for-later` |
| My Communities | `/me/communities` |
| Recommended | `/me/recommended` |
| Synthesis | `/me/synthesis` |
| Public Community Page (SEO) | `/share/community/[id]` |
| Public Highlight Card (SEO) | `/g/[group-id]/e/[highlight-id]` |

**Note:** Routes use "community" in URLs (user-facing language), mapped to NIP-29 group IDs internally.

---

## 4. Tab-by-Tab Screen Specifications

### 4.1 Tab 1: Communities (Default Landing)

#### Main Screen: Communities List

The user's home. Shows all communities they belong to.

- Each community card: cover image, name, member count, recent activity indicator, unread count
- Quick action: create new community
- Tap → enters Community Front Page

#### Screen: Create Community

Flow for creating a new NIP-29 group:
- Name, description, cover image
- Access: Open or Closed
- Visibility: Public or Private
- Invite members (post-creation)

#### Screen: Community Front Page

The heart of the product. A community's collection view showing:
- Community header (cover, name, description, member count)
- Featured artifacts with highlight teasers
- Highlight spotlight ("What caught our eye")
- Full artifact library
- Activity feed
- Members

Each artifact card uses the standard content card component (§2.5) with:
- Hero visual (book cover, podcast art, article image)
- Title + creator + source metadata
- Highlight teaser area (terracotta left border) showing the community's best excerpt
- Discussion preview (comment count, recent snippet)

#### Screen: Content Preview / Overview

Tapping an artifact card from the Community Front Page opens this screen:
- Full artifact metadata (title, author, source, cover)
- All highlights from community members, organized by position in the content
- Discussion entry point
- "Save to For Later" button (if not already saved)
- "Share to another community" action

#### Screen: Discussion Screen

Threaded conversation on an artifact or specific highlight:
- Artifact-level discussion: general conversation about the content
- Highlight-level discussion: conversation sparked by a specific excerpt
- Reply threading (NIP-22)
- @mentions of community members
- Reactions

### 4.2 Tab 2: Discover

**Purpose:** Public discovery — find new communities and content you haven't seen before.

This is NOT a personalized feed from your existing communities. It's the "explore" surface.

- Browse public communities by topic/interest
- Trending highlights across the platform
- Search (communities, artifacts, topics)
- Featured/editorial picks
- "Recommended for you" based on your community memberships and highlight patterns

### 4.3 Tab 3: Me

#### Main Screen: My Profile / Vault

- **Header:** Avatar, name, bio, stats (highlight count, communities, etc.)
- **Sub-tabs (5):**

| Sub-tab | Content |
|---|---|
| **Highlights** | All your highlights across all communities. Full content hero visual. Your personal intellectual trail. |
| **For Later** | Private personal queue (see §6 for full spec) |
| **Communities** | List of communities you belong to |
| **Recommended** | Content recommended based on your communities + items lingering in For Later |
| **Synthesis** | AI-generated connections and patterns across your highlights (future feature — placeholder) |

---

## 5. Content Capture Flow

The most important mobile improvement. Available from the universal capture button anywhere in the app.

### 5.1 Flow

```
┌──────────────────────┐
│  Tap Capture Button   │
└──────────┬───────────┘
           │
    ┌──────▼──────┐
    │ Choose Input │
    │   Method     │
    ├──────────────┤
    │ • Paste URL  │
    │ • Take Photo │
    │ • Share-sheet│
    └──────┬──────┘
           │
    ┌──────▼──────────────┐
    │   AI Processing      │
    │                      │
    │ Photo → OCR → clean  │
    │ text → suggest 1-3   │
    │ highlight teasers    │
    │                      │
    │ URL → fetch metadata │
    │ → suggest teasers    │
    └──────┬──────────────┘
           │
    ┌──────▼──────────────┐
    │  Review / Edit       │
    │  Teasers (optional)  │
    └──────┬──────────────┘
           │
    ┌──────▼──────────────┐
    │  Final Decision      │
    │                      │
    │ • Save to For Later  │
    │   (private, instant) │
    │ • Share to community │
    │   (with teasers)     │
    │ • Save privately     │
    │   (vault only)       │
    └─────────────────────┘
```

### 5.2 Take Photo Flow (Mobile Only)

1. Opens device camera with gentle overlay guide ("Position text clearly")
2. Auto-capture or manual shutter
3. On-device OCR (cloud fallback if needed)
4. AI cleans extracted text and suggests 1–3 highlight teasers
5. User can edit extracted text or choose which portions become teasers
6. Final step: "Save to For Later" or "Share to community"

### 5.3 AI Teaser Suggestion

When content is captured (photo or URL), AI processes it to suggest 1–3 "tasteful highlight teasers" — the most compelling, discussion-worthy excerpts. The user can:
- Accept suggested teasers as-is
- Edit them
- Choose different portions
- Skip (save without teasers)

---

## 6. For Later — Full Screen Specification

### 6.1 Purpose

A calm, private personal queue where saved content pieces wait until you're ready to add teasers or share them into communities. It feels like a minimalist "ideas inbox" — spacious, visual, never overwhelming.

### 6.2 Location

Me tab → For Later sub-tab

### 6.3 Layout

- **Full-screen view** with generous whitespace
- **Top header:** "For Later" in elegant small caps + subtle count ("12 items")
- **Sorting bar** (subtle, low-contrast): Newest • Oldest • By type (Books / Podcasts / Articles) • Estimated time
- **Main content area:** Vertical list of content cards
- **Floating action button** (bottom-right): "Capture new → For Later" (direct shortcut)

### 6.4 Content Card Components

Each card in For Later:

**1. Hero visual** (left or top on mobile)
- Large, rounded book cover / podcast artwork / article image (dominant, 3:4 ratio)
- Soft shadow for subtle depth

**2. Content metadata** (right of image or below on narrow screens)
- Title (1–2 lines max, bold)
- Creator / source (smaller, muted)
- Saved date + estimated read/listen time (tiny, e.g., "Saved 3 days ago • 42 min")

**3. Highlight teaser area** (only if already added)
- If teasers exist: one tasteful pull-quote with label "Your teaser" in tiny elegant caps
- If none: subtle placeholder "No teaser yet — tap to add" (muted, encouraging)

**4. Status & Quick Actions** (bottom row, clean horizontal)
- Status pill:
  - "Ready to share" (muted green `#8A9A7F`)
  - "Needs teaser" (neutral)
  - "Already in 2 communities" (blue)
- Three minimal buttons (icon-only or very short text):
  - **Add teaser** (pencil icon)
  - **Move to community** (arrow icon) → opens community selector
  - **Remove** (trash icon, only on long-press or swipe)

### 6.5 Card Interactions

| Interaction | Action |
|---|---|
| Tap anywhere on card | Opens Content Preview / Overview screen |
| Swipe left | Quick "Remove" |
| Swipe right | Quick "Move to community" |
| Long-press | Enter bulk selection mode |

### 6.6 Bulk Selection

Triggered by long-pressing one card:
- "Select all" / "Move selected to community" / "Remove selected"

### 6.7 Empty State

- Centered soft line illustration (quiet shelf, max 120 px tall)
- Headline: "Your For Later is empty"
- Body: "Capture highlights from physical books, articles, or podcasts here when you don't have time to share them yet."
- Primary button: "Capture something now" (opens universal capture flow with "Save to For Later" pre-selected)

### 6.8 Technical Notes

- Pull-to-refresh reloads the list
- Infinite scroll (no pagination — unlikely to grow very long for a personal queue)
- For Later items are **private by default** — stored locally or as private Nostr events (not published to the community relay until explicitly shared)

---

## 7. Community Highlight Sharing — Protocol Mechanic

### 7.1 The Core Interaction

When a user shares a highlight to a community, the app publishes a **Nostr `kind:16` repost** (NIP-18 generic repost for non-kind-1 events) of the highlight event, with an `h` tag pointing to the target community's NIP-29 group ID.

### 7.2 Event Structure

```jsonc
{
  "kind": 16,                           // NIP-18 generic repost
  "content": "",                        // Empty or JSON stringified original event
  "tags": [
    ["e", "<highlight-event-id>", "<relay-url>"],  // Reference to the kind:9802 highlight
    ["k", "9802"],                      // Original event kind
    ["p", "<highlight-author-pubkey>"], // Original highlight author
    ["h", "<community-group-id>"]       // Target community (NIP-29 group)
  ]
}
```

### 7.3 Implications

- **Highlights are portable.** The `kind:9802` highlight event lives wherever it was created (could be the user's personal relay, could be a community relay). Sharing to a community is a *reference*, not a copy.
- **One highlight, many communities.** A user can share the same highlight to multiple communities by publishing multiple `kind:16` reposts with different `h` tags.
- **Community relay stores the repost.** The `kind:16` repost event is published to the community's relay, which indexes it as part of that community's content.
- **Discovery through reposts.** When a community member views the community's feed, the relay returns `kind:16` events referencing highlights. The client fetches the original `kind:9802` events to display them.

### 7.4 Privacy Considerations

- For **public communities**: the repost is visible to anyone who queries the community relay.
- For **private communities**: the repost is only visible to authenticated members (NIP-42), consistent with all other NIP-29 private group events.
- The original highlight event's visibility depends on where it was published — sharing to a private community doesn't make the original highlight private if it was published elsewhere.

---

## 8. Component Library Summary

### 8.1 Reusable Components

| Component | Used In | Key Properties |
|---|---|---|
| **Content Card** | Community Front Page, For Later, Discover, Recommended | Hero visual (4:3), metadata, teaser area (terracotta border), discussion preview |
| **Highlight Teaser** | Content cards, Content Preview, shared highlight cards | Pull-quote text, terracotta left border (2 px), "WHAT CAUGHT OUR EYE" label |
| **Community Card** | Communities list, My Communities, Discover | Cover, name, member count, activity indicator |
| **Status Pill** | For Later cards, content items | "Ready to share" (green) / "Needs teaser" (neutral) / "Already in N communities" (blue) |
| **AI Co-host Pill** | Where AI-generated content is displayed | Tiny rounded rectangle, warm beige background, terracotta text, 12 px |
| **Floating Action Button** | Capture button (global), For Later shortcut | Soft terracotta fill, rounded 999 px |
| **Bottom Sheet** | Capture flow, community selector, share actions | Clean modal from bottom edge |
| **Empty State** | For Later, any empty list | Soft line illustration (120 px), headline, body, primary button |

### 8.2 Content Card Variants

| Variant | Context | Differences |
|---|---|---|
| **Community Front Page card** | Artifact in a community | Includes discussion preview, community-specific teaser |
| **For Later card** | Personal queue item | Includes status pill, "Saved X days ago", quick actions |
| **Discover card** | Public content discovery | Includes community attribution, member count |
| **Highlight card (shareable)** | External sharing (Twitter, WhatsApp) | Standalone, branded, includes community context, CTA to join |

---

## 9. User Stories (Reference)

These ground the spec in real-world usage:

**Physical book moment:** You're reading a paperback on the couch → open Highlighter → tap capture → "Take Photo" → snap the page → OCR gives clean text → pick a teaser → "Save to For Later" (discuss it with your group tomorrow).

**Quick capture from anywhere:** Reading an article on your phone or listening to a podcast → share to Highlighter → instantly goes to For Later until you have time to add it to a community.

**For Later as a bridge:** The shelf acts as a gentle personal inbox. Nothing gets lost. When ready, one tap moves it into the right community with your highlight teasers already attached.

**Cross-community sharing:** You find a brilliant highlight in one community → tap share → pick another community → a `kind:16` repost with `h` tag publishes it there. The original highlight stays where it is; the community gets a reference.

---

## 10. Screens Not Yet Specified

The following screens are referenced in this spec but need detailed component-level specifications:

| Screen | Status | Notes |
|---|---|---|
| Community Front Page | Referenced — needs component-level detail | Core screen. Previous proposals (A/B/C) deleted. Needs fresh spec using this design system. |
| Content Preview / Overview | Mentioned in v1.3 — needs full spec | The artifact detail view within a community |
| Discussion Screen | Mentioned in v1.3 — needs full spec | Threaded conversation on artifacts/highlights |
| Create Community | Mentioned — needs flow detail | NIP-29 group creation wizard |
| Discover | Purpose defined — needs layout spec | Public community/content discovery |
| Synthesis (Me sub-tab) | Placeholder — needs concept definition | AI-generated connections across highlights |
| Onboarding / Auth | Not covered in v1.3 | NIP-07/46 auth + first community join/create |
| Public Community Page | Referenced for SEO — needs spec | Growth engine — what non-members see |
| Public Highlight Card | Referenced for virality — needs spec | The atomic viral unit that travels outside the app |

---

*This spec supersedes the screen-level definitions in any prior wireframe or proposal documents. It is the canonical source for client-side UX, design system, navigation architecture, and interaction patterns. Core concepts (groups, artifacts, highlights, discussions) remain as defined in `product-spec-v2.0.md`. Technical surfaces (relay, webapp stack, mobile stack, desktop stack) remain as defined in `product-surfaces-v3.md`.*
