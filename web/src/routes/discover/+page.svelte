<script lang="ts">
  import type { PageProps } from './$types';
  import { ndk } from '$lib/ndk/client';
  import RoomGrid from '$lib/features/groups/RoomGrid.svelte';

  let { data }: PageProps = $props();

  const currentUser = $derived(ndk.$currentUser);
</script>

<svelte:head>
  <title>Discover — Highlighter</title>
</svelte:head>

<section class="pt-14 pb-20">
  <header class="pb-8 border-b border-base-300 mb-11">
    <h1 class="font-serif font-normal text-[clamp(44px,6vw,68px)] leading-[1.02] tracking-[-0.025em] text-base-content m-0 mb-3.5">Public <em class="italic text-primary">rooms.</em></h1>
    <p class="font-serif italic text-[19px] leading-[1.5] text-base-content/80 max-w-[52ch] m-0 mb-6">
      Open reading groups anyone can join — find a room that matches your interests.
    </p>
    <div class="flex gap-3 flex-wrap">
      <a
        class="inline-flex items-center justify-center px-[22px] py-[10px] text-[13px] font-medium no-underline rounded-full bg-base-content text-base-100 transition-colors duration-200 ease hover:bg-primary focus-visible:bg-primary"
        href="/r/create"
      >
        {currentUser ? 'Create a room' : 'Sign in to create'}
      </a>
    </div>
  </header>

  <RoomGrid
    rooms={data.rooms}
    showVisibilityFilter={false}
    searchPlaceholder="Search rooms by name, URL, or description"
    emptyLabel="No public rooms are visible yet."
    emptyCopy="Create the first room or check back soon."
    emptyCtaHref="/r/create"
    emptyCtaLabel="Create a room"
  />
</section>
