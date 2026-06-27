// ADR-0063 Lane F — host-side `refs.event` consumption helper.
//
// Mirrors refProfileStore.ts for the event namespace.
//
// `RefEventStore` is the host-side consumer of the `refs.event`
// row-delta projection. It wraps a `RefRowCache` and exposes typed
// `event(primaryId)` lookups over decoded `ClaimedEventWire` values.
//
// One instance lives for the lifetime of the client's update loop — NOT
// rebuilt per frame, because the cache is stateful (incremental deltas).
//
// refs.event row payload = single-entry KCEV ClaimedEventsSnapshot FlatBuffers buffer.
// file_identifier = "KCEV" (confirmed from claimed_events.fbs).
// ClaimedEventsSnapshot.entries[0].value = a ClaimedEvent with fields:
//   primaryId, id, authorPubkey, kind, createdAt, content, etc.
// Each row in the NRRD batch carries ONE KCEV buffer with ONE entry (the
// event for that key). The NRRD batch namespace is "event".

import * as flatbuffers from "flatbuffers";

import { ClaimedEventsSnapshot } from "../generated/nmp/kernel/claimed-events-snapshot.js";
import { RefRowCache, type RefRowApplyOutcome } from "./refRowCache.js";

/** Thin hydrated event type (host-side, wire-level). */
export type ClaimedEventWire = {
  primaryId: string;
  id: string;
  authorPubkey: string;
  kind: number;
  createdAt: bigint;
  content: string;
};

/** The kernel-emitted projection key for the event resolver. */
export const REFS_EVENT_KEY = "refs.event";
const REFS_EVENT_NAMESPACE = "event";

/** Decode a KCEV `ClaimedEventsSnapshot` row payload into a `ClaimedEventWire`, or
 *  `undefined` when the bytes are not a well-formed ClaimedEventsSnapshot. */
function decodeEventRow(bytes: Uint8Array): ClaimedEventWire | undefined {
  if (bytes.length < 8) return undefined;
  try {
    const bb = new flatbuffers.ByteBuffer(bytes);
    if (!ClaimedEventsSnapshot.bufferHasIdentifier(bb)) return undefined;
    const snap = ClaimedEventsSnapshot.getRootAsClaimedEventsSnapshot(bb);
    if (snap.entriesLength() === 0) return undefined;
    const entry = snap.entries(0);
    if (!entry) return undefined;
    const ev = entry.value();
    if (!ev) return undefined;
    const primaryId = ev.primaryId();
    if (primaryId === null) return undefined;
    const id = ev.id() ?? primaryId;
    const authorPubkey = ev.authorPubkey() ?? "";
    const kind = ev.kind();
    const createdAt = ev.createdAt();
    const content = ev.content() ?? "";
    return { primaryId, id, authorPubkey, kind, createdAt, content };
  } catch {
    return undefined;
  }
}

/** Host-side consumer of the `refs.event` row-delta projection. */
export class RefEventStore {
  private cache = new RefRowCache();

  /** Apply one frame's `refs.event` sidecar payload (an encoded NRRD batch)
   *  under the frame's `(sessionId, snapshotEpoch)` identity. */
  applySidecar(payload: Uint8Array, sessionId: bigint, snapshotEpoch: bigint): RefRowApplyOutcome {
    return this.cache.applySidecar(
      payload,
      sessionId,
      snapshotEpoch,
      (_key, bytes) => decodeEventRow(bytes) !== undefined,
    );
  }

  /** The decoded `ClaimedEventWire` for `primaryId`, or `undefined` if not cached. */
  event(primaryId: string): ClaimedEventWire | undefined {
    const payload = this.cache.get(REFS_EVENT_NAMESPACE, primaryId);
    if (!payload) return undefined;
    return decodeEventRow(payload);
  }

  /** The full materialised `primaryId -> ClaimedEventWire` set currently cached. */
  events(): Map<string, ClaimedEventWire> {
    const out = new Map<string, ClaimedEventWire>();
    for (const [key, payload] of this.cache.snapshot(REFS_EVENT_NAMESPACE)) {
      const wire = decodeEventRow(payload);
      if (wire) out.set(key, wire);
    }
    return out;
  }

  /** Whether the underlying cache has applied a baseline (UI-gating flag). */
  baselined(): boolean {
    return this.cache.baselined();
  }
}
