import { getRoom } from '$lib/server/rooms';
import { fetchEventsForSsr } from '$lib/server/nostr';
import { GROUP_RELAY_URLS } from '$lib/ndk/config';
import {
  ROOM_DISCUSSION_KIND,
  discussionFromEvent,
  isDiscussionThread,
  type RoomDiscussionRecord
} from '$lib/features/discussions/roomDiscussion';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ params }) => {
  const room = await getRoom(params.slug);
  if (!room) {
    return { room: null, discussion: null };
  }

  const discussion = await fetchDiscussionByIdOrSlug(room.id, params.id);
  return { room, discussion };
};

async function fetchDiscussionByIdOrSlug(
  groupId: string,
  idOrSlug: string
): Promise<RoomDiscussionRecord | null> {
  const trimmed = idOrSlug.trim();
  if (!trimmed) return null;

  const isHexEventId = /^[0-9a-f]{64}$/i.test(trimmed);

  const events = Array.from(
    (await fetchEventsForSsr(
      isHexEventId
        ? {
            kinds: [ROOM_DISCUSSION_KIND],
            ids: [trimmed],
            limit: 1
          }
        : {
            kinds: [ROOM_DISCUSSION_KIND],
            '#h': [groupId],
            '#d': [trimmed],
            '#t': ['discussion'],
            limit: 5
          },
      `fetchDiscussionByIdOrSlug(${groupId},${trimmed})`,
      { relays: GROUP_RELAY_URLS }
    )) ?? []
  )
    .filter((event) => isDiscussionThread(event))
    .sort((a, b) => (b.created_at ?? 0) - (a.created_at ?? 0));

  const event = events[0];
  if (!event) return null;

  const record = discussionFromEvent(event);
  if (record.groupId && record.groupId !== groupId) return null;
  return record;
}
