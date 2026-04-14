# Highlighter Web App — MVP Build Plan
## Version 1.0 | April 2026

---

## 0. Decisions Made

| Decision | Value |
|---|---|
| Starting template | [ndk-template-sveltekit-vercel](https://github.com/nostr-dev-kit/ndk-template-sveltekit-vercel) |
| Stack | SvelteKit + NDK + Vercel |
| Relay strategy | Wire real data from the start (relay being built in parallel) |
| Discover tab | **Cut from MVP** — added post-MVP |
| For Later shelf | **Cut from MVP** — post-MVP |
| OCR photo capture | **Cut from MVP** — mobile/post-MVP |
| AI teaser suggestion | **Cut from MVP** — post-MVP |
| Synthesis sub-tab | **Cut from MVP** — post-MVP |

---

## 1. MVP Screens (9 total)

| # | Screen | Route | Notes |
|---|---|---|---|
| 1 | **Onboarding** | `/onboarding` | Auth (NIP-07 + NIP-46), profile setup |
| 2 | **Communities List** (Home) | `/` | All communities you belong to; create new |
| 3 | **Group Home** | `/community/[id]` | Featured carousel + library |
| 4 | **Artifact Detail** | `/community/[id]/content/[contentId]` | Highlights + Discussions + Notes tabs |
| 5 | **Single Highlight View** | `/community/[id]/content/[contentId]/highlight/[highlightId]` | Full highlight + discussion |
| 6 | **Group Creation Flow** | `/community/create` | Name, desc, cover, access/visibility, invite |
| 7 | **Personal Vault (Me)** | `/me` | Profile header + Highlights sub-tab |
| 8 | **Public Group Page** | `/share/community/[id]` | SEO-optimized, non-member view |
| 9 | **Public Highlight Card** | `/share/highlight/[id]` | Shareable SEO card |

---

## 2. MVP Features (per screen)

### Screen 1: Onboarding
- NIP-07 login (browser extension: nos2x, Alby)
- NIP-46 login (remote signer: Nsec.app)
- Profile creation: name, bio, avatar (Blossom-backed)
- Post-login: redirect to communities list or group creation

### Screen 2: Communities List (Home `/`)
- List all NIP-29 groups the user is a member of
- Each card: cover image, name, member count, recent activity indicator
- Create new community CTA (→ Screen 6)
- Universal capture button (FAB): paste URL to share artifact to a group
- **Note:** No Discover tab in MVP — tab bar is Communities + Me only

### Screen 3: Group Home (`/community/[id]`)
- Community header: cover image, name, description, member count, invite CTA
- **Featured carousel**: pinned/recent artifacts, swipeable, each shows cover + best highlight teaser
- **Library**: full list of artifacts in group; varied layout (not uniform grid)
- Each artifact card: hero visual, title, author/source, highlight teaser (terracotta border), comment count
- Tap artifact → Screen 4
- Floating capture button: share new artifact to this group

### Screen 4: Artifact Detail (`/community/[id]/content/[contentId]`)
- Full artifact metadata header: title, author, source, cover image, external link
- **3 tabs:**
  - **Highlights**: all member highlights (kind:9802), ordered by position; tap → Screen 5
  - **Discussions**: threaded NIP-22 comments on the artifact; reply, react, @mention
  - **Notes**: lightweight remarks (kind:1 with NIP-73 artifact tag); commentable
- Create highlight: text selection triggers highlight creation (no separate button)
- Add note: simple compose box in Notes tab
- Bookmark button: save any highlight to personal vault
- Share artifact: cross-post to another group

### Screen 5: Single Highlight View (`…/highlight/[highlightId]`)
- Full highlight text display (large, beautiful)
- Attribution: author avatar + name
- Artifact context (which book/article/etc.)
- Discussion thread on this highlight (NIP-22 comments)
- Share as card button → generates public highlight card URL (Screen 9)
- Bookmark button

### Screen 6: Group Creation Flow (`/community/create`)
- Step 1: Name, description, cover image
- Step 2: Access (Open / Closed) + Visibility (Public / Private)
- Step 3: Post-creation invite flow
  - Shareable invite link (always)
  - Invite code (for Closed groups — kind:9009)
  - Direct add by npub/NIP-05

### Screen 7: Personal Vault / Me (`/me`)
- Profile header: avatar, name, bio, stats
- Sub-tabs (MVP):
  - **Highlights**: all highlights you've created + bookmarked across all groups
  - **Communities**: list of groups you're in
- (Recommended, For Later, Synthesis → post-MVP)

### Screen 8: Public Group Page (`/share/community/[id]`)
- SSR-rendered (SEO-optimized)
- Group header: cover, name, description, member count
- Best highlights from the group (public groups only)
- Recent artifact activity
- Clear CTA: "Join Community" (open) or "Request Invite" (closed)
- Dynamic OG image for social sharing

### Screen 9: Public Highlight Card (`/share/highlight/[id]`)
- SSR-rendered (SEO-optimized)
- Beautiful single-highlight display
- Excerpt text + attribution + artifact source
- Group branding / context
- CTA: "Join the conversation" → links to group page or onboarding
- Dynamic OG image (usable as a Twitter/WhatsApp card)

---

## 3. Core Features Summary

| Feature | Where | NIP / Kind |
|---|---|---|
| NIP-07 login | Onboarding | NIP-07 |
| NIP-46 login | Onboarding | NIP-46 |
| Share artifact by URL | FAB everywhere | NIP-73 entity tags on kind:1 |
| Create highlight by text selection | Artifact Detail | NIP-84 kind:9802 |
| Bookmark highlight to vault | Artifact Detail, Single Highlight | kind:10003 bookmarks list |
| Notes on artifacts | Artifact Detail (Notes tab) | kind:1 + NIP-73 tags |
| Threaded discussions on artifacts | Artifact Detail (Discussions tab) | NIP-22 kind:1111 |
| Threaded discussions on highlights | Single Highlight View | NIP-22 kind:1111 |
| Invite mechanics (link + code) | Group Creation, Group Home | kind:9009 |
| Group access/visibility settings | Group Creation + Settings | NIP-29 kind:39000 |
| Shareable highlight cards | Single Highlight View | Public route + OG image |
| Public group pages (SEO) | `/share/community/[id]` | SSR |

---

## 4. Technical Architecture

### Starting Point
Fork: `https://github.com/nostr-dev-kit/ndk-template-sveltekit-vercel`

### What the Template Gives Us
- SSR via SvelteKit + Vercel
- NDK wired up with `@ndk/svelte` stores
- NIP-07 + NIP-46 auth flows
- Profile creation + Blossom avatar uploads
- Dynamic OG images + SEO head component
- Vercel KV for NIP-05
- `/profile/[identifier]`, `/highlights`, `/bookmarks`, `/note/[id]` routes

### What We Build On Top

**New routes:**
```
/community/[id]                              → Group Home
/community/[id]/content/[contentId]          → Artifact Detail
/community/[id]/content/[contentId]/highlight/[highlightId]  → Single Highlight
/community/create                            → Group Creation
/me                                          → Vault / Profile
/share/community/[id]                        → Public Group Page (SSR)
/share/highlight/[id]                        → Public Highlight Card (SSR)
```

**Modified routes:**
```
/           → Communities List (was: generic Nostr home)
/onboarding → Add group creation + invite acceptance flows
```

**New lib modules:**
```
src/lib/
├── ndk/
│   ├── client.ts              # NDK setup + Highlighter relay config
│   ├── events/
│   │   ├── artifact.ts        # NIP-73 artifact event handling
│   │   └── highlight.ts       # NIP-84 kind:9802 event class
│   └── groups/
│       ├── state.ts           # NIP-29 group state management
│       └── membership.ts      # Join/leave/invite flows
├── components/
│   ├── highlight-card/        # The shareable highlight card
│   ├── artifact/              # Artifact display, URL extraction
│   ├── discussion/            # Threaded NIP-22 discussions
│   └── group/                 # Group header, settings, invite UI
├── server/
│   ├── nostr.ts               # Server-side NDK for SSR
│   └── og.ts                  # Dynamic OG image generation
└── url-extract.ts             # Serverless URL metadata extraction
```

### Relay Integration
- Primary relay: Highlighter's croissant fork (URL TBD — relay being built in parallel)
- NIP-42 auth for private/closed groups
- Fallback: well-known public NIP-29 relays for dev/staging

### Design System Implementation
- Tailwind CSS with custom tokens per client-spec-v1.0.md §2
- Colors: `#F8F5F0` off-white, `#1F1F1F` dark, `#C47E5E` terracotta accent
- Font: Inter (Google Fonts or local)
- Icons: Lucide (thin stroke, 24px)
- Cards: 20px border radius, subtle shadow, terracotta left border on highlights

---

## 5. Build Order (Phase Plan)

### Phase 1: Foundation (Week 1)
1. Fork + clone ndk-template-sveltekit-vercel
2. Configure NDK for Highlighter relay
3. Apply design system (Tailwind tokens, fonts, color palette)
4. Onboarding flow: NIP-07 + NIP-46 auth
5. Communities List (`/`) — read NIP-29 groups user is member of

### Phase 2: Core Reading Experience (Week 2)
6. Group Home (`/community/[id]`) — featured carousel + library
7. Artifact Detail (`/community/[id]/content/[contentId]`) — 3 tabs
8. Single Highlight View + discussion

### Phase 3: Content Creation (Week 2–3)
9. Share artifact by URL (capture button + metadata extraction)
10. Create highlight by text selection (kind:9802)
11. Add notes on artifacts
12. Threaded discussions (NIP-22 replies)

### Phase 4: Community Management (Week 3)
13. Group creation flow (name, desc, cover, access, visibility)
14. Invite mechanics (shareable link + invite code kind:9009)
15. Bookmark highlights to vault

### Phase 5: Personal + Growth Surfaces (Week 3–4)
16. Personal vault / Me screen (highlights + communities sub-tabs)
17. Public group page (SSR, SEO)
18. Public highlight card (SSR, OG image, shareable URL)

### Phase 6: Polish + Deploy (Week 4)
19. Dark mode
20. Responsive / mobile-first pass
21. Accessibility (contrast, tap targets)
22. Vercel deployment + environment config
23. Relay integration smoke test

---

## 6. Open Questions / Assumptions

| Question | Assumption for now |
|---|---|
| Relay URL for dev | Use environment variable `PUBLIC_HIGHLIGHTER_RELAY` — team to provide |
| Artifact URL extraction | Vercel serverless function calling opengraph.io or similar |
| OG image generation | Vercel `@vercel/og` (Edge) |
| NIP-05 for users | Keep from template (Vercel KV) |
| Group ID format | NIP-29: `<relay-url>'<group-id>` — use relay URL from env |
| Highlight card sharing | `/share/highlight/[nevent-bech32]` — server decodes bech32 |
| Mobile web | Fully responsive; native apps post-MVP |

---

## 7. Out of Scope (Post-MVP)

- Discover tab (public community browsing/search)
- For Later personal queue
- OCR photo capture
- AI teaser suggestion
- Synthesis sub-tab (AI connections across highlights)
- Browser extension
- Native mobile / desktop apps
- Cross-community discussion union
- Creator flywheel onboarding path
- AI co-host / weekly summaries

---

*This plan is the implementation authority for the MVP web app build. References: `product-spec-v2.0.md`, `product-surfaces-v3.md`, `client-spec-v1.0.md`, `technical-architecture.md`.*
