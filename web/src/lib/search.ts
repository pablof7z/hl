import type { RoomSummary } from '$lib/ndk/groups';

export const MIN_SEARCH_QUERY_LENGTH = 2;
export const DEFAULT_SEARCH_SECTION_LIMIT = 6;
export const MAX_SEARCH_SECTION_LIMIT = 12;

export type SearchArticleResult = {
  id: string;
  noteIdentifier: string;
  title: string;
  summary: string;
  image: string;
  authorName: string;
  authorIdentifier: string;
  authorPubkey: string;
  publishedLabel: string;
};

export type SearchProfileResult = {
  pubkey: string;
  npubBech32: string;
  displayName: string;
  nip05: string;
  picture: string;
  bio: string;
};

export type SearchHighlightResult = {
  id: string;
  neventBech32: string;
  content: string;
  authorName: string;
  authorPubkey: string;
  authorPicture: string;
  sourceLabel: string;
  createdAt: number;
};

export type SearchResponse = {
  query: string;
  rooms: RoomSummary[];
  articles: SearchArticleResult[];
  profiles: SearchProfileResult[];
  highlights: SearchHighlightResult[];
};
