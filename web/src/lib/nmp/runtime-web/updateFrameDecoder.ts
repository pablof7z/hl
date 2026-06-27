// updateFrameDecoder.ts — decode an `NMPU` UpdateFrame into typed projection data.
//
// Ported from nostr-multi-platform/web/chirp/src/nmp/feedDecoder.ts
// (NMP commit 37b606eaa839863c2644172a018814b95484981e).
//
// PURE: no worker/DOM globals — SSR-safe. Zero protocol logic: pure decode of
// bytes the Rust kernel emits via the `update_bytes` worker event.
//
// Caller pattern:
//   const frame = decodeUpdateFrame(latestUpdateBytes);
//   if (frame) {
//     // frame.projectionKeys — list of all projection keys in this snapshot
//     // frame.refsProfileBytes — raw NRRD sidecar for RefProfileStore
//     // frame.refsEventBytes — raw NRRD sidecar for RefEventStore (#65 S4)
//     // frame.sessionId / .snapshotEpoch — for RefRowCache identity tracking
//   }

import * as flatbuffers from "flatbuffers";

import { UpdateFrame } from "../generated/nmp/transport/update-frame.js";
import { FrameKind } from "../generated/nmp/transport/frame-kind.js";
import type { SnapshotFrame } from "../generated/nmp/transport/snapshot-frame.js";

/** The typed projection key for the profile resolver sidecar (mirrors REFS_PROFILE_KEY). */
export const REFS_PROFILE_PROJECTION_KEY = "refs.profile";
/** The typed projection key for the event resolver sidecar (mirrors REFS_EVENT_KEY). */
export const REFS_EVENT_PROJECTION_KEY = "refs.event";
const NRRD_FILE_IDENTIFIER = "NRRD";

/** Decoded UpdateFrame — projection keys + optional refs.profile / refs.event sidecars + identity fields. */
export type DecodedUpdateFrame = {
  /** All projection keys present in this snapshot (builtin + app-registered). */
  projectionKeys: string[];
  /** Raw NRRD bytes of the `refs.profile` sidecar, or undefined when absent. */
  refsProfileBytes?: Uint8Array;
  /** Raw NRRD bytes of the `refs.event` sidecar, or undefined when absent. */
  refsEventBytes?: Uint8Array;
  /** Session identity from the SnapshotFrame (for RefRowCache identity tracking). */
  sessionId: bigint;
  /** Snapshot epoch from the SnapshotFrame (for RefRowCache baseline rebuild). */
  snapshotEpoch: bigint;
};

/** Decode an `NMPU` UpdateFrame from raw bytes.
 *
 *  Returns `undefined` when:
 *  - bytes are malformed or the buffer lacks the `NMPU` file identifier
 *  - the frame is not a Snapshot kind (e.g. Panic frame)
 *
 *  On undefined the caller retains its last-good state (D6 fail-closed). */
export function decodeUpdateFrame(bytes: Uint8Array): DecodedUpdateFrame | undefined {
  try {
    const bb = new flatbuffers.ByteBuffer(bytes);
    if (!UpdateFrame.bufferHasIdentifier(bb)) return undefined;
    const frame = UpdateFrame.getRootAsUpdateFrame(bb);
    if (frame.kind() !== FrameKind.Snapshot) return undefined;
    const snap = frame.snapshot();
    if (!snap) return undefined;
    return decodeFromSnapshot(snap);
  } catch {
    return undefined;
  }
}

function decodeFromSnapshot(snap: SnapshotFrame): DecodedUpdateFrame {
  const result: DecodedUpdateFrame = {
    projectionKeys: [],
    sessionId: snap.sessionId(),
    snapshotEpoch: snap.snapshotEpoch(),
  };

  for (let i = 0; i < snap.typedProjectionsLength(); i++) {
    const proj = snap.typedProjections(i);
    if (!proj) continue;
    const key = proj.key();
    if (!key) continue;
    result.projectionKeys.push(key);

    const payload = proj.payload();
    if (!payload) continue;
    const payloadBytes = payload.payloadArray();
    if (!payloadBytes || payloadBytes.length === 0) continue;

    if (
      key === REFS_PROFILE_PROJECTION_KEY &&
      payload.fileIdentifier() === NRRD_FILE_IDENTIFIER
    ) {
      // Surface raw sidecar bytes for RefProfileStore to merge (ADR-0063).
      // Copy so the view outlives the ByteBuffer.
      result.refsProfileBytes = payloadBytes.slice();
    }

    if (
      key === REFS_EVENT_PROJECTION_KEY &&
      payload.fileIdentifier() === NRRD_FILE_IDENTIFIER
    ) {
      // Surface raw sidecar bytes for RefEventStore to merge (ADR-0063, #65 S4).
      // Copy so the view outlives the ByteBuffer.
      result.refsEventBytes = payloadBytes.slice();
    }
  }

  return result;
}
