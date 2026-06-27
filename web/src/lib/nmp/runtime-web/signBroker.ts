import type { WorkerRequest } from "./protocol";

/** #65 S2 — the main-thread NIP-07 broker.
 *
 * Web Workers have no `window.nostr`, so when the wasm worker emits a
 * `sign_request` the MAIN THREAD fulfils it here: call
 * `window.nostr.signEvent`, then post the result back as a
 * `deliver_signer_response` so the worker resumes its parked sign op (pure
 * message re-entry — no polling).
 *
 * Scope: this broker fulfils the NIP-07 (browser-extension) sign path ONLY.
 * Local-key (nsec) signing happens inside the Rust LocalKey provider in the
 * worker — the worker never emits `sign_request` for nsec accounts, so this
 * function is never called. NIP-46 (bunker) is unsupported upstream
 * (#2119/#2068).
 *
 * Ported from nostr-multi-platform/web/chirp/src/nmp/signBroker.ts with
 * `@nmp/runtime-web` import replaced by `./protocol`.
 *
 * `post` is the worker's message sink (`worker.postMessage`). Every failure
 * mode (no extension, account mismatch, malformed event, user rejection) is
 * posted back with `error` set so the worker fails the round-trip closed
 * rather than leaving the op parked forever.
 */
export async function fulfilSignRequestViaExtension(
  post: (request: WorkerRequest) => void,
  correlationId: string,
  unsignedJson: string,
  accountPubkey: string,
): Promise<void> {
  const deliver = (signedJson: string | null, error: string | null) => {
    post({
      type: "deliver_signer_response",
      correlation_id: correlationId,
      signed_json: signedJson,
      error,
    });
  };

  if (!window.nostr) {
    deliver(null, "window.nostr is unavailable — no NIP-07 extension installed");
    return;
  }

  // Account-pin guard: a NIP-07 extension signs with whichever account is
  // currently active. If the active account differs from the one this sign
  // round-trip was begun for, fail early with an actionable message rather
  // than producing a signature the worker will reject. The worker remains the
  // final authority; this is an early, honest short-circuit.
  try {
    const activePubkey = await window.nostr.getPublicKey();
    if (activePubkey.toLowerCase() !== accountPubkey.toLowerCase()) {
      deliver(
        null,
        `NIP-07 extension is on a different account (${activePubkey}) than the ` +
          `signing request (${accountPubkey}); switch the extension's active account`,
      );
      return;
    }
  } catch (e) {
    deliver(null, `window.nostr.getPublicKey rejected: ${String(e)}`);
    return;
  }

  let unsigned: Record<string, unknown>;
  try {
    unsigned = JSON.parse(unsignedJson) as Record<string, unknown>;
  } catch (e) {
    deliver(null, `unsigned event JSON did not parse: ${String(e)}`);
    return;
  }

  try {
    const signed = await window.nostr.signEvent(unsigned);
    deliver(JSON.stringify(signed), null);
  } catch (e) {
    deliver(null, `window.nostr.signEvent rejected: ${String(e)}`);
  }
}
