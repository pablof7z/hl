// ADR-0063 Lane F — host-side `refs.profile` consumption helper.
//
// Ported from nostr-multi-platform/web/chirp/src/nmp/refProfileStore.ts
// (NMP commit 37b606eaa839863c2644172a018814b95484981e).
//
// `RefProfileStore` is the host-side consumer of the `refs.profile`
// row-delta projection. It wraps a `RefRowCache` and exposes typed
// `profile(pubkey)` / `profiles()` lookups over decoded `ProfileWire` values.
//
// One instance lives for the lifetime of the client's update loop — NOT
// rebuilt per frame, because the cache is stateful (incremental deltas).
//
// The ONLY app-side mirror of hydrated profile facts (D4 / invariant v).

import * as flatbuffers from "flatbuffers";

import { ProfileSnapshot } from "../generated/nmp/kernel/profile-snapshot.js";
import { RefRowCache, type RefRowApplyOutcome } from "./refRowCache.js";

/** Thin hydrated profile type (host-side, wire-level). */
export type ProfileWire = {
  pubkey: string;
  displayName?: string;
  pictureUrl?: string;
  nip05?: string;
  about?: string;
  lnurl?: string;
};

/** The kernel-emitted projection key for the profile resolver. */
export const REFS_PROFILE_KEY = "refs.profile";
const REFS_PROFILE_NAMESPACE = "profile";

/** Decode a KPRF `ProfileSnapshot` row payload into a `ProfileWire`, or
 *  `undefined` when the bytes are not a well-formed ProfileSnapshot. */
function decodeProfileRow(bytes: Uint8Array): ProfileWire | undefined {
  if (bytes.length < 8) return undefined;
  try {
    const bb = new flatbuffers.ByteBuffer(bytes);
    if (!ProfileSnapshot.bufferHasIdentifier(bb)) return undefined;
    const snap = ProfileSnapshot.getRootAsProfileSnapshot(bb);
    const card = snap.card();
    if (!card) return undefined;
    const key = card.pubkey();
    if (key === null) return undefined;
    const wire: ProfileWire = { pubkey: key };
    if (card.hasDisplayName()) {
      const v = card.displayName();
      if (v) wire.displayName = v;
    }
    if (card.hasPictureUrl()) {
      const v = card.pictureUrl();
      if (v) wire.pictureUrl = v;
    }
    const nip05 = card.nip05();
    if (nip05) wire.nip05 = nip05;
    const about = card.about();
    if (about) wire.about = about;
    if (card.hasLnurl()) {
      const v = card.lnurl();
      if (v) wire.lnurl = v;
    }
    return wire;
  } catch {
    return undefined;
  }
}

/** Host-side consumer of the `refs.profile` row-delta projection. */
export class RefProfileStore {
  private cache = new RefRowCache();

  /** Apply one frame's `refs.profile` sidecar payload (an encoded NRRD batch)
   *  under the frame's `(sessionId, snapshotEpoch)` identity. */
  applySidecar(payload: Uint8Array, sessionId: bigint, snapshotEpoch: bigint): RefRowApplyOutcome {
    return this.cache.applySidecar(
      payload,
      sessionId,
      snapshotEpoch,
      (_key, bytes) => decodeProfileRow(bytes) !== undefined,
    );
  }

  /** The decoded `ProfileWire` for `pubkey`, or `undefined` if not cached. */
  profile(pubkey: string): ProfileWire | undefined {
    const payload = this.cache.get(REFS_PROFILE_NAMESPACE, pubkey);
    if (!payload) return undefined;
    return decodeProfileRow(payload);
  }

  /** The full materialised `pubkey -> ProfileWire` set currently cached. */
  profiles(): Map<string, ProfileWire> {
    const out = new Map<string, ProfileWire>();
    for (const [key, payload] of this.cache.snapshot(REFS_PROFILE_NAMESPACE)) {
      const wire = decodeProfileRow(payload);
      if (wire) out.set(key, wire);
    }
    return out;
  }

  /** Whether the underlying cache has applied a baseline (UI-gating flag). */
  baselined(): boolean {
    return this.cache.baselined();
  }
}
