import { NDKKind, nip19, profileFromEvent, type NDKEvent, type NDKUserProfile } from '@nostr-dev-kit/ndk';
import {
  articleImageUrl,
  articlePublishedAt,
  articleSummary,
  articleTitle,
  avatarUrl,
  cleanText,
  displayName,
  displayNip05,
  formatDisplayDate,
  profileIdentifier,
  shortPubkey,
  truncate
} from '$lib/ndk/format';
import { DEFAULT_SEARCH_RELAYS } from '$lib/ndk/search';
import { GROUP_RELAY_URLS } from '$lib/ndk/config';
import { buildRoomSummariesFromMetadataEvents } from '$lib/server/rooms';
import { fetchEventsForSsr, fetchProfilesByPubkeys } from '$lib/server/nostr';
import {
  DEFAULT_SEARCH_SECTION_LIMIT,
  MAX_SEARCH_SECTION_LIMIT,
  MIN_SEARCH_QUERY_LENGTH,
  type SearchArticleResult,
  type SearchHighlightResult,
  type SearchProfileResult,
  type SearchResponse
} from '$lib/search';

type SearchRelayContentOptions = {
  roomLimit?: number;
  articleLimit?: number;
  profileLimit?: number;
  highlightLimit?: number;
};

export async function searchRelayContent(
  query: string,
  options: SearchRelayContentOptions = {}
): Promise<SearchResponse> {
  const normalizedQuery = cleanText(query);
  const roomLimit = normalizeLimit(options.roomLimit);
  const articleLimit = normalizeLimit(options.articleLimit);
  const profileLimit = normalizeLimit(options.profileLimit);
  const highlightLimit = normalizeLimit(options.highlightLimit);

  if (normalizedQuery.length < MIN_SEARCH_QUERY_LENGTH) {
    return {
      query: normalizedQuery,
      rooms: [],
      articles: [],
      profiles: [],
      highlights: []
    };
  }

  const searchRelays = [...DEFAULT_SEARCH_RELAYS];

  const [roomEvents, articleEvents, profileEvents, highlightEvents] = await Promise.all([
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
        kinds: [NDKKind.Article],
        search: normalizedQuery,
        limit: articleLimit
      },
      `searchRelayContent:articles(${normalizedQuery})`,
      { relays: GROUP_RELAY_URLS }
    ),
    fetchEventsForSsr(
      {
        kinds: [NDKKind.Metadata],
        search: normalizedQuery,
        limit: profileLimit
      },
      `searchRelayContent:profiles(${normalizedQuery})`,
      { relays: searchRelays }
    ),
    fetchEventsForSsr(
      {
        kinds: [NDKKind.Highlight],
        search: normalizedQuery,
        limit: highlightLimit
      },
      `searchRelayContent:highlights(${normalizedQuery})`,
      { relays: searchRelays }
    )
  ]);

  const rooms = (await buildRoomSummariesFromMetadataEvents(Array.from(roomEvents ?? [])))
    .filter((room) => room.visibility === 'public')
    .slice(0, roomLimit);

  const articleList = Array.from(articleEvents ?? []);
  const articlePubkeys = articleList.map((event) => event.pubkey);
  const highlightList = Array.from(highlightEvents ?? []);
  const highlightPubkeys = highlightList.map((event) => event.pubkey);
  const allPubkeys = [...new Set([...articlePubkeys, ...highlightPubkeys])];

  const profilesByPubkey = await fetchProfilesByPubkeys(allPubkeys);

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

  const profiles = Array.from(profileEvents ?? [])
    .map((event) => {
      try {
        return buildSearchProfileResult(event);
      } catch (error) {
        console.warn(`searchRelayContent: failed to build profile ${event.pubkey}`, error);
        return null;
      }
    })
    .filter((profile): profile is SearchProfileResult => Boolean(profile))
    .slice(0, profileLimit);

  const highlights = highlightList
    .map((event) => {
      try {
        return buildSearchHighlightResult(event, profilesByPubkey[event.pubkey]);
      } catch (error) {
        console.warn(`searchRelayContent: failed to build highlight ${event.id}`, error);
        return null;
      }
    })
    .filter((highlight): highlight is SearchHighlightResult => Boolean(highlight))
    .slice(0, highlightLimit);

  return {
    query: normalizedQuery,
    rooms,
    articles,
    profiles,
    highlights
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

function buildSearchProfileResult(event: NDKEvent): SearchProfileResult {
  const profile = profileFromEvent(event);
  const pubkey = event.pubkey;

  let npubBech32: string;
  try {
    npubBech32 = nip19.npubEncode(pubkey);
  } catch {
    npubBech32 = pubkey;
  }

  return {
    pubkey,
    npubBech32,
    displayName: displayName(profile, shortPubkey(pubkey)),
    nip05: displayNip05(profile),
    picture: avatarUrl(profile) ?? '',
    bio: truncate(cleanText(profile?.about), 160)
  };
}

function buildSearchHighlightResult(
  event: NDKEvent,
  profile: NDKUserProfile | undefined
): SearchHighlightResult {
  const pubkey = event.pubkey;

  let neventBech32: string;
  try {
    neventBech32 = event.encode();
  } catch {
    neventBech32 = event.id;
  }

  const sourceLabel = resolveHighlightSource(event);

  return {
    id: event.id,
    neventBech32,
    content: truncate(cleanText(event.content), 280),
    authorName: displayName(profile, shortPubkey(pubkey)),
    authorPubkey: pubkey,
    authorPicture: avatarUrl(profile) ?? '',
    sourceLabel,
    createdAt: event.created_at ?? 0
  };
}

function resolveHighlightSource(event: NDKEvent): string {
  const contextTag = event.tags.find((tag) => tag[0] === 'context');
  if (contextTag?.[1]) {
    return truncate(cleanText(contextTag[1]), 80);
  }

  const aTag = event.tags.find((tag) => tag[0] === 'a');
  if (aTag?.[1]) {
    const parts = aTag[1].split(':');
    const identifier = parts[2];
    if (identifier) {
      return `from ${cleanText(identifier)}`;
    }
  }

  return '';
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
