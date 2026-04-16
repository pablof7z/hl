# AGENTS.md — Highlighter Web App

> The Highlighter web app is the primary development surface. Full feature parity, responsive mobile-first design. Connects to Highlighter relay infrastructure via Nostr WebSocket protocol.

## Tech Stack

- **Framework:** SvelteKit (or similar modern SPA framework — TBD)
- **Language:** TypeScript
- **Styling:** Tailwind CSS (or similar — TBD)
- **Nostr client:** nostr-tools (or similar JS/TS Nostr library)
- **Auth:** NIP-07 (browser extension signing) + NIP-46 (Nostr Connect / remote signing)
- **Deployment:** Vercel (static output)

## Setup Commands

```bash
cd web

# Install dependencies
npm install          # or: pnpm install

# Start dev server with hot reload
npm run dev          # or: pnpm dev

# Build for production
npm run build

# Preview production build
npm run preview
```

## Development Workflow

```bash
# Type checking
npm run check        # svelte-check or tsc

# Linting
npm run lint

# Format
npm run format

# Run all checks (lint + typecheck + test)
npm run check:all    # or whatever the combined script is named
```

## Project Structure

```
web/
├── src/
│   ├── lib/                  # Shared utilities, Nostr client, types
│   │   ├── nostr/            # Nostr client setup, relay pool, event helpers
│   │   ├── stores/           # Svelte stores (groups, artifacts, highlights)
│   │   └── utils/            # Helpers, formatters, validators
│   ├── routes/               # SvelteKit routes
│   │   ├── +layout.svelte   # Root layout (auth, nav)
│   │   ├── +page.svelte      # Home / discovery
│   │   ├── group/
│   │   │   └── [id]/         # Group home, artifacts, highlights, activity
│   │   ├── artifact/
│   │   │   └── [id]/         # Artifact detail, highlights + discussion
│   │   ├── vault/            # Personal highlights vault
│   │   └── auth/             # Login, NIP-07/NIP-46 flows
│   ├── components/           # Reusable Svelte components
│   │   ├── groups/           # Group cards, member list, settings
│   │   ├── artifacts/        # Artifact cards, share flow
│   │   ├── highlights/       # Highlight cards, creation, sharing
│   │   └── discussion/       # Thread, replies, reactions
│   └── app.html              # HTML shell
├── static/                   # Static assets
├── tests/                    # Test files
│   ├── unit/                 # Unit tests
│   └── e2e/                  # Playwright or similar
├── package.json
├── svelte.config.js
├── vite.config.ts
├── tsconfig.json
└── AGENTS.md
```

## Key Concepts

### Authentication Flows

1. **NIP-07** (primary for desktop browsers): Browser extension (nos2x, Alby) signs events
2. **NIP-46** (cross-device): Nostr Connect protocol — remote signer app
3. **New user onboarding**: Generate keypair → prompt backup → immediately usable (no email verification)

### NIP-42 Relay Auth

The relay requires NIP-42 authentication for restricted/private groups:
```
Client → Relay: AUTH challenge received
Client → Relay: Sign challenge event with user's key
Relay → Client: Validate membership, grant/deny access
```

### Group Pages (Public Groups)

Public groups serve dual purpose:
- **Member view**: Full interaction — post, highlight, discuss
- **Non-member view**: SEO-optimized read-only with "Join" / "Request Invite" CTA

### Highlight Cards

Highlights are the viral growth mechanism. Each highlight can be:
- Shared as an individual card (image or embed)
- Embedded on external sites
- Deep-linked back to the group discussion

## Testing

```bash
# Run all tests
npm run test

# Run unit tests only
npm run test:unit

# Run e2e tests
npm run test:e2e

# Run tests in watch mode
npm run test:watch

# Coverage
npm run test:coverage
```

## Build & Deployment

- **Vercel deployment** — auto-deploys from `main` branch
- **Static output** — `vercel.json` configures `outputDirectory: "public"`
- **Environment variables**:
  - `PUBLIC_RELAY_URL` — Highlighter relay WebSocket URL
  - `PUBLIC_RELAY_INFO_URL` — NIP-11 relay info endpoint

## Code Style

- **TypeScript strict mode** — no `any` types without explicit justification
- **Svelte component naming**: PascalCase for components, camelCase for utilities
- **Nostr event handling**: Always validate event IDs and signatures before trusting data
- **Store pattern**: Use Svelte stores for shared state; keep components dumb when possible

## Common Patterns

- **Adding a new route**: Create `src/routes/<path>/+page.svelte` and `+page.ts` for data loading
- **Fetching group data**: Use the Nostr relay pool subscription in `src/lib/nostr/`
- **NIP-29 event creation**: Use helpers in `src/lib/nostr/events.ts` — never construct raw events manually
- **Highlight sharing**: Use the share utilities in `src/lib/utils/share.ts`

## SEO for Public Pages

- Server-side rendering (or static generation) for public group pages and highlight cards
- `<meta>` tags: title, description, og:image from group/highlight data
- Structured data (JSON-LD) for group and artifact pages
- Canonical URLs for shared highlights