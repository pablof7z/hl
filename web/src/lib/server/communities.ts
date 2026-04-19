import { NDKKind, type NDKEvent } from '@nostr-dev-kit/ndk';
import { GROUP_RELAY_URLS } from '$lib/ndk/config';
import {
  buildCommunitySummary,
  type CommunitySummary,
  type CommunityVisibility
} from '$lib/ndk/groups';
import { fetchEventsForSsr } from '$lib/server/nostr';

type FetchCommunitiesOptions = {
  limit?: number;
  visibility?: CommunityVisibility | 'all';
};

export async function fetchCommunities(
  options: number | FetchCommunitiesOptions = 32
): Promise<CommunitySummary[]> {
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
      `fetchCommunities(${fetchLimit})`,
      { relays: GROUP_RELAY_URLS }
    )) ?? []
  )
    .sort((left, right) => (right.created_at ?? 0) - (left.created_at ?? 0))
    .slice(0, fetchLimit);

  return (await buildCommunitySummariesFromMetadataEvents(metadataEvents))
    .filter((community): community is CommunitySummary => Boolean(community))
    .filter((community) => visibility === 'all' || community.visibility === visibility)
    .slice(0, limit);
}

export async function buildCommunitySummariesFromMetadataEvents(
  metadataEvents: NDKEvent[]
): Promise<CommunitySummary[]> {
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
        return buildCommunitySummary(event, {
          adminEvent: adminEvents.get(id),
          memberEvent: memberEvents.get(id)
        });
      } catch {
        return null;
      }
    })
    .filter((community): community is CommunitySummary => Boolean(community));
}

export async function fetchCommunityById(groupId: string): Promise<CommunitySummary | undefined> {
  const trimmedGroupId = groupId.trim();
  if (!trimmedGroupId) return undefined;

  const metadataEvents = Array.from(
    (await fetchEventsForSsr(
      {
        kinds: [NDKKind.GroupMetadata],
        '#d': [trimmedGroupId]
      },
      `fetchCommunityById(${trimmedGroupId})`,
      { relays: GROUP_RELAY_URLS }
    )) ?? []
  ).sort((left, right) => (right.created_at ?? 0) - (left.created_at ?? 0));

  const metadataEvent = metadataEvents[0];
  if (!metadataEvent) return undefined;

  const [adminEvent, memberEvent] = await Promise.all([
    fetchLatestGroupEvent(NDKKind.GroupAdmins, trimmedGroupId),
    fetchLatestGroupEvent(NDKKind.GroupMembers, trimmedGroupId)
  ]);

  return buildCommunitySummary(metadataEvent, {
    adminEvent,
    memberEvent
  });
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

async function fetchLatestGroupEvent(
  kind: NDKKind.GroupAdmins | NDKKind.GroupMembers,
  groupId: string
): Promise<NDKEvent | undefined> {
  const events = Array.from(
    (await fetchEventsForSsr(
      {
        kinds: [kind],
        '#d': [groupId],
        limit: 1
      },
      `fetchLatestGroupEvent(${kind},${groupId})`,
      { relays: GROUP_RELAY_URLS }
    )) ?? []
  ).sort((left, right) => (right.created_at ?? 0) - (left.created_at ?? 0));

  return events[0];
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
