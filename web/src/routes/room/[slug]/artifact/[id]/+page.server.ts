import { NDKKind } from '@nostr-dev-kit/ndk';
import { fetchEventsForSsr } from '$lib/server/nostr';
import { GROUP_RELAY_URLS } from '$lib/ndk/config';
import { getRoom, type Artifact } from '$lib/features/room/api/room';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ params }) => {
  const room = await getRoom(params.slug);
  if (!room) {
    return { room: null, artifact: null };
  }

  // First try: look up the artifact in the room's already-loaded shelf
  let artifact: Artifact | undefined = room.artifacts.find((a) => a.id === params.id);

  // Fallback: fetch the specific kind:11 thread by id — only if id looks like a real event id
  const isHexEventId = /^[0-9a-f]{64}$/i.test(params.id);
  if (!artifact && isHexEventId) {
    const events = await fetchEventsForSsr(
      { kinds: [NDKKind.Thread], ids: [params.id] },
      `artifact:${params.id}`,
      { relays: GROUP_RELAY_URLS }
    );
    const event = [...(events ?? [])][0];
    if (event) {
      const title = event.tagValue('title') || event.tagValue('name') || 'Untitled';
      const author = event.tagValue('author') || event.tagValue('summary') || '';
      const typeRaw = event.tagValue('type') || '';
      const type: Artifact['type'] =
        typeRaw === 'book' ||
        typeRaw === 'podcast' ||
        typeRaw === 'article' ||
        typeRaw === 'essay' ||
        typeRaw === 'video'
          ? (typeRaw as Artifact['type'])
          : 'article';

      artifact = {
        id: event.id,
        type,
        title,
        author,
        cover: event.tagValue('image') || event.tagValue('picture') || '',
        url: event.tagValue('r') || event.tagValue('url') || '',
        progress: 0,
        highlightCount: 0,
        discussionCount: 0
      };
    }
  }

  return { room, artifact: artifact ?? null };
};
