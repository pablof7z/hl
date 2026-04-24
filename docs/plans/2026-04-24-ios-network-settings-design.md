# iOS Network Settings — Design

Status: **design** — not yet implemented. Supersedes the hardcoded
`DEFAULT_RELAYS` const.

Architecture contract: **nostrdb is the single source of truth.** Relay
config is persisted to relay-owned events (kind:10002, kind:30078) which
hydrate nostrdb via the Rust core; Swift reads config from nostrdb and
reacts to deltas. Live ephemeral telemetry (connection state, latency,
event counters) is not in nostrdb — it comes from the Rust core's
nostr-sdk `Client::handle_notifications` pump, bridged to Swift via
`EventBridge` with a new `RelayStatusChanged` delta.

## Goals

1. **Transparency** — user can see what their device is talking to and
   how healthy each relay is.
2. **Control** — user can add, remove, and retarget relays without
   rebuilding the app.
3. **Interop** — NIP-65 (kind:10002) published on every edit so the
   user's identity moves with them across clients.
4. **Approachable for non-technical users** — no intimidating
   classification questions; sane defaults for every add.

## Data model

```rust
pub struct RelayConfig {
    pub url: String,       // wss://...
    pub read: bool,        // NIP-65 read ("inbox")
    pub write: bool,       // NIP-65 write ("outbox")
    pub rooms: bool,       // NIP-29 group subs run here
    pub indexer: bool,     // outbox-bootstrap pool for kind:0/3/1xxxx
}
```

Four roles. All four are user-editable per relay. Same URL never appears
twice — one row per URL, chips toggle roles.

## Persistence

Split by what each flag is:

- **`read` / `write`** → **kind:10002 (NIP-65)**. Nostr identity;
  interops with Amethyst/Damus/Primal. Re-published on every edit,
  debounced 1s. Hydrated from nostrdb on login.
- **`rooms` / `indexer`** → **kind:30078 NIP-78 app-data** with
  `d = "com.highlighter.relays"`. Highlighter-specific routing, not
  nostr identity. Self-published; hydrated from nostrdb.

`DEFAULT_RELAYS` const is replaced by `fn seed_defaults() -> Vec<RelayConfig>`
used only when no kind:10002 and no app-data event exist. Seed values:

| URL                           | R | W | Rooms | Indexer |
| ----------------------------- | - | - | ----- | ------- |
| `wss://relay.highlighter.com` | ✓ | ✓ | ✓     |         |
| `wss://relay.damus.io`        | ✓ | ✓ |       |         |
| `wss://purplepag.es`          |   |   |       | ✓       |
| `wss://relay.primal.net`      |   |   |       | ✓       |

If the user already has a kind:10002 on the network, pull it — respect
their existing outbox — and only layer Rooms/Indexer on top from
defaults.

## Rust runtime changes

**`nostr_runtime.rs`:**

- Remove `spawn_connect`'s hardcoded `DEFAULT_RELAYS` loop.
- New `apply_relay_config(&[RelayConfig])` diffs current pool vs config:
  adds new URLs with proper `RelayMetadata` (read/write flags), removes
  stale URLs, updates existing rows' flags.
- Called on bootstrap with the result of `load_relay_config()` and
  re-called after every `upsert_relay` / `remove_relay` FFI call.

**Per-role routing** (the correctness payoff):

- NIP-29 subs (groups, admins, members, metadata) target relays with
  `rooms = true` via `SubscribeOptions::new().relays([...])`.
- Outbox-model lookups (kind:0, 3, 1xxxx for arbitrary pubkeys) target
  `indexer = true`.
- Event publishing targets `write = true`; a kind:39xxx group event
  additionally targets the `rooms` relay hosting that specific group.
- Relays with none of the four flags on are configured but not
  connected — an explicit "paused" state.

**New `nostrdb` + `Client` bridge for live telemetry:**

- `spawn_relay_status_pump()` on runtime start: listens to
  `Client::handle_notifications` for `RelayStatus::{Connected,
  Disconnected, Terminated}`, plus periodic RTT probe (small REQ/EOSE
  round-trip every 30s per connected relay).
- Maintains an in-memory `HashMap<String, RelayDiagnostic>`:
  ```rust
  pub struct RelayDiagnostic {
      pub url: String,
      pub state: RelayState,     // Connecting | Connected | Disconnected | Paused
      pub rtt_ms: Option<u32>,
      pub events_in: u64,        // since session start
      pub events_out: u64,
      pub subs_active: u32,
      pub last_notice: Option<String>,  // relay NOTICE message, if any
      pub nip11: Option<Nip11Document>,
  }
  ```
- Emits `RelayStatusChanged { url }` deltas through the same
  subscription-id plumbing `EventBridge` already uses.

## FFI surface

```rust
// config
fn get_relays() -> Vec<RelayConfig>;
fn upsert_relay(cfg: RelayConfig) -> Result<(), CoreError>;
fn remove_relay(url: String) -> Result<(), CoreError>;
fn set_relay_roles(url: String, read: bool, write: bool,
                   rooms: bool, indexer: bool) -> Result<(), CoreError>;
fn reconnect_all() -> Result<(), CoreError>;

// live telemetry
fn get_relay_diagnostics() -> Vec<RelayDiagnostic>;
fn subscribe_relay_status() -> u64;       // handle for EventBridge
fn probe_relay_nip11(url: String) -> Result<Nip11Document, CoreError>;
fn test_relay(url: String) -> Result<u32, CoreError>;  // RTT ms

// cache
fn get_cache_stats() -> CacheStats;       // bytes, event count, oldest ts
fn clear_cache_non_owned() -> Result<u64, CoreError>; // returns bytes freed

// import helper
fn import_relays_from_npub(npub: String) -> Result<Vec<RelayConfig>,
                                                   CoreError>;
```

All published via `SafeHighlighterCore` as `async throws` methods.

## Swift screens

Feature folder: `Features/Settings/Network/`.

### `NetworkSettingsView` — main screen

Sections top to bottom:

1. **Header card** — big, at top:
   - Pill: "Online — 3 of 4 relays" / "Connecting…" / "Offline"
   - Subtext: "12 subscriptions active • ↓ 1.2k events this session"
   - Tap → overlay with full-session diagnostics.

2. **Relays list** — each row:
   - URL (truncated middle if long)
   - Status indicator: `●` green connected / `◐` yellow connecting /
     `○` red disconnected / `·` gray paused
   - Latency next to it when connected ("34 ms")
   - Role chips on the second line: `Read` `Write` `Rooms` `Indexer`
     — tappable to toggle. Off chips are dimmed, not hidden.
   - Small muted traffic counter on the right ("↓ 248 ↑ 12") when
     connected.
   - Swipe-to-delete; long-press → "Reconnect now".
   - Tap row → `RelayDetailView`.

3. **Add button** — toolbar `+` opens `AddRelaySheet`.

4. **Import section** — single row "Import from another user…" → sheet
   that takes an npub, fetches their kind:10002 via indexer relays,
   previews their relays with checkboxes, merges selected into the
   user's list (non-destructive).

5. **Local cache** — inset card:
   - "2.1 GB • 48,203 events • oldest: 12 Feb 2026"
   - Destructive button: "Clear non-owned events" (keeps your own
     published events).

6. **Connect on cellular** toggle — default on. Off → NWPathMonitor
   pauses the pool until Wi-Fi returns. Footer:
   *"When off, Highlighter only syncs over Wi-Fi to save mobile data."*

7. **Footer**:
   *"Your Read and Write relays are published as a kind:10002 event.
   Other nostr users can see where you read and publish."*

### `RelayDetailView` — per-relay drilldown

Big status header (URL, state, RTT, uptime-this-session). Then:

- **Roles** — same chip row, toggleable.
- **Traffic** — events in / out / subs active, simple numeric readout.
- **NIP-11** — expandable card: software name + version, contact,
  supported NIPs as chips, limitations (max message length, auth
  required, payment required). Fetched lazily; cached.
- **Connection log** — last 20 state transitions with timestamps
  ("Connected 12:34:56", "Disconnected: handshake timeout 12:35:02").
- **Actions** — Reconnect / Disconnect / Remove (destructive).

### `AddRelaySheet`

Single text field (URL) + role chips below it.

- Validates `wss://` or `ws://` prefix. Warn in-line on `ws://`
  (unencrypted).
- **Paste detect**: on open, if clipboard contains a valid wss URL not
  already in the list, offer a one-tap "Paste `wss://foo.relay/`"
  button above the field.
- **NIP-11 probe** on blur / debounced typing: once URL parses, fire
  `probe_relay_nip11`. Show software name + icon next to the field in
  green if OK, muted red if unreachable. Probe failure doesn't block
  add (relays go up and down) — just a warning.
- **Test button**: round-trip a trivial REQ/EOSE before saving;
  displays RTT.
- **Default chips**: `Read ✓`, `Write ✓`, `Rooms ○`, `Indexer ○`.
  User taps to change before Add.

## Safety rails

Three first-principles guardrails, surfaced inline (no separate warnings
screen):

1. **Orphan-rooms check**: removing a relay with `rooms = true` that
   hosts joined rooms → confirmation sheet lists the affected rooms and
   asks to confirm or leave the rooms first.
2. **No-outbox warning**: turning off the last `write = true` relay →
   inline banner *"You have no outbox relays. Your posts won't reach
   anyone."* with a one-tap "Add back" action.
3. **No-indexer warning**: turning off the last `indexer = true` relay
   → inline banner *"Profile lookups for other users may fail until you
   add an Indexer relay."*

## Non-goals (YAGNI)

Deliberately **not** in this design:

- Per-kind routing rules beyond the four roles above.
- Paid-relay / NIP-42 AUTH flow (relays that need auth are listed as
  "requires auth" from NIP-11 but not automated — user can still try).
- A diagnostics "events by kind" breakdown screen — nobody's asked for
  it; add later if people do.
- Manual subscription editor — opaque for non-technical users; the
  diagnostics log is enough.

## Implementation order

Each PR is independently shippable.

1. **Rust — config storage**: `RelayConfig` + NIP-65 publish/hydrate +
   NIP-78 app-data publish/hydrate + `seed_defaults`. No runtime
   consumption yet.
2. **Rust — runtime reconciliation**: `apply_relay_config`; remove
   `DEFAULT_RELAYS` from the boot path.
3. **Rust — per-role routing**: NIP-29 subs target `rooms`, outbox
   lookups target `indexer`, publishes target `write`.
4. **Rust — telemetry pump**: `RelayDiagnostic` + status delta
   plumbing.
5. **Swift — `NetworkSettingsView`** (config only, no live state yet):
   list, chip toggles, add sheet with NIP-11 probe.
6. **Swift — live telemetry**: header card, per-row status dots &
   latency, reconnect action.
7. **Swift — `RelayDetailView`** with NIP-11 card & connection log.
8. **Swift — advanced**: import from npub, paste detect, safety rails
   (orphan/no-outbox/no-indexer banners), cache stats + clear, Wi-Fi-
   only toggle.

Each step lands with a commit + build + install + launch on Pablo's
iPhone for visible changes, per project convention.
