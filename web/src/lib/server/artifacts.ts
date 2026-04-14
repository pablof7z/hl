import type { NDKEvent, NDKUser, NDKUserProfile } from '@nostr-dev-kit/ndk';
import {
  HIGHLIGHTER_ARTIFACT_SHARE_KIND,
  artifactFromEvent,
  parseNostrAddress,
  type ArtifactRecord
} from '$lib/ndk/artifacts';
import { DEFAULT_RELAYS, GROUP_RELAY_URLS } from '$lib/ndk/config';
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
        kinds: [HIGHLIGHTER_ARTIFACT_SHARE_KIND],
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

export async function fetchNostrArticleForArtifact(
  artifact:
    | Pick<ArtifactRecord, 'referenceTagName' | 'referenceTagValue' | 'referenceKind'>
    | undefined
) : Promise<{
  event?: NDKEvent;
  author?: NDKUser;
  profile?: NDKUserProfile;
}> {
  if (!artifact || artifact.referenceTagName !== 'a' || artifact.referenceKind !== '30023') {
    return {};
  }

  const parsed = parseNostrAddress(artifact.referenceTagValue);
  if (!parsed || parsed.kind !== 30023) {
    return {};
  }

  const ndk = await getServerNdk(DEFAULT_RELAYS);
  const events = Array.from(
    (await ndk.fetchEvents(
      {
        kinds: [parsed.kind],
        authors: [parsed.pubkey],
        '#d': [parsed.identifier],
        limit: 10
      },
      { closeOnEose: true }
    )) ?? []
  ).sort((left, right) => (right.created_at ?? 0) - (left.created_at ?? 0));

  const event = events[0];
  if (!event) {
    return {};
  }

  const author = ndk.getUser({ pubkey: event.pubkey });
  const profile =
    author.profile ??
    (await author.fetchProfile({ closeOnEose: true }).catch(() => null)) ??
    undefined;

  return { event, author, profile };
}
