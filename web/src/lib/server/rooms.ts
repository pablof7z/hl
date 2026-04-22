import { NDKKind, NDKSimpleGroupMetadata, type NDKEvent } from '@nostr-dev-kit/ndk';
import { GROUP_RELAY_URLS } from '$lib/ndk/config';
import { fetchEventsForSsr } from '$lib/server/nostr';
import type { Room, RoomMember } from '$lib/features/room/api/room';

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

  return {
    id: trimmedSlug,
    name: metadata.name || trimmedSlug,
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
