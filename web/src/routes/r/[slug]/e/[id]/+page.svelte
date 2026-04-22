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
  <div class="py-20 text-center flex flex-col gap-4 items-center">
    <h1 class="font-serif text-4xl font-normal text-base-content m-0">Artifact not available</h1>
    <p class="text-base-content/80 text-[15px] max-w-[44ch] m-0">The event for this artifact wasn't found on the relays we queried.</p>
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
    <p class="text-base-content/50 text-sm py-10 text-center">Loading article…</p>
  {/if}
{:else if artifact.url}
  <div class="p-10 flex flex-col gap-3 items-start border border-base-300 rounded bg-base-100">
    <p class="m-0 text-base-content/80 text-sm">This artifact links to an external source.</p>
    <a class="text-sm font-medium text-primary no-underline hover:underline" href={artifact.url} target="_blank" rel="noreferrer noopener">
      Read at {artifact.domain || 'source'} ↗
    </a>
  </div>
{:else}
  <p class="text-base-content/50 text-sm py-10 text-center">No readable source is attached to this artifact.</p>
{/if}
