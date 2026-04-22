<script lang="ts">
  import { goto } from '$app/navigation';
  import DiscussionComposer from '$lib/features/discussions/DiscussionComposer.svelte';
  import type { PageData } from './$types';

  let { data }: { data: PageData } = $props();

  const room = $derived(data.room);

  function handleCancel() {
    void goto(room ? `/r/${room.id}` : '/rooms');
  }
</script>

<svelte:head>
  <title>New discussion · {room?.name ?? 'Room'}</title>
</svelte:head>

{#if !room}
  <div class="py-20 text-center flex flex-col gap-4 items-center">
    <h1 class="text-3xl font-semibold text-base-content m-0">Room not found</h1>
    <a href="/rooms" class="btn">Back to rooms</a>
  </div>
{:else}
  <div class="max-w-[720px] mx-auto pt-7 pb-24 flex flex-col gap-5">
    <nav class="flex items-center gap-2 text-[13px] text-base-content/50">
      <a href="/r/{room.id}" class="text-inherit no-underline hover:text-primary">{room.name ?? room.id}</a>
      <span aria-hidden="true">/</span>
      <span>New discussion</span>
    </nav>

    <h1 class="m-0 text-2xl font-semibold text-base-content">New discussion</h1>

    <DiscussionComposer groupId={room.id} onCancel={handleCancel} />
  </div>
{/if}
