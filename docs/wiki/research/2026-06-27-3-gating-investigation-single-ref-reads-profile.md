---
type: research-record
date: 2026-06-27
session: 9ae03596-fa74-4208-88c6-a90bd3b176e4
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/9ae03596-fa74-4208-88c6-a90bd3b176e4.jsonl
source_lines: 4938-4973
agent_attribution: plan65s3
has_preregistered_criteria: true
has_method: true
has_structured_report: true
characterization: "Gating investigation: single-ref reads (profile/event) doable with generic vendored WASM kernel builtins; hl's real feeds upstream-blocked on NMP app-composition crate"
captured_at: 2026-06-27T06:28:15Z
---

Gating investigation: single-ref reads (profile/event) doable with generic vendored WASM kernel builtins; hl's real feeds upstream-blocked on NMP app-composition crate

---

<teammate-message teammate_id="plan65s3" color="cyan" summary="Slice 3 verdict + plan: single-ref reads doable now">
## Slice 3 (#65) — GATING VERDICT: doable NOW for single-ref reads; hl feeds upstream-blocked

**Hybrid answer, decisively (a)+(b):**

- **(a) Single-ref reads work NOW with the GENERIC vendored wasm, ZERO app composition.** `nmp-browser-runtime` unconditionally calls `.consume_all_builtin_projections()` (`crates/nmp-browser-runtime/src/builder.rs` / `dispatch.rs` `handle_start`), which enables the kernel Tier-2 builtins listed in `crates/nmp-core/src/kernel/update/builtin_projection_keys.generated.rs` — including **`refs.profile`** and **`refs.event`**. `resolve_ref` recognizes `namespace 0=Profile, 1=Event` (`crates/nmp-browser-runtime/src/wasm/ref_routing.rs`), and resolution is **fully kernel-owned** (`crates/nmp-core/src/kernel/requests/profile.rs` `resolve_profile_ref` registers a kind:0 interest, emits the `refs.profile` row). No app-registered projection is required to resolve an arbitrary profile or event by ref.

- **(b) hl's REAL feeds (highlights / articles / room timelines) ARE blocked** on upstream NMP app-composition. `crates/nmp-defaults/src/lib.rs:60-67` explicitly states `register_defaults` registers NO app-specific projections (Chirp's timeline, group-chat, etc. are wired by a per-app composition crate). There is **no** `nmp-app-chirp`/`nmp-app-gallery` crate — composition lives in the app's own crate (see `crates/nmp-example-login-timeline` `register_following_timeline`). hl's web feeds would need an equivalent **`nmp-app-highlighter` registration baked into a NEW wasm build** — an upstream NMP task, NOT landable in hl this slice.

**Critical architecture note:** hl's `app/core/src/kernel/domains/*_feed.rs` (highlight_feed, home_feed, articles_feed) are the **bespoke NATIVE HighlighterCore** and are **NOT compiled into the NMP wasm** — the web bridge runs the GENERIC NMP kernel, not hl's core. So web reads will never come from `app/core`; they come from generic kernel builtins (profiles/events) or a future NMP app crate. Consistent with the unwired-lane memory.

## The smallest landable proof (Slice 3): resolve ONE profile by pubkey
Reference implementation already exists in `nostr-multi-platform/web/chirp` — port it. All additive; no NDK read removed.

**Decode chain (the real work):** `update_bytes` (NMPU `UpdateFrame`) → `SnapshotFrame.typedProjections[]` → match key `"refs.profile"` + fileId `"NRRD"` (`RefRowDeltaBatch`) → each row payload is `ProfileSnapshot` (`KPRF`) → `ProfileCard` fields. Schemas: `crates/nmp-core/schema/{nmp_update,profile,profile_card,ref_rowdelta}.fbs`.

**Files (hl/web), all additive:**
1. **Vendor generated TS decoders** from `nostr-multi-platform/web/chirp/src/nmp/generated/nmp/` → `web/src/lib/nmp/generated/`. Minimal subset: `transport/{update-frame,snapshot-frame,typed-projection,typed-payload}.ts`, `kernel/{profile-snapshot,profile-card}.ts`, `refs/ref-row-delta-batch.ts`. **`flatbuffers ^25.9.23` is already a hl dep** (matches NMP's pinned version).
2. **`web/src/lib/nmp/runtime-web/updateFrameDecoder.ts`** — port chirp's `feedDecoder.ts` + `refProfileStore.ts`. `decodeUpdateFrame(bytes)`: assert NMPU id, iterate typedProjections, return `{ projectionKeys: string[], profiles: ProfileWire[] }`.
3. **`client.svelte.ts`** — add `resolveRef(namespace, key, shape, liveness, consumerId)` + `releaseRef(...)` methods (chirp `client.ts` has the exact shape; protocol.ts already defines the variants). Decode `latestUpdateBytes` into a typed projection on each snapshot. **No worker.ts change needed** — `worker.ts:40` already forwards every request to `bridge.handle()` → `handle_json`.
4. **Probe page** `(dev)/nmp-probe/+page.svelte` — add `?resolve_profile=<pubkey>` param: after start, call `resolveRef(0=Profile, pubkey, shape=Ref, liveness=Live)`; surface `data-resolved-profile-displayname` and `data-projection-keys`. Release on unmount (consumer_id discipline — Live liveness holds a tailing sub).
5. **Test infra** — port `nostr-multi-platform/web/chirp/tests/fixture-relay.ts` (in-process Node `ws` relay, hermetic/CI-safe, seeds genuinely-signed events) to `web/tests/`. A minimal single-kind:0 variant suffices.

**TDD order (keep `bun run build` + fallback tier + existing @wasm boot/sign tiers green):**
- **Step 0 — deterministic, NO relay (land first):** decode the boot snapshot frame that's *already* emitted on Start (today's `data-has-snapshot=true`). Surface `data-projection-keys`; @wasm test asserts the frame decodes (NMPU) and lists builtin keys. Proves the decoder against real wasm output with zero relay flakiness.
- **Step 1 — read proof, fixture relay:** boot probe with `?relay_bootstrap=[fixtureUrl]&resolve_profile=<seededPubkey>`; assert `data-resolved-profile-displayname == "Alice Fixture"`. (`?relay_bootstrap=` already supported by the probe.)

## Risks
- **FlatBuffers drift (LOW):** vendor decoders from the SAME NMP checkout the wasm was built from; NMP enforces the 25.9.23 pin (hl already matches). Recommend extending `web/scripts/build-nmp-wasm.sh` to also copy generated TS so wasm + decoders never drift.
- **Relay determinism (SOLVED):** the ported in-process fixture relay serves seeded kind:0 on REQ and EOSEs unknown filters — handles the NIP-65 kind:10002 discovery hop hl's chirp feed test already exercises end-to-end.
- **SSR:** keep the decoder pure (no worker/DOM globals) so SSR import is safe; it's only invoked client-side (client is already browser-gated).
- **App-composition dependency:** this slice migrates NO NDK read surface — NDK still owns all reads. Real hl feeds remain upstream-blocked on `nmp-app-highlighter`.

**Future NDK read surfaces to migrate (NOT this slice), in `web/src/lib/ndk/`:** `highlights.ts` (7 subs, do last — needs app-composition), `artifacts.ts` (3), `groups.ts` (3), `search.ts` (2), `client.ts` (2), `config.ts` (1). Order: profile/event single-refs (generic, now) → article/event-by-naddr → feeds (blocked on upstream).

**Bottom line:** Slice 3 keeps #65 moving this session — land the profile single-ref read proof (Step 0 then Step 1) on the generic vendored wasm. hl's real feeds are a separate, upstream NMP task; do not attempt them here.
