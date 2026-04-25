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

/**
 * NIP-51 kind:10009 — user's "communities" list. Each `group` tag is
 * `['group', '<group-id>', '<relay-url>']`. iOS publishes this for friends'
 * rooms; we mirror the shape so the rooms explorer can light up its "friends
 * are here" shelf and Highlighter's curated featured-rooms list.
 */
export const COMMUNITIES_LIST_KIND = 10009 as const;

export type GroupRef = {
  groupId: string;
  relayUrl: string;
};

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

export function bookmarkUrlsFromEvent(event: NDKEvent | undefined): string[] {
  const list = bookmarkListFromEvent(event);
  if (!list) return [];

  return uniqueValues(
    list
      .getItems('r')
      .map((tag) => cleanText(tag[1]))
      .filter(Boolean)
  );
}

export function bookmarkListHasUrl(event: NDKEvent | undefined, url: string): boolean {
  const normalizedUrl = cleanText(url);
  if (!normalizedUrl) return false;

  const list = bookmarkListFromEvent(event);
  if (!list) return false;

  return list.getItems('r').some((tag) => cleanText(tag[1]) === normalizedUrl);
}

export async function setBookmarkUrlPresence(
  ndk: NDK,
  event: NDKEvent | undefined,
  url: string,
  present: boolean
): Promise<void> {
  const normalizedUrl = cleanText(url);
  if (!normalizedUrl) return;

  const list = event ? NDKList.from(event) : new NDKList(ndk);
  list.kind = BOOKMARK_LIST_KIND;

  const hasUrl = () => list.getItems('r').some((tag) => cleanText(tag[1]) === normalizedUrl);

  if (present) {
    if (hasUrl()) return;
    await list.addItem(['r', normalizedUrl]);
    await list.publishReplaceable();
    return;
  }

  let removed = false;
  while (hasUrl()) {
    await list.removeItemByValue(normalizedUrl, false);
    removed = true;
  }

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

export function groupRefsFromEvent(event: NDKEvent | undefined): GroupRef[] {
  if (!event) return [];

  return event.tags
    .filter((tag) => tag[0] === 'group')
    .map((tag) => ({
      groupId: cleanText(tag[1] ?? ''),
      relayUrl: cleanText(tag[2] ?? '')
    }))
    .filter((ref) => ref.groupId.length > 0);
}

export async function fetchCommunitiesList(
  ndk: NDK,
  pubkey: string
): Promise<NDKEvent | undefined> {
  return fetchLatestUserList(ndk, COMMUNITIES_LIST_KIND, pubkey);
}

export async function fetchFriendsCommunitiesLists(
  ndk: NDK,
  pubkeys: readonly string[]
): Promise<Map<string, GroupRef[]>> {
  const authors = uniqueValues([...pubkeys]);
  if (authors.length === 0) return new Map();

  const events = Array.from(
    (await ndk.fetchEvents(
      [{ kinds: [COMMUNITIES_LIST_KIND], authors, limit: authors.length * 2 }],
      { closeOnEose: true }
    )) ?? []
  );

  const byPubkey = new Map<string, NDKEvent>();
  for (const event of events) {
    const existing = byPubkey.get(event.pubkey);
    if (!existing || (event.created_at ?? 0) > (existing.created_at ?? 0)) {
      byPubkey.set(event.pubkey, event);
    }
  }

  const result = new Map<string, GroupRef[]>();
  for (const [pubkey, event] of byPubkey) {
    result.set(pubkey, groupRefsFromEvent(event));
  }
  return result;
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
