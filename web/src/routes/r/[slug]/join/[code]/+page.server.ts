import { getRoom } from '$lib/server/rooms';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ params }) => {
  const room = await getRoom(params.slug);
  return {
    room,
    code: params.code
  };
};
