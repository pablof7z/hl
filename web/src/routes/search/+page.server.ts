import type { PageServerLoad } from './$types';
import { searchRelayContent } from '$lib/server/search';

export const load: PageServerLoad = async ({ setHeaders, url }) => {
  setHeaders({
    'cache-control': 'public, max-age=15, s-maxage=30, stale-while-revalidate=120'
  });

  return {
    results: await searchRelayContent(url.searchParams.get('q') ?? '', {
      roomLimit: 12,
      articleLimit: 12,
      profileLimit: 12,
      highlightLimit: 12
    })
  };
};
