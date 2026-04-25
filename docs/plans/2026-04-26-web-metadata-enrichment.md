# Web metadata (OpenGraph) enrichment for web URL highlights

Date: 2026-04-26

## Context

`HighlightFeedCardView` renders article highlights with a rich resource header
(cover, author, real title) because `HighlighterStore` hydrates the NIP-23
article via `safeCore.getArticle(pubkeyHex:dTag:)` and the article author's
profile via `app.requestProfile(pubkeyHex:)`.

For **web URL** highlights (`source == "web"`), the header is bare:
`build_preview_with()` in `app/core/src/artifacts.rs` only normalizes the URL
and derives a domain label. It never fetches the page, so OpenGraph image,
title, site_name, etc. are empty. The Swift consumer falls back to a gradient
+ globe icon and the URL host.

## Goal

Add a Rust-side OG/favicon fetcher with a small persistent cache, expose it
via UniFFI, and surface it in `HighlightFeedCardView` so web URL highlights
look as rich as article highlights.

The pattern mirrors the iOS profile cache:
- `HighlighterStore.webMetadataCache: [String: WebMetadata]` (Observable)
- `func requestWebMetadata(url:)` lazy fetch + cache write
- `.task(id: webMetadataURL)` in the row triggers the fetch on appear

For v1 there is no push delta — the row's `.task` awaits the fetch, then the
Observable cache write triggers SwiftUI to recompute. (No `EventBridge`
plumbing needed.)

## Architecture

### Rust

New module `app/core/src/web_metadata.rs`:

```rust
pub struct WebMetadata {
    pub url: String,           // canonical URL the metadata was fetched for
    pub title: String,
    pub description: String,
    pub image: String,         // og:image absolutized
    pub site_name: String,
    pub author: String,
    pub favicon: String,       // best <link rel="icon"> resolved
    pub fetched_at: u64,       // unix seconds
}

pub async fn fetch(url: &str) -> Result<WebMetadata, CoreError>;
```

- `reqwest` client: 5s timeout, follow redirects, custom User-Agent
  `Highlighter/0.1 (+https://highlighter.com)`, `Accept: text/html`.
- Cap response body at 1 MiB. Short-circuit if `Content-Type` isn't HTML.
- Parse with `scraper` (added to Cargo.toml). Look in `<head>` for
  `og:*`, `twitter:*`, `<title>`, `meta[name=author]`, `meta[name=description]`,
  `<link rel="icon" | "apple-touch-icon" | "apple-touch-icon-precomposed">`.
- Resolve relative URLs with `Url::join`, including `<base href>` if present.
- Pick favicon: prefer apple-touch-icon (highest `sizes`), fall back to
  `<link rel="icon">`, fall back to `https://<host>/favicon.ico`.
- Failure modes: HTTP 4xx/5xx → `CoreError::NotFound`. Timeout/network →
  `CoreError::Network`. No panics.

### Cache

File-based, JSON, single file per cache dir. Simpler than sled and
zero-dependency churn.

- Path: `<runtime.data_dir()>/web_metadata.json` — already created by
  `NostrRuntime::with_data_dir`, so no extra dir setup.
- TTL: 7 days for hits, 1 hour for negative entries (so dead links don't
  hammer relays). `fetched_at == 0` flags negative entries.
- Coalesce concurrent fetches: in-flight `HashMap<String,
  Arc<tokio::sync::Notify>>`. First caller fetches; followers `await` the
  notify, then re-read the cache.
- `HighlighterCore::get_web_metadata(url)` → check cache → on miss/stale,
  fetch + write back → return.

### Swift

- `HighlighterStore` gains:
  - `var webMetadataCache: [String: WebMetadata] = [:]` (Observable)
  - `func requestWebMetadata(url: String) async` — coalesces by url, writes
    to cache on success.
- `SafeHighlighterCore` gets a `getWebMetadata(url:)` wrapper.
- `HighlightFeedCardView`:
  - New `private var normalizedWebURL: String?` derived from the
    artifact's `source == "web"` branch.
  - `.task(id: normalizedWebURL)` triggers `app.requestWebMetadata(url:)`.
  - `resourceCoverURL`, `resourceTitle`, `resourceAuthorOrDomain` consult
    `app.webMetadataCache[normalizedWebURL]` first for the web kind.
- For non-web kinds the new code path is a no-op.

## File inventory

Modified:
- `app/core/Cargo.toml` — add `scraper = "0.20"`.
- `app/core/src/lib.rs` — `pub mod web_metadata;` + re-export `WebMetadata`.
- `app/core/src/client.rs` — add `pub async fn get_web_metadata(...)` to
  the `#[uniffi::export]` impl. Hold the cache + in-flight map on
  `HighlighterCore`.
- `app/ios/.../Core/SafeHighlighterCore.swift` — `getWebMetadata` wrapper.
- `app/ios/.../Core/HighlighterStore.swift` — `webMetadataCache` +
  `requestWebMetadata`.
- `app/ios/.../Features/Highlights/HighlightFeedCardView.swift` — wire
  `.task` and read from cache in computed properties.

New:
- `app/core/src/web_metadata.rs` — fetcher + parser + cache.
- (No new Swift files — the row already exists.)

## Edge cases

- Same URL referenced by multiple rows → in-flight Notify deduplicates the
  HTTP request; followers re-read the cache once the lead caller writes.
- Highlight has both `artifactAddress` (NIP-23) AND `sourceUrl` →
  `artifactKind == .article` takes priority and we never fetch the URL.
- Highlight is `source == "podcast"` → `artifactKind != .web`, no fetch.
- Image absolute-URL resolution: handled by `Url::join`. Protocol-relative,
  root-relative, and full URLs all work uniformly.
- Privacy: every shown URL leaks the user's IP to that page. Match standard
  chat-app behavior — always fetch, no setting.
- Disk cache survives reboots; on every launch the fetcher reads the JSON
  file lazily on first request.
- 1 MiB response cap so `Content-Length: huge` pages don't blow memory; we
  also stop reading after the closing `</head>` heuristically (scraper
  parses what we give it; capping bytes is the simpler invariant).

## Verification

1. Rust unit tests:
   - `parses_open_graph_meta` — fixture HTML → expected fields.
   - `falls_back_to_title_tag_when_og_missing`.
   - `resolves_relative_image_against_base`.
   - `picks_largest_apple_touch_icon`.
   - `cache_round_trip` — write a synthetic record, read it back via
     `WebMetadataStore::get`.
   - `expires_negative_after_one_hour` — TTL bookkeeping.
2. iOS device build:
   ```
   cd /Users/pablofernandez/src/hl/app/ios/Highlighter
   xcodegen generate
   xcodebuild -project Highlighter.xcodeproj -scheme Highlighter \
     -destination 'id=00008150-001E118E3CD2401C' \
     -derivedDataPath build-device -configuration Debug build
   ```
3. Manual (Pablo, after install): open a feed showing a web highlight, see
   the OG cover + title appear within ~1s of the row entering screen.
