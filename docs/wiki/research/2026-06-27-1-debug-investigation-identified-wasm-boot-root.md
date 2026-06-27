---
type: research-record
date: 2026-06-27
session: 9ae03596-fa74-4208-88c6-a90bd3b176e4
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/9ae03596-fa74-4208-88c6-a90bd3b176e4.jsonl
source_lines: 4372-4427
agent_attribution: coder65boot
has_preregistered_criteria: true
has_method: true
has_structured_report: true
characterization: "Debug investigation: identified WASM boot root cause (missing sqlite3 artifacts in static files), implemented fix, verified with Playwright tests (@wasm tier green)"
captured_at: 2026-06-27T06:53:57Z
---

Debug investigation: identified WASM boot root cause (missing sqlite3 artifacts in static files), implemented fix, verified with Playwright tests (@wasm tier green)

---

User: [Agent task result: Agent "Debug + enable #65 wasm boot" finished]
Here is the full DoD report.

---

SUMMARY: Slice 1.5 complete — real WASM runtime boots in the browser worker, @wasm E2E tier green.

ANALYSIS:

Root cause (worker console error that would have appeared):

The wasm JS glue (`nmp-browser-runtime.js`) begins with three static top-level imports from `./snippets/nmp-sqlite-wasm-ee46999c2490e92d/vendor/sqlite-wasm/nmp-sqlite3-shim.mjs`. That shim itself has a static import:
```
import sqlite3InitModule from "./sqlite3.mjs";
```

wasm-pack emits `nmp-sqlite3-shim.mjs` into the snippets directory but does NOT copy `sqlite3.mjs` or `sqlite3.wasm` (the actual sqlite.org WASM engine). When the worker called `import(/* @vite-ignore */ moduleUrl)` for the JS glue, the browser resolved all static imports transitively. The missing `sqlite3.mjs` caused a module-load failure, which was caught by the `loadWasmBridge` try/catch, which returned `wasm_bridge_unavailable`. The worker then constructed `DegradedRuntime("browser_bridge_unavailable")` — hence `data-runtime-status="degraded:browser_bridge_unavailable"` even though the worker itself spawned fine.

The pre-Slice-1.5 run had an additional prior failure: `web/static/nmp-wasm/` was empty, so the HEAD availability check for the JS module returned 404 and `moduleAssetAvailable` returned false immediately (the sqlite3.mjs issue would have manifested next).

ACTIONS:

1. Copied pre-built wasm-pack artifacts from `nostr-multi-platform/crates/nmp-browser-runtime/pkg/nmp-browser-runtime/` into `web/static/nmp-wasm/`: `nmp-browser-runtime.js`, `nmp-browser-runtime_bg.wasm` (6.6 MB), `.d.ts` files, and `snippets/` directory.

2. Copied the missing `sqlite3.mjs` (432 KB) and `sqlite3.wasm` (868 KB) from `nostr-multi-platform/crates/nmp-sqlite-wasm/vendor/sqlite-wasm/` into `web/static/nmp-wasm/snippets/nmp-sqlite-wasm-ee46999c2490e92d/vendor/sqlite-wasm/`.

3. Updated `web/scripts/build-nmp-wasm.sh` to: export `CC_wasm32_unknown_unknown`/`AR_wasm32_unknown_unknown`/`CFLAGS_wasm32_unknown_unknown` pointing to Homebrew LLVM; run `wasm-pack` directly from the crate dir with `--out-name nmp-browser-runtime`; copy `sqlite3.mjs` + `sqlite3.wasm` from the nmp-sqlite-wasm vendor dir after every build.

4. Enabled the @wasm test (changed `test.skip(` to `test(`) and added `page.on("console",...)` + `page.on("pageerror",...)` listeners that dump captured output on assertion failure.

5. Added `vercel.json` `headers` rule for `/nmp-wasm/*.wasm` → `Content-Type: application/wasm` for correct Vercel production behavior (the wasm glue already handles the wrong-MIME fallback via `WebAssembly.instantiate`, but serving the correct type is best practice).

RESULTS:

`bun run preview` MIME types confirmed:
- `.js` → `text/javascript` (passes `isJavaScriptModule` check)
- `.wasm` → `application/wasm` (correct; Vite preview already handles this)

Playwright results:
```
Running 2 tests using 1 worker
  1 [chromium] fallback path: shell boots degraded without a Worker  (627ms)  PASS
  2 [chromium] @wasm real wasm runtime boots in a Worker and emits an UpdateFrame  (665ms)  PASS
2 passed (4.3s)
```

The @wasm test asserts `data-bridge-kind="worker"`, `data-has-snapshot="true"`, `data-runtime-status="running"` — all three pass, confirming that `NmpWasmRuntime` boots, receives a `Start` request, and emits an `UpdateFrame` synchronously without requiring any relay connection.

STATUS: Green. Branch `feat/65-wasm-boot-fix`, commit `0dc4970d`.

Did you have to touch the vendored client? No. The fix was entirely in the served static files and the build script. The vendored `runtime-web/` code (`wasmBridge.ts`, `worker.ts`, `client.svelte.ts`) is unchanged.

vercel.json header needed? Yes, added. Vite preview serves `application/wasm` natively; the Vercel CDN does not by default — the header rule ensures `WebAssembly.instantiateStreaming` succeeds in production without the slower fallback path.

Assistant: coder65boot nailed it: root cause was the wasm glue's transitive static import of `sqlite3.mjs`/`sqlite3.wasm` (which wasm-pack doesn't copy) → module-load failure → degraded. Fix: vendor the missing sqlite artifacts + build-script update + `vercel.json` `application/wasm` header + enable @wasm — **no vendored-client edits**. Result: @wasm tier **green** (real `NmpWasmRuntime` boots, emits UpdateFrame). Let me independently verify the diff + re-run @wasm, and run the opus review in parallel.

