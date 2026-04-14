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
import { buildCommunitySummariesFromMetadataEvents } from '$lib/server/communities';
import { fetchProfilesByPubkeys, getServerNdk } from '$lib/server/nostr';
import {
  DEFAULT_SEARCH_SECTION_LIMIT,
  MAX_SEARCH_SECTION_LIMIT,
  MIN_SEARCH_QUERY_LENGTH,
  type SearchArticleResult,
  type SearchResponse
} from '$lib/search';

type SearchRelayContentOptions = {
  communityLimit?: number;
  articleLimit?: number;
};

export async function searchRelayContent(
  query: string,
  options: SearchRelayContentOptions = {}
): Promise<SearchResponse> {
  const normalizedQuery = cleanText(query);
  const communityLimit = normalizeLimit(options.communityLimit);
  const articleLimit = normalizeLimit(options.articleLimit);

  if (normalizedQuery.length < MIN_SEARCH_QUERY_LENGTH) {
    return {
      query: normalizedQuery,
      communities: [],
      articles: []
    };
  }

  const ndk = await getServerNdk(GROUP_RELAY_URLS);
  const [communityEvents, articleEvents] = await Promise.all([
    ndk.fetchEvents(
      {
        kinds: [NDKKind.GroupMetadata],
        search: normalizedQuery,
        limit: Math.max(communityLimit * 3, DEFAULT_SEARCH_SECTION_LIMIT)
      },
      { closeOnEose: true }
    ),
    ndk.fetchEvents(
      {
        kinds: [30023],
        search: normalizedQuery,
        limit: articleLimit
      },
      { closeOnEose: true }
    )
  ]);

  const communities = (await buildCommunitySummariesFromMetadataEvents(Array.from(communityEvents ?? [])))
    .filter((community) => community.visibility === 'public')
    .slice(0, communityLimit);

  const articleList = Array.from(articleEvents ?? []);
  const profilesByPubkey = await fetchProfilesByPubkeys(articleList.map((event) => event.pubkey));
  const articles = articleList
    .map((event) => buildSearchArticleResult(event, profilesByPubkey[event.pubkey]))
    .slice(0, articleLimit);

  return {
    query: normalizedQuery,
    communities,
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
