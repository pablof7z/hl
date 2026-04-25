<script lang="ts">
  import { browser } from '$app/environment';
  import { NDKKind } from '@nostr-dev-kit/ndk';
  import { ndk } from '$lib/ndk/client';
  import { GROUP_RELAY_URLS } from '$lib/ndk/config';
  import {
    buildJoinedRooms,
    buildRoomSummary,
    groupIdFromEvent,
    type RoomSummary
  } from '$lib/ndk/groups';
  import { fetchFriendsCommunitiesLists } from '$lib/ndk/lists';
  import type { PageProps } from './$types';
  import ExplorerHero from '$lib/features/rooms-explorer/ExplorerHero.svelte';
  import RoomShelf from '$lib/features/rooms-explorer/RoomShelf.svelte';
  import RoomShelfCard from '$lib/features/rooms-explorer/RoomShelfCard.svelte';
  import RoomCard from '$lib/features/groups/RoomCard.svelte';

  let { data }: PageProps = $props();

  const currentUser = $derived(ndk.$currentUser);
  const signedIn = $derived(Boolean(currentUser));

  // ── Joined rooms (existing logic) ──────────────────────────────────────

  const membershipFeed = ndk.$subscribe(() => {
    if (!browser || !currentUser) return undefined;
    return {
      filters: [
        {
          kinds: [NDKKind.GroupAdmins, NDKKind.GroupMembers],
          '#p': [currentUser.pubkey],
          limit: 128
        }
      ],
      relayUrls: GROUP_RELAY_URLS,
      closeOnEose: true
    };
  });

  const membershipGroupIds = $derived.by(() => {
    const ids = new Set<string>();
    for (const event of membershipFeed.events) {
      const groupId = groupIdFromEvent(event);
      if (groupId) ids.add(groupId);
    }
    return [...ids];
  });

  const metadataFeed = ndk.$subscribe(() => {
    if (!browser || membershipGroupIds.length === 0) return undefined;
    return {
      filters: [
        {
          kinds: [NDKKind.GroupMetadata],
          '#d': membershipGroupIds,
          limit: Math.max(membershipGroupIds.length * 2, 32)
        }
      ],
      relayUrls: GROUP_RELAY_URLS,
      closeOnEose: true
    };
  });

  const joinedRooms: RoomSummary[] = $derived(
    currentUser
      ? buildJoinedRooms(
          currentUser.pubkey,
          [...metadataFeed.events],
          [...membershipFeed.events]
        )
      : []
  );

  const joinedGroupIds = $derived(new Set(joinedRooms.map((r) => r.id)));

  // ── Friends shelf — client-side ─────────────────────────────────────────

  const followPubkeys = $derived.by(() => {
    if (!browser || !currentUser) return [] as string[];
    return [...(ndk.$follows ?? [])].slice(0, 500);
  });

  let friendsRooms = $state<RoomSummary[]>([]);

  $effect(() => {
    if (!browser || followPubkeys.length === 0) {
      friendsRooms = [];
      return;
    }

    const pubkeys = followPubkeys.slice();

    fetchFriendsCommunitiesLists(ndk, pubkeys).then((listsByPubkey) => {
      const groupIdSet = new Set<string>();
      for (const refs of listsByPubkey.values()) {
        for (const ref of refs) {
          if (ref.groupId) groupIdSet.add(ref.groupId);
        }
      }

      const groupIds = [...groupIdSet].slice(0, 12);
      if (groupIds.length === 0) {
        friendsRooms = [];
        return;
      }

      ndk
        .fetchEvents(
          [{ kinds: [NDKKind.GroupMetadata], '#d': groupIds, limit: groupIds.length * 2 }],
          { closeOnEose: true }
        )
        .then((eventSet) => {
          const events = Array.from(eventSet ?? []);
          const resolved: RoomSummary[] = [];

          for (const event of events) {
            try {
              resolved.push(buildRoomSummary(event));
            } catch {
              // skip unresolvable
            }
          }

          const byId = new Map(resolved.map((r) => [r.id, r]));
          friendsRooms = groupIds.flatMap((id) => {
            const r = byId.get(id);
            return r ? [r] : [];
          });
        })
        .catch(() => {
          // friends shelf stays empty on network error
        });
    }).catch(() => {
      // friends shelf stays empty on network error
    });
  });

  // ── Derived slices from server data ────────────────────────────────────

  const heroRoom = $derived(data.featured[0] as RoomSummary | undefined);
  const featuredShelf = $derived(data.featured.slice(1));

  // All rooms grid: filter out rooms already shown in featured/friends
  const featuredIds = $derived(new Set(data.featured.map((r) => r.id)));

  const allRoomsFiltered = $derived(
    data.allRooms.filter((r) => !featuredIds.has(r.id))
  );
</script>

<svelte:head>
  <title>Rooms · Highlighter</title>
</svelte:head>

<div class="explorer">

  <!-- Hero ────────────────────────────────────────────────────────────── -->
  {#if heroRoom}
    <div class="explorer-hero">
      <ExplorerHero rooms={data.featured} />
    </div>
  {/if}

  <!-- Friends are here shelf ─────────────────────────────────────────── -->
  {#if signedIn && friendsRooms.length > 0}
    <div class="explorer-section">
      <RoomShelf title="Friends are here" subtitle="People you follow are members">
        {#each friendsRooms as room (room.id)}
          <RoomShelfCard {room} />
        {/each}
      </RoomShelf>
    </div>
  {/if}

  <!-- Featured shelf ─────────────────────────────────────────────────── -->
  {#if featuredShelf.length > 0}
    <div class="explorer-section">
      <RoomShelf title="Featured" subtitle="Curated by Highlighter">
        {#each featuredShelf as room (room.id)}
          <RoomShelfCard {room} />
        {/each}
      </RoomShelf>
    </div>
  {/if}

  <!-- All rooms grid ─────────────────────────────────────────────────── -->
  {#if allRoomsFiltered.length > 0}
    <div class="explorer-section">
      <header class="section-header">
        <span class="section-title">All rooms</span>
        <div class="section-actions">
          {#if signedIn}
            <a
              href="/r/create"
              class="btn btn-sm btn-outline rounded-full text-xs"
            >+ Create a room</a>
          {/if}
        </div>
      </header>
      <div class="rooms-grid">
        {#each allRoomsFiltered as room (room.id)}
          <RoomCard {room} joined={joinedGroupIds.has(room.id)} />
        {/each}
      </div>
    </div>
  {:else if data.allRooms.length === 0}
    <div class="empty-state">
      <p class="empty-label">No public rooms yet.</p>
      <p class="empty-copy">Create the first room or check back soon.</p>
      {#if signedIn}
        <a href="/r/create" class="btn btn-primary btn-sm rounded-full">Create a room</a>
      {/if}
    </div>
  {/if}

</div>

<style>
  .explorer {
    padding-top: 2.5rem;
    padding-bottom: 5rem;
    display: flex;
    flex-direction: column;
    gap: 2.5rem;
  }

  .explorer-section {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .section-header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 1rem;
  }

  .section-title {
    font-size: 0.7rem;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: color-mix(in srgb, var(--color-base-content, #111) 50%, transparent);
  }

  .rooms-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 1rem;
  }

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.75rem;
    text-align: center;
    padding: 3rem 1.5rem;
    border: 1px solid var(--color-base-300);
    border-radius: 1.25rem;
  }

  .empty-label {
    margin: 0;
    font-size: 1.1rem;
    font-weight: 700;
    color: var(--color-base-content);
  }

  .empty-copy {
    margin: 0;
    font-size: 0.9rem;
    color: color-mix(in srgb, var(--color-base-content, #111) 55%, transparent);
  }
</style>
