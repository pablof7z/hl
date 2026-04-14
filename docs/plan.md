# Highlighter — MVP Build Plan

## Overview

Highlighter is a Nostr-native social reading platform built on NIP-29 relay-based groups. Users create and join reading communities, share artifacts (articles, books, podcasts, videos), annotate them with highlights (kind:9802 / NIP-84), and discuss them in threaded comments (kind:1111 / NIP-22). The platform is structured around three navigation surfaces: **Communities** (group feed), **Discover** (public exploration), and **Me** (personal vault).

**Stack:** SvelteKit 2 + Svelte 5, TypeScript, NDK (`@nostr-dev-kit/ndk`, `@nostr-dev-kit/svelte`, `@nostr-dev-kit/sessions`), DaisyUI 5.5 + Tailwind CSS v4, `@sveltejs/adapter-vercel`. All code lives in `web/`.

**Deployment target:** `highlighter.f7z.io` via Vercel.

**Current state:** The `web/` directory contains a functional SvelteKit + NDK scaffold with existing routes (`/`, `/about`, `/bookmarks`, `/highlights`, `/note/[id]`, `/profile/[identifier]`, `/profile/edit`, `/onboarding`, `/relays`, `/relay/[hostname]`), a hand-crafted CSS design system in `web/src/app.css` (Inter + Source Serif 4 fonts, CSS custom property tokens), and NDK auth already wired through `@nostr-dev-kit/sessions`. DaisyUI and Tailwind are not yet installed.

---

## Design System Reference

The existing design tokens in `web/src/app.css` define the Highlighter visual language. DaisyUI must be mapped to these — do not override them with DaisyUI defaults.

| Token | Value | Usage |
|---|---|---|
| `--accent` | `#FF6719` | Terracotta — primary action, highlight borders, FABs |
| `--accent-hover` | `#e85d17` | Hover state for accent |
| `--canvas` | `#ffffff` | Page background |
| `--surface` | `#ffffff` | Card/panel background |
| `--surface-soft` | `#fafafa` | Subtle background variant |
| `--border` | `#eaeaea` | Default border |
| `--border-light` | `#f0f0f0` | Light border variant |
| `--text` | `#2f3437` | Body text |
| `--text-strong` | `#111111` | Headings and emphasis |
| `--muted` | `#787774` | Secondary/placeholder text |
| `--font-sans` | `Inter, -apple-system, sans-serif` | UI text |
| `--font-serif` | `Source Serif 4, Charter, Georgia, serif` | Reading content |
| `--font-mono` | `Geist Mono, SF Mono, JetBrains Mono` | Code |
| `--page-width` | `1080px` | Max page container width |
| `--content-width` | `680px` | Prose/article content width |
| `--radius-sm` | `4px` | Small radius |
| `--radius-md` | `8px` | Standard card/button radius |

**DaisyUI theme config:** Define a custom DaisyUI theme object in your Tailwind/DaisyUI config (e.g. `tailwind.config.ts` or a CSS-first `@plugin` block in `app.css` for Tailwind v4). Wire the Highlighter design tokens to DaisyUI semantic slots:

```ts
// tailwind.config.ts (if needed for DaisyUI v5 plugin config)
daisyui: {
  themes: [{
    highlighter: {
      "primary":          "#FF6719",   // --accent (terracotta)
      "primary-content":  "#ffffff",
      "base-100":         "#ffffff",   // --canvas
      "base-200":         "#fafafa",   // --surface-soft
      "base-300":         "#eaeaea",   // --border
      "base-content":     "#2f3437",   // --text
      "neutral":          "#787774",   // --muted
    }
  }]
}
```

**Note:** DaisyUI does NOT disable Tailwind Preflight — that is a Tailwind v4 concern only. If Tailwind v4 Preflight conflicts with existing `app.css` base styles, disable it via `@layer base { ... }` overrides or the Tailwind v4 `corePlugins.preflight: false` equivalent. Keep the custom property tokens in `app.css` as the single source of truth; the DaisyUI theme values above must match them.

**Typography:** Long-form reading content uses `font-serif`. UI chrome (nav, buttons, labels, captions) uses `font-sans`.

**Terracotta highlight border:** All highlight teasers use a 2px left border at `--accent` color with a small left padding — this is a recurring visual motif throughout the product.

---

## NIP-29 Implementation Notes

Highlighter is built on NIP-29 relay-based groups. The relay is **Croissant** — a fully NIP-29 compliant relay already deployed and operated by Highlighter. All relay-side enforcement (membership gating, event routing, admin actions) is handled by Croissant. Clients only need to include the correct `h` tag pointing to the group ID.

**`previous` tag policy — RESOLVED CONFLICT:** `docs/technical-architecture.md §2` states the relay enforces "at least 2 `previous` tags on group events." However, this plan was authored with the understanding that Croissant (the actual deployed relay) does not require them. **Resolution:** Include `previous` tags on all group-scoped events for safety and spec compliance, even if Croissant currently accepts events without them. The relay-side policy can relax this requirement, but clients should not rely on that leniency. Implement `previous` tag logic in the client using the last 2 event IDs seen in the group as the timeline reference. If the relay operator later confirms `previous` tags are fully optional, this can be removed as a follow-up cleanup.

### Key Event Kinds

| Kind | Protocol | Purpose |
|---|---|---|
| `39000` | NIP-29 | Group metadata (name, picture, about, access tags) |
| `39001` | NIP-29 | Group admin list |
| `39002` | NIP-29 | Group member list |
| `39003` | NIP-29 | Group roles |
| `9007` | NIP-29 | Create group (admin sends to relay) |
| `9000` | NIP-29 | Add member (admin action) |
| `9001` | NIP-29 | Remove member (admin action) |
| `9009` | NIP-29 | Generate invite code |
| `9021` | NIP-29 | Join request (user → relay) |
| `9022` | NIP-29 | Leave request (user → relay) |
| `9802` | NIP-84 | Highlight event (quote + source reference) |
| `16` | NIP-18 | Generic repost — used to share a highlight into a community |
| `1111` | NIP-22 | Comment / threaded reply |
| `30023` | NIP-23 | Long-form article (artifact type) |
| `TBD` | Custom | Artifact event (URL-based content shared to a group) |

### Group Type → NIP-29 Tag Mapping

| Highlighter Type | `restricted` | `closed` | `private` | `hidden` |
|---|---|---|---|---|
| Open + Public | ✅ | — | — | — |
| Open + Private | ✅ | — | ✅ | ✅ |
| Closed + Public | ✅ | ✅ | — | — |
| Closed + Private | ✅ | ✅ | ✅ | ✅ |

All Highlighter groups always set `restricted`. There are no open-write groups.

### Relay / Publication Matrix

Each event kind must be published to the correct relay(s):

| Event Category | Kinds | Where to Publish |
|---|---|---|
| Group management events | `39000–39003`, `9000–9022` | Highlighter relay (Croissant) only |
| Group-scoped content | Artifact kind, `1111`, `16` (with `h` tag) | Highlighter relay (Croissant) only |
| User-portable highlights | `9802` (canonical, no `h` tag) | User's write relays + Highlighter app relay as fallback |
| Bookmark lists | `10003` | User's write relays |
| User profile | `0`, `3` | User's write relays |

**Key principle:** Canonical `kind:9802` highlight events are group-neutral and should be published to the user's own relay set. A `kind:16` repost with an `h` tag is what scopes the highlight to a community and is published to the group relay. This allows one highlight to be shared to multiple communities via separate reposts, and the highlight remains in the user's personal vault independent of any community.

### Highlight Identity Model

**Canonical `kind:9802` is group-neutral.** A `kind:9802` highlight event carries no `h` tag — it belongs to the user, not any community. It is published to the user's personal relay set and represents the highlight in the user's vault regardless of where it was discovered or shared. `/me/highlights` subscribes to the user's own `kind:9802` events from their personal relays — no group filter needed.

**Group-specific sharing uses `kind:16` with `h` tag.** To surface a highlight inside a community, publish a `kind:16` repost event to the community relay with the group's `h` tag. The same `kind:9802` can be shared to multiple communities via separate `kind:16` reposts with different `h` tags.

**`/share/highlight/[id]` routing:** Do NOT key this route off the raw `kind:9802` event ID alone — the same highlight may be shared to multiple communities and that creates ambiguity. Use a composite key: `[groupId]/[highlightEventId]`, where `highlightEventId` is the `kind:9802` event id and `groupId` disambiguates community context. Alternatively, key off the `kind:16` repost event ID when community context is the intent. The server loader should: (1) fetch the `kind:16` repost to get community context and author metadata; (2) fetch the referenced `kind:9802` for highlight content; (3) resolve artifact metadata from the highlight's `a` tag.

**Private group privacy:** Canonical `kind:9802` events should not carry group-private content. The community scoping is entirely in the `kind:16` repost, so revocation of group membership does not retroactively affect the user's personal highlight events.

### Highlight Sharing Mechanic (kind:16)

When sharing a highlight to a community, a `kind:16` generic repost is published to the community relay with:
- `["e", "<highlight-event-id>", "<relay-url>"]` — reference to the original `kind:9802`
- `["k", "9802"]` — original event kind
- `["p", "<highlight-author-pubkey>"]`
- `["h", "<community-group-id>"]` — NIP-29 group routing tag

This keeps highlights portable: one `kind:9802` can be shared to multiple communities via separate `kind:16` reposts. The community relay indexes the repost; clients fetch the original highlight event separately.

### NIP-42 Relay Authentication

Private and closed groups require NIP-42 authentication on the relay connection. NDK handles this automatically when a session is active. The client must ensure `ndk.connect()` completes before subscribing to group events.

---

## Milestones

---

### Milestone 1 — Foundation

**Goal:** Configure `web/` as the primary app template. Install DaisyUI 5.5 + Tailwind CSS v4. Map existing design tokens to DaisyUI theme config. Create route stubs for all MVP screens so the routing skeleton exists before feature work begins.

**Deliverables:**
- DaisyUI 5.5 and Tailwind CSS v4 installed and configured in `web/`
- DaisyUI theme mapped to existing CSS custom property tokens (no visual regressions)
- Route stubs created for all new MVP routes (empty `+page.svelte` files with placeholder content)
- `jsrepo.config.ts` reviewed and any component dependencies noted

**New route stubs to create:**

| Route | File Path | Purpose |
|---|---|---|
| `/` | `src/routes/+page.svelte` | Reorient home → Communities list (authenticated) or Discover (guest) |
| `/community/[id]` | `src/routes/community/[id]/+page.svelte` | Community front page |
| `/community/[id]/content/[contentId]` | `src/routes/community/[id]/content/[contentId]/+page.svelte` | Artifact/content overview |
| `/community/[id]/content/[contentId]/discussion` | `src/routes/community/[id]/content/[contentId]/discussion/+page.svelte` | Threaded discussion |
| `/community/create` | `src/routes/community/create/+page.svelte` | Create community form |
| `/discover` | `src/routes/discover/+page.svelte` | Public group discovery |
| `/me` | `src/routes/me/+page.svelte` | Personal vault (My Profile, sub-tabs) |
| `/me/highlights` | `src/routes/me/highlights/+page.svelte` | All personal highlights |
| `/me/for-later` | `src/routes/me/for-later/+page.svelte` | Private personal queue |
| `/me/communities` | `src/routes/me/communities/+page.svelte` | Communities I belong to |
| `/me/recommended` | `src/routes/me/recommended/+page.svelte` | Recommendations placeholder |
| `/me/synthesis` | `src/routes/me/synthesis/+page.svelte` | AI synthesis placeholder |
| `/share/community/[id]` | `src/routes/share/community/[id]/+page.svelte` | SSR public community page |
| `/share/highlight/[groupId]/[highlightEventId]` | `src/routes/share/highlight/[groupId]/[highlightEventId]/+page.svelte` | SSR public highlight card |

**Success Criteria:**
- `cd web && npm run build` completes without error
- `npm run check` (svelte-check) passes with no type errors
- All new route stubs respond with 200 in dev mode
- No visual regression on existing pages (`/about`, `/note/[id]`, `/profile/[identifier]`)
- DaisyUI components (`btn`, `card`, `badge`, `modal`, `avatar`) render with Highlighter accent color `#FF6719`

**Dependencies:** None (first milestone).

**Complexity:** Low–Medium. DaisyUI v5 + Tailwind v4 have changed config conventions from v3 (CSS-first config, no `tailwind.config.js`). The main risk is token mapping conflicts between Tailwind v4's cascade layers and the existing hand-written `app.css`.

---

### Milestone 2 — Deployment

**Goal:** Deploy the `web/` SvelteKit app to Vercel, pointed at `highlighter.f7z.io`. Update `vercel.json` at the repo root to reflect the new build configuration. Resolve any SSR/adapter issues that surface during the first real deployment.

**Deliverables:**
- `vercel.json` updated to point to `web/` as the build root with correct build command, output directory, and framework
- `@sveltejs/adapter-vercel` confirmed compatible and generating correct serverless functions
- App live and functional at `highlighter.f7z.io`
- Environment variable `PUBLIC_NOSTR_RELAYS` set in Vercel project settings
- Static assets (`web/static/`) served correctly

**File Changes:**

| File | Action | Change |
|---|---|---|
| `vercel.json` (root) | Modify | Set `"buildCommand": "cd web && npm install && npm run build"`, `"outputDirectory": "web/.svelte-kit/output"`, `"framework": "sveltekit"` or use `"installCommand"` / `"rootDirectory"` |
| `web/svelte.config.js` | Verify | Confirm `adapter-vercel` is configured with correct output options |

**SSR seed + live-subscription hydration pattern:** Server loaders (`+page.server.ts`) fetch initial event data to seed the page with content before JS loads. On client hydration, NDK subscriptions open and stream live updates — new events are merged into the existing seed data. This prevents blank flash on load while keeping the UI live. Apply this pattern consistently across all routes that show feeds or lists: server loader seeds, client subscription updates.

**Success Criteria:**
- Vercel deployment succeeds on `main` branch push
- `https://highlighter.f7z.io` loads the home page
- SSR pages (public routes) render correctly without JS
- No 500 errors on cold-start serverless functions
- NDK relay connection established on client hydration

**Dependencies:** Milestone 1 (build must succeed locally first).

**Complexity:** Low–Medium. The current `vercel.json` points to `outputDirectory: "public"` which is the old static site — this will need correction. The SvelteKit Vercel adapter auto-generates the function config but the root-level `vercel.json` must correctly reference the `web/` subdirectory.

---

### Milestone 3 — Auth + Identity

**Goal:** Ensure NIP-07 (browser extension) and NIP-46 (remote signing / Nostr Connect) login flows are fully functional, user-facing, and visually polished. Set up NDK sessions so authenticated user state (profile, follows, relay list) is available reactively throughout the app.

**Deliverables:**
- Login modal/panel supporting both NIP-07 and NIP-46 entry points
- NIP-46 QR code display for remote signer pairing (using existing `qrcode` dep)
- NDK session persisted in `LocalStorage` (already configured in `web/src/lib/ndk/client.ts`) and restored on page load
- `$currentUser` reactive state available in all routes via Svelte context
- User profile metadata (kind:0) fetched and displayed in nav and Me page header
- Sign-out action that clears the NDK session
- Auth guard for protected routes (`/me/**`, `/community/create`) — see note below on implementation approach

**Key Files:**
- `web/src/lib/ndk/client.ts` — NDK instance and `ensureClientNdk()` (already exists; review session config for NIP-46)
- `web/src/lib/features/auth/auth.ts` — auth state and session management (extend as needed)
- `web/src/lib/features/auth/AuthPanel.svelte` — login UI in topbar (already exists; polish and complete)
- `web/src/lib/features/auth/AuthModal.svelte` — create if login needs a full modal flow
- `web/src/routes/(protected)/+layout.svelte` — SvelteKit route-group layout for auth guard. **Do not use a wrapper component `AuthGuard.svelte` for this** — wrapping components create a redirect flash because the page renders first and then the guard triggers a navigation. Instead, use a `(protected)` route group with a `+layout.svelte` that checks auth state and redirects synchronously on the server or before paint. Place all protected routes (`/me/**`, `/community/create`) inside this route group.

**NDK session config note:** `@nostr-dev-kit/sessions` `LocalStorage` is already set up with key `'ndk-sveltekit-template:sessions'` in `client.ts`. Update the key to `'highlighter:sessions'` before launch to avoid stale session conflicts from the template.

**Success Criteria:**
- NIP-07 login works in browsers with nos2x or Alby installed
- NIP-46 login shows QR code and pairing link; session established after remote signer approval
- Page refresh restores logged-in state without re-authentication
- Protected routes redirect to login when unauthenticated
- User avatar and display name appear in the topbar after login
- Sign-out clears session and returns user to guest state

**Dependencies:** Milestone 1 (routes exist), Milestone 2 (optional — auth can be developed locally without deployment).

**Complexity:** Low. NDK and `@nostr-dev-kit/sessions` already handle the heavy lifting. The main work is UI polish, NIP-46 QR display, and the AuthGuard component.

---

### Milestone 4 — NIP-29 Groups

**Goal:** Fully implement NIP-29 group features: browse groups, view a group's front page, create a new group, join/leave, and manage membership. This is the core of the product.

**Deliverables:**
- **Group list** on `/` (authenticated) showing communities the user belongs to, with kind:39000 metadata
- **Community front page** at `/community/[id]` showing group header (name, picture, about, member count), artifact list, and member panel
- **Create community flow** at `/community/create`: name, description, cover image, access type (Open/Closed), visibility (Public/Private) → publishes kind:9007 → relay creates kind:39000
- **Join flow**: For open groups, publish kind:9021 with `h` tag. For closed groups, entry via invite code (kind:9009 code in the kind:9021 `code` tag)
- **Leave action**: Publish kind:9022
- **Admin panel** (for group admins): Add members (kind:9000), remove members (kind:9001), edit group metadata (kind:9002)
- **Relay config**: NDK instance connects to the Highlighter relay for group subscriptions; `h` tag filtering used on all group-scoped subscriptions

**Key Event Flows:**
- Group creation: `kind:9007` → relay issues `kind:39000`
- Open join: `kind:9021` with `["h", "<groupId>"]` → relay updates `kind:39002`
- Closed join (invite): `kind:9021` with `["h", "<groupId>"]` + `["code", "<inviteCode>"]`
- Admin add: `kind:9000` with `["p", "<userPubkey>"]` + `["h", "<groupId>"]`
- Remove member: `kind:9001` with `["p", "<userPubkey>"]` + `["h", "<groupId>"]`
- Leave: `kind:9022` with `["h", "<groupId>"]`

**Key Files:**
- `web/src/routes/community/[id]/+page.svelte` — group front page
- `web/src/routes/community/[id]/+page.server.ts` — SSR: fetch kind:39000 metadata server-side for SEO
- `web/src/routes/community/create/+page.svelte` — create group form
- `web/src/lib/ndk/groups.ts` — NDK subscriptions, group membership parsing, and event publishing helpers (**not** in the feature dir — Nostr/NDK concerns belong in `src/lib/ndk/`, not `src/lib/features/`)
- `web/src/lib/features/groups/` — UI components only (no NDK logic)
  - `GroupCard.svelte` — group preview card (cover, name, member count, activity)
  - `GroupHeader.svelte` — group front page header
  - `JoinButton.svelte` — join/leave toggle with loading state
  - `CreateGroupForm.svelte` — multi-step group creation

**Group Creation Flow (kind:9007 → kind:9002):** NIP-29 defines `kind:9007` as the create-group event. Metadata fields on the `9007` event itself may be relay-specific and ignored by some relays. Model the create flow as a two-step publish: (1) publish `kind:9007` to create the group — relay responds with `kind:39000`; (2) immediately publish `kind:9002` (edit-metadata) with name, about, and picture tags. This is more robust and avoids dependence on relay-specific handling of creation metadata.

**Member count / membership logic:** `kind:39002` (member list) may be absent, partial, or access-restricted by the relay. Do not treat `39002` as the sole source of truth for member counts. Implement fallback counting: tally unique pubkeys from `kind:9000` (add-member), subtract `kind:9001` (remove-member), and account for join/leave via `kind:9021`/`kind:9022`. The `/community/[id]` SSR loader should display best-available count.

**SSR scope for `/community/[id]`:** The server loader (`+page.server.ts`) must fetch **only public-safe metadata** from `kind:39000` (name, picture, about, member count estimate). Do not attempt to SSR authenticated content (artifacts, highlights, discussions) — Vercel serverless functions cannot access browser session state or perform NIP-42-authenticated relay connections. Protected content hydrates client-side after NDK auth completes.

**Success Criteria:**
- Authenticated user can see their group list on home page
- Group front page SSR renders name, picture, about, and member count from kind:39000; protected content loads client-side
- Group creation publishes kind:9007 then kind:9002; relay responds with kind:39000; user is redirected to the new group page
- Open group join publishes kind:9021 and updates membership state
- Closed group join flow accepts invite code
- Admin can add/remove members
- All group subscriptions use `h` tag filter and connect to the correct relay
- Member count falls back to join/leave event tally if kind:39002 is unavailable

**Dependencies:** Milestone 3 (auth required to publish events).

**Complexity:** High. NIP-29 has many interacting event kinds and relay-side membership enforcement. The relay (Croissant) handles all enforcement — client only needs correct `h` tags and event kinds. NDK's group support should be verified against Croissant's implementation. Include `previous` tags on all group-scoped events (see NIP-29 Implementation Notes).

---

### Milestone 5 — Artifacts

**Goal:** Allow group members to share artifacts (external content: articles, books, podcasts, videos) into communities. Display artifact cards on the community front page with metadata (title, author, source type, cover image, URL).

**Deliverables:**
- **Share artifact flow**: URL input → metadata fetch (title, image, author via server-side URL scrape) → publish artifact event with `h` tag to group relay
- **Artifact card component**: Hero image, title, author, source type badge, highlight count, discussion count
- **Artifact overview page** at `/community/[id]/content/[contentId]`: Full metadata, all highlights from group members, discussion entry point, "Save to For Later" button
- **Artifact event kind**: Use the custom kind defined in `docs/technical-architecture.md §4` — tags: `h`, `d` (dedup by URL hash), `title`, `author`, `source`, `url`, `image`
- **Artifact addressability**: Artifacts are **addressable events** using the `d` tag for deduplication. They are referenced via `a` tag in the format `<kind>:<pubkey>:<d-tag>`. The route parameter `contentId` MUST be the `d` tag value (URL hash), not the raw event id. This is required for M7 comment tags (`A`/`a` references) and M6 highlight `a` tags to work correctly. When fetching an artifact for `/community/[id]/content/[contentId]`, query by `d` tag filter, not event id.
- **Server-side URL metadata scrape** at `POST /api/artifacts/preview`: Uses `sharp` (already in deps) for image processing; scrapes OG tags for title/description/image

**Key Files:**
- `web/src/routes/community/[id]/content/[contentId]/+page.svelte` — artifact overview
- `web/src/routes/community/[id]/content/[contentId]/+page.server.ts` — SSR artifact metadata fetch
- `web/src/routes/api/artifacts/preview/+server.ts` — URL metadata scraping endpoint
- `web/src/lib/features/artifacts/` — new feature directory
  - `ArtifactCard.svelte` — community front page card (hero, metadata, highlight teaser, discussion count)
  - `ArtifactForm.svelte` — share artifact form (URL input + metadata preview)
  - `artifact.ts` — event construction helpers for artifact kind

**Success Criteria:**
- Authenticated member can paste a URL, see scraped metadata preview, and publish an artifact event
- Artifact cards appear on the community front page
- Artifact overview page shows full metadata and highlight count
- Duplicate artifact deduplication: if a URL has been shared before in the same group (matched by `d` tag URL hash), the existing artifact is linked, not duplicated
- Source type badge displays correctly (book / article / podcast / video / paper / web)

**Dependencies:** Milestone 4 (group routing and membership in place).

**Complexity:** Medium. The custom artifact event kind requires coordination with the relay to ensure it accepts and indexes the kind. URL scraping needs server-side handling to avoid CORS issues on the client.

---

### Milestone 6 — Highlights

**Goal:** Implement highlight creation (text selection from artifacts), personal highlight management, and sharing highlights to communities via the kind:16 repost mechanic.

**Deliverables:**
- **Highlight creation**: Text selection on an artifact's content → highlight popover → publish `kind:9802` (NIP-84) with `quote` tag (selected text), `context` tag (surrounding text), and `a`/`e` reference to source artifact
- **Share to community**: Publish `kind:16` repost with `["h", "<groupId>"]` referencing the `kind:9802` event ID
- **Highlight display**: Highlight cards with terracotta 2px left border and `WHAT CAUGHT OUR EYE` label in community artifact views
- **Cross-community share**: UI to pick a different community for sharing an existing highlight (publishes new `kind:16` with different `h` tag)
- **Bookmark / save**: Private "For Later" queue using **local-only storage** (browser IndexedDB or `localStorage`) for MVP simplicity. This avoids publishing any event to a relay. For users who want cross-device sync, NIP-51 with NIP-44 encryption (`kind:10003` with encrypted content) is the correct Nostr-native approach — but this is out of scope for MVP. `kind:10003` is NOT an "encrypted DM" kind — it is a NIP-51 categorized bookmark list and uses NIP-44 encryption if private. Pick one approach and stick to it; do not describe `kind:10003` as encrypted DMs.

**Kind:9802 event structure (NIP-84):**
```
kind: 9802
content: "<selected text quote>"
tags:
  ["a", "<artifact-kind>:<pubkey>:<d-tag>"]   // source artifact reference (addressable)
  ["context", "<surrounding text>"]            // optional surrounding text
  ["r", "<source-url>"]                        // source URL for non-Nostr / external artifacts
```

**Important:** Canonical `kind:9802` carries **no `h` tag**. It is group-neutral and published to the user's own relay set. The `h` tag appears only on the `kind:16` repost that scopes the highlight to a community (see Highlight Identity Model in NIP-29 Implementation Notes). The `["r", "<source-url>"]` tag is required for external (non-Nostr) artifact sources so the highlight can stand alone without the artifact event for clients that resolve highlights independently.

**Key Files:**
- `web/src/lib/features/highlights/` — new feature directory
  - `HighlightCard.svelte` — display component (terracotta left border, quote text, author, source)
  - `HighlightForm.svelte` — creation flow (text input / paste selection, preview)
  - `highlight.ts` — event construction for kind:9802 and kind:16 repost
- `web/src/routes/me/highlights/+page.svelte` — personal highlights list (all kind:9802 by current user)

**Success Criteria:**
- User can create a highlight (text input) and publish kind:9802
- Highlights appear on artifact overview page grouped by position in source content
- Sharing a highlight to a community publishes kind:16 and it appears in the group feed
- One highlight can be shared to multiple communities (multiple kind:16 events)
- Highlight cards render with correct terracotta border and label
- Personal highlights page at `/me/highlights` lists all user's kind:9802 events

**Dependencies:** Milestone 5 (artifacts must exist as reference targets for highlights).

**Complexity:** Medium. NIP-84 (kind:9802) is well-defined. The kind:16 repost mechanic is the novel piece — ensure the community relay accepts kind:16 with `h` tag and indexes it correctly alongside native group events.

---

### Milestone 7 — Discussions

**Goal:** Implement threaded comments on artifacts and highlights using kind:1111 (NIP-22). Both artifact-level discussion and highlight-level discussion must work.

**Deliverables:**
- **Discussion page** at `/community/[id]/content/[contentId]/discussion`: Root-level comments (kind:1111 with uppercase `A` tag as root scope), reply threads (kind:1111 with lowercase `e` for the parent comment and unchanged uppercase `A`/`K` for root scope)
- **Comment composer**: Text input with `@mention` support (NIP-27 style mentions in content), publish kind:1111 with correct NIP-22 tags — uppercase for root scope, lowercase for parent item (see tag table below)
- **Highlight-level comments**: Tapping a highlight opens an inline or modal thread; the highlight itself is the root — root comment uses uppercase `E` referencing the highlight event
- **Reply nesting**: Two levels rendered visually (root → reply). Deep nesting collapsed behind "View thread" link.
- **Reactions**: kind:7 (NIP-25) emoji reactions on comments and highlights

**Kind:1111 tags (NIP-22) — critical semantics:**
- **Uppercase** tags (`A`, `E`, `I`, `K`, `P`) = **root scope** — identify the root of the entire thread
- **Lowercase** tags (`a`, `e`, `i`, `k`, `p`) = **parent item** — identify what this specific comment is directly replying to
- Root tags stay the same for all comments in a thread; parent tags change per reply level

*Root-level comment on artifact (artifact is root):*
```
["A", "<artifact-addr>"]      // root addressable event — uppercase = root scope
["K", "<artifact-kind>"]      // root event kind — uppercase = root scope
["h", "<groupId>"]            // NIP-29 group routing
```
*(No lowercase tags — there is no "parent" other than the root artifact itself)*

*Reply to a comment on an artifact (artifact is still root, comment is parent):*
```
["A", "<artifact-addr>"]      // root scope is still the artifact — uppercase
["K", "<artifact-kind>"]      // root kind — uppercase
["e", "<parent-comment-id>"]  // directly replying to this comment — lowercase = parent item
["p", "<parent-author>"]      // parent author — lowercase = parent scope
["k", "1111"]                 // parent kind — lowercase
["h", "<groupId>"]
```
*Note: no uppercase `E` here because the root is already captured by uppercase `A`. Uppercase `E` is only used when the root is a non-addressable event (regular event id, not an `a`-tag address).*

*Root-level comment on a highlight (highlight is root — non-addressable event):*
```
["E", "<highlight-event-id>"] // root event — uppercase E = root scope (highlight is root)
["K", "9802"]                 // root event kind — uppercase
["h", "<groupId>"]
```

*Reply to a comment on a highlight (highlight stays root, comment is parent):*
```
["E", "<highlight-event-id>"] // root event stays the highlight — uppercase (unchanged)
["K", "9802"]                 // root kind — uppercase (unchanged)
["e", "<parent-comment-id>"]  // directly replying to this comment — lowercase = parent item
["p", "<parent-author>"]      // parent author — lowercase = parent scope
["k", "1111"]                 // parent kind — lowercase
["h", "<groupId>"]
```

**Key Files:**
- `web/src/routes/community/[id]/content/[contentId]/discussion/+page.svelte` — discussion thread page
- `web/src/lib/features/discussions/` — new feature directory
  - `CommentThread.svelte` — recursive thread renderer
  - `CommentCard.svelte` — single comment display (avatar, name, timestamp, content, reactions, reply button)
  - `CommentComposer.svelte` — reply input with mention support
  - `discussion.ts` — event construction for kind:1111

**Success Criteria:**
- Root-level comments appear on discussion page, sorted by timestamp
- Replies nest visually under their parent comment
- Posting a comment publishes kind:1111 and appears in the thread immediately (optimistic update)
- Highlight-level threads are accessible by tapping a highlight card
- Comment count on artifact cards updates to reflect discussion activity
- Reactions (kind:7) can be added to comments and highlights

**Dependencies:** Milestone 5 (artifacts), Milestone 6 (highlights — for highlight-level discussion).

**Complexity:** Medium. NIP-22 threading is well-defined but rendering recursive threads cleanly in Svelte requires a recursive component or tree-building utility. The main complexity is keeping the uppercase/lowercase tag distinction correct across all discussion contexts — uppercase means root scope, lowercase means parent/traversal. Build a `buildCommentTags(context)` helper in `discussion.ts` that accepts the root event and parent event and returns the correct NIP-22 tag set, so comment construction logic is not scattered across components.

---

### Milestone 8 — Personal Vault (Me Page)

**Goal:** Build the `/me` route family — the user's personal profile and vault. Implements five sub-tabs: Highlights, For Later, Communities, Recommended, and a placeholder Synthesis tab.

**Deliverables:**
- **`/me`** — Profile header (avatar, display name, NIP-05, bio, stats: highlight count, community count) + sub-tab navigation
- **`/me/highlights`** — All kind:9802 events authored by the user, across all communities. Sorted newest-first. Full content card with artifact source context.
- **`/me/for-later`** — Private queue of saved-but-not-yet-shared artifacts. Items stored in **local-only browser storage** (IndexedDB via a simple wrapper) for MVP. Each card shows: hero image, title, source, save date, status pill (Ready to share / Needs teaser / Already in N communities), quick actions (Add teaser, Move to community, Remove). Cross-device sync via NIP-51 (`kind:10003` encrypted with NIP-44) is a post-MVP enhancement.
- **`/me/communities`** — List of groups the user belongs to (kind:39002 membership + kind:39000 metadata). Links to `/community/[id]`.
- **`/me/recommended`** — Placeholder: "Recommendations coming soon" — seeded with discovery suggestions based on community memberships (can use Discover data from M4).
- **`/me/synthesis`** — Placeholder: "Your reading synthesis is coming soon" — reserved for future AI feature.

**Status pills for For Later cards:**
- `Ready to share` — muted green `#8A9A7F`
- `Needs teaser` — neutral (--muted color)
- `Already in N communities` — blue (--pale-blue-text `#1f6c9f`)

**Key Files:**
- `web/src/routes/me/+layout.svelte` — profile header + sub-tab nav shared across all `/me/**` routes
- `web/src/routes/me/+page.svelte` — redirect to `/me/highlights` or default sub-tab
- `web/src/routes/me/highlights/+page.svelte` — personal highlights feed
- `web/src/routes/me/for-later/+page.svelte` — For Later queue
- `web/src/routes/me/communities/+page.svelte` — my communities list
- `web/src/routes/me/recommended/+page.svelte` — placeholder
- `web/src/routes/me/synthesis/+page.svelte` — placeholder
- `web/src/lib/features/vault/` — new feature directory
  - `ForLaterCard.svelte` — queue item card with status pill and quick actions
  - `StatusPill.svelte` — reusable status pill component
  - `vault.ts` — For Later storage helpers (private bookmark events)

**Success Criteria:**
- `/me` redirects to `/me/highlights` for authenticated users, to login for guests
- Profile header renders user avatar, name, and NIP-05 from kind:0 metadata
- Highlights sub-tab lists all user's kind:9802 events with correct source attribution
- For Later sub-tab shows private queue items with status pills
- Communities sub-tab lists group memberships with group cards linking to community pages
- Sub-tab navigation is keyboard-accessible and renders correct active state

**Dependencies:** Milestone 3 (auth/identity for user profile data), Milestone 6 (highlights for the highlights sub-tab).

**Complexity:** Medium. The profile header and sub-tab layout are straightforward. The For Later storage approach is decided: local-only IndexedDB for MVP. The NIP-51 path (kind:10003 with NIP-44 encryption) is documented but deferred post-MVP.

---

### Milestone 9 — Public / Share Pages

**Goal:** Develop SSR-rendered public pages for communities and highlights. These pages are crawlable by search engines and shareable on social media — they work without JavaScript and include Open Graph meta tags.

**Deliverables:**
- **`/share/community/[id]`** — Public community page: group metadata (kind:39000), member count, sample of recent public highlights. Includes OG tags: title = group name, description = group about, image = group picture. CTA: "Join this community on Highlighter."
- **`/share/highlight/[groupId]/[highlightEventId]`** — Public highlight card: the highlight quote (kind:9802), source artifact metadata, author info, community context. Uses composite key to disambiguate the same highlight shared to multiple communities (see Highlight Identity Model in NIP-29 Implementation Notes). Designed to be visually compelling for sharing on Twitter/social. OG image generated server-side (extends existing `web/src/routes/og/` pattern). CTA: "Read the discussion on Highlighter."
- **OG image generation** for both share pages via server-rendered canvas or SVG → PNG (using existing `sharp` dependency and the pattern in `web/src/routes/og/note/[id]/+server.ts`)
- **Canonical URLs** and `noindex` decisions: public community pages are indexable; private/closed community share pages return 404 or redirect to login

**Key Files:**
- `web/src/routes/share/community/[id]/+page.svelte` — public community page
- `web/src/routes/share/community/[id]/+page.server.ts` — SSR: fetch kind:39000; check `private` tag, return 404 if private
- `web/src/routes/share/highlight/[groupId]/[highlightEventId]/+page.svelte` — public highlight card
- `web/src/routes/share/highlight/[groupId]/[highlightEventId]/+page.server.ts` — SSR: fetch kind:16 repost for community context, fetch referenced kind:9802, resolve artifact metadata
- `web/src/routes/og/community/[id]/+server.ts` — OG image for community (follows pattern of `/og/note/[id]`)
- `web/src/routes/og/highlight/[id]/+server.ts` — OG image for highlight card
- `web/src/lib/seo.ts` — extend `SeoMetadata` type if needed for new OG fields

**Success Criteria:**
- `/share/community/[id]` renders with full community metadata and is readable without JS
- `/share/highlight/[id]` renders the highlight quote and source with no JS dependency
- OG tags (`og:title`, `og:description`, `og:image`) are present in the HTML `<head>` for both pages
- Private community share pages return 404 (or redirect) instead of leaking metadata
- Share pages link back to the app with correct deep link paths
- Lighthouse SEO score ≥ 90 on share pages

**Dependencies:** Milestone 4 (group metadata), Milestone 6 (highlight events).

**Complexity:** Medium. SSR with NDK on the server requires the server-side `ndk` instance (already in `web/src/lib/server/nostr.ts`). OG image generation follows the existing pattern and is low risk. The main consideration is graceful handling of missing or private events.

---

### Milestone 10 — Polish + Launch Prep

**Goal:** QA pass, responsive design audit, performance review, and final deployment configuration for launch.

**Deliverables:**
- **Responsive design audit**: All routes tested at mobile (375px), tablet (768px), and desktop (1080px) breakpoints. Navigation collapses to mobile layout. No horizontal overflow on any screen.
- **Loading and empty states**: Every list/feed has a skeleton loader and an empty state with illustration and contextual CTA. No "flash of nothing" on slow connections.
- **Error states**: Network errors, relay disconnects, and failed event publishes surface user-facing messages. Failed NIP-46 pairing shows retry option.
- **Accessibility pass**: Keyboard navigation, focus management, and ARIA labels on interactive elements (buttons, modals, forms).
- **Performance**: NDK subscription cleanup on route destroy (no leaked subscriptions). Bundle size review — chunking in `vite.config.ts` already set up for NDK packages.
- **Final Vercel config**: Set all required env vars (`PUBLIC_NOSTR_RELAYS`, `HIGHLIGHTER_RELAY_URL`). Configure custom domain `highlighter.f7z.io`. Set up preview deployments for PRs.
- **Analytics**: Add minimal, privacy-respecting page view tracking (or confirm none needed for MVP).
- **Final `AGENTS.md` update**: Update `web/src/routes/AGENTS.md` with the completed route map.

**Success Criteria:**
- All 10 milestone features pass manual QA on Chrome (desktop), Firefox (desktop), and Safari (mobile)
- No console errors in production build
- `npm run check` passes with zero type errors
- Vercel deployment succeeds on `main` push with green build
- `highlighter.f7z.io` loads under 3s on a throttled 4G connection (Lighthouse)
- Login → join group → share artifact → add highlight → comment flow completes end-to-end

**Dependencies:** All previous milestones.

**Complexity:** Medium. Polish work is often underestimated. Budget extra effort for responsive edge cases in the community front page layout (multi-column card grid collapsing to single column) and the highlight creation flow on mobile.

---

## Out-of-Scope for MVP

The following features are documented in the specs but explicitly excluded from this build plan. They are noted here to prevent scope creep and to ensure route stubs are future-proofed for them.

| Feature | Why Deferred |
|---|---|
| **Photo / OCR content capture** | Requires mobile camera API and on-device or cloud OCR — mobile-first feature |
| **AI teaser suggestions** | Requires LLM API integration and content processing pipeline |
| **AI Synthesis tab** (`/me/synthesis`) | Placeholder only in MVP — complex AI feature |
| **Recommended tab** (`/me/recommended`) | Placeholder only in MVP — requires recommendation algorithm |
| **Browser extension** | Separate codebase; companion to the webapp for highlight capture from web pages |
| **Mobile apps** (Android/iOS) | Separate Rust + native UI codebase |
| **Desktop app** | Separate Tauri/native codebase |
| **Relay moderation tools** | Admin-side relay management UI (ban, rate limit, etc.) |
| **Direct Messages** (NIP-17) | Not part of community reading flow |
| **Zaps / Lightning payments** | Not required for MVP reading community experience |
| **Group roles beyond admin/member** | kind:39003 roles exist in NIP-29 but MVP only needs admin/member distinction |
| **Invite code management UI** | Admin can generate kind:9009 invite codes; a full management UI is post-MVP |
| **Analytics dashboard** | Per-community analytics (top highlights, engagement metrics) |
| **Search** | Full-text search across groups, artifacts, and highlights |

---

## Execution Order

Milestones must be executed in dependency order:

```
M1 Foundation
  └─→ M2 Deployment
  └─→ M3 Auth + Identity
        └─→ M4 NIP-29 Groups
              └─→ M5 Artifacts
                    └─→ M6 Highlights
                          └─→ M7 Discussions
                    └─→ M8 Personal Vault    (also depends on M3, M6)
              └─→ M9 Public/Share Pages      (also depends on M6)
M10 Polish + Launch Prep  (depends on all above)
```

M2 (Deployment) and M3 (Auth) can run in parallel after M1. M5 and M8 can partially overlap once M4 is complete. M9 can be developed in parallel with M7 once M4 and M6 are done.

---

## Verification

The complete implementation is verified when:

```bash
cd web
npm run check          # zero TypeScript / Svelte type errors
npm run build          # clean production build, no warnings
npm run preview        # production build serves locally on :4173
```

**End-to-end smoke test:**
1. Visit `highlighter.f7z.io` — home page loads, guest sees Discover or community list
2. Login with NIP-07 (browser extension) — profile avatar appears in nav
3. Create a community — kind:9007 published, redirected to new group page
4. Share an artifact (paste URL) — artifact card appears on community front page
5. Add a highlight to the artifact — kind:9802 published, highlight card visible
6. Share the highlight to the community — kind:16 published, appears in group feed
7. Comment on the artifact — kind:1111 published, comment appears in discussion thread
8. Visit `/me/highlights` — personal highlight list shows the created highlight
9. Visit `/share/community/[id]` — public page renders without JavaScript
10. Visit `/share/highlight/[id]` — public highlight card renders with OG tags
