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
  type RuntimeStatus,
  type WorkerEvent,
  type WorkerRequest,
} from "./runtime-web/protocol";

export type { RuntimeStatus, WorkerEvent, WorkerRequest };

/** Snapshot emitted after every worker event. */
export type RuntimeSnapshot = {
  status: RuntimeStatus;
  /** Active runtime path: real worker or in-process degraded fallback. */
  bridgeKind: "worker" | "in_process_fallback";
  events: WorkerEvent[];
  /** Raw UpdateFrame bytes from the most recent `update_bytes` event.
   *  Undefined until the first frame arrives. */
  latestUpdateBytes?: Uint8Array;
};

export type NmpClient = {
  snapshot(): RuntimeSnapshot;
  subscribe(listener: (snapshot: RuntimeSnapshot) => void): () => void;
  hello(): void;
  start(opts: { relays?: string[]; relay_bootstrap?: { url: string; role: string }[] }): Promise<RuntimeSnapshot>;
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

  constructor(private readonly _bridgeKind: RuntimeSnapshot["bridgeKind"]) {}

  snapshot(): RuntimeSnapshot {
    return {
      status: this._status,
      bridgeKind: this._bridgeKind,
      events: [...this._events],
      latestUpdateBytes: this._latestUpdateBytes,
    };
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

  private send(request: WorkerRequest): RuntimeSnapshot {
    let snap = this.snapshot();
    for (const event of this.runtime.handle(request)) {
      snap = this.record(event);
    }
    return snap;
  }
}
