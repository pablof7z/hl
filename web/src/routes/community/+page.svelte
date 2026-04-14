<script lang="ts">
  import type { PageProps } from './$types';
  import { ndk } from '$lib/ndk/client';
  import CommunityGrid from '$lib/features/groups/CommunityGrid.svelte';

  let { data }: PageProps = $props();

  const currentUser = $derived(ndk.$currentUser);
</script>

<svelte:head>
  <title>Circles — Highlighter</title>
</svelte:head>

<section class="community-index">
  <header class="community-index-header">
    <div class="community-index-copy">
      <p class="eyebrow">Circles</p>
      <h1>Public reading circles are live now.</h1>
      <p class="lede">
        Browse the public reading circles already on Highlighter and jump into the one
        you want to build around.
      </p>
    </div>

    <a class="create-link" href="/community/create">
      {currentUser ? 'Create a circle' : 'Sign in to create'}
    </a>
  </header>

  <CommunityGrid
    communities={data.communities}
    showVisibilityFilter={false}
    searchPlaceholder="Search circles by name, URL, or description"
    emptyLabel="No public circles have been indexed yet."
    emptyCopy="Publish the first circle and it will appear here shortly."
    emptyCtaHref="/community/create"
    emptyCtaLabel="Create the first circle"
  />
</section>

<style>
  .community-index {
    display: grid;
    gap: 2rem;
    padding: 2.25rem 0 3rem;
  }

  .community-index-header {
    display: flex;
    justify-content: space-between;
    align-items: end;
    gap: 1.5rem;
    flex-wrap: wrap;
  }

  .community-index-copy {
    max-width: 42rem;
  }

  .eyebrow {
    margin: 0 0 0.5rem;
    color: var(--accent);
    font-size: 0.82rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  h1 {
    margin: 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: clamp(2rem, 4vw, 3rem);
    line-height: 1.05;
    letter-spacing: -0.03em;
  }

  .lede {
    margin: 0.9rem 0 0;
    max-width: 36rem;
    color: var(--muted);
    font-size: 1rem;
  }

  .create-link {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: 2.9rem;
    padding: 0 1rem;
    border-radius: 999px;
    background: var(--accent);
    color: white;
    font-weight: 600;
    transition: background 120ms ease;
  }

  .create-link:hover,
  .create-link:focus-visible {
    background: var(--accent-hover);
  }
</style>
