// NMP Svelte client — browser-gated worker lifecycle + snapshot subscription.
//
// Modeled on nostr-multi-platform/web/chirp/src/nmp/client.ts (Item B thin-shell
// bridge) but adapted for Svelte 5 runes and hl/web conventions.
//
// The Worker is spawned lazily on first call to `getClient()`. SSR-safe: the
// entire client construction is gated on `typeof Worker !== "undefined"`, so
// SvelteKit SSR never attempts to load the worker or the wasm module.
//
// Degraded fallback: when Worker is unavailable (SSR, CSP, or browser
// restriction) the client falls back to the in-process `DegradedRuntime` from
// the vendored runtime-web package. It never throws — every action returns a
// honest `capability_failure` instead.

import { DegradedRuntime } from "./runtime-web/degradedRuntime";
import {
  eventCorrelationId,
  protocolVersion,
  type IdentityRelayPermission,
  type RuntimeStatus,
  type WorkerEvent,
  type WorkerRequest,
} from "./runtime-web/protocol";
import { fulfilSignRequestViaExtension } from "./runtime-web/signBroker";
import { decodeUpdateFrame } from "./runtime-web/updateFrameDecoder";
import { RefProfileStore, type ProfileWire } from "./runtime-web/refProfileStore";
import { RefEventStore, type ClaimedEventWire } from "./runtime-web/refEventStore";

export type { IdentityRelayPermission, RuntimeStatus, WorkerEvent, WorkerRequest, ProfileWire, ClaimedEventWire };

/** Snapshot emitted after every worker event. */
export type RuntimeSnapshot = {
  status: RuntimeStatus;
  /** Active runtime path: real worker or in-process degraded fallback. */
  bridgeKind: "worker" | "in_process_fallback";
  events: WorkerEvent[];
  /** Raw UpdateFrame bytes from the most recent `update_bytes` event.
   *  Undefined until the first frame arrives. */
  latestUpdateBytes?: Uint8Array;
  // ── #65 Slice 3: decoded projection state ────────────────────────────────
  /** All projection keys present in the most recent decoded UpdateFrame. Empty
   *  until the first frame arrives. Includes kernel builtin keys (refs.profile
   *  etc.) and any app-registered projection keys. */
  projectionKeys: string[];
};

export type NmpClient = {
  snapshot(): RuntimeSnapshot;
  subscribe(listener: (snapshot: RuntimeSnapshot) => void): () => void;
  hello(): void;
  start(opts: { relays?: string[]; relay_bootstrap?: { url: string; role: string }[] }): Promise<RuntimeSnapshot>;
  /** #65 S2 — install a NIP-07 identity in the wasm runtime. The host must
   *  call `window.nostr.getPublicKey()` first and pass the resulting hex
   *  pubkey. Subsequent `beginSign` calls park a sign op that routes back to
   *  `window.nostr.signEvent` on the main thread.
   *
   *  NIP-46 (bunker) and local-key (nsec) are unsupported by the current wasm
   *  runtime (upstream #2119/#2068) — do not call this for those signer kinds.
   */
  setSigner(pubkeyHex: string, identityRelays?: IdentityRelayPermission[]): Promise<RuntimeSnapshot>;
  /** #65 S2 — park a NIP-07 sign capability round-trip. The wasm worker emits
   *  a `sign_request` event; the main-thread broker (signBroker.ts) calls
   *  `window.nostr.signEvent` and posts `deliver_signer_response` back. The
   *  caller should watch `subscribe()` for `sign_completed`/`sign_failed`.
   *  Fails closed (capability_failure) on the degraded path. */
  beginSign(accountPubkey: string, unsignedJson: string): void;
  /** #65 S3 — ADR-0063 structured reference-resolution control. Sends a
   *  `resolve_ref` to the wasm runtime; the runtime opens a subscription and
   *  pushes resolved data via the `refs.*` sidecar projections in subsequent
   *  UpdateFrame bytes. The decoded result surfaces via `resolvedProfiles` in
   *  the snapshot.
   *
   *  Namespace / shape / liveness integer codes mirror the Lane D FFI:
   *    namespace: 0 = profile, 1 = event
   *    shape:     profile → 0 = ref (avatar subset), 1 = card (full)
   *    liveness:  0 = CacheOk (background fetch), 1 = Live (tailing sub)
   */
  resolveRef(
    namespace: number,
    key: string,
    shape: number,
    liveness: number,
    consumerId: string,
    hints?: string[],
    eventAuthor?: string | null,
  ): Promise<RuntimeSnapshot>;
  /** #65 S3 — ADR-0063 structured reference release. Signals the runtime that
   *  this consumer no longer needs the resolved reference (refcount bookkeeping).
   *  Release the ref in onDestroy / onUnmount to avoid refcount leaks. */
  releaseRef(
    namespace: number,
    key: string,
    consumerId: string,
  ): Promise<RuntimeSnapshot>;
  /** #65 S3 — look up the decoded `ProfileWire` for a pubkey in the
   *  host-side cache. Returns `undefined` until a `refs.profile` sidecar
   *  has been applied for this pubkey (i.e. `resolveRef` must be called first
   *  and the runtime must have pushed a snapshot carrying the profile row).
   *  Pure read — does NOT trigger a network fetch. */
  resolveProfile(pubkeyHex: string): ProfileWire | undefined;
  /** #65 S4 — convenience wrapper around resolveRef for event resolution.
   *  Calls resolveRef(namespace=1, key=eventId, shape=0, liveness=1, consumerId).
   *  The wasm issues a REQ for the event; on receipt it emits a refs.event sidecar
   *  (NRRD KCEV batch) in subsequent UpdateFrames. Use `resolvedEvent()` to read
   *  the decoded result.
   *
   *  opts.relayHints: relay URL hints (NIP-19 nevent TLV hints) — the wasm uses
   *    these to know which relay to query for the event when no other relay info
   *    is available. Pass the relay URL where the event is known to exist.
   *  opts.eventAuthor: hex pubkey of the event author (NIP-19 nevent TLV author).
   *    Provides the wasm with author context for relay discovery (NIP-65). */
  resolveEvent(
    eventId: string,
    consumerId: string,
    opts?: { relayHints?: string[]; eventAuthor?: string },
  ): Promise<RuntimeSnapshot>;
  /** #65 S4 — look up the decoded `ClaimedEventWire` for a primaryId in the
   *  host-side cache. Returns `undefined` until a `refs.event` sidecar has been
   *  applied for this event (i.e. `resolveEvent` / `resolveRef` must be called
   *  first and the runtime must have pushed a snapshot carrying the event row).
   *  Pure read — does NOT trigger a network fetch. */
  resolvedEvent(primaryId: string): ClaimedEventWire | undefined;
};

const APP_ID = "highlighter";
const DATABASE_NAME = "highlighter-web";

// ─── Shared singleton ────────────────────────────────────────────────────────

let _client: NmpClient | undefined;

/**
 * Return the singleton NmpClient, constructing it on first call.
 *
 * Browser-only: always call from `onMount` or after checking `browser`.
 * SSR: returns a degraded no-op client (safe to call, never throws).
 */
export function getClient(): NmpClient {
  if (_client) return _client;
  _client = createNmpClient();
  return _client;
}

/** Reset the singleton (test helper — do not call in production code). */
export function _resetClientForTest(): void {
  _client = undefined;
}

// ─── Factory ─────────────────────────────────────────────────────────────────

function createNmpClient(): NmpClient {
  if (typeof Worker === "undefined") {
    console.warn(
      "[nmp] Web Worker API is unavailable (SSR, CSP, or browser restriction). " +
        "Falling back to in-process degraded runtime — every action will return capability_failure.",
    );
    return new InProcessNmpClient();
  }
  try {
    return new WorkerNmpClient();
  } catch (err) {
    console.warn(
      "[nmp] Worker construction failed — falling back to in-process degraded runtime. " +
        "Every action will return capability_failure. Worker error:",
      err,
    );
    return new InProcessNmpClient();
  }
}

// ─── Base ────────────────────────────────────────────────────────────────────

abstract class BaseNmpClient implements NmpClient {
  private _events: WorkerEvent[] = [];
  private _latestUpdateBytes: Uint8Array | undefined;
  private _status: RuntimeStatus = "ready";
  private _listeners = new Set<(snapshot: RuntimeSnapshot) => void>();
  // #65 S3 — stateful profile store: rebuilt incrementally each frame.
  private _profileStore = new RefProfileStore();
  // #65 S4 — stateful event store: rebuilt incrementally each frame.
  private _eventStore = new RefEventStore();
  private _projectionKeys: string[] = [];

  constructor(private readonly _bridgeKind: RuntimeSnapshot["bridgeKind"]) {}

  snapshot(): RuntimeSnapshot {
    return {
      status: this._status,
      bridgeKind: this._bridgeKind,
      events: [...this._events],
      latestUpdateBytes: this._latestUpdateBytes,
      projectionKeys: [...this._projectionKeys],
    };
  }

  resolveProfile(pubkeyHex: string): ProfileWire | undefined {
    return this._profileStore.profile(pubkeyHex);
  }

  // #65 S4 — event store wiring
  async resolveEvent(
    eventId: string,
    consumerId: string,
    opts?: { relayHints?: string[]; eventAuthor?: string },
  ): Promise<RuntimeSnapshot> {
    // namespace=1 (event), shape=0 (Embed), liveness=1 (Live)
    // Pass relay hints and event_author so the wasm knows where to fetch the event
    // (NIP-19 nevent TLV fields — relay hints + author pubkey for NIP-65 discovery).
    return this.resolveRef(1, eventId, 0, 1, consumerId, opts?.relayHints, opts?.eventAuthor ?? null);
  }

  resolvedEvent(primaryId: string): ClaimedEventWire | undefined {
    return this._eventStore.event(primaryId);
  }

  subscribe(listener: (snapshot: RuntimeSnapshot) => void): () => void {
    this._listeners.add(listener);
    listener(this.snapshot());
    return () => this._listeners.delete(listener);
  }

  protected record(event: WorkerEvent): RuntimeSnapshot {
    if (event.type === "runtime_status" || event.type === "hello_accepted") {
      this._status = event.status;
    }
    if (event.type === "update_bytes") {
      const bytes =
        event.bytes instanceof Uint8Array ? event.bytes : new Uint8Array(event.bytes);
      this._latestUpdateBytes = bytes;
      // Mirror the kernel run-state on first frame.
      this._status = "running";
      // #65 S3/S4 — decode projection keys + apply refs.profile / refs.event sidecars.
      const decoded = decodeUpdateFrame(bytes);
      if (decoded) {
        this._projectionKeys = decoded.projectionKeys;
        if (decoded.refsProfileBytes) {
          this._profileStore.applySidecar(
            decoded.refsProfileBytes,
            decoded.sessionId,
            decoded.snapshotEpoch,
          );
        }
        if (decoded.refsEventBytes) {
          this._eventStore.applySidecar(
            decoded.refsEventBytes,
            decoded.sessionId,
            decoded.snapshotEpoch,
          );
        }
      }
    }
    this._events = [event, ...this._events].slice(0, 32);
    const snap = this.snapshot();
    for (const listener of this._listeners) {
      listener(snap);
    }
    return snap;
  }

  abstract hello(): void;
  abstract start(opts: {
    relays?: string[];
    relay_bootstrap?: { url: string; role: string }[];
  }): Promise<RuntimeSnapshot>;
  abstract setSigner(pubkeyHex: string, identityRelays?: IdentityRelayPermission[]): Promise<RuntimeSnapshot>;
  abstract beginSign(accountPubkey: string, unsignedJson: string): void;
  abstract resolveRef(
    namespace: number,
    key: string,
    shape: number,
    liveness: number,
    consumerId: string,
    hints?: string[],
    eventAuthor?: string | null,
  ): Promise<RuntimeSnapshot>;
  abstract releaseRef(
    namespace: number,
    key: string,
    consumerId: string,
  ): Promise<RuntimeSnapshot>;
  // resolveProfile is implemented in BaseNmpClient; no abstract needed.
}

// ─── Worker implementation ────────────────────────────────────────────────────

class WorkerNmpClient extends BaseNmpClient {
  private readonly worker: Worker;
  private readonly pending = new Map<string, (snapshot: RuntimeSnapshot) => void>();
  private readonly helloReady: Promise<void>;
  private resolveHello?: () => void;
  private nextCorrelationId = 0;

  constructor() {
    super("worker");
    // Vite resolves this at build time via static analysis of new URL(...).
    // The worker file lives in the vendored runtime-web directory.
    this.worker = new Worker(
      new URL("./runtime-web/worker", import.meta.url),
      { type: "module" },
    );
    this.helloReady = new Promise((resolve) => {
      this.resolveHello = resolve;
    });
    this.worker.onmessage = (message: MessageEvent<WorkerEvent>) => {
      this.accept(message.data);
    };
  }

  hello(): void {
    this.worker.postMessage({
      type: "hello",
      app_id: APP_ID,
      platform: "web",
      protocol_version: protocolVersion,
    } satisfies WorkerRequest);
  }

  async start(opts: {
    relays?: string[];
    relay_bootstrap?: { url: string; role: string }[];
  }): Promise<RuntimeSnapshot> {
    await this.helloReady;
    const correlationId = `web-start-${this.nextCorrelationId++}`;
    return this.request(
      {
        type: "start",
        app_id: APP_ID,
        relays: opts.relays ?? [],
        relay_bootstrap: opts.relay_bootstrap ?? [],
        database_name: DATABASE_NAME,
        correlation_id: correlationId,
      },
      correlationId,
    );
  }

  async setSigner(
    pubkeyHex: string,
    identityRelays?: IdentityRelayPermission[],
  ): Promise<RuntimeSnapshot> {
    await this.helloReady;
    const correlationId = `web-signer-${this.nextCorrelationId++}`;
    return this.request(
      {
        type: "set_identity",
        kind: "nip07",
        pubkey_hex: pubkeyHex,
        correlation_id: correlationId,
        identity_relays: identityRelays,
      },
      correlationId,
    );
  }

  beginSign(accountPubkey: string, unsignedJson: string): void {
    this.worker.postMessage({
      type: "begin_sign",
      account_pubkey: accountPubkey,
      unsigned_json: unsignedJson,
    } satisfies WorkerRequest);
  }

  async resolveRef(
    namespace: number,
    key: string,
    shape: number,
    liveness: number,
    consumerId: string,
    hints?: string[],
    eventAuthor?: string | null,
  ): Promise<RuntimeSnapshot> {
    await this.helloReady;
    const correlationId = `web-resolve-${this.nextCorrelationId++}`;
    console.log('[nmp-s4-diag] resolveRef ns=' + namespace + ' key=' + key + ' consumer=' + consumerId + ' liveness=' + liveness + ' hints=' + JSON.stringify(hints ?? []));
    return this.request(
      {
        type: "resolve_ref",
        namespace,
        key,
        consumer_id: consumerId,
        shape,
        liveness,
        hints: hints ?? [],
        event_author: eventAuthor ?? null,
        correlation_id: correlationId,
      },
      correlationId,
    );
  }

  async releaseRef(
    namespace: number,
    key: string,
    consumerId: string,
  ): Promise<RuntimeSnapshot> {
    await this.helloReady;
    const correlationId = `web-release-${this.nextCorrelationId++}`;
    return this.request(
      {
        type: "release_ref",
        namespace,
        key,
        consumer_id: consumerId,
        correlation_id: correlationId,
      },
      correlationId,
    );
  }

  private request(request: WorkerRequest, explicitCorrelationId?: string): Promise<RuntimeSnapshot> {
    const correlationId =
      explicitCorrelationId ?? ("correlation_id" in request ? request.correlation_id : undefined);
    if (!correlationId) {
      this.worker.postMessage(request);
      return Promise.resolve(this.snapshot());
    }
    return new Promise((resolve) => {
      this.pending.set(correlationId, resolve);
      this.worker.postMessage(request);
    });
  }

  private accept(event: WorkerEvent): void {
    const snap = this.record(event);
    if (event.type === "hello_accepted") {
      this.resolveHello?.();
    }
    // #65 S2 — sign broker: the wasm worker emits sign_request when it parks a
    // NIP-07 sign op. The main thread fulfils it via window.nostr.signEvent and
    // posts deliver_signer_response back (pure message re-entry, no polling).
    // sign_request is NOT correlation-keyed (it is a broker instruction, not a
    // pending-request reply) so we return early after dispatching it.
    if (event.type === "sign_request") {
      void fulfilSignRequestViaExtension(
        (request) => this.worker.postMessage(request),
        event.correlation_id,
        event.unsigned_json,
        event.account_pubkey,
      );
      return;
    }
    const correlationId = eventCorrelationId(event);
    if (!correlationId) return;
    const resolve = this.pending.get(correlationId);
    if (resolve) {
      this.pending.delete(correlationId);
      resolve(snap);
    }
  }
}

// ─── In-process degraded fallback ────────────────────────────────────────────

class InProcessNmpClient extends BaseNmpClient {
  private readonly runtime = new DegradedRuntime(
    "browser_bridge_unavailable",
    "Web Worker support is unavailable, so the nmp-wasm bridge cannot start",
  );
  private nextCorrelationId = 0;

  constructor() {
    super("in_process_fallback");
  }

  hello(): void {
    this.send({
      type: "hello",
      app_id: APP_ID,
      platform: "web",
      protocol_version: protocolVersion,
    });
  }

  async start(opts: {
    relays?: string[];
    relay_bootstrap?: { url: string; role: string }[];
  }): Promise<RuntimeSnapshot> {
    const correlationId = `web-start-${this.nextCorrelationId++}`;
    return this.send({
      type: "start",
      app_id: APP_ID,
      relays: opts.relays ?? [],
      relay_bootstrap: opts.relay_bootstrap ?? [],
      database_name: DATABASE_NAME,
      correlation_id: correlationId,
    });
  }

  // NIP-46 and local-key are unsupported by the current wasm runtime
  // (upstream #2119/#2068). The degraded path returns honest capability_failure
  // for all signer operations — no throw.
  async setSigner(
    pubkeyHex: string,
    identityRelays?: IdentityRelayPermission[],
  ): Promise<RuntimeSnapshot> {
    return this.send({
      type: "set_identity",
      kind: "nip07",
      pubkey_hex: pubkeyHex,
      correlation_id: `web-signer-${this.nextCorrelationId++}`,
      identity_relays: identityRelays,
    });
  }

  beginSign(accountPubkey: string, unsignedJson: string): void {
    this.send({
      type: "begin_sign",
      account_pubkey: accountPubkey,
      unsigned_json: unsignedJson,
    });
  }

  async resolveRef(
    namespace: number,
    key: string,
    shape: number,
    liveness: number,
    consumerId: string,
    hints?: string[],
    eventAuthor?: string | null,
  ): Promise<RuntimeSnapshot> {
    const correlationId = `web-resolve-${this.nextCorrelationId++}`;
    return this.send({
      type: "resolve_ref",
      namespace,
      key,
      consumer_id: consumerId,
      shape,
      liveness,
      hints: hints ?? [],
      event_author: eventAuthor ?? null,
      correlation_id: correlationId,
    });
  }

  async releaseRef(
    namespace: number,
    key: string,
    consumerId: string,
  ): Promise<RuntimeSnapshot> {
    const correlationId = `web-release-${this.nextCorrelationId++}`;
    return this.send({
      type: "release_ref",
      namespace,
      key,
      consumer_id: consumerId,
      correlation_id: correlationId,
    });
  }

  private send(request: WorkerRequest): RuntimeSnapshot {
    let snap = this.snapshot();
    for (const event of this.runtime.handle(request)) {
      snap = this.record(event);
    }
    return snap;
  }
}
