import type { CommunitySummary } from '$lib/ndk/groups';

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

export type SearchResponse = {
  query: string;
  rooms: CommunitySummary[];
  articles: SearchArticleResult[];
};
