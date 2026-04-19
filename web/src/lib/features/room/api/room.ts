import { NDKKind, NDKSimpleGroupMetadata, profileFromEvent } from '@nostr-dev-kit/ndk';
import type { NDKEvent } from '@nostr-dev-kit/ndk';
import { fetchEventsForSsr } from '$lib/server/nostr';
import { GROUP_RELAY_URLS } from '$lib/ndk/config';
import { displayName, shortPubkey } from '$lib/ndk/format';

export interface RoomMember {
  pubkey: string;
  name: string;
  colorIndex: number; // renamed from color — matches component expectations
  joinedAt: string;
}

export interface Artifact {
  id: string;
  type: 'book' | 'podcast' | 'article' | 'essay' | 'video';
  title: string;
  author: string;
  cover: string; // renamed from coverUrl — matches component expectations
  url: string;
  progress: number; // 0-100
  highlightCount: number;
  discussionCount: number; // added — displayed by ArtifactCard
}

export interface Highlight {
  id: string;
  artifactId: string;
  quote: string;          // renamed from text
  memberName: string;     // renamed from author
  memberColorIndex: number;
  timestamp: string;      // renamed from createdAt
}

export interface Note {
  id: string;
  memberColorIndex: number;
  memberName: string;
  content: string;
  timestamp: string;
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

// kind:999 is the made-up pin/vote kind used by Highlighter (see decision D-01 in room-ui.md)
const KIND_PIN = 999;

export async function getRoom(slug: string): Promise<Room | null> {
  const groupId = slug.trim();
  if (!groupId) return null;

  // Fetch all room data in parallel for fast SSR
  const [metadataEvents, memberEvents, artifactEvents, highlightEvents] = await Promise.all([
    // kind:39000 — group metadata (name, about, picture)
    fetchEventsForSsr(
      { kinds: [NDKKind.GroupMetadata], '#d': [groupId] },
      `getRoom:metadata(${groupId})`,
      { relays: GROUP_RELAY_URLS }
    ),
    // kind:39002 — group member list
    fetchEventsForSsr(
      { kinds: [NDKKind.GroupMembers], '#d': [groupId] },
      `getRoom:members(${groupId})`,
      { relays: GROUP_RELAY_URLS }
    ),
    // kind:11 — thread shares (each thread = an artifact shared to the room)
    fetchEventsForSsr(
      { kinds: [NDKKind.Thread], '#h': [groupId], limit: 32 },
      `getRoom:artifacts(${groupId})`,
      { relays: GROUP_RELAY_URLS }
    ),
    // kind:9802 — highlights posted to this room
    fetchEventsForSsr(
      { kinds: [NDKKind.Highlight], '#h': [groupId], limit: 64 },
      `getRoom:highlights(${groupId})`,
      { relays: GROUP_RELAY_URLS }
    )
  ]);

  // ── Room metadata ─────────────────────────────────────────────────────────
  const metadataEvent = sortByCreatedAtDesc([...(metadataEvents ?? [])])[0];
  if (!metadataEvent) return null;

  const metadata = NDKSimpleGroupMetadata.from(metadataEvent);
  const roomName = metadata.name?.trim() || groupId;

  // ── Member list ───────────────────────────────────────────────────────────
  const memberEvent = sortByCreatedAtDesc([...(memberEvents ?? [])])[0];
  const memberPubkeys = memberEvent
    ? memberEvent.getMatchingTags('p').map((tag) => tag[1]).filter(Boolean)
    : [];

  // Fetch kind:0 profiles for up to 30 members
  const profiles = await fetchMemberProfiles(memberPubkeys.slice(0, 30));

  const members: RoomMember[] = memberPubkeys.map((pubkey, index) => ({
    pubkey,
    name: displayName(profiles[pubkey], shortPubkey(pubkey)),
    colorIndex: (index % 6) + 1,
    joinedAt: ''
  }));

  // ── Artifacts (kind:11 threads) ───────────────────────────────────────────
  const sortedArtifactEvents = sortByCreatedAtDesc([...(artifactEvents ?? [])]);
  const artifacts: Artifact[] = sortedArtifactEvents.map((event) =>
    artifactFromThreadEvent(event)
  );

  // ── Highlights (kind:9802) ────────────────────────────────────────────────
  const sortedHighlightEvents = sortByCreatedAtDesc([...(highlightEvents ?? [])]);
  const highlights: Highlight[] = sortedHighlightEvents.slice(0, 30).map((event) => {
    const memberIndex = memberPubkeys.indexOf(event.pubkey);
    const memberColorIndex = memberIndex >= 0 ? (memberIndex % 6) + 1 : 1;
    const profile = profiles[event.pubkey];
    const memberName = displayName(profile, shortPubkey(event.pubkey));

    return {
      id: event.id,
      artifactId: event.tagValue('a') || event.tagValue('e') || '',
      quote: event.content.trim(),
      memberName,
      memberColorIndex,
      timestamp: relativeTime(event.created_at)
    };
  });

  // ── Pinned artifact (kind:999) or fallback to most recent kind:11 ─────────
  let pinnedArtifact: Artifact | undefined;

  const pinnedEvents = await fetchEventsForSsr(
    { kinds: [KIND_PIN], '#h': [groupId], limit: 10 },
    `getRoom:pinned(${groupId})`,
    { relays: GROUP_RELAY_URLS }
  );
  const latestPin = sortByCreatedAtDesc([...(pinnedEvents ?? [])])[0];

  if (latestPin) {
    const pinnedThreadId = latestPin.tagValue('e');
    const pinnedFromShelf = pinnedThreadId
      ? artifacts.find((a) => a.id === pinnedThreadId)
      : undefined;
    pinnedArtifact = pinnedFromShelf ?? artifacts[0];
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

function artifactFromThreadEvent(event: NDKEvent): Artifact {
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

async function fetchMemberProfiles(
  pubkeys: string[]
): Promise<Record<string, import('@nostr-dev-kit/ndk').NDKUserProfile>> {
  if (pubkeys.length === 0) return {};

  const profileEvents = [
    ...(
      (await fetchEventsForSsr(
        { kinds: [0], authors: pubkeys },
        `getRoom:profiles(${pubkeys.length})`
      )) ?? []
    )
  ];

  const latestByPubkey = new Map<string, NDKEvent>();
  for (const event of profileEvents) {
    const existing = latestByPubkey.get(event.pubkey);
    if (!existing || (event.created_at ?? 0) > (existing.created_at ?? 0)) {
      latestByPubkey.set(event.pubkey, event);
    }
  }

  const result: Record<string, import('@nostr-dev-kit/ndk').NDKUserProfile> = {};
  for (const [pubkey, event] of latestByPubkey) {
    try {
      result[pubkey] = profileFromEvent(event);
    } catch {
      // skip malformed profiles
    }
  }
  return result;
}

function relativeTime(createdAt: number | undefined): string {
  if (!createdAt) return '';
  const diffMs = Date.now() - createdAt * 1000;
  const diffDays = Math.floor(diffMs / 86_400_000);

  if (diffDays === 0) return 'today';
  if (diffDays === 1) return '1d ago';
  if (diffDays < 7) return `${diffDays}d ago`;
  if (diffDays < 30) return `${Math.floor(diffDays / 7)}w ago`;
  return `${Math.floor(diffDays / 30)}mo ago`;
}
