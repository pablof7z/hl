import { NDKKind, NDKSimpleGroupMetadata, type NDKEvent } from '@nostr-dev-kit/ndk';
import { GROUP_RELAY_URLS, HIGHLIGHTER_RELAY_URL } from '$lib/ndk/config';
import {
  buildRoomSummary,
  type RoomSummary,
  type RoomVisibility
} from '$lib/ndk/groups';
import { groupRefsFromEvent } from '$lib/ndk/lists';
import { fetchRelayCuratorPubkey } from '$lib/ndk/relay-probe';
import { fetchEventsForSsr } from '$lib/server/nostr';
import type { Room, RoomMember } from '$lib/features/room/api/room';

type FetchRoomsOptions = {
  limit?: number;
  visibility?: RoomVisibility | 'all';
};

/**
 * Fetch the curator's featured room list (kind:10009 authored by the relay
 * operator pubkey found via NIP-11). Returns rooms in curator order; unresolvable
 * group refs are dropped silently.
 */
export async function fetchFeaturedRooms(): Promise<RoomSummary[]> {
  const curatorPubkey = await fetchRelayCuratorPubkey(HIGHLIGHTER_RELAY_URL);
  if (!curatorPubkey) return [];

  const listEvents = Array.from(
    (await fetchEventsForSsr(
      { kinds: [10009], authors: [curatorPubkey], limit: 5 },
      `fetchFeaturedRooms(${curatorPubkey})`,
      { relays: GROUP_RELAY_URLS }
    )) ?? []
  ).sort((a, b) => (b.created_at ?? 0) - (a.created_at ?? 0));

  const latestList = listEvents[0];
  if (!latestList) return [];

  const refs = groupRefsFromEvent(latestList);
  if (refs.length === 0) return [];

  const groupIds = refs.map((ref) => ref.groupId);

  const metadataEvents = Array.from(
    (await fetchEventsForSsr(
      { kinds: [NDKKind.GroupMetadata], '#d': groupIds, limit: groupIds.length * 2 },
      `fetchFeaturedRooms:metadata(${groupIds.length})`,
      { relays: GROUP_RELAY_URLS }
    )) ?? []
  );

  const summaries = await buildRoomSummariesFromMetadataEvents(metadataEvents);
  const byId = new Map(summaries.map((s) => [s.id, s]));

  // Preserve curator order, drop unresolvable
  return groupIds.flatMap((id) => {
    const s = byId.get(id);
    return s ? [s] : [];
  });
}

export async function fetchRooms(
  options: number | FetchRoomsOptions = 32
): Promise<RoomSummary[]> {
  const {
    limit,
    visibility
  } = typeof options === 'number' ? { limit: options, visibility: 'all' as const } : {
    limit: options.limit ?? 32,
    visibility: options.visibility ?? 'all'
  };
  const fetchLimit = visibility === 'all' ? limit : Math.max(limit * 4, 96);
  const metadataEvents = Array.from(
    (await fetchEventsForSsr(
      {
        kinds: [NDKKind.GroupMetadata],
        limit: fetchLimit
      },
      `fetchRooms(${fetchLimit})`,
      { relays: GROUP_RELAY_URLS }
    )) ?? []
  )
    .sort((left, right) => (right.created_at ?? 0) - (left.created_at ?? 0))
    .slice(0, fetchLimit);

  return (await buildRoomSummariesFromMetadataEvents(metadataEvents))
    .filter((room): room is RoomSummary => Boolean(room))
    .filter((room) => visibility === 'all' || room.visibility === visibility)
    .slice(0, limit);
}

export async function buildRoomSummariesFromMetadataEvents(
  metadataEvents: NDKEvent[]
): Promise<RoomSummary[]> {
  if (metadataEvents.length === 0) {
    return [];
  }

  const groupIds = uniqueValues(metadataEvents.map((event) => event.tagValue('d') ?? '').filter(Boolean));
  const [adminEvents, memberEvents] = await Promise.all([
    fetchReplaceableGroupEvents(NDKKind.GroupAdmins, groupIds),
    fetchReplaceableGroupEvents(NDKKind.GroupMembers, groupIds)
  ]);

  return metadataEvents
    .map((event) => {
      const id = event.tagValue('d') ?? '';
      if (!id) return null;

      try {
        return buildRoomSummary(event, {
          adminEvent: adminEvents.get(id),
          memberEvent: memberEvents.get(id)
        });
      } catch {
        return null;
      }
    })
    .filter((room): room is RoomSummary => Boolean(room));
}

async function fetchReplaceableGroupEvents(
  kind: NDKKind.GroupAdmins | NDKKind.GroupMembers,
  groupIds: string[]
): Promise<Map<string, NDKEvent>> {
  if (groupIds.length === 0) {
    return new Map();
  }

  const events = Array.from(
    (await fetchEventsForSsr(
      {
        kinds: [kind],
        '#d': groupIds,
        limit: Math.max(groupIds.length * 2, 32)
      },
      `fetchReplaceableGroupEvents(${kind},${groupIds.length})`,
      { relays: GROUP_RELAY_URLS }
    )) ?? []
  );

  return latestEventsByGroupId(events);
}

function latestEventsByGroupId(events: NDKEvent[]): Map<string, NDKEvent> {
  const latest = new Map<string, NDKEvent>();

  for (const event of events) {
    const groupId = event.tagValue('d');
    if (!groupId) continue;

    const existing = latest.get(groupId);
    if (!existing || (event.created_at ?? 0) > (existing.created_at ?? 0)) {
      latest.set(groupId, event);
    }
  }

  return latest;
}

function uniqueValues(values: string[]): string[] {
  return [...new Set(values)];
}

/**
 * Fetch a NIP-29 room by its group identifier (slug).
 *
 * SSR phase only resolves kind:39000 (group metadata) and kind:39002
 * (member list). Heavier data — pinned artifact, highlights, activity feed —
 * is deferred to client-side NDK subscriptions per spec §8.1.
 */
export async function getRoom(slug: string): Promise<Room | null> {
  const trimmedSlug = slug.trim();
  if (!trimmedSlug) return null;

  // 1. Fetch kind:39000 group metadata — group identifier matches the slug via `d` tag
  const metadataEvents = Array.from(
    (await fetchEventsForSsr(
      {
        kinds: [NDKKind.GroupMetadata],
        '#d': [trimmedSlug]
      },
      `getRoom(${trimmedSlug})`,
      { relays: GROUP_RELAY_URLS }
    )) ?? []
  ).sort((a, b) => (b.created_at ?? 0) - (a.created_at ?? 0));

  const metadataEvent = metadataEvents[0];
  if (!metadataEvent) return null;

  const metadata = NDKSimpleGroupMetadata.from(metadataEvent);

  // 2. Fetch kind:39002 member list and kind:39001 admin list in parallel
  const [rawMemberEvents, rawAdminEvents] = await Promise.all([
    fetchEventsForSsr(
      { kinds: [NDKKind.GroupMembers], '#d': [trimmedSlug] },
      `getRoom:members(${trimmedSlug})`,
      { relays: GROUP_RELAY_URLS }
    ),
    fetchEventsForSsr(
      { kinds: [NDKKind.GroupAdmins], '#d': [trimmedSlug] },
      `getRoom:admins(${trimmedSlug})`,
      { relays: GROUP_RELAY_URLS }
    )
  ]);
  const memberEvents = Array.from(rawMemberEvents ?? []).sort((a, b) => (b.created_at ?? 0) - (a.created_at ?? 0));
  const adminEvents = Array.from(rawAdminEvents ?? []).sort((a, b) => (b.created_at ?? 0) - (a.created_at ?? 0));

  const memberEvent = memberEvents[0];
  const members: RoomMember[] = memberEvent ? extractMembers(memberEvent) : [];

  const adminEvent = adminEvents[0];
  const adminPubkeys: string[] = adminEvent
    ? adminEvent.getMatchingTags('p').map((tag) => tag[1]).filter(Boolean)
    : [];

  const visibility: 'public' | 'private' = metadataEvent.tags.some((t) => t[0] === 'private')
    ? 'private'
    : 'public';
  const access: 'open' | 'closed' =
    visibility === 'private' || metadataEvent.tags.some((t) => t[0] === 'closed')
      ? 'closed'
      : 'open';

  return {
    id: trimmedSlug,
    name: metadata.name || trimmedSlug,
    about: (metadata.about ?? '').trim() || undefined,
    picture: (metadata.picture ?? '').trim() || undefined,
    access,
    visibility,
    members,
    adminPubkeys,
    // Heavier collections are empty at SSR time; client subscriptions hydrate them
    artifacts: [],
    highlights: [],
    upNext: [],
    notes: []
  };
}

/**
 * Extract RoomMember entries from a kind:39002 event's `p` tags.
 * Display names and avatars are hydrated client-side from kind:0 profiles.
 */
function extractMembers(memberEvent: NDKEvent): RoomMember[] {
  return memberEvent
    .getMatchingTags('p')
    .flatMap((tag, index) => {
      const pubkey = tag[1]?.trim();
      if (!pubkey) return [];
      return [
        {
          pubkey,
          colorIndex: (index % 6) + 1,
          joinedAt: memberEvent.created_at
            ? new Date(memberEvent.created_at * 1000).toLocaleDateString('en-US', {
                month: 'short',
                year: 'numeric'
              })
            : ''
        } satisfies RoomMember
      ];
    });
}
