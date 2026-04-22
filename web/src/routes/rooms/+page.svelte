<script lang="ts">
  import { browser } from '$app/environment';
  import { NDKKind } from '@nostr-dev-kit/ndk';
  import { ndk } from '$lib/ndk/client';
  import { GROUP_RELAY_URLS } from '$lib/ndk/config';
  import {
    buildJoinedRooms,
    groupIdFromEvent,
    type RoomSummary
  } from '$lib/ndk/groups';

  const currentUser = $derived(ndk.$currentUser);
  const signedIn = $derived(Boolean(currentUser));

  // Subscribe to user's admin/member records across NIP-29 relays
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

  // Fetch metadata for those groups
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

  const rooms: RoomSummary[] = $derived(
    currentUser
      ? buildJoinedRooms(
          currentUser.pubkey,
          [...metadataFeed.events],
          [...membershipFeed.events]
        )
      : []
  );

  const loading = $derived(signedIn && !membershipFeed.eosed);
</script>

<svelte:head>
  <title>Your rooms · Highlighter</title>
</svelte:head>

<section class="pt-14 pb-20">
  <header class="pb-8 border-b border-base-300 mb-11">
    <div class="flex items-baseline justify-between gap-6 mb-3.5">
      <h1 class="font-serif font-normal text-[clamp(44px,6vw,68px)] leading-[1.02] tracking-[-0.025em] text-base-content m-0">Your <em class="italic text-primary">rooms.</em></h1>
      {#if signedIn}
        <a
          href="/r/create"
          class="inline-block px-4 py-[7px] text-[13px] font-medium no-underline rounded bg-base-content text-base-100 transition-colors duration-200 ease hover:bg-primary"
        >+ Create a room</a>
      {/if}
    </div>
    <p class="font-serif italic text-[19px] leading-[1.5] text-base-content/80 max-w-[52ch] m-0">
      Rooms you're a member of — small, invitation-only reading groups.
    </p>
  </header>

  {#if !signedIn}
    <div class="bg-base-100 border border-base-300 rounded px-8 py-11 text-center">
      <p class="text-base-content/80 text-[15px] m-0 mx-auto max-w-[44ch]">Log in to see your rooms.</p>
    </div>
  {:else if loading}
    <div class="bg-base-100 border border-base-300 rounded px-8 py-11 text-center">
      <p class="text-base-content/80 text-[15px] m-0 mx-auto max-w-[44ch]">Loading your rooms…</p>
    </div>
  {:else if rooms.length === 0}
    <div class="bg-base-100 border border-base-300 rounded px-8 py-11 text-center">
      <h2 class="font-serif text-[26px] font-medium text-base-content m-0 mb-2">You're not in any rooms yet.</h2>
      <p class="text-base-content/80 text-[15px] m-0 mx-auto max-w-[44ch]">Rooms are closed by default. Either bring one of your own, or find a public one to read along with.</p>
      <div class="flex gap-3 justify-center mt-6 flex-wrap">
        <a href="/discover" class="inline-block px-5 py-[10px] text-[13px] font-medium no-underline rounded bg-base-content text-base-100 transition-all duration-200 ease hover:bg-primary">Discover rooms</a>
        <a href="/onboarding" class="inline-block px-5 py-[10px] text-[13px] font-medium no-underline rounded bg-base-100 text-base-content/80 border border-base-300 transition-all duration-200 ease hover:border-primary hover:text-primary">Bring a room</a>
      </div>
    </div>
  {:else}
    <div class="grid gap-4 [grid-template-columns:repeat(auto-fill,minmax(280px,1fr))]">
      {#each rooms as room (room.id)}
        <a
          href="/r/{room.id}"
          class="bg-base-100 border border-base-300 rounded px-6 py-[22px] flex flex-col gap-2.5 no-underline text-inherit transition-[border-color,transform] duration-200 ease hover:border-primary hover:-translate-y-0.5"
        >
          <div class="font-mono text-[10px] tracking-[0.14em] uppercase text-base-content/50">
            {#if room.visibility === 'private'}Private{:else}Public{/if} ·
            {room.memberCount ?? '?'} members
          </div>
          <h3 class="font-serif font-medium text-[24px] leading-[1.1] text-base-content m-0 tracking-[-0.01em]">{room.name}</h3>
          {#if room.about}
            <p class="text-[13.5px] leading-[1.5] text-base-content/80 m-0 flex-1 line-clamp-3">{room.about}</p>
          {/if}
          <div class="font-mono text-[10px] tracking-[0.1em] uppercase text-primary pt-2.5 border-t border-dotted border-base-300 mt-auto">Open →</div>
        </a>
      {/each}
    </div>
  {/if}
</section>
