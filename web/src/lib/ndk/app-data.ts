import type NDK from '@nostr-dev-kit/ndk';
import { NDKEvent, NDKKind, type NDKFilter } from '@nostr-dev-kit/ndk';
import { cleanText } from './format';

/**
 * NIP-78 application-specific data (kind:30078) — addressable replaceable
 * event keyed by `d` tag, scoped per pubkey. We use it to persist things like
 * relay roles (rooms-host / indexer / search) that are not covered by NIP-65.
 */
export const APP_DATA_KIND = NDKKind.AppSpecificData;

export const APP_DATA_DTAG_RELAY_ROLES = 'highlighter:relay-roles';

export type AppDataPayload = {
  content: string;
  tags?: string[][];
};

export async function fetchAppData(
  ndk: NDK,
  pubkey: string,
  dTag: string
): Promise<NDKEvent | undefined> {
  const author = cleanText(pubkey);
  const d = cleanText(dTag);
  if (!author || !d) return undefined;

  const filter: NDKFilter = {
    kinds: [APP_DATA_KIND],
    authors: [author],
    '#d': [d],
    limit: 1
  };

  const event = await ndk.fetchEvent(filter, { closeOnEose: true });
  return event ?? undefined;
}

export async function publishAppData(
  ndk: NDK,
  dTag: string,
  payload: AppDataPayload
): Promise<NDKEvent> {
  if (!ndk.signer) {
    throw new Error('Connect a signer before publishing app data.');
  }

  const d = cleanText(dTag);
  if (!d) throw new Error('App-data d-tag is required');

  const event = new NDKEvent(ndk);
  event.kind = APP_DATA_KIND;
  event.content = payload.content;

  const passthroughTags = (payload.tags ?? []).filter(
    (tag) => Array.isArray(tag) && tag[0] !== 'd'
  );
  event.tags = [['d', d], ...passthroughTags];

  await event.sign();
  await event.publishReplaceable();
  return event;
}

/**
 * Relay roles stored as JSON in the kind:30078 content blob, keyed by relay URL.
 * Mirrors the iOS RelayConfig structure (NIP-65 kind:10002 stays the source of
 * truth for read/write, this layer adds Highlighter-specific roles on top).
 */
export type RelayRole = 'rooms-host' | 'indexer' | 'search';

export type RelayRoleMap = Record<string, RelayRole[]>;

export function parseRelayRoles(event: NDKEvent | undefined): RelayRoleMap {
  if (!event) return {};

  try {
    const parsed = JSON.parse(event.content) as unknown;
    if (!parsed || typeof parsed !== 'object') return {};

    const result: RelayRoleMap = {};
    for (const [url, roles] of Object.entries(parsed as Record<string, unknown>)) {
      if (!Array.isArray(roles)) continue;
      const normalized = roles
        .map((role) => cleanText(typeof role === 'string' ? role : ''))
        .filter((role): role is RelayRole =>
          role === 'rooms-host' || role === 'indexer' || role === 'search'
        );
      if (normalized.length > 0) {
        result[cleanText(url)] = normalized;
      }
    }
    return result;
  } catch {
    return {};
  }
}

export async function publishRelayRoles(ndk: NDK, roles: RelayRoleMap): Promise<NDKEvent> {
  return publishAppData(ndk, APP_DATA_DTAG_RELAY_ROLES, {
    content: JSON.stringify(roles)
  });
}
