import { HIGHLIGHTER_ARTIFACT_KIND, artifactFromEvent, type ArtifactRecord } from '$lib/ndk/artifacts';
import { GROUP_RELAY_URLS } from '$lib/ndk/config';
import { getServerNdk } from '$lib/server/nostr';

export async function fetchArtifactForGroup(
  groupId: string,
  artifactId: string
): Promise<ArtifactRecord | undefined> {
  const trimmedGroupId = groupId.trim();
  const trimmedArtifactId = artifactId.trim();
  if (!trimmedGroupId || !trimmedArtifactId) return undefined;

  const ndk = await getServerNdk(GROUP_RELAY_URLS);
  const events = Array.from(
    (await ndk.fetchEvents(
      {
        kinds: [HIGHLIGHTER_ARTIFACT_KIND],
        '#h': [trimmedGroupId],
        '#d': [trimmedArtifactId],
        limit: 10
      },
      { closeOnEose: true }
    )) ?? []
  ).sort((left, right) => (right.created_at ?? 0) - (left.created_at ?? 0));

  const event = events[0];
  return event ? artifactFromEvent(event) : undefined;
}
