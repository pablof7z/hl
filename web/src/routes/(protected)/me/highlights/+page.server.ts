import type { PageServerLoad } from './$types';
import { fetchCommunities } from '$lib/server/communities';

export const load: PageServerLoad = async () => {
  return {
    communities: await fetchCommunities()
  };
};
