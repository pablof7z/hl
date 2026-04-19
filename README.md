# Highlighter

A Nostr-based platform for sharing highlights and annotations of content within communities. Built on NIP-29 (relay-based groups) and NIP-22 (threaded comments).

## What is Highlighter?

Highlighter lets you:

- **Share artifacts** — Books, articles, podcasts, videos, and other content shared to communities
- **Highlight passages** — Pull excerpts from shared artifacts and annotate them
- **Discuss in groups** — NIP-29 relay-based communities for organizing around topics and interests
- **Build your vault** — Personal collection of your highlights across all communities

## Project Structure

```
highlighter/
├── web/              # SvelteKit web application (main frontend)
│   ├── src/          # Source code
│   ├── static/       # Static assets
│   └── README.md     # Web app-specific documentation
├── relay/            # NIP-29 relay implementation
├── app/              # Static landing/app shell
├── docs/             # Product specs, architecture, research
│   ├── product-spec-v2.0.md
│   ├── technical-architecture.md
│   └── market-research-2026.md
└── scripts/          # Build and deployment scripts
```

## Quick Start

### Web App

```bash
cd web
bun install
bun run dev
```

### Build for Production

```bash
bun run build
```

### Deploy to Vercel

```bash
bun run deploy:web:prod
```

## Documentation

| Document | Description |
|----------|-------------|
| [Product Spec v2.0](docs/product-spec-v2.0.md) | Core concepts, features, growth loops |
| [Technical Architecture](docs/technical-architecture.md) | NIP mapping, data models, relay design |
| [Client Spec](docs/client-spec-v1.0.md) | Navigation, screens, design system |
| [Market Research](docs/market-research-2026.md) | Competitor analysis, positioning |

## Tech Stack

- **Frontend**: SvelteKit, TailwindCSS, DaisyUI
- **Nostr**: NDK (Nip-26 Delegated Signing), NIP-29 groups, NIP-22 threaded comments
- **Deployment**: Vercel

## Environment Variables

See `web/.env.example` for required environment variables.

## License

Private project — all rights reserved.
