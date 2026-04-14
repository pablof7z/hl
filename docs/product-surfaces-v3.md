# Product Surfaces Specification: Highlighter
## Version 3.0 | April 2026

---

## 0. Decisions Log

| Decision | Choice | Rationale |
|---|---|---|
| Relay | Custom fork of croissant | Full NIP-29 relay with web UI, Bleve search, Blossom, LiveKit, presence verification. We fork and add Highlighter-specific policies. |
| Event kinds | NIP-84 + NIP-73 (no custom kinds) | Highlights use NIP-84 (`kind:9802`). Artifacts identified via NIP-73 entity tagging. No custom kinds needed. |
| Webapp stack | ndk-template-sveltekit-vercel (SvelteKit + NDK + Vercel) | Massive head start: SSR, auth, SEO, onboarding, NDK primitives all pre-built |
| Webapp priority | Build first | Template gives a working starting point; validates the product fastest |
| Mobile approach | Rust core + native Swift/Kotlin UIs | True native feel, shared protocol/data layer, platform-native UX |
| Desktop | In scope for launch | Same Rust core as mobile, native desktop UI (macOS/Windows/Linux) |
| Relay hosting | Highlighter-operated default relay | Full control over UX; protocol remains open for third-party relays |

---

## 1. Surface Overview

Highlighter ships as **four client/application surfaces** plus a **relay backend**:

```
┌─────────────────────────────────────────────────────────────────┐
│                     HIGHLIGHTER RELAY                            │
│        Custom fork of croissant (khatru, Go)                    │
│                                                                  │
│   NIP-29 groups · Bleve search · Blossom media · LiveKit       │
│   Highlighter event kinds · Custom policies · Web UI           │
└──────────────────────────┬──────────────────────────────────────┘
                           │  WebSocket (NIP-01/29/42)
          ┌────────────────┼────────────────┐
          │                │                │
    ┌─────▼─────┐   ┌─────▼─────┐   ┌─────▼──────┐
    │  WEB APP   │   │  MOBILE   │   │  DESKTOP   │
    │ SvelteKit  │   │ Rust+Nat  │   │ Rust+Nat   │
    │  + NDK     │   │ iOS/And   │   │  macOS/     │
    │  Vercel    │   │           │   │  Win/Linux  │
    └────────────┘   └───────────┘   └────────────┘
```

### Build Priority

| Priority | Surface | Why |
|---|---|---|
| **1 — Relay** | Relay backend | All clients depend on it; must exist first |
| **2 — Webapp** | SvelteKit + NDK | Fastest path to working product; template head start; validates core flows |
| **3 — Mobile** | Rust + native | Second surface; shares protocol logic with desktop via Rust core |
| **4 — Desktop** | Rust + native | Third surface; same Rust core, native desktop UI |

---

## 2. Relay — Custom Fork of Croissant

### Starting Point

- **Base:** [croissant](https://github.com/fiatjaf/croissant) (by fiatjaf, same author as khatru)
- **Language:** Go
- **Approach:** Fork croissant, extend with Highlighter-specific logic

**Why croissant instead of relay29:** Croissant is a complete, production-grade NIP-29 relay — not just a library like relay29, but a running server with web UI, storage, search, media uploads, and more. relay29 provides group management primitives; croissant provides an entire deployable relay. Starting from croissant saves us from reimplementing storage, search, admin UI, presence verification, and media handling.

### What Croissant Already Provides

| Feature | Details |
|---|---|
| **Full NIP-29 groups** | Group creation, membership (join/leave/invite), moderation (admin/moderator roles), closed/restricted/private group enforcement |
| **Web UI** | Built-in templ-based web pages for group home, settings, and admin (`home.templ`, `group.templ`, `layout.templ`) |
| **Full-text search** | Bleve-based per-group search with automatic language detection and stemming (40+ languages) |
| **Blossom media uploads** | NIP-96/Blossom support for file uploads — local filesystem or S3-compatible storage (Minio, Backblaze B2, etc.) |
| **LiveKit voice/video** | Optional LiveKit integration for real-time audio/video in groups |
| **Presence verification** | Rate-limited group creation requiring presence on configured relays; spam-resistant free transit |
| **MMM storage** | Multi-mmap event store (`fiatjaf.com/nostr/eventstore/mmm`) for efficient event persistence |
| **NIP-42 auth** | Authentication for private/restricted groups |
| **Admin settings UI** | Web-based settings management for relay owner (name, description, contact, Blossom config, rate limits) |
| **Owner controls** | Group creation rate limiting, presence relay configuration, group deletion |
| **Group search** | Per-group Bleve search indexes with language-aware analysis |
| **Invite codes** | Full `kind:9009` invite code system for closed groups |
| **Kick/ban** | Member removal with self-removal tracking |
| **Metadata protection** | Private group metadata hidden from non-members; closed groups enforce invite codes |

### Architecture (from code review)

```
croissant/
├── main.go              → Relay init, HTTP server, khatru setup
├── state.go             → GroupsState struct: in-memory group map, member tracking
├── group.go             → Group model, DB loading, metadata sync, search indexing
├── process_event.go     → Event processing: moderation actions, join/leave, group creation
├── reject_event.go      → Event rejection: auth, membership, rate limits, kind filtering
├── query.go             → Query handling for subscriptions
├── search.go            → Bleve full-text search per group (language detection, indexing)
├── store.go             → MMM storage initialization
├── presence.go          → Presence verification via external relays (LRU cache)
├── blossom.go           → Blossom/NIP-96 media upload support
├── livekit.go           → LiveKit voice/video integration
├── global/              → Config, env, auth, logging, rate limits, settings
│   ├── settings.go      → JSON settings: relay config, Blossom, LiveKit, rate limits
│   ├── env.go           → Environment vars (PORT, HOST, DATAPATH, OWNER_PUBLIC_KEY)
│   ├── auth.go          → NIP-42 auth helpers
│   └── rate_limits.go   → Rate limiting middleware
├── group.templ          → HTML template for group pages
├── home.templ           → HTML template for relay home
└── static/              → CSS, assets
```

### Highlighter Extensions (Custom Fork)

These are what we add on top of croissant:

#### No Custom Event Kinds — We Use Existing NIPs

Highlighter does **not** define custom event kinds. Instead, we use established Nostr protocols:

| Concept | NIP | Kind | How It Works |
|---|---|---|---|
| **Highlight** | NIP-84 | `9802` | Standard highlight/annotation event. Contains the excerpt text, source reference, and context. Already a defined kind. |
| **Artifact** | NIP-73 | N/A (no dedicated kind) | External content is identified through NIP-73 entity tagging. The artifact is not a separate event — it's a reference using NIP-73 tags (`r`, `i`, etc.) on any event type (e.g., a `kind:1` note with NIP-73 tags describing the external content). The entity type (book, article, podcast, video) is determined by the NIP-73 tag classification. |

**Why no custom kinds:** The domain logic lives in the **tags**, not the kind number. This means:
- Croissant doesn't need custom event kind additions in its codebase — it just accepts standard kinds within groups
- Any Nostr client that understands NIP-84 and NIP-73 can render Highlighter content
- We stay interoperable with the broader Nostr ecosystem from day one
- The relay validates tag structure (NIP-73 tags present for artifact references, NIP-84 format for highlights), not kind numbers

#### Custom Policies (Additions to `reject_event.go`)

| Policy | Description |
|---|---|
| **Artifact validation** | Verify artifact events have required tags (`d`, `title`, `source`, at least one of `url` or manual entry fields). Reject malformed artifacts. |
| **Highlight validation** | Verify highlight events reference a valid artifact within the same group (`a` tag points to an artifact event coordinate). Reject orphan highlights. |
| **Content type enforcement** | Extend croissant's `SupportedKinds` to include Highlighter custom kinds. Groups can optionally restrict to specific content types. |
| **Late publication window** | Reject events with timestamps older than 1 hour (anti-replay, in addition to croissant's existing moderation timestamp check). |

#### Custom Features (Additions to `process_event.go`)

| Feature | Description |
|---|---|
| **NIP-84 highlight indexing** | When a `kind:9802` event is saved to a group, index it for per-artifact highlight counts and sorting. |
| **NIP-73 artifact tracking** | When events with NIP-73 entity tags are saved to a group, track them as artifacts for the group's library view. The entity type (book, article, podcast, video) is read from the NIP-73 tag classification. |
| **Activity metrics** | Track per-group activity (event counts, active members) for discovery and ranking. |

#### Web UI Customization (Replacing croissant's templ pages)

Croissant ships with basic HTML pages. We replace these with Highlighter-branded versions:

| Page | Current (croissant) | Highlighter Version |
|---|---|---|
| Group home | `group.templ` — basic message list | Rich group home: featured artifacts, highlight spotlight, library, activity |
| Relay home | `home.templ` — group listing | Highlighter landing: featured groups, trending highlights, discover |
| Settings | Admin-only form | Extended: group creation, moderation dashboard |
| **New: Artifact page** | — | Artifact detail with highlights and discussion |
| **New: Highlight page** | — | Shareable highlight card (SEO-optimized, OG image) |

**Note on web UI vs webapp:** The relay's web UI serves *public-facing* pages (SEO-optimized, shareable links, relay home). The main webapp (SvelteKit) handles *authenticated* user interactions. The relay's templ pages handle unauthenticated visitors and crawlers — they're a complement to the SvelteKit app, not a replacement.

### Relay Configuration

Key settings (extending croissant's `settings.json`):

```json
{
  "relay_name": "Highlighter",
  "relay_description": "Nostr-native communities for readers, thinkers, and learners",
  "relay_icon": "https://highlighter.com/icon.png",
  "owner_pubkey": "<highlighter-owner-pubkey>",
  "groups": {
    "create_group_presence_relays": ["wss://relay.damus.io", "wss://purplepag.es"],
    "free_transit_presence_relays": ["wss://relay.damus.io"],
    "create_group_rate_limit": {
      "tokens_per_interval": 1,
      "interval_seconds": 10800,
      "max_tokens": 3
    }
  },
  "blossom": {
    "enabled": true,
    "s3_endpoint": "...",
    "s3_bucket": "highlighter-media"
  }
}
```

### Fork Maintenance Strategy

- Track upstream croissant and rebase periodically
- Highlighter extensions live in clean, isolated files (`highlighter.go`, `highlighter_events.go`, `highlighter_policies.go`)
- Keep policy hooks separate from core event processing so upstream merges stay clean
- Open-source the fork; custom kinds are documented for interop
- The templ web UI is fully replaced with Highlighter-specific pages (this is the least mergeable part — expect divergence here)

---

## 3. Webapp — SvelteKit + NDK + Vercel

### Starting Point

- **Template:** [ndk-template-sveltekit-vercel](https://github.com/nostr-dev-kit/ndk-template-sveltekit-vercel)
- **Stack:** SvelteKit, NDK (TypeScript Nostr library), Vercel deployment
- **Language:** TypeScript / Svelte 5

### What the Template Already Provides

| Feature | Details |
|---|---|
| SSR | Server-rendered pages that work for crawlers and real users |
| Client-side live updates | NDK subscriptions layered on top of SSR pages |
| SEO + social previews | Dynamic OG images, `<SeoHead>` component, canonical URLs |
| Auth | NIP-07 (browser extension) and NIP-46 (remote signer) login flows |
| Onboarding | Profile creation, interest selection, Blossom-backed avatars, optional NIP-05 |
| NDK primitives | `@ndk/svelte` registry components wired into app structure |
| Routes | `/` (front page), `/profile/[identifier]`, `/note/[id]`, `/highlights`, `/bookmarks` |
| Vercel deployment | Zero-config deploy, KV for NIP-05 persistence |

### What We Build on Top

#### New Routes (Highlighter-specific)

| Route | Purpose |
|---|---|
| `/group/[groupId]` | Group home page: featured artifacts, highlight spotlight, full library, activity feed |
| `/group/[groupId]/artifact/[artifactId]` | Artifact detail: highlights + threaded discussion |
| `/group/[groupId]/highlight/[highlightId]` | Single highlight view + discussion |
| `/discover` | Public group discovery: browse, search, trending |
| `/vault` | Personal vault: all your highlights across groups, searchable |
| `/share/highlight/[highlightId]` | Public highlight card (SEO-optimized, shareable) |
| `/share/group/[groupId]` | Public group page (SEO landing page for growth) |
| `/onboarding/group-create` | Creator flywheel: create a group, set access/visibility, invite flow |

#### Modified Existing Routes

| Route | Modification |
|---|---|
| `/` | Front page becomes Highlighter's discovery/home — trending groups, featured highlights, new artifacts |
| `/profile/[identifier]` | Add: groups joined, highlights created, artifacts shared |
| `/highlights` | Repurpose as global highlight feed or redirect to `/vault` |
| `/onboarding` | Add group creation flow, invite acceptance |

#### Key Extensions

| Area | What We Add |
|---|---|
| **NDK event handling** | Custom NDK event classes for Artifact and Highlight kinds; NIP-29 group subscription management |
| **Relay configuration** | Point to Highlighter relay(s) as primary; support additional user relays |
| **NIP-42 auth** | Implement NIP-42 authentication flow for group membership verification on the relay |
| **Group state management** | Client-side store for group metadata, membership lists, roles — synced from relay |
| **Artifact extraction** | URL metadata extraction (OpenGraph, oEmbed, manual entry) for creating artifact events |
| **Highlight card rendering** | Beautiful, shareable highlight cards with group branding — the core growth mechanic |
| **Invite mechanics** | Shareable links, invite codes (`kind:9009`), group join flows — baked into every surface |
| **Public pages** | SSR-optimized group and highlight pages for SEO and social sharing |
| **Browser extension** | Companion extension for one-click highlight capture from any webpage (post-MVP, but design for it) |

### Architecture Notes

```
src/
├── routes/
│   ├── +layout.svelte          # NDK context, auth state, nav
│   ├── +page.svelte             # Home / discover
│   ├── group/[groupId]/
│   │   ├── +page.svelte         # Group home (SSR)
│   │   ├── +page.server.ts      # SSR data for SEO
│   │   ├── artifact/[id]/       # Artifact detail (SSR)
│   │   └── highlight/[id]/      # Highlight detail
│   ├── discover/                # Public group discovery
│   ├── vault/                   # Personal highlights
│   ├── share/                   # Public shareable pages (SEO)
│   ├── profile/[identifier]/    # User profile (SSR)
│   ├── onboarding/              # Auth + group creation
│   └── .well-known/nostr.json   # NIP-05
├── lib/
│   ├── ndk/                     # NDK instance, custom event classes
│   │   ├── client.ts            # NDK setup + Highlighter relay config
│   │   ├── events/              # Artifact, Highlight event classes
│   │   └── groups/              # NIP-29 group state management
│   ├── components/              # Shared UI components
│   │   ├── highlight-card/      # The shareable highlight card (growth)
│   │   ├── artifact/            # Artifact display components
│   │   ├── discussion/          # Threaded discussion components
│   │   └── group/               # Group header, members, settings
│   ├── server/
│   │   ├── nostr.ts             # Server-side NDK for SSR
│   │   └── og.ts                # Dynamic OG image generation
│   └── seo.ts                   # SEO metadata builders
└── static/
    └── og-default.png           # Default OG image
```

### Webapp-Specific Concerns

| Concern | Approach |
|---|---|
| **NIP-07 / NIP-46 auth** | Inherited from template. NIP-07 for browser extension signers (nos2x, Alby), NIP-46 for remote signers (Nsec.app). |
| **SSR for public content** | Public groups + shared highlight cards get SSR for SEO. Private group content is client-only. |
| **Progressive enhancement** | SSR page loads, then NDK subscriptions hydrate live updates. No flash of empty content. |
| **Vercel deployment** | Edge functions for SSR, KV for NIP-05, serverless functions for URL metadata extraction. |
| **Browser extension companion** | Design highlight creation API to work from extension context. Extension is post-MVP but the API surface should accommodate it from day one. |
| **Mobile web** | Responsive, mobile-first. The webapp IS the mobile experience until native apps ship. |

---

## 4. Mobile Apps — Rust Core + Native UI

### Architecture

```
┌─────────────────────────────────────────────────┐
│                  RUST CORE                       │
│                  (shared library)                │
│                                                  │
│  ┌──────────────┐  ┌──────────────────────────┐│
│  │ Nostr Client  │  │ NIP-29 Groups            ││
│  │ - Relay pool  │  │ - Join/leave/create      ││
│  │ - Event sign  │  │ - Membership state       ││
│  │ - NIP-42 auth │  │ - Moderation             ││
│  │ - Sub filters │  │ - Group metadata cache    ││
│  └──────────────┘  └──────────────────────────┘│
│                                                  │
│  ┌──────────────┐  ┌──────────────────────────┐│
│  │ Data Layer   │  │ Content Engine           ││
│  │ - SQLite DB  │  │ - URL metadata extraction││
│  │ - Sync engine│  │ - Highlight management   ││
│  │ - Event cache│  │ - Search / full-text idx ││
│  └──────────────┘  └──────────────────────────┘│
│                                                  │
│  ┌──────────────────────────────────────────┐   │
│  │ FFI Bridge (C ABI)                       │   │
│  │ Exposes typed API to Swift and Kotlin    │   │
│  └──────────────────────────────────────────┘   │
└────────────────────────┬────────────────────────┘
                         │
            ┌────────────┼────────────┐
            │            │            │
      ┌─────▼─────┐ ┌───▼────┐ ┌────▼──────┐
      │   iOS App  │ │Android │ │  Desktop  │
      │  (Swift   │ │(Kotlin │ │  (native) │
      │   UI)     │ │  UI)   │ │           │
      └───────────┘ └────────┘ └───────────┘
```

### Rust Core — Responsibilities

The Rust core is the single source of truth for protocol logic, data, and sync. Native UIs are thin presentation layers.

| Module | Responsibility |
|---|---|
| **Nostr Client** | Relay pool management, WebSocket connections, event creation/signing/verification, NIP-42 auth handshake, subscription filters |
| **NIP-29 Groups** | Group lifecycle (create, join, leave, fork), membership state machine, role management, invite code handling |
| **Data Layer** | SQLite for local event storage, background sync engine, conflict resolution, event cache with expiry |
| **Content Engine** | URL metadata extraction (title, author, image, description), highlight text management, full-text search indexing |
| **FFI Bridge** | C ABI exposing the core API to Swift (iOS) and Kotlin (Android) via UniFFI or hand-written bindings |

### Rust Core — Key Decisions

| Decision | Choice | Rationale |
|---|---|---|
| **Nostr library** | `nostr-sdk` (Rust) | Official Rust Nostr SDK; supports NIP-01 through NIP-46+, relay pool, event builder |
| **Local storage** | SQLite via `rusqlite` | Proven, embedded, works on all platforms. Full-text search via FTS5. |
| **FFI approach** | UniFFI (Mozilla) | Auto-generates Swift and Kotlin bindings from Rust. Battle-tested (used by Firefox, Matrix). |
| **Async runtime** | `tokio` | Industry standard; `nostr-sdk` uses it. |
| **Sync strategy** | Optimistic local-first | Post events locally, confirm when relay accepts. Queue offline events. |

### iOS App — Swift UI

| Aspect | Detail |
|---|---|
| **Minimum iOS** | 17.0+ (SwiftUI, latest APIs) |
| **UI framework** | SwiftUI |
| **Architecture** | MVVM — ViewModels call Rust core via FFI |
| **Navigation** | SwiftUI NavigationStack |
| **Key screens** | Group list → Group home → Artifact detail → Highlight view → Discussion thread; Vault (personal highlights); Discover; Profile |
| **Share extension** | iOS Share Sheet integration for capturing highlights from Safari, Books, etc. |
| **Notifications** | APNs for group activity (new highlights, discussions, mentions) — relay pushes to push notification service |
| **Offline** | Full offline via Rust core local SQLite. Optimistic UI. Background sync. |

### Android App — Kotlin UI

| Aspect | Detail |
|---|---|
| **Minimum Android** | API 26+ (Android 8.0) |
| **UI framework** | Jetpack Compose |
| **Architecture** | MVVM — ViewModels call Rust core via FFI |
| **Navigation** | Compose Navigation |
| **Key screens** | Same as iOS, adapted to Material Design 3 |
| **Share extension** | Android Share Sheet for highlight capture from any app |
| **Notifications** | FCM for group activity |
| **Offline** | Same as iOS — Rust core handles all offline logic |

### Mobile-Specific Concerns

| Concern | Approach |
|---|---|
| **Push notifications** | Relay pushes events to a lightweight notification service → APNs/FCM. Not all events — just mentions, group invites, and highlights-on-your-artifacts. |
| **Background sync** | Rust core manages background WebSocket connections. iOS: background fetch intervals. Android: WorkManager. |
| **Key management** | Nsec stored in platform secure enclave (iOS Keychain, Android Keystore). NIP-46 remote signing as alternative. |
| **Deep linking** | `highlighter://group/{id}`, `highlighter://artifact/{id}`, `highlighter://highlight/{id}` — opens directly in app. |
| **Share-to-app** | Both platforms: share URL/text from any app → Highlighter creates draft artifact or highlight. |
| **Biometric auth** | Optional biometric unlock for the app. Keys stay encrypted until biometric approval. |

---

## 5. Desktop App — Rust Core + Native UI

### Architecture

Same Rust core as mobile, with a desktop-native UI layer.

| Aspect | Detail |
|---|---|
| **UI framework** | Platform-native (macOS: AppKit/SwiftUI, Windows: WinUI, Linux: GTK4) — OR a single cross-platform desktop UI (TBD based on team capacity) |
| **Architecture** | Same Rust core via FFI. Desktop-optimized layouts. |
| **Key differentiators** | Multi-column layout, keyboard-driven workflows, drag-and-drop highlight creation, larger artifact reading view |
| **Offline** | Same as mobile — Rust core SQLite, full offline capability |

### Desktop-Specific Concerns

| Concern | Approach |
|---|---|
| **Window management** | Multi-column layout for groups + artifacts + discussion side-by-side. Not a stretched phone layout. |
| **Keyboard shortcuts** | Full keyboard navigation. Vim-style or macOS-style. |
| **Reading experience** | Dedicated reading/annotation view — side-by-side with source content and highlight panel. |
| **File watching** | Local file integration (watch a PDF folder, auto-create artifacts). |
| **Auto-start** | Optional system tray / menu bar presence. |

---

## 6. Cross-Surface Consistency

All four surfaces (web, iOS, Android, desktop) present the same data and same core interactions. Differences are in presentation and platform affordances, not in feature set.

### Feature Parity Matrix

| Feature | Webapp | iOS | Android | Desktop |
|---|---|---|---|---|
| Create/join groups | ✅ | ✅ | ✅ | ✅ |
| Share artifacts | ✅ | ✅ | ✅ | ✅ |
| Create highlights | ✅ | ✅ | ✅ | ✅ |
| Threaded discussion | ✅ | ✅ | ✅ | ✅ |
| Public group pages | ✅ (SSR) | ✅ | ✅ | ✅ |
| Share highlight cards | ✅ | ✅ | ✅ | ✅ |
| Personal vault | ✅ | ✅ | ✅ | ✅ |
| NIP-07 auth | ✅ | — | — | — |
| NIP-46 auth | ✅ | ✅ | ✅ | ✅ |
| Nsec direct | — | ✅ (Keychain) | ✅ (Keystore) | ✅ (encrypted) |
| Offline mode | Limited (SW) | ✅ | ✅ | ✅ |
| Push notifications | — | ✅ (APNs) | ✅ (FCM) | ✅ (native) |
| Browser extension | ✅ | — | — | — |
| Share sheet capture | — | ✅ | ✅ | ✅ (drag-drop) |
| Multi-column view | — | — | — | ✅ |
| Deep linking | URL | URL scheme | URL scheme | URL scheme |

### Data Flow Consistency

All surfaces talk to the same relay infrastructure. The data model is identical:

- Groups are NIP-29 groups on the relay
- Artifacts and highlights are Nostr events (custom kinds)
- Memberships and roles are relay-managed NIP-29 state
- A user's identity (Nostr keypair) works across all surfaces
- A user's data follows them — groups, highlights, artifacts are portable

The webapp uses NDK (TypeScript) for protocol. Mobile/desktop use the Rust core. Both implement the same Nostr protocol interactions. Any divergence in event handling is a bug.

---

## 7. What's NOT in This Spec

This document defines the **what** and **how** of each product surface. These topics are covered in other documents:

| Topic | Document |
|---|---|
| Core product concepts (groups, artifacts, highlights, discussions) | `product-spec-v2.0.md` |
| NIP-29 group model, event schemas, membership flows | `technical-architecture.md` |
| Growth loops, virality mechanics, user control principles | `product-spec-v2.0.md` §5 |
| UI/UX wireframes and proposals | `community-page-proposals-v1.4.md`, `landing-page-proposals.md` |
| Market research and competitive landscape | `market-research-2026.md` |

---

## 8. Open Questions

| # | Question | Impact | Status |
|---|---|---|---|
| ~~1~~ | ~~**Custom event kind numbers**~~ | ~~Protocol interop, relay filtering~~ | **Resolved: NIP-84 for highlights, NIP-73 for artifacts. No custom kinds.** |
| 2 | **Desktop UI framework** — Platform-native (AppKit/WinUI/GTK) or cross-platform (Tauri, egui, Slint)? | Team capacity, consistency, dev speed | Needs decision |
| 3 | **Browser extension scope for MVP** — Which browsers? Chrome-only first? Firefox? | Webapp extension companion scope | Needs decision |
| 4 | **Notification relay architecture** — Separate push notification service, or embedded in the relay fork? | Relay complexity, scaling | Needs decision |
| 5 | **Webapp state management** — Svelte stores, custom NDK store layer, or something else? | Webapp codebase architecture | Needs decision |
| 6 | **URL metadata extraction** — Server-side (Vercel edge function) or client-side (NDK + fetch)? | SSR quality, rate limiting, privacy | Needs decision |

---

*This spec supersedes the surface definitions in `product-spec-v2.0.md` §3 (Platform Architecture) and `technical-architecture.md` §5 (Client Architecture). The core concepts, group model, and growth principles from v2.0 remain unchanged.*