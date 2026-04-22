<script lang="ts">
  import { goto } from '$app/navigation';
  import ArticleView from '$lib/features/room/components/ArticleView.svelte';
  import PodcastView from '$lib/features/room/components/PodcastView.svelte';
  import type { PageData } from './$types';

  let { data }: { data: PageData } = $props();

  const artifact = $derived(data.artifact);
  const room = $derived(data.room);
  const podcast = $derived(data.podcast);
  const roomMemberPubkeys = $derived(room?.members.map((m) => m.pubkey) ?? []);

  const isPodcast = $derived(artifact?.source === 'podcast');

  function handleBack() {
    if (room) {
      void goto(`/r/${room.id}`);
    } else {
      void goto('/rooms');
    }
  }
</script>

<svelte:head>
  <title>{artifact?.title ?? 'Artifact'} · Room</title>
</svelte:head>

{#if !artifact}
  <div class="artifact-missing">
    <h1>Artifact not available</h1>
    <p>The event for this artifact wasn't found on the relays we queried.</p>
    {#if room}
      <a href={`/r/${room.id}`} class="btn">Back to {room.name}</a>
    {:else}
      <a href="/rooms" class="btn">Back to your rooms</a>
    {/if}
  </div>
{:else if isPodcast}
  <PodcastView {artifact} {podcast} {roomMemberPubkeys} onBack={handleBack} />
{:else}
  <ArticleView {artifact} {roomMemberPubkeys} onBack={handleBack} />
{/if}

<style>
  .artifact-missing {
    padding: 80px 0;
    text-align: center;
    display: flex;
    flex-direction: column;
    gap: 16px;
    align-items: center;
  }

  .artifact-missing h1 {
    font-family: var(--font-serif);
    font-size: 36px;
    font-weight: 400;
    color: var(--ink);
    margin: 0;
  }

  .artifact-missing p {
    color: var(--ink-soft);
    font-size: 15px;
    max-width: 44ch;
    margin: 0;
  }

  .btn {
    padding: 10px 20px;
    background: var(--ink);
    color: var(--surface);
    font-family: var(--font-sans);
    font-size: 13px;
    font-weight: 500;
    text-decoration: none;
    border-radius: var(--radius);
    transition: background 200ms ease;
  }

  .btn:hover {
    background: var(--brand-accent);
  }
</style>
