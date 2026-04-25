import type { NDKEvent, NDKFilter } from '@nostr-dev-kit/ndk';
import { NDKKind } from '@nostr-dev-kit/ndk';
import { HIGHLIGHTER_HIGHLIGHT_KIND } from '$lib/ndk/highlights';

export type FeedItem =
  | { kind: 'read'; eventId: string; createdAt: number; articleAddress: string | null; rawEvent: NDKEvent }
  | { kind: 'highlight'; eventId: string; createdAt: number; articleAddress: string | null; rawEvent: NDKEvent };

export function buildFollowingFeedFilters(follows: string[]): NDKFilter[] {
  if (follows.length === 0) return [];
  return [
    { kinds: [NDKKind.Article], authors: follows, limit: 60 } as NDKFilter,
    { kinds: [HIGHLIGHTER_HIGHLIGHT_KIND], authors: follows, limit: 60 } as NDKFilter
  ];
}

function articleAddressFromEvent(event: NDKEvent): string | null {
  // kind:30023 — address is kind:pubkey:d-tag
  if (event.kind === NDKKind.Article) {
    const dTag = event.tagValue('d');
    if (dTag != null) return `${NDKKind.Article}:${event.pubkey}:${dTag}`;
    return null;
  }
  // kind:9802 highlight — look for 'a' tag pointing to a 30023
  const aTag = event.tagValue('a');
  if (aTag) {
    const parts = aTag.split(':');
    if (parts[0] === String(NDKKind.Article)) return aTag;
  }
  return null;
}

/**
 * Merges a flat list of kind:30023 and kind:9802 NDKEvents into a
 * deduped, sorted FeedItem array.
 *
 * Dedup rule (mirrors iOS HomeFeedStore.recompute):
 *   If an article has at least one highlight from the same social graph,
 *   drop the bare article-card row — the highlight row already represents
 *   the piece with a friend's voice. An article only gets the bare card
 *   treatment when no follow has highlighted it.
 */
export function mergeFeed(events: NDKEvent[]): FeedItem[] {
  const highlightedAddresses = new Set<string>();
  const articles: FeedItem[] = [];
  const highlights: FeedItem[] = [];

  for (const event of events) {
    const id = event.id;
    if (!id) continue;
    const createdAt = event.created_at ?? 0;
    const articleAddress = articleAddressFromEvent(event);

    if (event.kind === NDKKind.Article) {
      articles.push({ kind: 'read', eventId: id, createdAt, articleAddress, rawEvent: event });
    } else if (event.kind === HIGHLIGHTER_HIGHLIGHT_KIND) {
      highlights.push({ kind: 'highlight', eventId: id, createdAt, articleAddress, rawEvent: event });
      if (articleAddress) highlightedAddresses.add(articleAddress);
    }
  }

  const dedupedArticles = articles.filter(
    (item) => !item.articleAddress || !highlightedAddresses.has(item.articleAddress)
  );

  return [...highlights, ...dedupedArticles].sort((a, b) => b.createdAt - a.createdAt);
}
