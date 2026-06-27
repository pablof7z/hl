/**
 * NMP web bridge boot acceptance — GitHub #65 Slice 1.
 *
 * Ported from nostr-multi-platform/web/chirp/tests/boot.spec.ts and adapted
 * for hl/web (SvelteKit + Vite 6 + adapter-vercel).
 *
 * Two tiers, separated by the `@wasm` tag:
 *
 *   • Fallback tier (NO @wasm, PR-blocking):
 *       Deletes `window.Worker` before app code loads so `createNmpClient()`
 *       falls back to InProcessNmpClient / DegradedRuntime. Proves the probe
 *       page mounts with honest degraded data-* attributes. Requires NO wasm
 *       artifact — passes on every PR/CI run.
 *
 *   • @wasm tier (optional):
 *       Requires the built wasm artifact under web/static/nmp-wasm/ (populated
 *       by `bun run build:wasm`). The real NmpWasmRuntime boots in a Worker,
 *       receives an UpdateFrame, and dials a fixture relay.
 *       Currently marked `test.skip` because the artifact has not been built in
 *       this Slice 1 landing; re-enable once `bun run build:wasm` runs in CI.
 *
 * Probe page (web/src/routes/(dev)/nmp-probe/+page.svelte) data-* hooks:
 *   data-bridge-kind     = "worker" | "in_process_fallback" | "pending"
 *   data-runtime-status  = "running" | "ready" | "degraded:<reason>" | "pending"
 *   data-has-snapshot    = "true" | "false"
 */

import { test, expect } from "@playwright/test";

const PROBE = "/nmp-probe";
const SHELL = "main.nmp-probe";

test.describe("NMP bridge boot", () => {
  test(
    "fallback path: shell boots degraded without a Worker (no wasm artifact needed)",
    async ({ page }) => {
      // Delete window.Worker BEFORE any app code runs. getClient() calls
      // createNmpClient() which sees `typeof Worker === "undefined"` and
      // returns InProcessNmpClient driving DegradedRuntime("browser_bridge_unavailable").
      await page.addInitScript(() => {
        // intentionally delete the global for the fallback path test
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        delete (window as any).Worker;
      });

      await page.goto(PROBE);

      const shell = page.locator(SHELL);

      // The probe page mounts and renders the bridge-kind hook for the fallback path.
      await expect(shell).toHaveAttribute("data-bridge-kind", "in_process_fallback", {
        timeout: 15_000,
      });

      // DegradedRuntime("browser_bridge_unavailable") transitions to degraded status
      // after start() is dispatched.
      await expect(shell).toHaveAttribute(
        "data-runtime-status",
        "degraded:browser_bridge_unavailable",
        { timeout: 15_000 },
      );

      // No UpdateFrame is ever emitted on the degraded path.
      await expect(shell).toHaveAttribute("data-has-snapshot", "false");
    },
  );

  // @wasm tier — enabled once the wasm artifact is built and vendored.
  // The test name includes @wasm so it can also be excluded via --grep-invert.
  test(
    "@wasm real wasm runtime boots in a Worker and emits an UpdateFrame",
    async ({ page }) => {
      // Capture all worker/page console output and errors to aid diagnosis.
      const consoleMessages: string[] = [];
      page.on("console", (msg) => {
        consoleMessages.push(`[${msg.type()}] ${msg.text()}`);
      });
      page.on("pageerror", (err) => {
        consoleMessages.push(`[pageerror] ${err.message}`);
      });

      // NOTE: This test requires web/static/nmp-wasm/ to be populated.
      // Run `bun run build:wasm` first, or populate via the vendored copies.
      await page.goto(PROBE);

      const shell = page.locator(SHELL);

      // The real client constructs a Worker, so bridge kind is "worker".
      await expect(shell).toHaveAttribute("data-bridge-kind", "worker", { timeout: 30_000 });

      // The first UpdateFrame flips data-has-snapshot to "true" — an event only
      // the real NmpWasmRuntime can emit (DegradedRuntime never produces one).
      await expect(shell)
        .toHaveAttribute("data-has-snapshot", "true", { timeout: 30_000 })
        .catch((err: unknown) => {
          // On failure, dump the captured console log so the root cause is
          // visible in the Playwright report without needing a trace viewer.
          console.error(
            "[nmp-bridge @wasm] Worker console at failure:\n" +
              (consoleMessages.length ? consoleMessages.join("\n") : "(none)"),
          );
          throw err;
        });

      // Once data-has-snapshot is true, the status should be "running".
      await expect(shell).toHaveAttribute("data-runtime-status", "running", { timeout: 30_000 });
    },
  );
});
