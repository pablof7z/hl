# AGENTS.md — Highlighter Relay (Croissant)

> The Highlighter relay is based on **Croissant** by fiatjaf — a full-featured NIP-29 group relay built on khatru. It provides group lifecycle, membership, moderation, full-text search, Blossom media, and LiveKit audio rooms. We fork and customize.

**Source:** `nak git clone npub180cvv07tjdrrgpa0j7j7tmnyl2yr6yr7l8j4s3evf6u64th6gkwsyjh6w6/croissant`

## Tech Stack

- **Language:** Go
- **Framework:** khatru (custom relay framework) + NIP-29 group logic from Croissant
- **Search:** Bleve (per-group full-text search indexes with language detection)
- **Storage:** MMM MultiMmapManager (embedded event store with indexing layer)
- **Media:** Blossom (NIP-96 compatible) with local filesystem or S3 backend
- **Audio:** LiveKit integration for group voice rooms
- **Build:** `just` (justfile) + `templ` for HTML templates + Tailwind CSS

## Setup Commands

```bash
cd relay

# Install Go dependencies
go mod download

# Install Node dependencies (for Tailwind)
npm install

# Generate templ templates
just templ

# Build Tailwind CSS
just tailwind

# Build the binary
go build -o ./croissant .

# Run locally (requires env vars)
PORT=9888 HOST=127.0.0.1 DATAPATH=data OWNER_PUBLIC_KEY=<hex-pubkey> ./croissant
```

### Environment Variables

| Variable | Default | Description |
|---|---|---|
| `PORT` | `9888` | HTTP/WebSocket port |
| `HOST` | `127.0.0.1` | Bind address |
| `DATAPATH` | `data` | Data directory (event store, search indexes) |
| `OWNER_PUBLIC_KEY` | *(required)* | Hex pubkey of the relay owner/admin |
| `DOMAIN` | *(empty)* | Public domain for NIP-11 and CORS |

## Development Workflow

```bash
# Dev mode with hot reload (requires `entr` and `fd`)
just dev

# Generate templ templates (after editing .templ files)
just templ

# Build Tailwind CSS (after editing base.css or templates)
just tailwind

# Run tests
go test ./...

# Run clippy-equivalent (Go vet)
go vet ./...

# Format
gofmt -w .
```

### Live Relay Restart Requirement

`relay.highlighter.com` on this machine runs under the launch agent `io.f7z.relay-highlighter`, serving `/Users/customer/Work/highlighter/relay/croissant` behind Caddy on `127.0.0.1:9888`.

After any relay code or config change that should affect the live relay, do not stop at tests or a local build. Rebuild the binary in `relay/` and restart the launch agent so the running relay picks up the change:

```bash
cd relay
go build -o ./croissant.new .
mv ./croissant.new ./croissant
launchctl kickstart -k gui/$(id -u)/io.f7z.relay-highlighter
```

Verify the service after restarting:

```bash
launchctl print gui/$(id -u)/io.f7z.relay-highlighter
curl -sS -H 'Accept: application/nostr+json' http://127.0.0.1:9888
```

## Project Structure

```
relay/
├── main.go              # Entry point, embeds static assets, initializes store
├── relay.go             # khatru relay handler with atomic swap
├── group.go             # Group struct, NIP-29 group lifecycle (create, delete, roles)
├── group.templ          # HTML template for group pages
├── state.go             # GroupsState: in-memory group management, join/leave
├── process_event.go     # Event processing pipeline (accept/reject/filter)
├── reject_event.go      # Policy engine (rate limits, closed group enforcement)
├── query.go             # Subscription/query handling
├── query_parser.go      # Filter parsing for nostr queries
├── search.go            # Full-text search (Bleve) per group
├── blossom.go           # Blossom (NIP-96) media upload handling
├── livekit.go           # LiveKit audio room integration
├── presence.go          # User presence tracking
├── favicon.go           # Favicon handling
├── wipe.go              # Data wipe utilities
├── lexer.go             # Query lexer
├── utils.go             # Utility functions
├── global/              # Global state, settings, auth, logging
│   ├── init.go          # Initialization
│   ├── settings.go      # Relay settings (JSON config)
│   ├── auth.go          # NIP-42 authentication
│   ├── relay.go         # Relay URL management
│   ├── rate_limits.go   # Per-IP rate limiting
│   ├── log.go           # Zerolog setup
│   └── env.go           # Environment variable parsing
├── fs/                  # Filesystem abstraction (local + S3)
│   ├── fs.go
│   ├── subdirfs.go
│   └── s3fs.go
├── static/              # Static assets (logo, CSS)
├── home.templ           # Home page template
├── layout.templ         # Layout template
├── base.css             # Tailwind source CSS
├── justfile             # Build/task runner
├── go.mod / go.sum      # Go module deps
├── package.json         # Node deps (Tailwind only)
└── AGENTS.md
```

## Key Concepts

### NIP-29 Group Types

| Highlighter Type | `restricted` | `closed` | `private` | `hidden` |
|---|---|---|---|---|
| Open + Public | ✅ | — | — | — |
| Open + Private | ✅ | — | ✅ | ✅ |
| Closed + Public | ✅ | ✅ | — | — |
| Closed + Private | ✅ | ✅ | ✅ | ✅ |

The `restricted` tag is **always set** on Highlighter groups — only members can write.

### Croissant Built-in Features

- **Full-text search**: Bleve indexes per group with automatic language detection (first 10 messages)
- **Blossom media**: Upload images/files via NIP-96, store locally or S3-compatible
- **LiveKit audio**: Group voice rooms via LiveKit protocol integration
- **Group management**: Full NIP-29 lifecycle — create, delete, fork, membership, roles, moderation
- **Rate limiting**: Per-IP rate limits configurable via settings
- **HTML UI**: Templ-based web UI for group pages (server-rendered)

### Highlighter Event Model

- **Artifact shares** use standard `kind:11` threads inside NIP-29 groups
- **Artifacts** are identified by source-reference tags (`a`, `e`, `i`/`k`, `r`), not a custom Highlighter kind
- **Highlights** use standard `kind:9802`
- **Discussion replies** use standard `kind:1111`

Do not introduce a made-up artifact or highlight event kind in the relay without first changing the canonical architecture docs.

When changing relay-supported event kinds or their policy handling, update:
1. The event handler in `process_event.go`
2. The rejection policy in `reject_event.go`
3. `docs/technical-architecture.md` §4

## Testing

```bash
# Run all tests
go test ./...

# Run specific package tests
go test -run TestGroupLifecycle ./...

# Run with verbose output
go test -v ./...

# Run integration tests against a live relay
# (requires a running instance)
go test -tags=integration ./...
```

## Build & Deployment

```bash
# Production build (static musl binary)
just build

# Build for specific architecture
just build cc='musl-gcc' arch='arm64'

# Deploy to server
just deploy target=user@server.com

# Or manual deploy:
CGO_ENABLED=1 GOOS=linux go build -tags=libsecp256k1 \
  -ldflags="-X main.currentVersion=$(git describe --tags)" \
  -o ./croissant
```

### Build Dependencies

- **Go 1.25+**
- **just** (command runner): `cargo install just` or `brew install just`
- **templ** (HTML template compiler): `go install github.com/a-h/templ/cmd/templ@latest`
- **Node.js** (for Tailwind): npm install in project root
- **musl-gcc** (for static Linux builds): for cross-compilation

## Code Style

- Follow `gofmt` defaults — `gofmt -w .` before every commit
- `go vet` should pass clean
- Use `zerolog` for structured logging (see `global/log.go`)
- Use `envconfig` for all configuration — never hardcode values
- Group-related state lives in `state.go` and `group.go`
- Event processing pipeline: `process_event.go` → policy checks → store

## Common Patterns

- **Adding a new policy**: Add a rejection rule in `reject_event.go`
- **Adding a new event kind**: Add handling in `process_event.go`, update `reject_event.go`, update docs
- **Changing group behavior**: Croissant handles most NIP-29 logic — check `group.go` and `state.go` first
- **Adding a web UI page**: Create a `.templ` template, register route in `main.go`, run `just templ`
- **Media uploads**: Blossom handlers are in `blossom.go`, filesystem abstraction in `fs/`

## Security Considerations

- Never log or expose user private keys (nsec)
- NIP-42 auth enforced for restricted/private group operations
- Rate limiting is per-IP by default — configure per-pubkey if needed
- Validate all event signatures before processing (khatru handles this)
- `OWNER_PUBLIC_KEY` has admin privileges — protect this key
- S3 credentials (if using remote media storage) should be in environment variables, not code
