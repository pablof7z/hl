import type { PageServerLoad } from './$types';
import { fetchRooms, fetchFeaturedRooms } from '$lib/server/rooms';

export const load: PageServerLoad = async ({ setHeaders }) => {
  setHeaders({
    'cache-control': 'public, max-age=30, s-maxage=120, stale-while-revalidate=600'
  });

  const [featured, allRooms] = await Promise.all([
    fetchFeaturedRooms(),
    fetchRooms({ limit: 64, visibility: 'public' })
  ]);

  return { featured, allRooms };
};
