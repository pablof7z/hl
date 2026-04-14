import { json } from '@sveltejs/kit';
import type { RequestHandler } from './$types';
import { searchRelayContent } from '$lib/server/search';
import { MAX_SEARCH_SECTION_LIMIT, MIN_SEARCH_QUERY_LENGTH } from '$lib/search';

export const GET: RequestHandler = async ({ setHeaders, url }) => {
  const query = url.searchParams.get('q') ?? '';
  const rawLimit = Number.parseInt(url.searchParams.get('limit') ?? '', 10);
  const limit = Number.isFinite(rawLimit)
    ? Math.min(MAX_SEARCH_SECTION_LIMIT, Math.max(MIN_SEARCH_QUERY_LENGTH, rawLimit))
    : undefined;

  setHeaders({
    'cache-control': 'public, max-age=15, s-maxage=30, stale-while-revalidate=120'
  });

  return json(
    await searchRelayContent(query, {
      communityLimit: limit,
      articleLimit: limit
    })
  );
};
