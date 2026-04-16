# SvelteKit + NDK Template

Ship a real Nostr web app without spending your first week rebuilding SSR, auth, SEO, onboarding, and deployment plumbing.

This template gives you the hard parts up front: server-rendered pages that still feel live, session-aware client UX, shareable previews, reusable NDK primitives, and a deployment path that already makes sense.

The included UI uses profiles, notes, articles, comments, and highlights to show the stack in action. Those are examples of what the foundation supports, not the limit of what you can build with it.

## What You Get

- SSR that works for crawlers, links, and real users
- live client updates layered on top of server-rendered pages
- built-in SEO and social previews, including dynamic OG images
- login flows for common Nostr signer setups
- onboarding for profiles, interests, and Blossom-backed avatars
- optional managed NIP-05 registration with username availability checks and `.well-known/nostr.json`
- reusable `@ndk/svelte` primitives already wired into the app structure
- example app surfaces for profiles, event pages, threaded discussion, and richer content views
- Vercel-ready deployment without extra platform setup work

## Why the split matters

`NDKSvelte` is the right tool for client subscriptions, live feeds, and session-aware UI. It is not the right thing to depend on for social crawlers. Crawlers only see the HTML returned by the server, so preview-critical routes need to fetch their own Nostr data in `+page.server.ts` and emit SEO tags there.

This template makes that explicit instead of hiding it behind one cross-environment singleton.

## Registry integration

The starter now behaves like a real `@ndk/svelte` jsrepo consumer:

- `jsrepo.config.ts` points at `@ndk/svelte`
- registry-installed code lives under `src/lib/ndk/*`
- the root layout seeds the shared NDK instance into context for registry components
- the homepage, profile page, and note/article page already render author UI through the registry-backed `ui/user` primitive

To add more registry items into the same structure:

```bash
bunx jsrepo add ui/user
bunx jsrepo add components/session-switcher
```

## Routes

- `/` shows a publication-style front page seeded with long-form articles
- `/profile/[identifier]` SSR-fetches an author profile and recent articles
- `/note/[id]` SSR-fetches an article or note and author metadata

Both SSR routes return `seo` data that the root layout renders through `SeoHead.svelte`.

## Local development

```bash
bun install
bun run dev
```

## Environment

Set relays with:

```bash
PUBLIC_NOSTR_RELAYS=wss://relay.damus.io,wss://purplepag.es,wss://relay.primal.net
```

If omitted, the template uses those three relays by default.

To enable managed NIP-05 registration in onboarding, add:

```bash
PUBLIC_NIP05_DOMAIN=your-domain.com
```

The value can be a bare domain or a full URL. If set, onboarding shows an optional handle field, checks `username@your-domain.com` availability, and the app serves `/.well-known/nostr.json`.

For durable registrations on Vercel, also provide a writable KV-compatible store:

```bash
KV_REST_API_URL=
KV_REST_API_TOKEN=
```

Without those two variables, the template falls back to an in-memory registry that is only suitable for local development.

## Deploying to Vercel

1. Import the project into Vercel.
2. Leave the framework preset on `SvelteKit`.
3. Add `PUBLIC_NOSTR_RELAYS` if you want custom relays.
4. Add `PUBLIC_NIP05_DOMAIN` if you want the template to issue handles for your domain.
5. Add `KV_REST_API_URL` and `KV_REST_API_TOKEN` if you want those handle registrations to persist across instances.
6. Deploy.

No custom `vercel.json` is required for the base template.

## File map

- `src/lib/ndk/client.ts`: browser `NDKSvelte` instance with session persistence
- `src/lib/ndk/ui/`: registry-installed UI primitives from `@ndk/svelte`
- `src/lib/ndk/builders/`: registry-installed builders used by those primitives
- `src/lib/ndk/utils/ndk/`: the NDK context helper expected by registry items
- `src/lib/server/nostr.ts`: server-only `NDK` helpers for SSR loads
- `src/lib/seo.ts`: preview metadata builders
- `src/lib/components/SeoHead.svelte`: canonical, OG, and Twitter tags
- `jsrepo.config.ts`: consumer config for adding more registry items with jsrepo

## Social preview note

This template ships with a stable default OG image in `static/og-default.png` plus route-specific
titles and descriptions. If you want fully dynamic per-note images, layer that on top of the same
SSR metadata flow rather than moving preview generation into client code.
