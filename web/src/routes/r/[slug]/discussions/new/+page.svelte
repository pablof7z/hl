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
  <div class="missing">
    <h1>Room not found</h1>
    <a href="/rooms" class="btn">Back to rooms</a>
  </div>
{:else}
  <div class="page">
    <nav class="crumbs">
      <a href="/r/{room.id}">{room.name ?? room.id}</a>
      <span aria-hidden="true">/</span>
      <span>New discussion</span>
    </nav>

    <h1>New discussion</h1>

    <DiscussionComposer groupId={room.id} onCancel={handleCancel} />
  </div>
{/if}

<style>
  .page {
    max-width: 720px;
    margin: 0 auto;
    padding: 28px 0 96px;
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  .crumbs {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    color: var(--ink-fade, #8a8378);
  }

  .crumbs a {
    color: inherit;
    text-decoration: none;
  }

  .crumbs a:hover { color: var(--brand-accent, #C24D2C); }

  h1 {
    margin: 0;
    font-size: 24px;
    font-weight: 600;
    color: var(--ink, #15130F);
  }

  .missing {
    padding: 80px 0;
    text-align: center;
    display: flex;
    flex-direction: column;
    gap: 16px;
    align-items: center;
  }

  .missing h1 {
    font-size: 28px;
  }
</style>
