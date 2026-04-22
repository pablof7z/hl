import type { PageServerLoad } from './$types';
import { fetchCommunities } from '$lib/server/communities';

export const load: PageServerLoad = async ({ setHeaders }) => {
  setHeaders({
    'cache-control': 'public, max-age=30, s-maxage=120, stale-while-revalidate=600'
  });

  return {
    rooms: await fetchCommunities({ limit: 64, visibility: 'public' })
  };
};
