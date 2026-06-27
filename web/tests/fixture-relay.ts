/**
 * NIP-01 in-memory fixture relay for the NMP web bridge acceptance suite.
 *
 * Ported from nostr-multi-platform/web/chirp/tests/fixture-relay.ts
 * (NMP commit 37b606eaa839863c2644172a018814b95484981e) and extended to use
 * TWO relay ports for hermetic single-ref profile resolution:
 *
 *   - url (bootstrap) — the relay the wasm connects to via relay_bootstrap.
 *                       Serves Alice's kind:10002 listing outboxUrl as write relay.
 *   - outboxUrl       — Alice's outbox relay. The wasm discovers this via the
 *                       NIP-65 hop, then connects here to fetch Alice's kind:0
 *                       for refs.profile.
 *
 * Two separate ports ensure the wasm establishes a NEW connection to the outbox
 * relay rather than reusing its existing bootstrap connection. The wasm's relay
 * pool treats the same URL with two roles differently, so distinct ports guarantee
 * the outbox subscription fires.
 *
 * Both relays run in the Node.js Playwright process. No external network.
 */

import { WebSocketServer } from "ws";
import type { WebSocket } from "ws";
import type { AddressInfo } from "net";
import { finalizeEvent, generateSecretKey, getPublicKey } from "nostr-tools/pure";

export type NostrEvent = {
  id: string;
  pubkey: string;
  created_at: number;
  kind: number;
  tags: string[][];
  content: string;
  sig: string;
};

export type NostrFilter = {
  kinds?: number[];
  authors?: string[];
  ids?: string[];
  since?: number;
  until?: number;
  limit?: number;
  [key: string]: unknown;
};

export type ProfileFixtureRelay = {
  /** Bootstrap relay URL — pass as relay_bootstrap host param. */
  url: string;
  /** Alice's outbox relay URL (NIP-65 discovery hop resolves here). */
  outboxUrl: string;
  /** Secret key for the seeded profile account (Uint8Array). */
  viewerSk: Uint8Array;
  /** Hex pubkey of the seeded profile account. */
  viewerPubkey: string;
  /** Display name of the seeded profile. */
  displayName: string;
  /** Number of inbound WebSocket connections to the bootstrap relay. */
  connectionCount(): number;
  /** Number of inbound WebSocket connections to the outbox relay. */
  outboxConnectionCount(): number;
  /** All NIP-01 REQ filter arrays received by both relays combined. */
  receivedFilters(): NostrFilter[][];
  /** Gracefully close both relay servers. */
  close(): Promise<void>;
};

function matchesFilter(event: NostrEvent, filter: NostrFilter): boolean {
  if (filter.kinds !== undefined && !filter.kinds.includes(event.kind)) return false;
  if (filter.authors !== undefined && !filter.authors.includes(event.pubkey)) return false;
  if (filter.ids !== undefined && !filter.ids.includes(event.id)) return false;
  if (filter.since !== undefined && event.created_at < filter.since) return false;
  if (filter.until !== undefined && event.created_at > filter.until) return false;
  return true;
}

/** Start a single bare WebSocket NIP-01 relay on a random port.
 *  `events` is a live reference — push events into it after the server starts. */
function startServer(
  events: NostrEvent[],
  allFilters: NostrFilter[][],
): Promise<{
  url: string;
  connectionCount(): number;
  close(): Promise<void>;
}> {
  return new Promise((resolve, reject) => {
    const wss = new WebSocketServer({ host: "127.0.0.1", port: 0 });
    let connections = 0;

    wss.once("error", reject);

    wss.once("listening", () => {
      const { port } = wss.address() as AddressInfo;
      const url = `ws://127.0.0.1:${port}`;

      wss.on("connection", (ws: WebSocket) => {
        connections += 1;

        ws.on("message", (raw: Buffer | string) => {
          let msg: unknown;
          try {
            msg = JSON.parse(typeof raw === "string" ? raw : raw.toString());
          } catch {
            return;
          }
          if (!Array.isArray(msg) || msg.length === 0) return;
          const [verb, ...rest] = msg as [string, ...unknown[]];

          if (verb === "REQ" && typeof rest[0] === "string") {
            const subId = rest[0];
            const filters = (rest.slice(1) as NostrFilter[]).filter(
              (f) => typeof f === "object" && f !== null,
            );
            // Append to the shared diagnostics array.
            allFilters.push(filters);
            // Serve seeded events that match the filter.
            for (const event of events) {
              const matched =
                filters.length === 0 || filters.some((f) => matchesFilter(event, f));
              if (matched) {
                ws.send(JSON.stringify(["EVENT", subId, event]));
              }
            }
            ws.send(JSON.stringify(["EOSE", subId]));
          } else if (verb === "EVENT") {
            const event = rest[0] as Record<string, unknown> | undefined;
            const eventId = typeof event?.id === "string" ? event.id : "";
            ws.send(JSON.stringify(["OK", eventId, true, ""]));
          }
          // CLOSE: no response required per NIP-01.
        });

        ws.on("error", () => {});
      });

      const close = (): Promise<void> =>
        new Promise<void>((res, rej) => {
          for (const client of wss.clients) client.terminate();
          wss.close((err) => (err ? rej(err) : res()));
        });

      resolve({ url, connectionCount: () => connections, close });
    });
  });
}

/**
 * Two-relay profile fixture for hermetic NMP single-ref profile resolution.
 *
 * Bootstrap relay:
 *   - Serves Alice's kind:0 and kind:10002 (relay list → outbox relay).
 *   - The wasm's initial bootstrap connects here.
 *
 * Outbox relay:
 *   - Serves Alice's kind:0.
 *   - The wasm discovers this URL from Alice's kind:10002 and connects here
 *     for the refs.profile outbox subscription.
 */
export async function startProfileFixtureRelay(): Promise<ProfileFixtureRelay> {
  const sk = generateSecretKey();
  const pubkey = getPublicKey(sk);
  const displayName = "Alice Fixture";
  const now = Math.floor(Date.now() / 1000);
  const allFilters: NostrFilter[][] = [];

  const profile = finalizeEvent(
    {
      kind: 0,
      created_at: now - 10,
      tags: [],
      content: JSON.stringify({
        display_name: displayName,
        name: "alice-fallback",
        about: "Fixture profile for NMP Slice 3 tests",
      }),
    },
    sk,
  ) as NostrEvent;

  // Outbox relay event list — starts with Alice's kind:0.
  // The kind:10002 (which self-references outboxUrl) will be pushed in after
  // the server starts (we capture a live reference to the array).
  const outboxEvents: NostrEvent[] = [profile];
  const outboxServer = await startServer(outboxEvents, allFilters);
  const outboxUrl = outboxServer.url;

  // NIP-65 kind:10002 — Alice's relay list. No role tag means both read+write
  // so the wasm's NIP-65 discovery hop treats this as an outbox relay regardless
  // of which role it looks for when resolving a profile (write = outbox, or
  // combined = both). Using no role is the most permissive.
  const relayList = finalizeEvent(
    {
      kind: 10002,
      created_at: now - 5,
      tags: [["r", outboxUrl]],
      content: "",
    },
    sk,
  ) as NostrEvent;

  // Add the relay list to the outbox events (live reference mutation).
  outboxEvents.push(relayList);

  // Bootstrap relay event list — serves Alice's kind:0 AND kind:10002 (so the
  // wasm can discover the outbox relay URL during the initial account bootstrap).
  const bootstrapEvents: NostrEvent[] = [profile, relayList];
  const bootstrapServer = await startServer(bootstrapEvents, allFilters);

  const close = async (): Promise<void> => {
    await Promise.all([bootstrapServer.close(), outboxServer.close()]);
  };

  return {
    url: bootstrapServer.url,
    outboxUrl,
    connectionCount: bootstrapServer.connectionCount,
    outboxConnectionCount: outboxServer.connectionCount,
    receivedFilters: () => allFilters,
    close,
    viewerSk: sk,
    viewerPubkey: pubkey,
    displayName,
  };
}

export type EventFixtureRelay = {
  /** Relay URL — pass as relay_bootstrap host param. */
  url: string;
  /** Hex event id of the seeded kind:1 event. */
  eventId: string;
  /** Content of the seeded kind:1 event. */
  content: string;
  /** Hex pubkey of the event author (used as set_identity_pubkey to anchor relay). */
  pubkey: string;
  /** Number of inbound WebSocket connections to the relay. */
  connectionCount(): number;
  /** All NIP-01 REQ filter arrays received by the relay. */
  receivedFilters(): NostrFilter[][];
  /** Gracefully close the relay server. */
  close(): Promise<void>;
};

/**
 * Single-relay event fixture for hermetic NMP single-ref event resolution.
 *
 * CURRENT STATE (HONEST FAILURE — as of committed wasm binary):
 *
 * The wasm DOES issue `{ids:[eventId], limit:1}` REQs when resolve_ref(1,
 * eventId) is called. The relay serves the kind:1 event. But the refs.event
 * NRRD sidecar (ClaimedEventsSnapshot) stays empty — 0 rows in both the
 * baseline and incremental batches.
 *
 * ROOT CAUSE: The wasm's refs.event projection (ClaimedEventsSnapshot) only
 * populates events that have been "claimed" through the kernel's internal write
 * pathway (authored, published, or explicitly saved by the user). Events fetched
 * externally via resolve_ref {ids:[eventId]} subscriptions are stored in the
 * raw Nostr-SDK event buffer but not promoted to the ClaimedEventsSnapshot tier.
 *
 * The fixture is deliberately kept simple (kind:1 + kind:10002) so it can be
 * evolved when the upstream wasm wires the event-claim path:
 *   - kind:1 event: authored by the identity keypair, should eventually
 *     appear as a KCEV row in refs.event once the claim path is wired.
 *   - kind:10002 relay list: NIP-65 discovery so the wasm routes the
 *     {ids:[eventId]} REQ to the correct relay.
 *
 * Expected (not yet working) flow:
 *   1. Wasm connects via relay_bootstrap.
 *   2. Wasm calls set_identity → subscriptions → relay serves kind:10002.
 *   3. Probe retries resolveEvent(eventId) on each UpdateFrame.
 *   4. Wasm issues {ids:[eventId],limit:1} → relay serves kind:1 event.
 *   5. Wasm claims event → emits refs.event NRRD with KCEV row. [NOT YET]
 *   6. Host decodes ClaimedEventsSnapshot → surfaces id + content.   [NOT YET]
 */
export async function startEventFixtureRelay(): Promise<EventFixtureRelay> {
  const sk = generateSecretKey();
  const pubkey = getPublicKey(sk);
  const content = "Hello NMP event resolve round-trip";
  const now = Math.floor(Date.now() / 1000);
  const allFilters: NostrFilter[][] = [];

  // Start the server first so we know its URL, then build events that
  // reference that URL. Use a live-reference array so events pushed after
  // server.start() are still served to subsequent REQ messages.
  const events: NostrEvent[] = [];
  const server = await startServer(events, allFilters);

  // kind:1 (regular note) — the most basic event type. The wasm stores kind:1
  // events under their hex event ID (no NIP-33 addressable key remapping).
  // When resolve_ref(1, eventId) is called, the wasm issues {ids:[eventId]}
  // to the relay and ingests the event. The NRRD row key matches the hex event
  // ID, so host-side cache.get("event", eventId) finds the KCEV payload.
  const event = finalizeEvent(
    {
      kind: 1,
      created_at: now - 10,
      tags: [],
      content,
    },
    sk,
  ) as NostrEvent;

  // NIP-65 relay list (kind:10002) pointing to this relay itself.
  // The wasm discovers this after setSigner, confirming our relay as the
  // author's outbox (so the relay-pool sends the {ids:[eventId]} REQ here).
  const relayList = finalizeEvent(
    {
      kind: 10002,
      created_at: now - 5,
      tags: [["r", server.url]],
      content: "",
    },
    sk,
  ) as NostrEvent;

  // Populate the live events array (server has a reference to it).
  events.push(event, relayList);

  return {
    url: server.url,
    eventId: event.id,
    content,
    pubkey,
    connectionCount: server.connectionCount,
    receivedFilters: () => allFilters,
    close: server.close,
  };
}
