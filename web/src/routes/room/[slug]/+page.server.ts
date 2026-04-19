import { getRoom } from '$lib/features/room/api/room';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ params }) => {
  const room = await getRoom(params.slug);
  // Return room even when null — component uses seed fallback while API stub is in place.
  // Only throw 404 once real API can distinguish "not connected" from "room doesn't exist".
  return { room };
};
