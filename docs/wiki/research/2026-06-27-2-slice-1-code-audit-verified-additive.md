---
type: research-record
date: 2026-06-27
session: 9ae03596-fa74-4208-88c6-a90bd3b176e4
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/9ae03596-fa74-4208-88c6-a90bd3b176e4.jsonl
source_lines: 4189-4210
agent_attribution: review65s1
has_preregistered_criteria: true
has_method: true
has_structured_report: true
characterization: "Slice 1 code audit - verified additive-ness, SSR-safety, degradation honesty, no secrets - APPROVE WITH NITS verdict"
captured_at: 2026-06-27T05:39:56Z
---

Slice 1 code audit - verified additive-ness, SSR-safety, degradation honesty, no secrets - APPROVE WITH NITS verdict

---


Reviewed committed state `git diff origin/main...HEAD`. Clean additive bootstrap. No blocking issues.

### Verified-correct
- **Additive-ness (critical): PASS.** `git diff --name-only` shows only new files under `web/src/lib/nmp/`, `web/scripts/build-nmp-wasm.sh`, `web/src/routes/(dev)/nmp-probe/+page.svelte`, `web/playwright.config.ts`, `web/tests/nmp-bridge.spec.ts`, plus manifest-only edits to `web/package.json` + `web/bun.lock`. The "1 deletion" is the package.json `check` line being rewritten (kept, comma added) — not a code removal. Zero existing NDK/route/component files touched. No `(dev)` route collision (only `nmp-probe` exists under it).
- **SSR-safety: PASS.** `client.svelte.ts:15-22` imports only `degradedRuntime` + `protocol` at module top — no wasm, no worker. Worker is constructed lazily inside `WorkerNmpClient`'s constructor, only reached via `getClient()` from `onMount`+`browser` guard (`+page.svelte:22-25`). `createNmpClient()` gates on `typeof Worker === "undefined"` (`client.svelte.ts:71`). The wasm module is `await import(/* @vite-ignore */ moduleUrl)`'d only inside `worker.ts` (`wasmBridge.ts:170`), behind a HEAD-fetch availability check (`wasmBridge.ts:167,186-202`). Nothing wasm/worker runs at SSR or module-top.
- **Honest degradation: PASS.** `InProcessNmpClient` drives `DegradedRuntime("browser_bridge_unavailable")`; never throws — `start` emits `runtime_status {degraded: mode}` and every capability returns `capability_failure`/`sign_failed` (`degradedRuntime.ts:21-131`). Probe surfaces `data-bridge-kind="in_process_fallback"` and `data-runtime-status="degraded:browser_bridge_unavailable"`. Fallback test (`nmp-bridge.spec.ts:40-65`) deletes `window.Worker` via `addInitScript` before load and asserts exactly those attrs + `data-has-snapshot="false"`. Correct.
- **Vendored files: PASS.** All 7 runtime-web imports resolve — only relative `./` or `flatbuffers` (pinned `^25.9.23` in package.json/bun.lock). No NMP-internal/`chirp`/`$lib` paths leaked. Vendoring (vs path dep) is appropriate given the separate repo + stated "no Rust toolchain on Vercel/CI" posture.
- **Worker instantiation: PASS.** `new Worker(new URL("./runtime-web/worker", import.meta.url), {type:"module"})` (`client.svelte.ts:154`) — path is correct relative to the store and is the Vite-6 statically-analyzable form that bundles a module worker for adapter-vercel.
- **prebuild deviation: BENIGN.** `prebuild` wires `scripts/generate-changelog.js`, which is **already tracked on origin/main** (not a new file). It writes `web/src/lib/changelog.json`, which **is gitignored** — confirmed it won't dirty the tree. Additive, no runtime behavior change.
- **No secrets / no large committed artifacts.** `git ls-files web/static/nmp-wasm/` is empty — the @wasm tier ships correctly skipped (`test.skip` at `nmp-bridge.spec.ts:71`), exactly the acceptable landing you described.

### Sound foundation? Yes.
The worker protocol already enumerates the full action surface (resolve/release_ref, dispatch_bytes binary doorway, set_identity, begin_sign/deliver_signer_response, routing_decisions) with honest fail-closed handlers, and `wasmBridge.ts` has the binary dispatch + OPFS `prepare_store` hooks stubbed. Clean base for signer → reads → writes → persistence.

### Non-blocking nits
1. **`(dev)` route group still ships to production.** SvelteKit parenthesized groups are organizational only — they don't gate by env, so `/nmp-probe` is reachable in a prod build despite the "Dev-only" comment (`+page.svelte:2`). Harmless today (degrades to capability_failure, no wasm committed), but before GA consider a guard or `export const prerender = false` + removal. Not a Slice 1 blocker.
2. **When the wasm artifact lands it's a ~6.6MB committed binary** (`nmp-browser-runtime_bg.wasm`). Consistent with the stated iOS-sibling vendoring posture, but flag it for repo-size awareness / possible Git LFS later.

### Heads-up on working-tree state (NOT part of the committed slice)
The worktree is currently dirty from the parallel wasm build: `web/static/nmp-wasm/` is populated (untracked, 6.6MB wasm + js + d.ts), and `web/tests/nmp-bridge.spec.ts` has an **uncommitted** edit flipping the @wasm `test.skip` → `test`. My verdict is on the committed HEAD (skip intact). When you fold the @wasm enablement in, commit the artifact alongside the un-skip so CI can actually run that tier — and re-run the @wasm spec green before claiming it. (`web/test-results/` is also untracked Playwright output — gitignore it.)

