import { NDKKind, NDKSimpleGroupMetadata } from '@nostr-dev-kit/ndk';
import type { NDKEvent, NDKKind as NDKKindType } from '@nostr-dev-kit/ndk';
import { fetchEventsForSsr } from '$lib/server/nostr';
import { GROUP_RELAY_URLS } from '$lib/ndk/config';
import { relativeTime } from '$lib/utils/time';

export interface RoomMember {
  pubkey: string;
  colorIndex: number; // positional color (1..6), based on member list order
  joinedAt: string;
}

export interface Artifact {
  id: string;
  type: 'book' | 'podcast' | 'article' | 'essay' | 'video';
  title: string;
  author: string;
  cover: string;
  url: string;
  progress: number; // 0-100
  highlightCount: number;
  discussionCount: number;
}

export interface Highlight {
  id: string;
  artifactId: string;
  quote: string;
  authorPubkey: string;
  authorColorIndex: number;   // positional, from room member list order
  createdAt: number;          // raw unix seconds; components format for display
}

export interface Note {
  id: string;
  authorPubkey: string;
  authorColorIndex: number;
  content: string;
  createdAt: number;
}

export interface UpNextItem {
  id: string;
  title: string;
  type: 'book' | 'podcast' | 'article';
  voterCount: number;
  voterColors: number[];
}

export interface Room {
  id: string;
  name: string;
  members: RoomMember[];
  pinnedArtifact?: Artifact;
  artifacts: Artifact[];
  highlights: Highlight[];
  upNext: UpNextItem[];
  notes: Note[];
}

// kind:999 — made-up pin/vote kind (see decision D-01 in room-ui.md)
export const KIND_PIN = 999 as NDKKindType;

export async function getRoom(slug: string): Promise<Room | null> {
  const groupId = slug.trim();
  if (!groupId) return null;

  const [metadataEvents, memberEvents, artifactEvents, highlightEvents] = await Promise.all([
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
    pinnedArtifact,
    artifacts,
    highlights,
    upNext: [],
    notes: []
  };
}

// ── Helpers ───────────────────────────────────────────────────────────────────

function sortByCreatedAtDesc(events: NDKEvent[]): NDKEvent[] {
  return events.sort((a, b) => (b.created_at ?? 0) - (a.created_at ?? 0));
}

const ARTIFACT_TYPE_TAG_VALUES: Artifact['type'][] = [
  'book',
  'podcast',
  'article',
  'essay',
  'video'
];

export function artifactFromThreadEvent(event: NDKEvent): Artifact {
  const title = event.tagValue('title') || event.tagValue('name') || 'Untitled';
  const author = event.tagValue('author') || event.tagValue('summary') || '';
  const url = event.tagValue('r') || event.tagValue('url') || '';
  const typeRaw = event.tagValue('type') || '';
  const type: Artifact['type'] = ARTIFACT_TYPE_TAG_VALUES.includes(typeRaw as Artifact['type'])
    ? (typeRaw as Artifact['type'])
    : 'article';

  return {
    id: event.id,
    type,
    title,
    author,
    cover: event.tagValue('image') || event.tagValue('picture') || '',
    url,
    progress: 0,
    highlightCount: 0,
    discussionCount: 0
  };
}

// Re-export for any legacy importers
export { relativeTime };
