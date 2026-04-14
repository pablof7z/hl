<script lang="ts">
  import { browser } from '$app/environment';
  import type { PageProps } from './$types';
  import type { ArtifactRecord } from '$lib/ndk/artifacts';
  import HighlightCard from '$lib/features/highlights/HighlightCard.svelte';
  import { fetchArtifactsByAddresses } from '$lib/ndk/artifacts';
  import { ndk } from '$lib/ndk/client';
  import { DEFAULT_RELAYS, GROUP_RELAY_URLS } from '$lib/ndk/config';
  import {
    HIGHLIGHTER_HIGHLIGHT_KIND,
    HIGHLIGHTER_HIGHLIGHT_REPOST_KIND,
    hydrateHighlights
  } from '$lib/ndk/highlights';

  let { data }: PageProps = $props();
  const currentUser = $derived(ndk.$currentUser);

  const authoredHighlightFeed = ndk.$subscribe(() => {
    if (!browser || !currentUser) return undefined;

    return {
      filters: [{ kinds: [HIGHLIGHTER_HIGHLIGHT_KIND], authors: [currentUser.pubkey], limit: 96 }],
      relayUrls: DEFAULT_RELAYS,
      closeOnEose: true
    };
  });

  const authoredShareFeed = ndk.$subscribe(() => {
    if (!browser || !currentUser) return undefined;

    return {
      filters: [{ kinds: [HIGHLIGHTER_HIGHLIGHT_REPOST_KIND], authors: [currentUser.pubkey], limit: 128 }],
      relayUrls: GROUP_RELAY_URLS,
      closeOnEose: true
    };
  });

  const highlights = $derived(
    hydrateHighlights([...authoredHighlightFeed.events], [...authoredShareFeed.events])
  );

  let artifactsByAddress = $state<Map<string, ArtifactRecord>>(new Map());
  let resolvingArtifacts = $state(false);

  $effect(() => {
    if (!browser) {
      artifactsByAddress = new Map();
      return;
    }

    const addresses = [...new Set(highlights.map((highlight) => highlight.artifactAddress).filter(Boolean))];
    if (addresses.length === 0) {
      artifactsByAddress = new Map();
      return;
    }

    let cancelled = false;
    resolvingArtifacts = true;

    void fetchArtifactsByAddresses(ndk, addresses)
      .then((artifacts) => {
        if (cancelled) return;
        artifactsByAddress = artifacts;
      })
      .finally(() => {
        if (!cancelled) {
          resolvingArtifacts = false;
        }
      });

    return () => {
      cancelled = true;
    };
  });
</script>

<svelte:head>
  <title>My Highlights — Highlighter</title>
</svelte:head>

<section class="me-highlights-page">
  <header class="me-header">
    <p class="eyebrow">Me</p>
    <h1>My Highlights</h1>
    <p>
      Canonical `kind:9802` events you authored live here, regardless of which communities you
      reposted them into.
    </p>
  </header>

  <nav class="me-tabs">
    <a href="/me/highlights" class="me-tab active">Highlights</a>
    <a href="/me/communities" class="me-tab">Communities</a>
    <a href="/me/for-later" class="me-tab">For Later</a>
    <a href="/me/recommended" class="me-tab">Recommended</a>
    <a href="/me/synthesis" class="me-tab">Synthesis</a>
  </nav>

  <section class="me-summary">
    <div class="summary-card">
      <p class="summary-label">Canonical highlights</p>
      <strong>{highlights.length}</strong>
      <span>Portable across communities via `kind:16` reposts.</span>
    </div>
    <div class="summary-card">
      <p class="summary-label">Loaded communities</p>
      <strong>{data.communities.length}</strong>
      <span>Available as share-again targets on each card.</span>
    </div>
  </section>

  {#if highlights.length === 0}
    <section class="empty-state">
      <p>No highlights yet.</p>
      <p>
        Share an artifact into a community, save an excerpt, and it will appear here as your
        canonical vault entry.
      </p>
    </section>
  {:else}
    <section class="highlight-stack">
      {#each highlights as highlight (highlight.eventId)}
        <HighlightCard
          {highlight}
          artifact={artifactsByAddress.get(highlight.artifactAddress)}
          communities={data.communities}
          showShareControl={true}
        />
      {/each}
    </section>
  {/if}

  {#if resolvingArtifacts}
    <p class="loading">Resolving artifact metadata…</p>
  {/if}
</section>

<style>
  .me-highlights-page {
    display: grid;
    gap: 1.5rem;
    padding: 1rem 0 3rem;
  }

  .me-header {
    display: grid;
    gap: 0.35rem;
  }

  .eyebrow,
  .summary-label {
    margin: 0;
    color: var(--accent);
    font-size: 0.8rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  h1 {
    margin: 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: clamp(2rem, 4vw, 2.8rem);
    line-height: 1.05;
    letter-spacing: -0.03em;
  }

  .me-header p:last-child,
  .summary-card span,
  .empty-state p,
  .loading {
    margin: 0;
    color: var(--muted);
    line-height: 1.65;
  }

  .me-tabs {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .me-tab {
    padding: 0.45rem 1rem;
    border-radius: var(--radius-md);
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text);
    font-size: 0.88rem;
    font-weight: 500;
    text-decoration: none;
    transition: border-color 140ms, color 140ms;
  }

  .me-tab:hover,
  .me-tab.active {
    border-color: var(--accent);
    color: var(--accent);
  }

  .me-summary {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 0.9rem;
  }

  .summary-card,
  .empty-state {
    display: grid;
    gap: 0.4rem;
    padding: 1rem 1.1rem;
    border: 1px solid var(--border);
    border-radius: 1.1rem;
    background: var(--surface);
  }

  .summary-card strong {
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: 2rem;
    line-height: 1;
  }

  .highlight-stack {
    display: grid;
    gap: 0.9rem;
  }

  .empty-state p:first-child {
    color: var(--text-strong);
    font-weight: 700;
  }

  .loading {
    font-size: 0.88rem;
  }
</style>
