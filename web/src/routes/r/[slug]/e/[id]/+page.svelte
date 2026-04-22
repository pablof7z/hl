<script lang="ts">
  import { browser } from '$app/environment';
  import { goto } from '$app/navigation';
  import { ndk } from '$lib/ndk/client';
  import { parseNostrAddress } from '$lib/ndk/artifacts';
  import { buildArtifactHighlightFilters } from '$lib/ndk/highlights';
  import ArticleView from '$lib/features/articles/ArticleView.svelte';
  import PodcastView from '$lib/features/room/components/PodcastView.svelte';
  import type { PageData } from './$types';

  let { data }: { data: PageData } = $props();

  const artifact = $derived(data.artifact);
  const room = $derived(data.room);
  const podcast = $derived(data.podcast);
  const roomMemberPubkeys = $derived(room?.members.map((m) => m.pubkey) ?? []);
  const isPodcast = $derived(artifact?.source === 'podcast');

  // Resolve nostrRef — only kind-30023 Nostr articles have one
  const nostrRef = $derived.by(() => {
    if (!artifact || artifact.referenceTagName !== 'a') return undefined;
    const parsed = parseNostrAddress(artifact.referenceTagValue);
    if (!parsed || parsed.kind !== 30023) return undefined;
    return parsed;
  });

  // Subscribe to the article NDKEvent
  const articleSub = ndk.$subscribe(() => {
    if (!browser || !nostrRef) return undefined;
    return {
      filters: [{
        kinds: [nostrRef.kind],
        authors: [nostrRef.pubkey],
        '#d': [nostrRef.identifier],
        limit: 1
      }]
    };
  });

  const articleEvent = $derived(articleSub.events[0]);

  // Highlights filtered to room members only
  const highlightsSub = ndk.$subscribe(() => {
    if (!browser || !artifact) return undefined;
    const filters = buildArtifactHighlightFilters([artifact], roomMemberPubkeys);
    if (filters.length === 0) return undefined;
    return { filters };
  });

  const highlightEvents = $derived(highlightsSub.events);

  // Room context feeds RoomContextBar and DiscussionPanel inside ArticleView
  const roomContext = $derived.by(() => {
    if (!room || !artifact) return undefined;
    return {
      groupId: room.id,
      roomName: room.name ?? room.id,
      roomUrl: `/r/${room.id}`,
      artifact,
      rootContext: {
        type: 'artifact' as const,
        artifactAddress: artifact.referenceTagValue,
        artifactKind: '30023'
      }
    };
  });

  function handleBack() {
    void goto(room ? `/r/${room.id}` : '/rooms');
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
{:else if nostrRef}
  {#if articleEvent}
    <ArticleView
      event={articleEvent}
      {highlightEvents}
      {roomContext}
    />
  {:else}
    <p class="loading-note">Loading article…</p>
  {/if}
{:else if artifact.url}
  <div class="external-source">
    <p>This artifact links to an external source.</p>
    <a class="external-link" href={artifact.url} target="_blank" rel="noreferrer noopener">
      Read at {artifact.domain || 'source'} ↗
    </a>
  </div>
{:else}
  <p class="loading-note">No readable source is attached to this artifact.</p>
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

  .loading-note {
    font-family: var(--font-sans);
    color: var(--ink-fade);
    font-size: 14px;
    padding: 40px 0;
    text-align: center;
  }

  .external-source {
    padding: 40px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    align-items: flex-start;
    border: 1px solid var(--rule);
    border-radius: var(--radius);
    background: var(--surface);
  }

  .external-source p {
    margin: 0;
    color: var(--ink-soft);
    font-family: var(--font-sans);
    font-size: 14px;
  }

  .external-link {
    font-family: var(--font-sans);
    font-size: 14px;
    font-weight: 500;
    color: var(--brand-accent);
    text-decoration: none;
  }

  .external-link:hover {
    text-decoration: underline;
  }
</style>
