<script lang="ts">
  import { browser } from '$app/environment';
  import type { PageProps } from './$types';
  import HighlightCard from '$lib/features/highlights/HighlightCard.svelte';
  import HighlightForm from '$lib/features/highlights/HighlightForm.svelte';
  import { ndk } from '$lib/ndk/client';
  import {
    HIGHLIGHTER_HIGHLIGHT_REPOST_KIND,
    fetchHighlightsForShares,
    type HydratedHighlight
  } from '$lib/ndk/highlights';
  import { GROUP_RELAY_URLS } from '$lib/ndk/config';

  let { data }: PageProps = $props();
  const currentUser = $derived(ndk.$currentUser);

  const highlightShareFeed = ndk.$subscribe(() => {
    if (!browser || !data.community) return undefined;

    return {
      filters: [{ kinds: [HIGHLIGHTER_HIGHLIGHT_REPOST_KIND], '#h': [data.community.id], limit: 128 }],
      relayUrls: GROUP_RELAY_URLS,
      closeOnEose: true
    };
  });

  let communityHighlights = $state<HydratedHighlight[]>([]);
  let resolvingHighlights = $state(false);

  $effect(() => {
    if (!browser || !data.community) {
      communityHighlights = [];
      return;
    }

    const shareEvents = [...highlightShareFeed.events];
    if (shareEvents.length === 0) {
      communityHighlights = [];
      return;
    }

    let cancelled = false;
    resolvingHighlights = true;

    void fetchHighlightsForShares(ndk, shareEvents)
      .then((highlights) => {
        if (cancelled) return;
        communityHighlights = highlights;
      })
      .finally(() => {
        if (!cancelled) {
          resolvingHighlights = false;
        }
      });

    return () => {
      cancelled = true;
    };
  });

  const artifactHighlights = $derived(
    data.artifact
      ? communityHighlights.filter((highlight) => highlight.artifactAddress === data.artifact?.address)
      : []
  );
</script>

<svelte:head>
  <title>{data.artifact ? `${data.artifact.title} — Highlighter` : 'Artifact — Highlighter'}</title>
</svelte:head>

{#if data.missing || !data.community || !data.artifact}
  <section class="artifact-missing">
    <p class="eyebrow">Artifact</p>
    <h1>Artifact not found.</h1>
    <p>
      Nothing currently resolves to <span>/community/{data.groupId}/content/{data.contentId}</span>.
      Share the URL into this community first, then come back here.
    </p>
    <div class="actions">
      <a href={`/community/${data.groupId}`}>Back to community</a>
      <a href="/community">Browse communities</a>
    </div>
  </section>
{:else}
  <article class="artifact-page">
    <header class="artifact-hero">
      <div class="artifact-hero-media">
        {#if data.artifact.image}
          <img src={data.artifact.image} alt="" />
        {:else}
          <div class="artifact-fallback">
            <span>{data.artifact.domain.charAt(0).toUpperCase() || '#'}</span>
          </div>
        {/if}
      </div>

      <div class="artifact-hero-copy">
        <p class="eyebrow">Artifact</p>
        <h1>{data.artifact.title}</h1>
        <div class="artifact-badges">
          <span>{data.artifact.source}</span>
          <span>{data.artifact.domain}</span>
          {#if data.artifact.author}
            <span>{data.artifact.author}</span>
          {/if}
        </div>

        {#if data.artifact.note}
          <p class="artifact-note">{data.artifact.note}</p>
        {/if}

        <div class="artifact-actions">
          <a href={data.artifact.url} target="_blank" rel="noreferrer">Open source</a>
          <a href={`/community/${data.community.id}`}>Back to {data.community.name}</a>
          <a href={`/community/${data.community.id}/content/${data.artifact.id}/discussion`}>
            Discussion
          </a>
        </div>
      </div>
    </header>

    <section class="artifact-panels">
      <div class="artifact-panel">
        <p class="panel-label">Route</p>
        <p class="panel-value">/community/{data.community.id}/content/{data.artifact.id}</p>
      </div>

      <div class="artifact-panel">
        <p class="panel-label">Artifact ID</p>
        <p class="panel-value mono">{data.artifact.id}</p>
      </div>

      <div class="artifact-panel">
        <p class="panel-label">Artifact Coordinate</p>
        <p class="panel-value mono">{data.artifact.address}</p>
      </div>
    </section>

    <section class="artifact-next">
      <p class="panel-label">What lands here next</p>
      <p>
        Artifact-level discussion hangs off this same coordinate on the discussion route, while
        canonical highlights resolve back here through their `a` tag.
      </p>
    </section>

    {#if currentUser}
      <HighlightForm artifact={data.artifact} groupId={data.community.id} />
    {:else}
      <section class="artifact-next">
        <p class="panel-label">Create Highlight</p>
        <p>Sign in to save a canonical highlight and repost it into this community.</p>
      </section>
    {/if}

    <section class="artifact-highlights">
      <div class="artifact-highlights-header">
        <div>
          <p class="panel-label">Highlights</p>
          <h2>What this community pulled out</h2>
        </div>
        <span>{artifactHighlights.length} item{artifactHighlights.length === 1 ? '' : 's'}</span>
      </div>

      {#if artifactHighlights.length === 0}
        <div class="artifact-empty">
          <p>No shared highlights yet.</p>
          <p>
            Save the first excerpt from this artifact and it will appear here after the repost hits
            the group relay.
          </p>
        </div>
      {:else}
        <div class="highlight-stack">
          {#each artifactHighlights as highlight (highlight.eventId)}
            <HighlightCard {highlight} artifact={data.artifact} />
          {/each}
        </div>
      {/if}

      {#if resolvingHighlights}
        <p class="artifact-loading">Refreshing highlight shares…</p>
      {/if}
    </section>
  </article>
{/if}

<style>
  .artifact-page,
  .artifact-missing {
    display: grid;
    gap: 1.5rem;
    padding: 2rem 0 3rem;
  }

  .artifact-hero {
    display: grid;
    grid-template-columns: minmax(180px, 240px) minmax(0, 1fr);
    gap: 1.4rem;
    padding: 1.35rem;
    border: 1px solid var(--border);
    border-radius: 1.45rem;
    background:
      radial-gradient(circle at top left, rgba(255, 103, 25, 0.1), transparent 38%),
      var(--surface);
  }

  .artifact-hero-media,
  .artifact-hero-media img,
  .artifact-fallback {
    width: 100%;
    aspect-ratio: 4 / 5;
    border-radius: 1.1rem;
  }

  .artifact-hero-media {
    overflow: hidden;
    background: linear-gradient(160deg, rgba(255, 103, 25, 0.12), rgba(255, 103, 25, 0.04));
  }

  .artifact-hero-media img {
    object-fit: cover;
  }

  .artifact-fallback {
    display: grid;
    place-items: center;
    color: var(--accent);
    font-size: 2rem;
    font-weight: 700;
  }

  .artifact-hero-copy {
    display: grid;
    align-content: start;
    gap: 0.8rem;
  }

  .eyebrow,
  .panel-label {
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
    font-size: clamp(2rem, 4vw, 3rem);
    line-height: 1.05;
    letter-spacing: -0.03em;
  }

  .artifact-badges,
  .artifact-actions,
  .actions {
    display: flex;
    gap: 0.45rem;
    flex-wrap: wrap;
  }

  .artifact-badges span,
  .artifact-actions a,
  .actions a {
    display: inline-flex;
    align-items: center;
    min-height: 2rem;
    padding: 0 0.75rem;
    border-radius: 999px;
    background: var(--surface-soft);
    color: var(--text);
    font-size: 0.8rem;
    font-weight: 600;
  }

  .artifact-actions a:first-child,
  .actions a:last-child {
    background: var(--accent);
    color: white;
  }

  .artifact-note,
  .artifact-next p,
  .artifact-missing p {
    margin: 0;
    color: var(--muted);
    line-height: 1.65;
  }

  .artifact-panels {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 0.85rem;
  }

  .artifact-panel,
  .artifact-next,
  .artifact-empty {
    padding: 1rem 1.1rem;
    border: 1px solid var(--border);
    border-radius: 1.1rem;
    background: var(--surface);
  }

  .panel-value,
  .mono,
  .artifact-missing span {
    margin: 0.55rem 0 0;
    color: var(--text-strong);
    font-family: var(--font-mono);
    line-height: 1.55;
    overflow-wrap: anywhere;
  }

  .artifact-highlights {
    display: grid;
    gap: 1rem;
  }

  .artifact-highlights-header {
    display: flex;
    align-items: end;
    justify-content: space-between;
    gap: 1rem;
    flex-wrap: wrap;
  }

  .artifact-highlights-header h2 {
    margin: 0.3rem 0 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: 1.55rem;
    line-height: 1.15;
    letter-spacing: -0.02em;
  }

  .artifact-highlights-header span {
    display: inline-flex;
    align-items: center;
    min-height: 2rem;
    padding: 0 0.75rem;
    border-radius: 999px;
    background: var(--surface-soft);
    color: var(--muted);
    font-size: 0.8rem;
    font-weight: 600;
  }

  .highlight-stack {
    display: grid;
    gap: 0.85rem;
  }

  .artifact-empty p,
  .artifact-loading {
    margin: 0;
    color: var(--muted);
    line-height: 1.6;
  }

  .artifact-loading {
    font-size: 0.88rem;
  }

  @media (max-width: 760px) {
    .artifact-page,
    .artifact-missing {
      padding-top: 1.5rem;
    }

    .artifact-hero {
      grid-template-columns: 1fr;
    }

    .artifact-hero-media,
    .artifact-hero-media img,
    .artifact-fallback {
      aspect-ratio: 16 / 9;
    }
  }
</style>
