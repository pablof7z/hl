# Product Specification: Highlighter
## Version 2.0 | April 2026

---

## 1. What Highlighter Is

Highlighter is a **Nostr-native social reading platform** built around NIP-29 relay-based groups. Users create and join communities (NIP-29 groups) where they share, highlight, and discuss content — books, articles, podcasts, videos, research papers, and anything else worth reading.

The atomic unit is the **artifact** (a piece of external content). Members annotate artifacts with **highlights** (excerpts they find compelling), and those highlights spark **discussions**. Communities are the container that gives all of this context and trust.

### What makes Highlighter different

1. **Nostr-native from day one.** Communities are NIP-29 groups on Nostr relays. Users own their identity (Nostr keypairs), their data is portable, and groups can move between relays. No platform lock-in.
2. **The artifact is the hero.** Unlike social feeds where posts are the unit, Highlighter organizes everything around the source content — a book, an article, a podcast. Highlights and discussions exist *in service of* that content.
3. **Growth is the product, not a department.** Every feature, flow, and interaction is designed with virality loops and user control in mind. Sharing is frictionless. Invite mechanics are baked into every surface. Public groups are discovery engines.

---

## 2. Core Concepts

### Groups (NIP-29)

Communities in Highlighter are **NIP-29 relay-based groups**. This is not an abstraction — the technical implementation IS NIP-29 groups running on our khatru-based relay infrastructure.

Groups have two independent axes of configuration:

| Axis | Options | What it controls |
|---|---|---|
| **Access** | **Open** (anyone can join) or **Closed** (invite-only / admin approval) | Who can become a member and participate |
| **Visibility** | **Public** (anyone can see content) or **Private** (only members see content) | Who can read group messages and metadata |

This creates four group types:

| Type | Access | Visibility | Use case |
|---|---|---|---|
| **Open + Public** | Anyone joins | Anyone reads | Discovery communities, public book clubs, topic-based groups |
| **Open + Private** | Anyone joins | Members only | "Join to see" communities — low barrier, but content stays within the group |
| **Closed + Public** | Invite-only | Anyone reads | Curated reading groups with public showcase — strongest growth/FOMO loop |
| **Closed + Private** | Invite-only | Members only | Intimate circles, family groups, sensitive discussions |

**NIP-29 mapping:**
- **Closed** → `closed` tag on `kind:39000` group metadata (join requests ignored; members added via `kind:9000` or invite codes via `kind:9009`)
- **Private** → `private` tag on `kind:39000` (only members can read group messages)
- The `restricted` tag (only members can write) is always set — all Highlighter groups require membership to post
- The `hidden` tag (metadata hidden from non-members) is set for **Private** groups

### Artifacts

An artifact is any piece of external content shared to a group:
- Books (Kindle highlights, physical book quotes)
- Articles and essays (web, newsletters)
- Podcasts (episode-level)
- Videos (YouTube, etc.)
- Research papers, PDFs
- Any URL with extractable content

Each artifact has: title, author/creator, source, cover image/thumbnail, and a canonical URL when available.

### Highlights

Highlights are excerpts pulled from artifacts by community members. They are the unique layer Highlighter adds — what people found compelling, surprising, or discussion-worthy in a piece of content.

Highlights are:
- **Created by selecting text** (or clipping a timestamp for audio/video) — there is no "Add Highlight" button. The action is triggered by the natural gesture of selection, not a separate UI affordance.
- Attributed to the member who highlighted them
- The spark that drives discussion (comments attach to highlights or to the artifact directly)
- Shareable individually as cards (for virality — see Growth section)
- **Bookmarkable by other members** — any member can save someone else's highlight to their own vault ("My Highlights") without needing to re-create it

### Discussions

Every artifact in a group has a discussion thread. Discussions can happen at two levels:
- **Artifact-level**: General discussion about the content itself — threaded, intentional, debate-ready. Someone creates a discussion when they want to go deep or spark a structured conversation.
- **Highlight-level**: Conversation sparked by a specific excerpt

**Cross-community union**: When the same artifact is shared to multiple groups, discussions remain per-group (respecting group privacy boundaries). However, users who belong to multiple groups can see and cross-reference discussions they have access to.

### Notes

Notes are a lighter-weight annotation type at the artifact level — distinct from discussions in intent and weight.

- **Notes are off-the-cuff remarks**: "This book reminded me of X", "Interesting parallel to Y", a passing thought or tangential connection
- **Not debate-starters**: Notes don't carry the social weight of opening a discussion thread. They're more like margin scribbles.
- **Commentable, but not the point**: Other members can reply to notes, but the expectation is light — a quick "+1" or one-liner, not a full thread
- **Separate tab on the artifact page**: Notes live in their own tab alongside Highlights, Discussions, etc. — they don't clutter the main discussion space

---

## 3. Platform Architecture

### Client Applications

Highlighter ships as **three client surfaces** from launch:

| Platform | Technology | Notes |
|---|---|---|
| **Web app** | Modern web stack (SPA) | Primary development surface. Full feature parity. Responsive, mobile-first design. |
| **Mobile apps** (Android + iOS) | Rust-based, multi-platform | Shared Rust core with native UI layers. First-class mobile experience, not a wrapper. |
| **Desktop app** | Rust-based, multi-platform | Same Rust core. macOS, Windows, Linux. Focus on deep reading and annotation workflows. |

The Rust core handles:
- Nostr protocol (NIP-29 group operations, event signing, relay management)
- Local data storage and caching
- Highlight extraction and content processing
- Offline support and sync

The web app may use a different stack for the UI layer but shares protocol-level logic where practical.

### Relay Infrastructure

Highlighter operates a **khatru-based Nostr relay** as part of the project:

- **[khatru](https://github.com/fiatjaf/khatru)** is a Go framework for building custom Nostr relays
- **[relay29](https://github.com/fiatjaf/relay29)** (built on khatru) provides NIP-29 group management out of the box
- Our relay handles: group creation/management, membership, moderation, message storage and retrieval
- The relay signs group metadata events (`kind:39000`, `kind:39001`, `kind:39002`) with its own keypair
- Users authenticate via NIP-42 for membership-gated operations

**Why we run our own relay:**
- Full control over group policies, moderation rules, and the user experience
- Can implement Highlighter-specific event kinds (artifact metadata, highlight events) alongside standard NIP-29
- Performance and reliability for our users
- Groups are still portable — users can fork/move groups to other NIP-29-compatible relays if they choose

**Decentralization posture:** We run the default relay, but the protocol is open. Other relays can host Highlighter-compatible groups. Users own their keys and data. This is not "decentralized in Phase 2" — it's the foundation.

---

## 4. Features

### MVP (Launch)

#### Group Creation & Management
- Create a group: name, description, cover image, rules
- Choose access (open/closed) and visibility (public/private) at creation — changeable later
- Invite members via: shareable link, invite code (`kind:9009`), direct add by admin (`kind:9000`)
- Roles: admin, moderator, member (mapped to NIP-29 roles via `kind:39003`)
- Moderation: remove members, delete messages, edit group metadata (all via NIP-29 moderation events `kinds:9000-9020`)

#### Artifact Sharing
- Share any URL → auto-extract title, author, cover image, source
- Manual entry for books, physical media
- Browser extension for one-click capture from any webpage
- Share to one or multiple groups simultaneously

#### Highlighting
- Pull excerpts from shared artifacts
- AI-assisted highlight suggestion (smart extraction from URLs)
- Universal capture: browser extension, mobile share sheet, Kindle integration, manual paste
- Each highlight attributed to the member who created it

#### Discussion & Notes
- Threaded discussions on artifacts (artifact-level) and individual highlights (highlight-level)
- **Notes**: lightweight off-the-cuff annotations at the artifact level — separate from discussions, lighter social weight, their own tab on the artifact page
- Reactions and replies
- @mentions of group members

#### Discovery & Browsing
- **Group home page**: Featured section uses a **carousel** — the active artifact and its highlights/discussions are visible, and members can swipe to see other featured artifacts. Below the featured carousel, the rest of the group's library is presented with **varied visual representations** (not a uniform card grid) — sections switch up layout and density, like the Apple TV app does, so the page feels alive and curated rather than a wall of identical cards.
- **Artifact detail page**: tabs for Highlights, Notes, Discussions — plus artifact-specific rendering per content type (see wireframes)
- **Personal vault**: all your highlights across all groups, searchable. Also includes highlights you've bookmarked from others.

#### Public Pages (for public groups)
- SEO-optimized public views of group content
- Individual highlight cards with beautiful formatting
- Non-member views with clear "Join" or "Request Invite" CTAs
- Embeddable highlight cards for external sharing

### Post-MVP

- AI co-host: discussion facilitation, weekly summaries, "What the group learned this month"
- Creator-owned groups (authors create communities around their books/content)
- Advanced cross-group discovery (for public groups): "Groups discussing similar content"
- Voice/audio highlights (podcast timestamp clips)
- Reading challenges and group goals
- API for third-party integrations

---

## 5. Growth: Baked In, Not Bolted On

Growth is not a strategy section — it's a **design principle** that governs every feature decision. Every interaction should ask: *does this create a loop? does this give the user control to share on their terms?*

### Core Growth Principles

1. **User control maximizes virality.** People share more when they choose what, where, and how. Highlighter never auto-posts, never shares without consent, never leaks private group content. Paradoxically, this control makes users *more* willing to share, not less.

2. **Every surface is a growth surface.** Not just the "invite" button — every highlight card, every public group page, every artifact discussion is a potential entry point for new users.

3. **Value before signup.** Public groups and shared highlight cards should deliver value (beautiful content, interesting discussions) before asking for anything. The conversion happens because the content is good, not because we gated it.

### Virality Loops

| Loop | Mechanism | Entry Point |
|---|---|---|
| **Invite Loop** | Member invites friends → friends get immediate value in a small group → they invite their friends | Every group has prominent, frictionless invite mechanics |
| **Highlight Card Loop** | Member shares a highlight card externally (Twitter, WhatsApp, email) → card is beautiful and contains the excerpt + group context → viewer clicks through → lands on public group or "request invite" | Every highlight has a one-tap "share as card" action |
| **Public Group Discovery** | Public groups are indexed, SEO-optimized → organic traffic discovers group discussions → signs up to participate | Public group pages are designed as landing pages |
| **Cross-Group Pollination** | User belongs to 3+ groups → shares same artifact across groups → different discussions create curiosity → "You should join this other group too" | Cross-posting is a first-class action, not buried |
| **Creator Flywheel** | Author/creator makes a group for their book/podcast → fans join → fans share highlights from the content externally → new fans discover the creator + the platform | Creator group creation is a specific onboarding path |

### Growth Metrics (Targets, First 12 Months)

- Viral coefficient: ≥ 1.4 (each user brings in 1.4+ new users)
- Average groups per user: 3+
- 40%+ of users invite at least 3 friends in first week
- 30%+ of highlights shared externally within 24h of creation
- Public group pages convert visiting non-members at 8%+

### Growth in Every Feature Decision

Examples of how growth thinking shapes product choices:

| Feature | Without growth thinking | With growth thinking |
|---|---|---|
| Highlight creation | Save highlight to my library | Save highlight → prompt "Share to group?" → prompt "Share as card?" → beautiful card with group branding + CTA |
| Group creation | Fill out form, done | Fill out form → immediate "Invite 5 friends" flow → pre-written invite message → track who joined |
| Public group page | List of content | Designed as a landing page: hero content, social proof (member count, activity), FOMO triggers, dead-simple join CTA |
| Non-member view | "Sign up to see more" | Rich preview of best content + best discussions, enough to demonstrate value, with contextual CTAs throughout |
| Browser extension | Highlight and save | Highlight → "Which groups want to see this?" → instant share → notification to group members ("Sarah just highlighted something in...") |

---

## 6. User Control & Data Ownership

Highlighter is built on Nostr because user control is non-negotiable:

- **Identity**: Users own their Nostr keypair. No email/password accounts. Login with any Nostr signer (NIP-07, NIP-46, etc.)
- **Data portability**: All events (highlights, comments, group memberships) are standard Nostr events. Users can export, move, or use them in any compatible client.
- **Group portability**: NIP-29 groups can be forked or moved to different relays. A community is not locked to our infrastructure.
- **Privacy by design**: Private groups use NIP-29's `private` tag — content is only served to authenticated members. We don't mine or sell user data.
- **Moderation is local**: Each group's moderators control their space. There is no platform-level content moderation beyond legal requirements. Trust is at the group level, not the platform level.

---

## 7. Doc Index

| Document | Description | Status |
|---|---|---|
| `product-spec-v2.0.md` | This document. Core product spec. | **Active** |
| `technical-architecture.md` | NIP-29 implementation details, khatru relay config, Nostr event kinds, platform architecture | **Active** |
| `community-page-proposals-v1.4.md` | UI proposals for group home pages (design-level) | Active (design reference) |
| `landing-page-proposals.md` | Landing page concepts (design-level) | Active (design reference) |
| `community-page-proposals.md` | Earlier UI proposals (v1.3) | Archived |
| `product-spec-v1.2.md` | Previous product spec (pre-Nostr, pre-NIP-29) | **Superseded by v2.0** |

---

*This spec supersedes `product-spec-v1.2.md` and `product-spec-v1.2-grok-draft.md`. The core community-first vision from v1.2 is preserved and strengthened — what's new is the Nostr/NIP-29 foundation, multi-platform commitment, explicit group access/visibility model, and growth as a design principle rather than a strategy appendix.*
