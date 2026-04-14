<script lang="ts">
  import type { PageProps } from './$types';
  import { ndk } from '$lib/ndk/client';
  import CommunityGrid from '$lib/features/groups/CommunityGrid.svelte';

  let { data }: PageProps = $props();

  const currentUser = $derived(ndk.$currentUser);
  const openCount = $derived(data.communities.filter((community) => community.access === 'open').length);
  const closedCount = $derived(data.communities.length - openCount);
</script>

<svelte:head>
  <title>Discover — Highlighter</title>
</svelte:head>

<section class="discover-page">
  <header class="discover-hero">
    <div class="discover-copy">
      <p class="eyebrow">Discover</p>
      <h1>See every public community in one place.</h1>
      <p class="lede">
        Start with rooms you can join now, scan what is already active on the relay, and move
        straight into the artifact shelves that look worth your time.
      </p>
    </div>

    <div class="discover-actions">
      <a class="primary-link" href="/community">Open community index</a>
      <a class="secondary-link" href="/community/create">
        {currentUser ? 'Create a community' : 'Sign in to create'}
      </a>
    </div>
  </header>

  <section class="discover-summary">
    <article class="summary-card">
      <p class="summary-label">Public communities</p>
      <strong>{data.communities.length}</strong>
      <span>Relay-indexed groups visible to guests.</span>
    </article>
    <article class="summary-card">
      <p class="summary-label">Open now</p>
      <strong>{openCount}</strong>
      <span>Groups a reader can join without an invite code.</span>
    </article>
    <article class="summary-card">
      <p class="summary-label">Invite only</p>
      <strong>{closedCount}</strong>
      <span>Closed communities that are still publicly legible.</span>
    </article>
  </section>

  <CommunityGrid
    communities={data.communities}
    showVisibilityFilter={false}
    searchPlaceholder="Search public communities by name, URL, or description"
    emptyLabel="No public communities are visible yet."
    emptyCopy="Create the first public group or wait for relay metadata to propagate."
    emptyCtaHref="/community/create"
    emptyCtaLabel="Create a community"
  />
</section>

<style>
  .discover-page {
    display: grid;
    gap: 1.5rem;
    padding: 1.5rem 0 3rem;
  }

  .discover-hero {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(16rem, 22rem);
    gap: 1.2rem;
    align-items: end;
  }

  .discover-copy {
    max-width: 44rem;
  }

  .eyebrow,
  .summary-label {
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
    max-width: 11ch;
  }

  .lede,
  .summary-card span {
    margin: 0.9rem 0 0;
    color: var(--muted);
    line-height: 1.65;
  }

  .discover-actions {
    display: grid;
    gap: 0.75rem;
  }

  .primary-link,
  .secondary-link {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: 2.9rem;
    padding: 0 1rem;
    border-radius: 999px;
    border: 1px solid var(--border);
    font-weight: 600;
  }

  .primary-link {
    background: var(--accent);
    border-color: var(--accent);
    color: white;
  }

  .secondary-link {
    background: var(--surface);
    color: var(--text);
  }

  .discover-summary {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 0.9rem;
  }

  .summary-card {
    display: grid;
    gap: 0.35rem;
    padding: 1rem 1.1rem;
    border: 1px solid var(--border);
    border-radius: 1.1rem;
    background:
      radial-gradient(circle at top left, rgba(255, 103, 25, 0.08), transparent 40%),
      var(--surface);
  }

  .summary-card strong {
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: 2rem;
    line-height: 1;
  }

  @media (max-width: 760px) {
    .discover-hero {
      grid-template-columns: 1fr;
    }
  }
</style>
