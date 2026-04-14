# AGENTS.md — Highlighter

> This is a monorepo. Each subproject has its own AGENTS.md with specific commands and conventions.
> The closest AGENTS.md to the file you're editing takes precedence.

## Project Overview

**Highlighter** is a Nostr-native social reading platform built around NIP-29 relay-based groups. Users create and join communities where they share **artifacts** (books, articles, podcasts, videos), annotate them with **highlights** (compelling excerpts), and spark **discussions**. Growth is baked into every surface — not a separate department.

Key differentiators:
- **Nostr-native from day one** — communities are NIP-29 groups, users own their keys, data is portable
- **The artifact is the hero** — everything organizes around source content, not ephemeral posts
- **Growth is the product** — every interaction is designed for virality loops and user-controlled sharing

## Repository Structure

```
highlighter/
├── relay/          # Go relay (Croissant by fiatjaf) — NIP-29 groups, search, Blossom media, LiveKit
├── web/            # Web SPA (SvelteKit or similar) — primary client surface
├── app/            # Rust core + native UI (Kotlin/Swift) — mobile & desktop
├── docs/           # Product specs, architecture docs, market research, wireframes
├── .agents/        # TENEX agent configurations and skills
└── AGENTS.md       # You are here
```

> **Note:** `/relay`, `/web`, and `/app` are scaffolded from their respective AGENTS.md files. The repo currently holds docs and specs; code directories will be created as development begins.

## Tech Stack

| Component | Technology | Notes |
|---|---|---|
| **Relay** | Go (Croissant by fiatjaf) | NIP-29 groups, full-text search (Bleve), Blossom media, LiveKit audio, NIP-42 auth |
| **Web app** | Modern SPA (framework TBD) | Responsive, mobile-first, NIP-07/NIP-46 signing |
| **Mobile apps** | Rust core + Kotlin (Android) / Swift (iOS) | Shared Rust core via FFI |
| **Desktop app** | Rust core + Tauri or native | Same shared logic as mobile |
| **Browser extension** | JS/TS | Highlight capture from any webpage |
| **Relay storage** | Bleve (full-text search) + MMM index (embedded) | Croissant uses Bleve per-group search indexes + MMM MultiMmapManager for event storage |

## Key NIPs

| NIP | Purpose |
|---|---|
| NIP-01 | Core protocol (events, subscriptions, relay communication) |
| NIP-07 | Browser extension signing |
| NIP-11 | Relay information document |
| NIP-19 | Bech32 encoding (npub, nprofile, nevent, naddr) |
| NIP-21 | `nostr:` URI scheme (deep linking) |
| NIP-22 | Comments (threaded discussion) |
| NIP-23 | Long-form content |
| NIP-25 | Reactions |
| NIP-29 | **Core** — Relay-based groups (the foundation of communities) |
| NIP-42 | Relay authentication (membership enforcement) |
| NIP-46 | Remote signing (Nostr Connect) |
| NIP-55 | Android signer integration |
| NIP-96 | File/media storage |

## Custom Event Kinds

Highlighter defines custom event kinds within NIP-29 groups:

- **Artifact event** (kind TBD) — shared content item with title, author, source, URL, image
- **Highlight event** (kind TBD) — excerpt from an artifact, with context tag
- Standard kinds used within groups: `kind:1` (text notes), `kind:1111` (NIP-22 comments)

## Getting Started

The repo is in early specification phase. No build commands exist yet. When code directories are scaffolded:

1. Navigate to the subproject directory
2. Read that directory's AGENTS.md for setup and dev commands
3. Follow platform-specific instructions

## Deployment

- **Manual deployment**: deploy the web app from the repo root with the Vercel CLI.
- Run `vercel deploy --prod` after verifying the static build output in `public/`.
- Do **not** rely on Git-based auto deployment for the current web app setup.

## Documentation

All product and architecture docs live in `/docs`:

| File | Content |
|---|---|
| `product-spec-v2.0.md` | Current product specification (authoritative) |
| `product-spec-v1.2.md` | Previous spec version (historical reference) |
| `technical-architecture.md` | System architecture, NIP mapping, data models |
| `market-research-2026.md` | Market research, competitor analysis, positioning |
| `community-page-proposals-v1.4.md` | Community/shelf page wireframes (latest) |
| `community-page-proposals.md` | Community page wireframes (v1) |
| `landing-page-proposals.md` | Landing page wireframes |

## Pull Request Guidelines

- Title format: `[component] Brief description` (e.g., `[relay] add invite code validation`)
- Keep PRs focused — one concern per PR
- Reference relevant NIP numbers when changing protocol behavior
- When modifying event schemas, update both the implementation and `technical-architecture.md`

## Architecture Decisions

### Why Croissant?
Croissant (by fiatjaf) is a full-featured NIP-29 relay built on khatru. It provides group lifecycle, membership, moderation, full-text search (Bleve), Blossom media upload, and LiveKit audio rooms out of the box. We fork and customize rather than building from scratch. See `docs/technical-architecture.md` §2.

### Why Rust core for mobile/desktop?
Shared protocol logic (Nostr client, NIP-29, signing, sync) across all native platforms via FFI bridge. Only the UI layer differs per platform.

### Why NIP-29 groups (not DMs or channels)?
Groups are relay-native, portable, have built-in membership/role/moderation semantics, and support the four access×visibility combinations Highlighter needs. See `docs/technical-architecture.md` §3.

## Navigation

- **Root** → `AGENTS.md` (this file)
- **Relay** → `relay/AGENTS.md`
- **Web app** → `web/AGENTS.md`
- **Mobile/Desktop** → `app/AGENTS.md`
- **Docs** → `docs/AGENTS.md`