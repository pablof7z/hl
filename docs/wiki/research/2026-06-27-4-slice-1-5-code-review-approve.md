---
type: research-record
date: 2026-06-27
session: 9ae03596-fa74-4208-88c6-a90bd3b176e4
transcript: /Users/pablofernandez/.claude/projects/-Users-pablofernandez-Work-hl/9ae03596-fa74-4208-88c6-a90bd3b176e4.jsonl
source_lines: 4493-4510
agent_attribution: review65boot
has_preregistered_criteria: true
has_method: true
has_structured_report: true
characterization: "Slice 1.5 code review: APPROVE WITH NITS with provenance verification, build reproducibility assessment, artifact size justification"
captured_at: 2026-06-27T05:56:53Z
---

Slice 1.5 code review: APPROVE WITH NITS with provenance verification, build reproducibility assessment, artifact size justification

---

<teammate-message teammate_id="review65boot" color="purple" summary="#65 Slice 1.5 verdict: APPROVE WITH NITS">
## #65 Slice 1.5 (wasm vendor + @wasm enablement) — VERDICT: APPROVE WITH NITS

No blocking issues. Strictly additive; the boot path is real and the artifacts are provenance-verified.

**(a) Additive / no client edits — CONFIRMED.** `git diff origin/main...HEAD --name-only` touches only: `vercel.json`, `web/scripts/build-nmp-wasm.sh`, `web/tests/nmp-bridge.spec.ts`, and 6 vendored files under `web/static/nmp-wasm/`. Zero changes to `web/src/lib/nmp/runtime-web/**`, `client.svelte.ts`, or any NDK file.

**(b) Provenance & correctness — CONFIRMED, not stale.** All three sqlite artifacts (`sqlite3.mjs`, `sqlite3.wasm`, `nmp-sqlite3-shim.mjs`) are byte-identical (`diff -q` MATCH) to `nostr-multi-platform/crates/nmp-sqlite-wasm/vendor/sqlite-wasm/`. The snippet hash is consistent end-to-end: glue `nmp-browser-runtime.js:2-3` imports `./snippets/nmp-sqlite-wasm-ee46999c2490e92d/vendor/sqlite-wasm/nmp-sqlite3-shim.mjs`, the committed tree sits at that exact hash dir, and the shim (`:24`) imports sibling `./sqlite3.mjs`. Caveat (nit, not blocking): hl pins NMP via `branch = "master"` (`app/core/Cargo.toml:23+`), not a SHA, and the wasm was built from the local NMP working tree (`37b606eaa`) — so wasm and Rust core can drift between rebuilds. Same floating posture as the rest of hl's NMP deps.

**(c) build-nmp-wasm.sh reproducibility — SOUND, one nit.** Gates on `wasm-pack`/`rustup target`, exports `CC/AR/CFLAGS_wasm32_unknown_unknown` to Homebrew LLVM (needed for secp256k1-sys wasm32), documents `brew install llvm`, and copies sqlite3.mjs/.wasm next to the shim. NIT (worth fixing before relying on rebuilds): `SNIPPET_HASH_DIR=$(ls .../snippets/ | head -1)` + a `cp -r snippets` *merge* means a future NMP hash change leaves the stale dir and `head -1` may pick the wrong one → sqlite copied beside the wrong shim, @wasm silently degrades. Fix: `rm -rf static/nmp-wasm/snippets` before copy, or iterate all snippet dirs.

**(d) vercel.json header — CORRECT.** Adds a `headers` block alongside existing keys (none modified). Source `"/nmp-wasm/(.*\\.wasm)"` → `Content-Type: application/wasm`; Vercel path-to-regexp `(.*)` spans slashes so it covers both `nmp-browser-runtime_bg.wasm` and nested `snippets/.../sqlite3.wasm`.

**(e) @wasm test — RIGOROUS, no network dependency.** Asserts `data-bridge-kind=worker` (only the real worker client), `data-has-snapshot=true`, `data-runtime-status=running`. Probe `+page.svelte:60` derives has-snapshot from `snapshot.latestUpdateBytes != null` — a real UpdateFrame that DegradedRuntime can never emit, so it cannot false-pass. Added `.catch()` only dumps console then re-throws (assertion not weakened). Non-flakiness rests on the runtime emitting an initial snapshot frame on Start (Slice 1 behavior, unchanged here) — no relay/network needed.

**(f) ~7.4MB committed binaries (6.6MB _bg.wasm + 868KB sqlite3.wasm + 432KB sqlite3.mjs) — JUSTIFIED.** No Rust/wasm toolchain on Vercel/CI (script documents the Homebrew-LLVM secp256k1 dep CI can't satisfy), so vendoring is the right call; same posture as the iOS sibling-core dep. No LFS in repo. Git LFS only worth considering if these get re-generated frequently. Non-blocking.

No secrets in text artifacts, no scope creep. Worktree `/Users/pablofernandez/Work/hl/.claude/worktrees/agent-a9cf3bc5223f05501`, commit `0dc4970d`.
