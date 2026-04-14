# Technical Architecture: Highlighter
## Version 1.0 | April 2026

---

## 1. System Overview

Highlighter is a Nostr-native application. All data flows through Nostr relays as signed events. The system has three layers:

```
┌──────────────────────────────────────────────────┐
│                   CLIENTS                         │
│  Web App    │    Mobile (Android/iOS)   │ Desktop │
│  (SPA)      │    (Rust + native UI)     │ (Rust)  │
└──────────────────────┬───────────────────────────┘
                       │ Nostr protocol (WebSocket)
                       │ NIP-01, NIP-29, NIP-42, etc.
┌──────────────────────▼───────────────────────────┐
│              HIGHLIGHTER RELAY                    │
│         khatru-based (Go) + relay29              │
│                                                   │
│  NIP-29 group management                         │
│  Custom Highlighter event kinds                  │
│  NIP-42 authentication                           │
│  Moderation policies                             │
└──────────────────────┬───────────────────────────┘
                       │
┌──────────────────────▼───────────────────────────┐
│              STORAGE / PERSISTENCE                │
│  Event database (configurable backend)           │
│  Media/blob storage (NIP-96 or external)         │
└──────────────────────────────────────────────────┘
```

---

## 2. Relay: khatru + relay29

### Why khatru

[khatru](https://github.com/fiatjaf/khatru) is a Go framework for building custom Nostr relays. It provides:
- Pluggable event storage backends
- Middleware hooks for custom policies (accept/reject events, query filtering)
- NIP-11 relay information document
- NIP-42 authentication
- WebSocket management

[relay29](https://github.com/fiatjaf/relay29) extends khatru with full NIP-29 group support:
- Group lifecycle (create, delete, fork)
- Membership management (add/remove users, roles)
- Moderation events (kinds 9000–9020)
- Group metadata events (kinds 39000–39003)
- Invite codes (kind 9009)
- Relay-signed group state events

### Relay Responsibilities

Our relay handles:

| Responsibility | How |
|---|---|
| **Group creation** | Admin sends `kind:9007` → relay creates group state → signs `kind:39000` metadata |
| **Membership** | Join requests (`kind:9021`), admin adds (`kind:9000`), invite codes (`kind:9009`) |
| **Access control** | Enforce `private`/`closed`/`restricted`/`hidden` tags per group |
| **Message routing** | Accept community-scoped events with `h` tag → validate membership → store and broadcast. Canonical `kind:30023` and `kind:9802` events may be stored without `h`. |
| **Moderation** | Process `kinds:9000-9020` from authorized admins |
| **Group state** | Maintain and serve `kind:39000` (metadata), `kind:39001` (admins), `kind:39002` (members), `kind:39003` (roles) |
| **Highlighter events** | Accept and serve custom event kinds for artifacts and highlights (see §4) |
| **Cross-community artifact lookup** | Resolve artifact/share-thread lookups by artifact address or canonical `d` tag without requiring an explicit `#h` filter, while still respecting private-group visibility |

### Relay Configuration

Key policies we implement on top of relay29's defaults:

- **`restricted` is always set**: All Highlighter groups require membership to post. There are no fully "open write" groups.
- **Late publication window**: Reject events with timestamps older than 1 hour (prevents replay/confusion).
- **Timeline references enforced**: At least 2 `previous` tags required on group events (NIP-29 anti-fork protection).
- **Rate limiting**: Per-pubkey rate limits on message events to prevent spam.
- **Content types**: The relay accepts the standard Nostr kinds Highlighter relies on inside groups, especially `kind:11` share threads, `kind:1111` replies, and `kind:16` highlight reposts. It also accepts canonical group-neutral `kind:9802` highlights and `kind:30023` articles without an `h` tag.
- **Non-`#h` artifact resolution**: Clients must be able to fetch artifacts and "other communities that shared this" data starting from an artifact identifier, not just from a group ID. The relay therefore needs to support REQs by artifact address / canonical `d` tag / event id without forcing an `#h` filter for public content. Private-group content still requires membership-aware filtering even when the filter omits `#h`.

---

## 3. NIP-29 Group Model

### Group Metadata (kind:39000)

Every Highlighter group maps to a NIP-29 group with this metadata structure:

```jsonc
{
  "kind": 39000,
  "content": "",
  "tags": [
    ["d", "<group-id>"],
    ["name", "The Curious Readers"],
    ["picture", "https://..."],
    ["about", "A community for people who read deeply and discuss honestly"],
    // Access + visibility tags:
    ["restricted"],    // always set — only members can write
    ["closed"],        // present if invite-only (omitted if open-join)
    ["private"],       // present if members-only reading (omitted if public)
    ["hidden"]         // present if metadata hidden from non-members (set when private)
  ]
}
```

### Group Type → NIP-29 Tag Mapping

| Highlighter Type | `restricted` | `closed` | `private` | `hidden` |
|---|---|---|---|---|
| Open + Public | ✅ | — | — | — |
| Open + Private | ✅ | — | ✅ | ✅ |
| Closed + Public | ✅ | ✅ | — | — |
| Closed + Private | ✅ | ✅ | ✅ | ✅ |

### Membership Flow

**Open groups:**
1. User sends `kind:9021` (join request) with the group's `h` tag
2. Relay auto-accepts → user is added as member
3. Relay updates `kind:39002` (member list)

**Closed groups:**
1. Two paths:
   - **Invite code**: Admin creates invite via `kind:9009` → user sends `kind:9021` with `code` tag → relay validates and accepts
   - **Admin add**: Admin sends `kind:9000` with the user's pubkey → user is directly added
2. Unprompted `kind:9021` to a closed group → relay rejects with appropriate error message

**Leaving:**
- User sends `kind:9022` (leave request) → relay auto-removes → issues `kind:9001`

### Roles

Default Highlighter roles (defined via `kind:39003`):

| Role | Capabilities |
|---|---|
| `owner` | All permissions. Transfer ownership. Delete group. |
| `admin` | Add/remove members, manage roles (except owner), edit metadata, moderate content |
| `moderator` | Delete messages, mute members temporarily |
| `member` | Post artifacts, highlights, comments. React. |

---

## 4. Event Model

Highlighter uses standard Nostr kinds plus structured tags for its domain-specific data. The important distinction is:
- the **community-facing share/proposal** is a `kind:11` group thread
- the **artifact identity** is a source reference carried in tags, not a separate custom event kind
- the **highlight** is a standard `kind:9802`

### Artifact Share Thread (kind:11 / NIP-7D)

When a member proposes an artifact to a group, Highlighter publishes a lightweight group-scoped thread that carries both the community framing and the artifact metadata:

```jsonc
{
  "kind": 11,
  "content": "Interesting article for this group: nostr:naddr1...",
  "tags": [
    ["h", "<group-id>"],
    ["d", "<artifact-id>"],                  // stable local route key derived from the source reference
    ["title", "Interesting article for the group"],
    ["source", "article"],                  // book | article | podcast | video | paper | web
    ["author", "Author name"],              // optional
    ["image", "https://..."],               // optional
    ["summary", "Short metadata summary"],  // optional
    ["podcast_guid", "<guid>"],             // optional episode GUID when the source is a podcast
    ["podcast_show_title", "Show name"],    // optional, podcast-only
    ["audio", "https://...mp3"],            // optional direct audio enclosure for podcast playback
    ["audio_preview", "https://...mp3"],    // optional preview-only clip when the source withholds full audio
    ["transcript", "https://..."],          // optional transcript URL
    ["feed", "https://...rss"],             // optional RSS feed URL used to refresh podcast metadata
    ["published_at", "2026-04-14T12:00:00Z"], // optional ISO timestamp for books/podcasts/articles
    ["duration", "3691"],                   // optional duration in seconds, especially for audio/video
    ["a", "<30023:pubkey:identifier>"],     // for Nostr addressable content like long-form articles
    ["e", "<event-id>"],                    // for non-addressable Nostr events when needed
    ["i", "isbn:9780374533557", "https://..."], // for external content identity (ISBN, DOI, URL, podcast GUID, etc.)
    ["k", "isbn"],                          // classification for the `i` tag value
    ["r", "https://..."],                   // canonical URL when available
    ["previous", "..."]
  ]
}
```

Important consequences:
- There is **no custom artifact event kind** and specifically no `30403`-style Highlighter artifact event
- The **`kind:11` thread** stores the community-specific framing and enough metadata to render the artifact card
- The same underlying artifact can therefore be shared into multiple groups, each with its own `kind:11` thread and reply context
- Artifact identity is the source reference itself:
  - `a` tag for addressable Nostr content like `kind:30023`
  - `e` tag for non-addressable Nostr events
  - `i` tag plus optional `k` tag for external entities like URLs, ISBNs, DOIs, or podcast GUIDs

### Highlight Event (`kind:9802` / NIP-84)

When a member highlights an excerpt from an artifact source:

```jsonc
{
  "kind": 9802,
  "content": "People tend to assess the relative importance of issues by the ease with which they are retrieved from memory",
  "tags": [
    ["a", "<30023:pubkey:identifier>"],   // for addressable Nostr sources
    ["e", "<event-id>"],                  // for non-addressable Nostr sources
    ["r", "https://..."],                 // for external sources
    ["context", "Chapter 12: The Availability Heuristic"],
    ["comment", "Optional note from the reader"],
    ["start", "262.000"],                 // optional clip start for podcasts/videos, in seconds
    ["end", "295.400"],                   // optional clip end for podcasts/videos, in seconds
    ["speaker", "Maria Chen"],            // optional speaker label when transcript data exists
    ["segment", "seg-42"]                 // optional repeated transcript segment ids covered by the clip
  ]
}
```

Highlights are canonical and group-neutral. Community scoping happens when a member reposts the highlight into a group via `kind:16` with an `h` tag.

For podcast clips, the same `kind:9802` event remains canonical. The excerpt text stays in `content` when transcript text is available; timestamp tags carry the clip boundaries so the client can seek and replay the moment inside a podcast artifact page.

The relay should therefore accept canonical `kind:9802` highlights and `kind:30023` articles even when they are not tagged to any group. Only community-scoped events require `h`-tag routing.

### Discussion Events

Discussions use standard Nostr kinds within groups:
- `kind:11` (NIP-7D) for the root share/proposal thread when an artifact is introduced to a group
- `kind:1` (text note) for top-level comments on artifacts
- `kind:1111` (NIP-22 comments) for threaded replies
- Standard `e` and `p` tags for threading and mentions
- `h` tag for group routing

Replies to artifact share threads follow NIP-7D: the `kind:11` event is the root, and replies use `kind:1111`.

**Note:** The important model constraint is that Highlighter does not invent an artifact kind. Artifact identity is carried in source-reference tags, while highlights use the standard `kind:9802`.

---

## 5. Client Architecture

### Shared Rust Core

The mobile (Android/iOS) and desktop apps share a Rust core library that handles:

```
┌─────────────────────────────────────────┐
│              RUST CORE                   │
│                                          │
│  ┌──────────────┐  ┌──────────────────┐ │
│  │ Nostr Client │  │ NIP-29 Groups    │ │
│  │ - Relay mgmt │  │ - Join/leave     │ │
│  │ - Event sign │  │ - Membership     │ │
│  │ - NIP-42 auth│  │ - Moderation     │ │
│  └──────────────┘  └──────────────────┘ │
│                                          │
│  ┌──────────────┐  ┌──────────────────┐ │
│  │ Data Layer   │  │ Content Engine   │ │
│  │ - Local DB   │  │ - URL extraction │ │
│  │ - Sync       │  │ - Highlight mgmt │ │
│  │ - Cache      │  │ - Search/index   │ │
│  └──────────────┘  └──────────────────┘ │
│                                          │
│  ┌──────────────────────────────────┐   │
│  │ FFI Bridge (C ABI)              │   │
│  │ Exposes API to native UI layers │   │
│  └──────────────────────────────────┘   │
└─────────────────────────────────────────┘
         │              │            │
    ┌────▼────┐   ┌────▼────┐  ┌───▼────┐
    │ Android │   │   iOS   │  │Desktop │
    │  (Kotlin│   │ (Swift  │  │(Tauri/ │
    │   UI)   │   │   UI)   │  │ native)│
    └─────────┘   └─────────┘  └────────┘
```

### Web App

The web app is a separate codebase (modern SPA framework — specific stack TBD) that implements the same Nostr protocol interactions but through JavaScript/TypeScript Nostr libraries. It connects to the same relay infrastructure.

Key web-specific concerns:
- NIP-07 (browser extension signing) and NIP-46 (remote signing) support
- SEO for public group pages (server-side rendering or static generation for public content)
- Browser extension companion for highlight capture
- Progressive web app capabilities for mobile web

### Offline & Sync

The Rust core provides:
- Local event database (SQLite or similar)
- Background sync when connectivity returns
- Optimistic UI (post events locally, confirm when relay accepts)
- Conflict resolution follows Nostr conventions (latest timestamp wins for replaceable events)

---

## 6. Authentication & Identity

All authentication is Nostr-native:

| Method | Platform | How it works |
|---|---|---|
| NIP-07 | Web (desktop browsers) | Browser extension (nos2x, Alby, etc.) signs events |
| NIP-46 | Web, Mobile, Desktop | Remote signer (Nostr Connect) — best for cross-device |
| NIP-55 | Android | Android signer app (Amber, etc.) |
| Local keypair | Mobile, Desktop | Key stored securely on device (keychain/keystore) |

**For new users without a Nostr identity:**
- Onboarding flow generates a keypair
- User is prompted to back up their nsec or connect to a signer
- The generated key is usable immediately — no email verification, no waiting

**NIP-42 relay auth:**
- Our relay requires NIP-42 authentication for any action on restricted/private groups
- Auth challenge → client signs with user's key → relay validates membership

---

## 7. Nostr NIPs Used

| NIP | Purpose in Highlighter |
|---|---|
| NIP-01 | Core protocol (events, subscriptions, relay communication) |
| NIP-02 | Contact list / follow list |
| NIP-07 | Browser extension signing |
| NIP-11 | Relay information document |
| NIP-19 | Bech32 encoding (npub, nprofile, nevent, naddr) |
| NIP-21 | `nostr:` URI scheme (for deep linking) |
| NIP-22 | Comments (threaded discussion) |
| NIP-23 | Long-form content (if groups share long-form posts) |
| NIP-25 | Reactions |
| NIP-29 | **Core** — Relay-based groups (the foundation of communities) |
| NIP-7D | Threads (`kind:11`) for community-level artifact share / proposal posts |
| NIP-42 | Relay authentication (membership enforcement) |
| NIP-46 | Remote signing (Nostr Connect) |
| NIP-55 | Android signer integration |
| NIP-96 | File/media storage (cover images, avatars) |

---

## 8. Infrastructure Summary

| Component | Technology | Notes |
|---|---|---|
| **Relay** | Go (khatru + relay29) | Our NIP-29-compatible relay. Single deployable binary. |
| **Relay storage** | Configurable (PostgreSQL, BadgerDB, etc.) | khatru supports pluggable event stores |
| **Web app** | SPA (framework TBD) | Deployed as static site + API routes for SSR of public pages |
| **Mobile apps** | Rust core + Kotlin (Android) / Swift (iOS) | Shared logic, native UI |
| **Desktop app** | Rust core + native or Tauri | Same shared logic as mobile |
| **Media storage** | NIP-96 compatible server or S3-backed | For images, avatars, file uploads |
| **Browser extension** | JS/TS | Highlight capture from any webpage |

---

*This document covers the technical architecture. For product features, growth strategy, and user experience details, see `product-spec-v2.0.md`. For UI proposals, see `community-page-proposals-v1.4.md` and `landing-page-proposals.md`.*
