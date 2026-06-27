---
type: research-record
date: 2026-06-27
session: 9ae03596-fa74-4208-88c6-a90bd3b176e4
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/9ae03596-fa74-4208-88c6-a90bd3b176e4.jsonl
source_lines: 4620-4678
agent_attribution: plan65s2
has_preregistered_criteria: true
has_method: true
has_structured_report: true
characterization: "Investigation of upstream signer implementation: NIP-07 wired in WASM runtime, NIP-46 and nsec signing blocked (upstream #2119/#2068); TDD plan provided for NIP-07 path"
captured_at: 2026-06-27T06:28:15Z
---

Investigation of upstream signer implementation: NIP-07 wired in WASM runtime, NIP-46 and nsec signing blocked (upstream #2119/#2068); TDD plan provided for NIP-07 path

---

<teammate-message teammate_id="plan65s2" color="orange" summary="Slice 2 signer-boundary TDD plan delivered">
Slice 2 (M1) investigation complete for /Users/pablofernandez/Work/hl. Read-only; no code written. Lead finding first, then the contract, current model, design, and an ordered TDD plan.

## LEAD: the signer broker IS wired upstream — but NIP-07 ONLY. NIP-46 + local-key are runtime-blocked.

The vendored wasm artifact (`web/static/nmp-wasm/nmp-browser-runtime_bg.wasm`, landed Slice 1.5) was inspected via `strings`. It contains the full S6 sign broker (`set_identity`, `begin_sign`, `deliver_signer_response`, `sign_request`/`sign_completed`/`sign_failed`) AND this verbatim string:
  - `only "nip07" is wired. NIP-46 bunker signing is deferred to #2119/#2068.`
  - `unsupported_signer_kind: "<kind>` (the `local_key`/nsec path returns a capability failure)
  - `signer_not_installed: ... send WorkerRequest::SetIdentity with kind = "nip07" before dispatching app-level writes.`

So Slice 2 is **executable for NIP-07 extension login only**. The bunker (NIP-46) and private-key (nsec) login paths CANNOT move to the bridge yet — the Rust runtime rejects them. This is a partial block, and it dictates the scope line below. Upstream NMP issues to track: **#2119 / #2068** (NIP-46), and the `unsupported_signer_kind` local-key door.

## 1. The bridge contract (cite-exact)
Protocol: `web/src/lib/nmp/runtime-web/protocol.ts`. The TS the hl store must call (all already typed in hl's protocol.ts, just not exposed by the client):
- **Identity**: `WorkerRequest { type:"set_identity", kind:"nip07", pubkey_hex, correlation_id, identity_relays? }`. Host calls `window.nostr.getPublicKey()` first and passes the hex. Does NOT install a persistent signer — signing is a per-request capability round-trip. (`kind:"local_key"` carries `secret_key_bech32` verbatim to Rust but currently returns `unsupported_signer_kind`.)
- **Sign round-trip**: `beginSign` posts `{type:"begin_sign", account_pubkey, unsigned_json}` → worker emits `WorkerEvent {type:"sign_request", correlation_id, account_pubkey, unsigned_json}` → main thread calls `window.nostr.signEvent` and posts back `{type:"deliver_signer_response", correlation_id, signed_json|error}` → worker emits `{type:"sign_completed", correlation_id, signed_json}` or `{type:"sign_failed", correlation_id, reason}`. `sign_request` is NOT correlation-resolving (it's a broker instruction); `sign_completed`/`sign_failed` ARE (see `eventCorrelationId`). Account-pinned: worker rejects a signature from a different pubkey.

**Reference impl to port (it is complete and working):**
- `../nostr-multi-platform/web/chirp/src/nmp/client.ts` — `setSigner(pubkeyHex, identityRelays?)`, `setLocalKeySigner(...)`, `beginSign(...)`, and the `accept()` branch that routes `sign_request` → `fulfilSignRequestViaExtension`.
- `../nostr-multi-platform/web/chirp/src/nmp/signBroker.ts` — `fulfilSignRequestViaExtension(post, correlationId, unsignedJson, accountPubkey)`: extension presence check, account-pin guard via `getPublicKey()`, `signEvent`, posts `deliver_signer_response`. Fail-closed on every error.

The mechanics work synchronously through `wasmBridge.handle()` → `handle_json` (begin_sign returns `[sign_request]`; deliver_signer_response returns `[sign_completed|sign_failed]`); `worker.ts` already forwards them. No worker changes needed.

## 2. hl/web's current session + signer model
- NDK instance + session manager: `web/src/lib/ndk/client.ts` — `createNDK({ session: { storage: new LocalStorage('highlighter:sessions'), autoSave: true, ... } })`. **Source of truth today = `ndk.$sessions`** (persisted to localStorage), active user = `ndk.$currentUser` / `ndk.activeUser`, signing = `ndk.signer` used by `NDKEvent.sign()/.publish()`.
- Login entry point: `web/src/lib/features/auth/LoginDialog.svelte` — three paths, all `await ndk.$sessions.login(signer)`:
  - extension → `new NDKNip07Signer()` (line 82)  ← **the only path the bridge can own this slice**
  - private-key → `new NDKPrivateKeySigner(...)` (97)  ← bridge-blocked
  - bunker/remote → `new NDKNip46Signer(...)` / `prepareRemoteSignerPairing` in `auth.ts` (146, 114) ← bridge-blocked
- Logout: `web/src/lib/features/auth/AuthPanel.svelte:53` `ndk.$sessions.logout()`.
- Blast radius: `ndk.$currentUser`/`ndk.$sessions`/`activeUser` are read reactively in ~20 components (HighlightForm, ArticleView, DiscussionComposer, AuthGuard, +layout.svelte, etc.). This is exactly why NDK session/signer must stay — ripping it out is out of scope and would break reads/writes.
- The hl NMP client (`web/src/lib/nmp/client.svelte.ts`) currently exposes ONLY `hello()` + `start()`. It has no `setSigner`/`beginSign`/`sign_request` handling — this is the gap to fill.

## 3. Cutover design (recommended: parallel authority, NIP-07 only)
- Do NOT migrate reads or writes (S3/S4). NDK keeps its signer and keeps publishing. The bridge becomes a **parallel** identity+signing authority proven end-to-end, not yet the write path.
- Port `setSigner` + `beginSign` + the `sign_request`→`signBroker` wiring into `web/src/lib/nmp/client.svelte.ts` (extend `NmpClient`, `WorkerNmpClient`, `InProcessNmpClient`; add `web/src/lib/nmp/runtime-web/signBroker.ts` adapted to hl's relative imports). Keep `degradedRuntime` honest-failing these.
- Wire `loginWithExtension()` in `LoginDialog.svelte`: after `ndk.$sessions.login(new NDKNip07Signer())` succeeds, also `await getClient().setSigner(activeUser.pubkey)`. Gate behind `hasNostrExtension()`. Leave private-key + bunker branches untouched (NDK-only) — add a one-line comment citing #2119/#2068 as the reason they don't call the bridge yet.
- Answer to "does NDK still need its own signer": **YES, this slice.** Routing NDK's writes through the bridge broker is S4. Keep them independent now; the bridge owns its own nip07 round-trip, NDK owns its own.
- NIP-46-via-broker is a tempting generalization (main thread could fulfil `sign_request` by calling an `NDKNip46Signer.sign` instead of `window.nostr`), but it's **out of scope**: `set_identity` has no nip46 kind and the runtime defers it. Note it as the S-future seam, don't build it.

## 4. TDD plan (one PR, additive, build + Playwright green)
Order:
1. **Add `web/src/lib/nmp/runtime-web/signBroker.ts`** — port `fulfilSignRequestViaExtension` (swap `@nmp/runtime-web` import for `./protocol`). 
2. **Extend `web/src/lib/nmp/client.svelte.ts`** — add to the `NmpClient` type + both classes: `setSigner(pubkeyHex, identityRelays?)`, `beginSign(accountPubkey, unsignedJson)`, and in `WorkerNmpClient.accept()` add the `sign_request` → `fulfilSignRequestViaExtension((r)=>this.worker.postMessage(r), …)` branch (do NOT add local-key/nip46 helpers — omit to keep the unsupported paths off the surface). Surface `latestSignedEvent`/sign events in the snapshot if convenient for the probe.
3. **Extend the dev probe** `web/src/routes/(dev)/nmp-probe/+page.svelte` — add a test-only path: read `?set_identity_pubkey=` and `?begin_sign=` query params (mirroring the existing `?relay_bootstrap=` pattern), call `setSigner` then `beginSign`, and expose new hooks: `data-identity-set`, `data-sign-result` ("completed:<id>" | "failed:<reason>" | "pending"). Keeps it dev-only, no nav link.
4. **Playwright** `web/tests/nmp-bridge.spec.ts` (extend; same `@wasm` tag gating):
   - `@wasm set_identity installs a nip07 identity without capability_failure` — inject a mock `window.nostr` (getPublicKey + signEvent that returns a deterministic signed event) via `page.addInitScript`, navigate `/nmp-probe?set_identity_pubkey=<hex>`, assert `data-identity-set="true"` and no `capability_failure` event.
   - `@wasm sign broker round-trips a nip07 signature via the bridge` — same mock, navigate with `?set_identity_pubkey=<hex>&begin_sign=<unsignedJson>`, assert `data-sign-result="completed:<id>"`. The mock `signEvent` proves the round-trip without a real extension; the bridge proves account-pin + `sign_completed`.
   - Keep the fallback (non-`@wasm`) tier asserting `setSigner`/`beginSign` on `InProcessNmpClient` return honest `capability_failure` (no crash).
   - Mirror the mock-`window.nostr` helper from chirp's tests if present (`../nostr-multi-platform/web/chirp/tests/`).
5. `bun run build` + `bun run test:e2e:fallback` (PR-blocking) green; `@wasm` tier green locally where the artifact is present.

## 5. Risks / unknowns
- **NIP-46 + local-key are runtime-blocked** (evidence above) — the single biggest constraint. Bunker/nsec logins stay NDK-only; flag #2119/#2068.
- **`set_identity` needs no relay connection** for nip07 (pubkey-only, no persistent signer install) — safe to call right after login, independent of `start()`. `identity_relays` is optional.
- **SSR/session-restore timing**: `getClient()` is browser-gated; call `setSigner` from `onMount`/login handlers only. On page reload, NDK restores the session from localStorage but the bridge does NOT auto-restore identity — if you want the bridge identity to survive reload this slice, re-call `setSigner` from a browser-only effect that watches `ndk.$currentUser` and only fires for the extension signer kind. (Recommend: yes, add that effect so the bridge authority isn't lost on refresh.)
- **Scope line**: writes/reads stay 100% on NDK; the bridge owns only the nip07 identity + sign round-trip, proven via the probe. Do not touch `ndk.$sessions` storage, logout, or the ~20 `$currentUser` consumers.

This is executable by a sonnet engineer as one additive PR, NIP-07 path only. Want me to also draft the exact diffs for client.svelte.ts + signBroker.ts, or file the NMP tracking note for the NIP-46 gap?
