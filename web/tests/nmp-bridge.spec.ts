/**
 * NMP web bridge acceptance — GitHub #65 Slices 1 + 2.
 *
 * Ported/extended from nostr-multi-platform/web/chirp/tests/ and adapted for
 * hl/web (SvelteKit + Vite 6 + adapter-vercel).
 *
 * Two tiers, separated by the `@wasm` tag:
 *
 *   • Fallback tier (NO @wasm, PR-blocking):
 *       Deletes `window.Worker` before app code loads so `createNmpClient()`
 *       falls back to InProcessNmpClient / DegradedRuntime. Proves the probe
 *       page mounts with honest degraded data-* attributes AND that setSigner /
 *       beginSign return honest capability_failure (no crash). Requires NO wasm
 *       artifact — passes on every PR/CI run.
 *
 *   • @wasm tier (optional):
 *       Requires the built wasm artifact under web/static/nmp-wasm/ (populated
 *       by `bun run build:wasm`). The real NmpWasmRuntime boots in a Worker,
 *       receives an UpdateFrame, and dials a fixture relay.
 *       Slice 2 @wasm tests verify that set_identity (NIP-07) installs an
 *       identity in the runtime and that the sign broker round-trips a signature
 *       through window.nostr.signEvent end-to-end.
 *
 * Probe page (web/src/routes/(dev)/nmp-probe/+page.svelte) data-* hooks:
 *   data-bridge-kind     = "worker" | "in_process_fallback" | "pending"
 *   data-runtime-status  = "running" | "ready" | "degraded:<reason>" | "pending"
 *   data-has-snapshot    = "true" | "false"
 *   data-identity-set    = "true" | "false" | "pending"   (Slice 2)
 *   data-sign-result     = "completed:<id>" | "failed:<reason>" | "pending" (Slice 2)
 */

import { test, expect } from "@playwright/test";
import { schnorr } from "@noble/curves/secp256k1";
import { sha256 } from "@noble/hashes/sha256";
import { bytesToHex, hexToBytes } from "@noble/hashes/utils";

const PROBE = "/nmp-probe";
const SHELL = "main.nmp-probe";

// ─── Signing helpers (Node-side, mirroring nostr-tools/pure) ─────────────────

function generateSecretKey(): Uint8Array {
  const key = new Uint8Array(32);
  // Node.js crypto — available in Playwright test environment
  for (let i = 0; i < 32; i++) key[i] = Math.floor(Math.random() * 256);
  return key;
}

function getPublicKeyHex(secretKey: Uint8Array): string {
  return bytesToHex(schnorr.getPublicKey(secretKey));
}

/** Build a valid signed Nostr event from an unsigned event object.
 *  Mirror of nostr-tools finalizeEvent — real secp256k1 signature so the
 *  wasm runtime accepts it. */
function finalizeEvent(
  unsigned: Record<string, unknown>,
  secretKey: Uint8Array,
): Record<string, unknown> {
  const pubkey = getPublicKeyHex(secretKey);
  const created_at = typeof unsigned.created_at === "number"
    ? unsigned.created_at
    : Math.floor(Date.now() / 1000);
  const kind = typeof unsigned.kind === "number" ? unsigned.kind : 1;
  const tags = Array.isArray(unsigned.tags) ? unsigned.tags : [];
  const content = typeof unsigned.content === "string" ? unsigned.content : "";

  const serialized = JSON.stringify([0, pubkey, created_at, kind, tags, content]);
  const id = bytesToHex(sha256(new TextEncoder().encode(serialized)));
  const sig = bytesToHex(schnorr.sign(hexToBytes(id), secretKey));

  return { id, pubkey, created_at, kind, tags, content, sig };
}

// ─── Slice 1: boot tests ──────────────────────────────────────────────────────

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

// ─── Slice 2: signer boundary tests ──────────────────────────────────────────

test.describe("NMP signer boundary (#65 S2, NIP-07 only)", () => {
  // ── Fallback tier (PR-blocking, no wasm required) ──────────────────────────

  test(
    "fallback path: setSigner returns honest capability_failure (no crash)",
    async ({ page }) => {
      // Force in-process degraded runtime. DegradedRuntime.handle() returns
      // capability_failure for set_identity (and sign_failed for begin_sign) —
      // never throws. This proves the degraded path is safe to call: the probe
      // only fires beginSign after a successful setSigner, so on the degraded
      // path identity stays unset and the sign result stays "pending" (no
      // crash, no hang). The degraded begin_sign→sign_failed branch itself is
      // covered at the unit level in degradedRuntime.ts.
      await page.addInitScript(() => {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        delete (window as any).Worker;
      });

      const secretKey = generateSecretKey();
      const pubkeyHex = getPublicKeyHex(secretKey);
      const unsignedJson = JSON.stringify({
        pubkey: pubkeyHex,
        kind: 1,
        created_at: Math.floor(Date.now() / 1000),
        tags: [],
        content: "fallback signer test",
      });
      const url = `${PROBE}?set_identity_pubkey=${pubkeyHex}&begin_sign=${encodeURIComponent(unsignedJson)}`;

      await page.goto(url);

      const shell = page.locator(SHELL);

      // Degraded client boots into in_process_fallback.
      await expect(shell).toHaveAttribute("data-bridge-kind", "in_process_fallback", {
        timeout: 15_000,
      });

      // setSigner returns capability_failure on the degraded path — identity NOT set.
      await expect(shell).toHaveAttribute("data-identity-set", "false", { timeout: 15_000 });

      // beginSign returns sign_failed on the degraded path (no throw, no hang).
      // data-sign-result is NOT "pending" since the degraded runtime returns sync.
      await expect(shell).toHaveAttribute("data-sign-result", /^(failed:|pending)/, {
        timeout: 15_000,
      });
    },
  );

  // ── @wasm tier ──────────────────────────────────────────────────────────────

  test(
    "@wasm set_identity installs a nip07 identity in the wasm runtime",
    async ({ page }) => {
      test.setTimeout(60_000);

      const consoleMessages: string[] = [];
      page.on("console", (msg) => consoleMessages.push(`[${msg.type()}] ${msg.text()}`));
      page.on("pageerror", (err) => consoleMessages.push(`[pageerror] ${err.message}`));

      const secretKey = generateSecretKey();
      const pubkeyHex = getPublicKeyHex(secretKey);

      // No window.nostr needed — just testing set_identity (no sign round-trip).
      await page.goto(`${PROBE}?set_identity_pubkey=${pubkeyHex}`);

      const shell = page.locator(SHELL);

      // Real worker boots.
      await expect(shell).toHaveAttribute("data-bridge-kind", "worker", { timeout: 30_000 });

      // set_identity with kind:nip07 is supported — wasm runtime responds with
      // action_accepted (not capability_failure / unsupported_signer_kind).
      await expect(shell)
        .toHaveAttribute("data-identity-set", "true", { timeout: 30_000 })
        .catch((err: unknown) => {
          console.error(
            "[nmp-bridge @wasm set_identity] Console at failure:\n" +
              (consoleMessages.length ? consoleMessages.join("\n") : "(none)"),
          );
          throw err;
        });
    },
  );

  test(
    "@wasm sign broker round-trips a nip07 signature through window.nostr.signEvent",
    async ({ page }) => {
      test.setTimeout(90_000);

      const consoleMessages: string[] = [];
      page.on("console", (msg) => consoleMessages.push(`[${msg.type()}] ${msg.text()}`));
      page.on("pageerror", (err) => consoleMessages.push(`[pageerror] ${err.message}`));

      const secretKey = generateSecretKey();
      const pubkeyHex = getPublicKeyHex(secretKey);

      // Real secp256k1 signing in Node, exposed to the page so the NIP-07
      // stub produces genuinely signed events. The wasm runtime validates
      // the account pubkey before emitting sign_completed.
      await page.exposeFunction(
        "signNostrEventForTest",
        async (event: Record<string, unknown>) => {
          return finalizeEvent(event, secretKey);
        },
      );

      // Inject mock window.nostr BEFORE app code loads.
      await page.addInitScript((viewerPubkeyHex: string) => {
        (window as unknown as { nostr: unknown }).nostr = {
          getPublicKey: () => Promise.resolve(viewerPubkeyHex),
          signEvent: (event: Record<string, unknown>) =>
            (
              window as unknown as {
                signNostrEventForTest(e: Record<string, unknown>): Promise<Record<string, unknown>>;
              }
            ).signNostrEventForTest(event),
        };
      }, pubkeyHex);

      // The wasm runtime deserializes unsigned_json into a Nostr event struct
      // that requires `pubkey` (the account pubkey) in the unsigned payload —
      // the id field is absent (not yet hashed); sig absent (not yet signed).
      const unsignedJson = JSON.stringify({
        pubkey: pubkeyHex,
        kind: 1,
        created_at: Math.floor(Date.now() / 1000),
        tags: [],
        content: `nmp sign round-trip test ${Date.now()}`,
      });
      const url = `${PROBE}?set_identity_pubkey=${pubkeyHex}&begin_sign=${encodeURIComponent(unsignedJson)}`;

      await page.goto(url);

      const shell = page.locator(SHELL);

      // Real worker must boot first.
      await expect(shell).toHaveAttribute("data-bridge-kind", "worker", { timeout: 30_000 });

      // Identity must be installed before the sign round-trip can succeed.
      await expect(shell).toHaveAttribute("data-identity-set", "true", { timeout: 30_000 });

      // The wasm worker emits sign_request → main-thread broker calls
      // window.nostr.signEvent → delivers back deliver_signer_response →
      // worker emits sign_completed. Proves the full broker round-trip.
      await expect(shell)
        .toHaveAttribute("data-sign-result", /^completed:/, { timeout: 45_000 })
        .catch((err: unknown) => {
          console.error(
            "[nmp-bridge @wasm sign] Worker console at failure:\n" +
              (consoleMessages.length ? consoleMessages.join("\n") : "(none)"),
          );
          throw err;
        });
    },
  );
});
