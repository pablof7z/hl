// ADR-0063 Lane A — TypeScript host-side per-key reference cache.
//
// Ported from nostr-multi-platform/web/chirp/src/nmp/refRowCache.ts
// (NMP commit 37b606eaa839863c2644172a018814b95484981e).
//
// `RefRowCache` is the TypeScript mirror of the canonical Rust `RefRowCache`
// (crates/nmp-core/src/refs/cache.rs). The merge algorithm is byte-for-byte
// equivalent so a web host materialises the SAME full set the producer's
// ground-truth snapshot carries.
//
// It decodes the `refs.profile` / `refs.event` projection's opaque
// `TypedPayload.payload` (an NRRD `RefRowDeltaBatch`) and merges each tick's
// row deltas into a persistent `namespace -> key -> row` cache. Row payloads
// stay raw bytes; the consumer decodes them via the `decodeOk` preflight.
//
// The five ADR-0063 invariants enforced at ROW grain:
//   1. an absent row is Unchanged (retained), never Cleared;
//   2. decode-before-commit: a `Changed` row commits only after its payload
//      decodes; a malformed row leaves the prior cached row intact + latches
//      `needsResync` (D6, fail-closed);
//   3. a `baseline` batch / session-or-epoch change reconstructs the full set;
//   4. payloads are namespace-typed bytes;
//   5. the cache is host-side read-model only — truth stays kernel-owned.

import * as flatbuffers from "flatbuffers";

import { RefRowDeltaBatch as RefRowDeltaBatchFb } from "../generated/nmp/refs/ref-row-delta-batch.js";
import { RefRowState } from "../generated/nmp/refs/ref-row-state.js";

/** Decoded row state. Mirrors Rust `RefRowState` — Unchanged is ABSENCE. */
type DecodedRowState = "changed" | "cleared";

/** One decoded NRRD row. */
type DecodedRow = {
  key: string;
  rev: bigint;
  state: DecodedRowState;
  payload: Uint8Array;
};

/** A fully-decoded NRRD `RefRowDeltaBatch`. */
type DecodedBatch = {
  namespace: string;
  baseline: boolean;
  rows: DecodedRow[];
};

/** One cached row: the last committed per-key rev + raw typed payload bytes. */
type CachedRow = {
  rev: bigint;
  payload: Uint8Array;
};

/** Outcome of applying one batch. */
export type RefRowApplyOutcome = {
  changedKeys: string[];
  decodeFailed: boolean;
};

/** Decode-before-commit preflight: `(key, payload) -> bool`. */
export type DecodeOk = (key: string, payload: Uint8Array) => boolean;

const EMPTY_OUTCOME = (): RefRowApplyOutcome => ({ changedKeys: [], decodeFailed: false });

/**
 * Decode an NRRD `RefRowDeltaBatch` from finished FlatBuffers bytes, FAILING
 * CLOSED on any malformation. Returns `undefined` when the buffer is too short,
 * lacks the `NRRD` file identifier, is missing a required row key, OR carries an
 * unknown `state` discriminant.
 */
function decodeRefRowDeltaBatch(bytes: Uint8Array): DecodedBatch | undefined {
  if (bytes.length < 8) return undefined;
  let batch: RefRowDeltaBatchFb;
  try {
    const bb = new flatbuffers.ByteBuffer(bytes);
    if (!RefRowDeltaBatchFb.bufferHasIdentifier(bb)) return undefined;
    batch = RefRowDeltaBatchFb.getRootAsRefRowDeltaBatch(bb);
  } catch {
    return undefined;
  }
  const rows: DecodedRow[] = [];
  const len = batch.rowsLength();
  for (let i = 0; i < len; i += 1) {
    const row = batch.rows(i);
    if (!row) return undefined;
    const key = row.key();
    if (key === null) return undefined;
    // Fail closed: an unknown `state` discriminant is a decode failure.
    const rawState = row.state() as number;
    let state: DecodedRowState;
    if (rawState === RefRowState.Changed) {
      state = "changed";
    } else if (rawState === RefRowState.Cleared) {
      state = "cleared";
    } else {
      return undefined;
    }
    const payloadArray = row.payloadArray();
    const payload = payloadArray ? payloadArray.slice() : new Uint8Array(0);
    rows.push({ key, rev: row.rev(), state, payload });
  }
  return { namespace: batch.namespace() ?? "", baseline: batch.baseline(), rows };
}

/** The host-side per-namespace row cache. */
export class RefRowCache {
  private rows = new Map<string, Map<string, CachedRow>>();
  private appliedSession = 0n;
  private appliedEpoch = 0n;
  private baselinedFlag = false;
  private needsResyncFlag = false;

  baselined(): boolean {
    return this.baselinedFlag;
  }

  needsResync(): boolean {
    return this.needsResyncFlag;
  }

  get(namespace: string, key: string): Uint8Array | undefined {
    return this.rows.get(namespace)?.get(key)?.payload;
  }

  snapshot(namespace: string): Map<string, Uint8Array> {
    const out = new Map<string, Uint8Array>();
    const ns = this.rows.get(namespace);
    if (ns) for (const [k, v] of ns) out.set(k, v.payload);
    return out;
  }

  applySidecar(
    payload: Uint8Array,
    sessionId: bigint,
    epoch: bigint,
    decodeOk: DecodeOk,
  ): RefRowApplyOutcome {
    const batch = decodeRefRowDeltaBatch(payload);
    if (!batch) return EMPTY_OUTCOME();
    return this.apply(batch, sessionId, epoch, decodeOk);
  }

  private apply(
    batch: DecodedBatch,
    sessionId: bigint,
    epoch: bigint,
    decodeOk: DecodeOk,
  ): RefRowApplyOutcome {
    const identityChanged = sessionId !== this.appliedSession || epoch !== this.appliedEpoch;

    if (batch.baseline) {
      return this.applyBaseline(batch, identityChanged, sessionId, epoch, decodeOk);
    }

    if (identityChanged) {
      this.appliedSession = sessionId;
      this.appliedEpoch = epoch;
      this.baselinedFlag = false;
      this.needsResyncFlag = true;
      return EMPTY_OUTCOME();
    }

    return this.applyIncremental(batch, decodeOk);
  }

  private applyBaseline(
    batch: DecodedBatch,
    identityChanged: boolean,
    sessionId: bigint,
    epoch: bigint,
    decodeOk: DecodeOk,
  ): RefRowApplyOutcome {
    const scratch = new Map<string, CachedRow>();
    for (const row of batch.rows) {
      if (row.state === "cleared") {
        scratch.delete(row.key);
        continue;
      }
      if (!decodeOk(row.key, row.payload)) {
        this.needsResyncFlag = true;
        return { changedKeys: [], decodeFailed: true };
      }
      const existing = scratch.get(row.key);
      if (!existing || row.rev > existing.rev) {
        scratch.set(row.key, { rev: row.rev, payload: row.payload });
      }
    }

    if (identityChanged) {
      for (const ns of [...this.rows.keys()]) {
        if (ns !== batch.namespace) this.rows.delete(ns);
      }
      this.appliedSession = sessionId;
      this.appliedEpoch = epoch;
      this.needsResyncFlag = false;
    }

    const prior = identityChanged ? undefined : this.rows.get(batch.namespace);
    const changed = new Set<string>();
    for (const [key, row] of scratch) {
      const prev = prior?.get(key);
      if (!prev || !bytesEqual(prev.payload, row.payload)) {
        changed.add(key);
      }
    }
    if (prior) {
      for (const key of prior.keys()) {
        if (!scratch.has(key)) changed.add(key);
      }
    }
    this.rows.set(batch.namespace, scratch);
    this.baselinedFlag = true;
    return { changedKeys: [...changed].sort(), decodeFailed: false };
  }

  private applyIncremental(batch: DecodedBatch, decodeOk: DecodeOk): RefRowApplyOutcome {
    let ns = this.rows.get(batch.namespace);
    if (!ns) {
      ns = new Map<string, CachedRow>();
      this.rows.set(batch.namespace, ns);
    }
    const changed = new Set<string>();
    let decodeFailed = false;

    for (const row of batch.rows) {
      if (row.state === "cleared") {
        const existing = ns.get(row.key);
        if (existing && row.rev > existing.rev) {
          ns.delete(row.key);
          changed.add(row.key);
        }
        continue;
      }
      const existing = ns.get(row.key);
      if (existing && row.rev <= existing.rev) continue;
      if (decodeOk(row.key, row.payload)) {
        ns.set(row.key, { rev: row.rev, payload: row.payload });
        changed.add(row.key);
      } else {
        decodeFailed = true;
        this.needsResyncFlag = true;
      }
    }

    this.baselinedFlag = true;
    return { changedKeys: [...changed].sort(), decodeFailed };
  }
}

function bytesEqual(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i += 1) {
    if (a[i] !== b[i]) return false;
  }
  return true;
}
