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

<section class="discover-page">
  <header class="discover-header">
    <h1 class="page-title">Public <em>rooms.</em></h1>
    <p class="page-lead">
      Open reading groups anyone can join — find a room that matches your interests.
    </p>
    <div class="header-actions">
      <a class="btn-create" href="/r/create">
        {currentUser ? 'Create a room' : 'Sign in to create'}
      </a>
    </div>
  </header>

  <RoomGrid
    communities={data.communities}
    showVisibilityFilter={false}
    searchPlaceholder="Search rooms by name, URL, or description"
    emptyLabel="No public rooms are visible yet."
    emptyCopy="Create the first room or check back soon."
    emptyCtaHref="/r/create"
    emptyCtaLabel="Create a room"
  />
</section>

<style>
  .discover-page {
    padding: 56px 0 80px;
  }

  .discover-header {
    padding-bottom: 32px;
    border-bottom: 1px solid var(--rule);
    margin-bottom: 44px;
  }

  .page-title {
    font-family: var(--font-serif);
    font-weight: 400;
    font-size: clamp(44px, 6vw, 68px);
    line-height: 1.02;
    letter-spacing: -0.025em;
    color: var(--ink);
    margin: 0 0 14px;
  }

  .page-title em {
    font-style: italic;
    color: var(--brand-accent);
  }

  .page-lead {
    font-family: var(--font-serif);
    font-style: italic;
    font-size: 19px;
    line-height: 1.5;
    color: var(--ink-soft);
    max-width: 52ch;
    margin: 0 0 24px;
  }

  .header-actions {
    display: flex;
    gap: 12px;
    flex-wrap: wrap;
  }

  .btn-create {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 10px 22px;
    font-family: var(--font-sans);
    font-size: 13px;
    font-weight: 500;
    text-decoration: none;
    border-radius: 999px;
    background: var(--ink);
    color: var(--surface);
    transition: background 200ms ease;
  }

  .btn-create:hover,
  .btn-create:focus-visible {
    background: var(--brand-accent);
  }
</style>
