import { NDKKind, type NDKEvent, type NDKUserProfile } from '@nostr-dev-kit/ndk';
import {
  articleImageUrl,
  articlePublishedAt,
  articleSummary,
  articleTitle,
  cleanText,
  displayName,
  formatDisplayDate,
  profileIdentifier,
  shortPubkey
} from '$lib/ndk/format';
import { GROUP_RELAY_URLS } from '$lib/ndk/config';
import { buildRoomSummariesFromMetadataEvents } from '$lib/server/rooms';
import { fetchEventsForSsr, fetchProfilesByPubkeys } from '$lib/server/nostr';
import {
  DEFAULT_SEARCH_SECTION_LIMIT,
  MAX_SEARCH_SECTION_LIMIT,
  MIN_SEARCH_QUERY_LENGTH,
  type SearchArticleResult,
  type SearchResponse
} from '$lib/search';

type SearchRelayContentOptions = {
  roomLimit?: number;
  articleLimit?: number;
};

export async function searchRelayContent(
  query: string,
  options: SearchRelayContentOptions = {}
): Promise<SearchResponse> {
  const normalizedQuery = cleanText(query);
  const roomLimit = normalizeLimit(options.roomLimit);
  const articleLimit = normalizeLimit(options.articleLimit);

  if (normalizedQuery.length < MIN_SEARCH_QUERY_LENGTH) {
    return {
      query: normalizedQuery,
      rooms: [],
      articles: []
    };
  }

  const [roomEvents, articleEvents] = await Promise.all([
    fetchEventsForSsr(
      {
        kinds: [NDKKind.GroupMetadata],
        search: normalizedQuery,
        limit: Math.max(roomLimit * 3, DEFAULT_SEARCH_SECTION_LIMIT)
      },
      `searchRelayContent:rooms(${normalizedQuery})`,
      { relays: GROUP_RELAY_URLS }
    ),
    fetchEventsForSsr(
      {
        kinds: [30023],
        search: normalizedQuery,
        limit: articleLimit
      },
      `searchRelayContent:articles(${normalizedQuery})`,
      { relays: GROUP_RELAY_URLS }
    )
  ]);

  const rooms = (await buildRoomSummariesFromMetadataEvents(Array.from(roomEvents ?? [])))
    .filter((room) => room.visibility === 'public')
    .slice(0, roomLimit);

  const articleList = Array.from(articleEvents ?? []);
  const profilesByPubkey = await fetchProfilesByPubkeys(articleList.map((event) => event.pubkey));
  const articles = articleList
    .map((event) => {
      try {
        return buildSearchArticleResult(event, profilesByPubkey[event.pubkey]);
      } catch (error) {
        console.warn(`searchRelayContent: failed to build article ${event.id}`, error);
        return null;
      }
    })
    .filter((article): article is SearchArticleResult => Boolean(article))
    .slice(0, articleLimit);

  return {
    query: normalizedQuery,
    rooms,
    articles
  };
}

function buildSearchArticleResult(
  event: NDKEvent,
  profile: NDKUserProfile | undefined
): SearchArticleResult {
  const rawEvent = event.rawEvent();

  return {
    id: event.id,
    noteIdentifier: eventIdentifier(event),
    title: articleTitle(rawEvent),
    summary: articleSummary(rawEvent, 180),
    image: articleImageUrl(rawEvent) ?? '',
    authorName: displayName(profile, shortPubkey(event.pubkey)),
    authorIdentifier: profileIdentifier(profile, event.pubkey),
    authorPubkey: event.pubkey,
    publishedLabel: formatDisplayDate(articlePublishedAt(rawEvent))
  };
}

function eventIdentifier(event: NDKEvent): string {
  try {
    return event.encode();
  } catch {
    return event.tagId() || event.id;
  }
}

function normalizeLimit(value: number | undefined): number {
  if (!Number.isFinite(value)) {
    return DEFAULT_SEARCH_SECTION_LIMIT;
  }

  return Math.min(
    MAX_SEARCH_SECTION_LIMIT,
    Math.max(MIN_SEARCH_QUERY_LENGTH, Math.trunc(value ?? DEFAULT_SEARCH_SECTION_LIMIT))
  );
}
