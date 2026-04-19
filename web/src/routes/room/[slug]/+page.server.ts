import { getRoom } from '$lib/features/room/api/room';
import { error } from '@sveltejs/kit';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ params }) => {
  const room = await getRoom(params.slug);
  if (!room) {
    throw error(404, 'Room not found');
  }
  return { room };
};
