import type NDK from '@nostr-dev-kit/ndk';
import {
  NDKKind,
  NDKList,
  NDKRelayFeedList,
  type NDKEvent,
  type NDKFilter
} from '@nostr-dev-kit/ndk';
import { cleanText } from '$lib/ndk/format';

export const BOOKMARK_LIST_KIND = NDKKind.BookmarkList;
export const RELAY_FEED_LIST_KIND = NDKKind.RelayFeedList;
export type ListTag = string[];

export function latestListEvent(events: NDKEvent[]): NDKEvent | undefined {
  return [...events].toSorted((left, right) => (right.created_at ?? 0) - (left.created_at ?? 0))[0];
}

export async function fetchLatestUserList(
  ndk: NDK,
  kind: number,
  pubkey: string
): Promise<NDKEvent | undefined> {
  const normalizedPubkey = cleanText(pubkey);
  if (!normalizedPubkey) return undefined;

  const events = Array.from(
    (await ndk.fetchEvents(
      [{ kinds: [kind], authors: [normalizedPubkey], limit: 20 }],
      { closeOnEose: true }
    )) ?? []
  );

  return latestListEvent(events);
}

export function bookmarkListFromEvent(event: NDKEvent | undefined): NDKList | undefined {
  return event ? NDKList.from(event) : undefined;
}

export function ensureList(
  ndk: NDK,
  kind: number,
  event: NDKEvent | undefined
): NDKList {
  const list = event ? NDKList.from(event) : new NDKList(ndk);
  list.kind = kind as NDKKind;
  return list;
}

export function bookmarkAddressesFromEvent(
  event: NDKEvent | undefined,
  prefix?: string
): string[] {
  const list = bookmarkListFromEvent(event);
  if (!list) return [];

  const normalizedPrefix = cleanText(prefix);

  return uniqueValues(
    list
      .getItems('a')
      .map((tag) => cleanText(tag[1]))
      .filter((value) => !normalizedPrefix || value.startsWith(normalizedPrefix))
  );
}

export function bookmarkAddressFilters(addresses: string[]): NDKFilter[] {
  const normalizedAddresses = uniqueValues(addresses);
  if (normalizedAddresses.length === 0) return [];

  const list = new NDKList();
  list.kind = BOOKMARK_LIST_KIND;
  list.tags = normalizedAddresses.map((address) => ['a', address]);

  return list.filterForItems();
}

export function bookmarkListHasAddress(
  event: NDKEvent | undefined,
  address: string
): boolean {
  const normalizedAddress = cleanText(address);
  if (!normalizedAddress) return false;

  return bookmarkListFromEvent(event)?.has(normalizedAddress) ?? false;
}

export async function setBookmarkAddressPresence(
  ndk: NDK,
  event: NDKEvent | undefined,
  address: string,
  present: boolean
): Promise<void> {
  const normalizedAddress = cleanText(address);
  if (!normalizedAddress) return;

  const list = event ? NDKList.from(event) : new NDKList(ndk);
  list.kind = BOOKMARK_LIST_KIND;

  if (present) {
    if (list.has(normalizedAddress)) return;
    await list.addItem(['a', normalizedAddress]);
    await list.publishReplaceable();
    return;
  }

  const removed = await removeAllListItemsByValue(list, normalizedAddress);
  if (removed) {
    await list.publishReplaceable();
  }
}

export function relayFeedListFromEvent(event: NDKEvent | undefined): NDKRelayFeedList | undefined {
  return event ? NDKRelayFeedList.from(event) : undefined;
}

export function relayUrlsFromEvent(event: NDKEvent | undefined): string[] {
  return uniqueValues(relayFeedListFromEvent(event)?.relayUrls.map((relayUrl) => cleanText(relayUrl)) ?? []);
}

export function relayFeedHasUrl(event: NDKEvent | undefined, relayUrl: string): boolean {
  const normalizedRelayUrl = cleanText(relayUrl);
  if (!normalizedRelayUrl) return false;

  return relayFeedListFromEvent(event)?.has(normalizedRelayUrl) ?? false;
}

export async function setRelayFeedUrlPresence(
  ndk: NDK,
  event: NDKEvent | undefined,
  relayUrl: string,
  present: boolean
): Promise<void> {
  const normalizedRelayUrl = cleanText(relayUrl);
  if (!normalizedRelayUrl) return;

  const list = event ? NDKRelayFeedList.from(event) : new NDKRelayFeedList(ndk);
  list.kind = RELAY_FEED_LIST_KIND;

  if (present) {
    if (list.has(normalizedRelayUrl)) return;
    await list.addRelay(normalizedRelayUrl);
    await list.publishReplaceable();
    return;
  }

  const removed = await removeAllListItemsByValue(list, normalizedRelayUrl);
  if (removed) {
    await list.publishReplaceable();
  }
}

async function removeAllListItemsByValue(list: NDKList, value: string): Promise<boolean> {
  let removed = false;

  while (list.has(value)) {
    await list.removeItemByValue(value, false);
    removed = true;
  }

  return removed;
}

function uniqueValues(values: string[]): string[] {
  return [...new Set(values.map((value) => cleanText(value)).filter(Boolean))];
}
