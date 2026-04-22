import { getRoom } from '$lib/server/rooms';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ params, url }) => {
  const room = await getRoom(params.slug);
  return {
    room,
    fresh: url.searchParams.get('fresh') === '1'
  };
};
