import type NDK from '@nostr-dev-kit/ndk';
import {
  NDKEvent,
  NDKKind,
  NDKRelaySet,
  type NDKFilter
} from '@nostr-dev-kit/ndk';
import { cleanText } from './format';

/**
 * NIP-50 relay-side search.
 *
 * NIP-50 adds an optional `search` field on REQ filters, only honored by relays
 * that advertise NIP-50 support. NIP-51 kind:10007 is a user-curated list of
 * search relays — when present we fan out there; otherwise we fall back to the
 * provided defaults.
 */
export const SEARCH_RELAY_LIST_KIND = 10007 as const;

export const DEFAULT_SEARCH_RELAYS = [
  'wss://relay.nostr.band',
  'wss://search.nos.today'
] as const;

export type SearchableKind =
  | NDKKind.Article
  | NDKKind.Metadata
  | NDKKind.Highlight
  | NDKKind.GroupMetadata
  | number;

type Filter = NDKFilter & { search?: string };

export function buildSearchFilter(input: {
  kinds: SearchableKind[] | readonly SearchableKind[];
  query: string;
  limit?: number;
}): Filter {
  const trimmed = cleanText(input.query);
  return {
    kinds: [...input.kinds],
    search: trimmed,
    limit: input.limit ?? 20
  };
}

export async function readSearchRelayList(ndk: NDK, pubkey: string): Promise<string[]> {
  const author = cleanText(pubkey);
  if (!author) return [];

  const event = await ndk.fetchEvent(
    { kinds: [SEARCH_RELAY_LIST_KIND], authors: [author], limit: 1 },
    { closeOnEose: true }
  );

  if (!event) return [];

  return uniqueRelayUrls(
    event.tags
      .filter((tag) => tag[0] === 'relay')
      .map((tag) => cleanText(tag[1]))
  );
}

export async function fetchSearch(
  ndk: NDK,
  input: {
    kinds: SearchableKind[] | readonly SearchableKind[];
    query: string;
    limit?: number;
    relays?: readonly string[];
  }
): Promise<NDKEvent[]> {
  const trimmed = cleanText(input.query);
  if (!trimmed) return [];

  const relayUrls = uniqueRelayUrls([
    ...(input.relays ?? []),
    ...DEFAULT_SEARCH_RELAYS
  ]);

  const relaySet = NDKRelaySet.fromRelayUrls(relayUrls, ndk);
  const filter = buildSearchFilter({
    kinds: input.kinds,
    query: trimmed,
    limit: input.limit
  });

  const events = await ndk.fetchEvents(filter, { closeOnEose: true }, relaySet);
  return Array.from(events ?? []);
}

function uniqueRelayUrls(urls: readonly (string | null | undefined)[]): string[] {
  return [...new Set(urls.map((url) => cleanText(url ?? '')).filter(Boolean))];
}
