<script lang="ts">
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import ArticleView from '$lib/features/room/components/ArticleView.svelte';
  import PodcastView from '$lib/features/room/components/PodcastView.svelte';

  type ArtifactType = 'book' | 'podcast' | 'article' | 'essay' | 'video';

  interface SeedArtifact {
    id: string;
    type: ArtifactType;
    title: string;
    author?: string;
  }

  // Seed map — until real API is wired, derives artifact metadata from id.
  // Matches ids referenced by tiles in /room/[slug]/+page.svelte.
  const SEED: Record<string, SeedArtifact> = {
    'tftc-642': {
      id: 'tftc-642',
      type: 'podcast',
      title: 'Broken Money, Two Years In',
      author: 'Marty Bent & Lyn Alden · TFTC #642'
    },
    'dergigi-purple': {
      id: 'dergigi-purple',
      type: 'essay',
      title: 'Purple Text, Orange Highlights',
      author: 'Dergigi'
    }
  };

  const seedMembers = [
    { colorIndex: 1, name: 'DK' },
    { colorIndex: 2, name: 'Pablo F' },
    { colorIndex: 4, name: 'Miljan' },
    { colorIndex: 3, name: 'Bob S' },
    { colorIndex: 5, name: 'Steve L' },
    { colorIndex: 6, name: 'Max W' }
  ];

  const slug = $derived(page.params.slug);
  const id = $derived(page.params.id);
  const artifact = $derived(
    SEED[id ?? ''] ?? {
      id: id ?? '',
      type: 'article' as ArtifactType,
      title: 'Untitled',
      author: ''
    }
  );

  const isPodcast = $derived(artifact.type === 'podcast');

  function handleBack() {
    void goto(`/room/${slug}`);
  }
</script>

<svelte:head>
  <title>{artifact.title} · Room</title>
</svelte:head>

{#if isPodcast}
  <PodcastView {artifact} members={seedMembers} onBack={handleBack} />
{:else}
  <ArticleView {artifact} members={seedMembers} onBack={handleBack} />
{/if}
