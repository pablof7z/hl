<script lang="ts">
  import type { PageProps } from './$types';
  import { ndk } from '$lib/ndk/client';
  import CommunityGrid from '$lib/features/groups/CommunityGrid.svelte';

  let { data }: PageProps = $props();

  const currentUser = $derived(ndk.$currentUser);
</script>

<svelte:head>
  <title>Discover — Highlighter</title>
</svelte:head>

<section class="discover-page">
  <header class="discover-hero">
    <p class="eyebrow">Discover</p>
    <h1>Explore public circles</h1>
    <p class="lede">
      Browse active reading circles on Highlighter and jump into the ones that interest you.
    </p>
    <a class="create-link" href="/community/create">
      {currentUser ? 'Create a circle' : 'Sign in to create'}
    </a>
  </header>

  <CommunityGrid
    communities={data.communities}
    showVisibilityFilter={false}
    searchPlaceholder="Search circles by name, URL, or description"
    emptyLabel="No public circles are visible yet."
    emptyCopy="Create the first circle or check back soon."
    emptyCtaHref="/community/create"
    emptyCtaLabel="Create a circle"
  />
</section>

<style>
  .discover-page {
    display: grid;
    gap: 1.5rem;
    padding: 1.5rem 0 3rem;
  }

  .discover-hero {
    max-width: 44rem;
  }

  .eyebrow {
    margin: 0;
    color: var(--accent);
    font-size: 0.8rem;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }

  h1 {
    margin: 0.35rem 0 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: clamp(2.2rem, 5vw, 3.8rem);
    line-height: 0.98;
    letter-spacing: -0.04em;
  }

  .lede {
    margin: 0.9rem 0 0;
    color: var(--muted);
    line-height: 1.65;
  }

  .create-link {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: 2.9rem;
    margin-top: 1.2rem;
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
