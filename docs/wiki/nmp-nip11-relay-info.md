---
title: NMP NIP-11 Relay Info
slug: nmp-nip11-relay-info
topic: nostr-protocol
summary: NMP (nostr-multi-platform) provides first-class NIP-11 relay info (name, icon, description, pubkey, contact, software, version, supported_nips, payment_required
tags:
  - capture
volatility: warm
confidence: medium
created: 2026-06-12
updated: 2026-06-12
verified: 2026-06-12
compiled-from: conversation
sources:
  - session:f54b4a16-dacb-41e6-b32f-b737d606254f
---

# NMP NIP-11 Relay Info

## Purpose

NMP (nostr-multi-platform) provides first-class NIP-11 relay info (name, icon, description, pubkey, contact, software, version, supported_nips, payment_required, auth_required, restricted_writes) through the relay_diagnostics projection, eliminating the need for apps to perform their own HTTP fetching or NIP-11 parsing. Relay list rows display the relay icon from the NIP-11 document. NIP-11 fetching and parsing is implemented entirely within NMP, not in Highlighter; Highlighter apps receive relay info through NMP's diagnostics surface with zero awareness of NIP-11.

<!-- citations: [^f54b4-3] [^f54b4-10] [^f54b4-20] -->
## Highlighter Integration

After NMP PR #1195 merges and releases, Highlighter deletes relay_polish.rs and the ProbeNetworkRelayNip11 action plumbing, maps NMP-provided row.info into the existing network.nip11 projection, and routes the add-relay preview through NMP's probe API. The FFI-visible shape of relay info stays identical across the integration, so iOS and Android need zero app-side changes to display NIP-11 data.

<!-- citations: [^f54b4-4] [^f54b4-11] [^f54b4-21] -->
## Fetching & Caching

NIP-11 data is automatically fetched when a relay connects, cached with a 5-minute per-URL TTL, and also available via an on-demand C-ABI probe API for add-relay preview flows (URLs not yet in the pool). The HTTP fetch runs off-thread with a 64 KiB response cap and a 10-second timeout, keeping the actor non-blocking. Relay probe operations are keyed by a stable hash of the relay URL so that different relays probe independently while re-probing the same URL supersedes the previous in-flight probe.

<!-- citations: [^f54b4-5] [^f54b4-12] -->
## Architecture & Trigger

NIP-11 relay info is implemented in a dedicated nmp-nip11 protocol crate using the off-thread worker pattern, keeping HTTP dependencies (ureq) out of the wasm target tree and nmp-core doctrine-clean. The crate follows the ADR-0043 precedent of keeping HTTP dependencies out of nmp-core (D0 rule: nmp-core names no NIP-11 noun and imports no HTTP crate) and mirrors the nmp-nip57/nmp-blossom pattern; protocol crates satisfy their own doctrine lint (D8 rule). The wasm build includes no HTTP crate (ureq is absent from the wasm dependency tree). A new RelayConnectedHook substrate seam in nmp-core fires on every relay connect, which the nmp-nip11 crate registers to trigger NIP-11 fetches, using the same hook pattern established by V-38's RelayTextInterceptorSlot. The ADR for NIP-11 in NMP is numbered ADR-0051, renumbered from the colliding 0049 (claimed by the defaults-yield decision) and 0050 (claimed by the signer-session-port branch).

<!-- citations: [^f54b4-6] [^f54b4-13] [^f54b4-22] -->
## FFI & Ownership Model

The C-ABI probe function (nmp_app_probe_relay_info) uses the borrowed-during-callback string model consistent with the UpdateCallback precedent, not the retired owned *mut c_char model; it therefore requires no nmp_free_string. The probe was extracted into its own relay_info_probe.rs module to minimize per-file LOC growth in large existing files.

<!-- citations: [^f54b4-7] [^f54b4-15] [^f54b4-23] -->
## Release Packaging

The nmp-nip11 crate is registered in the release manifest (release/nmp-release.toml) in NIP-number order, and the Cargo.lock version for nmp-nip11 inherits from the workspace (0.6.0).

<!-- citations: [^f54b4-8] [^f54b4-16] -->
## Codegen & Schema Gating

The RelayStatus.info field (of type Option<RelayInfoDoc>) is gated out of the codegen-schema JSON schema with schemars(skip), because iOS consumes relay info via the diagnostics projection and FlatBuffers sidecar, not through the flat KernelTypes.generated.swift mirror; thus zero Swift types change. <!-- [^f54b4-14] -->
