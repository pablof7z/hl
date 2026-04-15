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

export async function decryptPrivateListTags(list: NDKList): Promise<ListTag[]> {
  const content = cleanText(list.content);
  if (!content) return [];
  if (!list.ndk?.signer) {
    throw new Error('A signer is required to read private list items.');
  }

  const user = await list.ndk.signer.user();

  for (const scheme of preferredEncryptionSchemes(content)) {
    try {
      const decrypted = await list.ndk.signer.decrypt(user, content, scheme);
      const parsed = JSON.parse(decrypted);
      if (!Array.isArray(parsed)) {
        return [];
      }

      return parsed
        .filter((tag): tag is unknown[] => Array.isArray(tag))
        .map((tag) => tag.map((value) => String(value ?? '')));
    } catch {
      continue;
    }
  }

  return [];
}

export async function publishPrivateListTags(list: NDKList, tags: ListTag[]): Promise<void> {
  if (!list.ndk?.signer) {
    throw new Error('A signer is required to publish private list items.');
  }

  if (tags.length === 0) {
    list.content = '';
  } else {
    const user = await list.ndk.signer.user();
    const payload = JSON.stringify(tags);
    let lastError: unknown;

    for (const scheme of ['nip44', 'nip04'] as const) {
      try {
        list.content = await list.ndk.signer.encrypt(user, payload, scheme);
        lastError = undefined;
        break;
      } catch (error) {
        lastError = error;
      }
    }

    if (typeof list.content !== 'string' || list.content.length === 0) {
      throw lastError instanceof Error
        ? lastError
        : new Error('Could not encrypt the private list content.');
    }
  }

  list.created_at = Math.floor(Date.now() / 1e3);
  await list.publishReplaceable();
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

function preferredEncryptionSchemes(content: string): Array<'nip44' | 'nip04'> {
  return /(?:\?|&)iv=/.test(content) || content.includes('?iv=')
    ? ['nip04', 'nip44']
    : ['nip44', 'nip04'];
}
