import { getRoom } from '$lib/server/rooms';
import {
  fetchArtifactRecordByEventId,
  fetchPodcastExperienceForArtifact
} from '$lib/server/artifacts';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ params }) => {
  const room = await getRoom(params.slug);
  if (!room) {
    return { room: null, artifact: null, podcast: undefined };
  }

  const isHexEventId = /^[0-9a-f]{64}$/i.test(params.id);
  if (!isHexEventId) {
    return { room, artifact: null, podcast: undefined };
  }

  const artifact = await fetchArtifactRecordByEventId(room.id, params.id);
  if (!artifact) {
    return { room, artifact: null, podcast: undefined };
  }

  const podcast =
    artifact.source === 'podcast'
      ? await fetchPodcastExperienceForArtifact(artifact).catch((error) => {
          console.warn('Podcast SSR load failed', { groupId: room.id, eventId: params.id, error });
          return undefined;
        })
      : undefined;

  return { room, artifact, podcast };
};
