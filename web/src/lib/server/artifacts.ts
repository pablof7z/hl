import type { NDKEvent, NDKUser, NDKUserProfile } from '@nostr-dev-kit/ndk';
import {
  HIGHLIGHTER_ARTIFACT_SHARE_KIND,
  artifactFromEvent,
  parseNostrAddress,
  type ArtifactRecord
} from '$lib/ndk/artifacts';
import type { NDKFilter } from '@nostr-dev-kit/ndk';
import type { PodcastArtifactData } from '$lib/features/podcasts/types';
import { DEFAULT_RELAYS, GROUP_RELAY_URLS } from '$lib/ndk/config';
import { fetchPodcastExperienceForArtifact as fetchPodcastData } from '$lib/server/podcasts';
import { fetchEventsForSsr, getServerNdk } from '$lib/server/nostr';

export async function fetchArtifactForGroup(
  groupId: string,
  artifactId: string
): Promise<ArtifactRecord | undefined> {
  const trimmedGroupId = groupId.trim();
  const trimmedArtifactId = artifactId.trim();
  if (!trimmedGroupId || !trimmedArtifactId) return undefined;

  const events = Array.from(
    (await fetchEventsForSsr(
      {
        kinds: [HIGHLIGHTER_ARTIFACT_SHARE_KIND],
        '#h': [trimmedGroupId],
        '#d': [trimmedArtifactId],
        limit: 10
      },
      `fetchArtifactForGroup(${trimmedGroupId},${trimmedArtifactId})`,
      { relays: GROUP_RELAY_URLS }
    )) ?? []
  )
    .sort((left, right) => (right.created_at ?? 0) - (left.created_at ?? 0))
    .slice(0, 10);

  const event = events[0];
  return event ? artifactFromEvent(event) : undefined;
}

export async function fetchArtifactRecordByEventId(
  groupId: string,
  eventId: string
): Promise<ArtifactRecord | undefined> {
  const trimmedGroupId = groupId.trim();
  const trimmedEventId = eventId.trim();
  if (!trimmedGroupId || !trimmedEventId) return undefined;

  const filter: NDKFilter = {
    kinds: [HIGHLIGHTER_ARTIFACT_SHARE_KIND],
    ids: [trimmedEventId],
    limit: 1
  };

  const events = Array.from(
    (await fetchEventsForSsr(
      filter,
      `fetchArtifactRecordByEventId(${trimmedGroupId},${trimmedEventId})`,
      { relays: GROUP_RELAY_URLS }
    )) ?? []
  );

  const event = events[0];
  if (!event) return undefined;

  const record = artifactFromEvent(event);
  if (record.groupId && record.groupId !== trimmedGroupId) return undefined;
  return record;
}

export async function fetchNostrArticleForArtifact(
  artifact:
    | Pick<ArtifactRecord, 'referenceTagName' | 'referenceTagValue' | 'referenceKind'>
    | undefined
): Promise<{
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

  const ndk = await getServerNdk(DEFAULT_RELAYS, { connect: false });
  const events = Array.from(
    (await fetchEventsForSsr(
      {
        kinds: [parsed.kind],
        authors: [parsed.pubkey],
        '#d': [parsed.identifier],
        limit: 10
      },
      `fetchNostrArticleForArtifact(${artifact.referenceTagValue})`,
      { relays: DEFAULT_RELAYS }
    )) ?? []
  )
    .sort((left, right) => (right.created_at ?? 0) - (left.created_at ?? 0))
    .slice(0, 10);

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

export async function fetchPodcastExperienceForArtifact(
  artifact:
    | Pick<
        ArtifactRecord,
        | 'source'
        | 'url'
        | 'title'
        | 'description'
        | 'image'
        | 'publishedAt'
        | 'durationSeconds'
        | 'audioUrl'
        | 'audioPreviewUrl'
        | 'transcriptUrl'
        | 'feedUrl'
        | 'podcastGuid'
        | 'podcastShowTitle'
        | 'catalogId'
        | 'catalogKind'
        | 'domain'
      >
    | undefined
): Promise<PodcastArtifactData | undefined> {
  if (!artifact || artifact.source !== 'podcast') {
    return undefined;
  }

  return fetchPodcastData(artifact);
}
