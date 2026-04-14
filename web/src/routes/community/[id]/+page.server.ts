import type { PageServerLoad } from './$types';
import { fetchCommunityById } from '$lib/server/communities';

export const load: PageServerLoad = async ({ params, setHeaders }) => {
  setHeaders({
    'cache-control': 'public, max-age=30, s-maxage=120, stale-while-revalidate=600'
  });

  const community = await fetchCommunityById(params.id);

  return {
    community,
    groupId: params.id,
    missing: !community
  };
};
