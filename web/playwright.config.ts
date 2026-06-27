import { defineConfig, devices } from "@playwright/test";

/**
 * Playwright configuration for the NMP web bridge acceptance suite
 * (GitHub #65, Slice 1).
 *
 * Two tiers, separated by the `@wasm` tag:
 *
 *   - Fallback tier (no @wasm, PR-blocking): proves the shell boots into
 *     DegradedRuntime when Worker is deleted, rendering honest degraded
 *     data-* attributes. Requires NO wasm artifact.
 *
 *   - @wasm tier (optional): proves the real NmpWasmRuntime boots in a
 *     worker, receives an UpdateFrame, and dials a fixture relay. Requires
 *     web/static/nmp-wasm/ to be populated via `bun run build:wasm`.
 *
 * Run locally:
 *   bun run build && bun run test:e2e              # all specs
 *   bun run build && bun run test:e2e:fallback     # fallback tier only (no wasm)
 */
export default defineConfig({
  testDir: "./tests",

  timeout: 60_000,
  expect: { timeout: 15_000 },

  fullyParallel: false,
  workers: 1,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? [["list"], ["html", { open: "never" }]] : "list",

  use: {
    baseURL: "http://localhost:4173",
    headless: true,
    trace: "retain-on-failure",
  },

  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],

  webServer: {
    command: "bun run preview",
    url: "http://localhost:4173",
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
    stdout: "pipe",
    stderr: "pipe",
  },
});
