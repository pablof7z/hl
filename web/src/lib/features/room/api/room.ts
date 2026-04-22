import { NDKKind, NDKSimpleGroupMetadata } from '@nostr-dev-kit/ndk';
import { fetchEventsForSsr } from '$lib/server/nostr';
import { GROUP_RELAY_URLS } from '$lib/ndk/config';
import { relativeTime } from '$lib/utils/time';
import {
  KIND_PIN,
  artifactFromThreadEvent,
  sortByCreatedAtDesc,
  type Artifact,
  type Highlight,
  type Room,
  type RoomMember
} from './types';

export {
  KIND_PIN,
  artifactFromThreadEvent,
  sortByCreatedAtDesc,
  type Artifact,
  type Highlight,
  type Note,
  type Room,
  type RoomMember,
  type UpNextItem
} from './types';

export async function getRoom(slug: string): Promise<Room | null> {
  const groupId = slug.trim();
  if (!groupId) return null;

  const [metadataEvents, memberEvents, adminEvents, artifactEvents, highlightEvents] = await Promise.all([
    fetchEventsForSsr(
      { kinds: [NDKKind.GroupMetadata], '#d': [groupId] },
      `getRoom:metadata(${groupId})`,
      { relays: GROUP_RELAY_URLS }
    ),
    fetchEventsForSsr(
      { kinds: [NDKKind.GroupMembers], '#d': [groupId] },
      `getRoom:members(${groupId})`,
      { relays: GROUP_RELAY_URLS }
    ),
    fetchEventsForSsr(
      { kinds: [NDKKind.GroupAdmins], '#d': [groupId] },
      `getRoom:admins(${groupId})`,
      { relays: GROUP_RELAY_URLS }
    ),
    fetchEventsForSsr(
      { kinds: [NDKKind.Thread], '#h': [groupId], limit: 32 },
      `getRoom:artifacts(${groupId})`,
      { relays: GROUP_RELAY_URLS }
    ),
    fetchEventsForSsr(
      { kinds: [NDKKind.Highlight], '#h': [groupId], limit: 64 },
      `getRoom:highlights(${groupId})`,
      { relays: GROUP_RELAY_URLS }
    )
  ]);

  const metadataEvent = sortByCreatedAtDesc([...(metadataEvents ?? [])])[0];
  if (!metadataEvent) return null;

  const metadata = NDKSimpleGroupMetadata.from(metadataEvent);
  const roomName = metadata.name?.trim() || groupId;

  const memberEvent = sortByCreatedAtDesc([...(memberEvents ?? [])])[0];
  const memberPubkeys = memberEvent
    ? memberEvent.getMatchingTags('p').map((tag) => tag[1]).filter(Boolean)
    : [];

  const adminEvent = sortByCreatedAtDesc([...(adminEvents ?? [])])[0];
  const adminPubkeys = adminEvent
    ? adminEvent.getMatchingTags('p').map((tag) => tag[1]).filter(Boolean)
    : [];

  const members: RoomMember[] = memberPubkeys.map((pubkey, index) => ({
    pubkey,
    colorIndex: (index % 6) + 1,
    joinedAt: ''
  }));

  const colorByPubkey = new Map(members.map((m) => [m.pubkey, m.colorIndex]));

  const sortedArtifactEvents = sortByCreatedAtDesc([...(artifactEvents ?? [])]);
  const artifacts: Artifact[] = sortedArtifactEvents.map(artifactFromThreadEvent);

  const sortedHighlightEvents = sortByCreatedAtDesc([...(highlightEvents ?? [])]);
  const highlights: Highlight[] = sortedHighlightEvents.slice(0, 30).map((event) => ({
    id: event.id,
    artifactId: event.tagValue('a') || event.tagValue('e') || '',
    quote: event.content.trim(),
    authorPubkey: event.pubkey,
    authorColorIndex: colorByPubkey.get(event.pubkey) ?? 1,
    createdAt: event.created_at ?? 0
  }));

  // Pinned artifact: latest kind:999 for the group, fallback to most recent kind:11
  let pinnedArtifact: Artifact | undefined;
  const pinnedEvents = await fetchEventsForSsr(
    { kinds: [KIND_PIN], '#h': [groupId], limit: 10 },
    `getRoom:pinned(${groupId})`,
    { relays: GROUP_RELAY_URLS }
  );
  const latestPin = sortByCreatedAtDesc([...(pinnedEvents ?? [])])[0];
  if (latestPin) {
    const pinnedThreadId = latestPin.tagValue('e');
    pinnedArtifact = (pinnedThreadId && artifacts.find((a) => a.id === pinnedThreadId)) || artifacts[0];
  } else {
    pinnedArtifact = artifacts[0];
  }

  return {
    id: groupId,
    name: roomName,
    members,
    adminPubkeys,
    pinnedArtifact,
    artifacts,
    highlights,
    upNext: [],
    notes: []
  };
}

// Re-export for any legacy importers
export { relativeTime };
