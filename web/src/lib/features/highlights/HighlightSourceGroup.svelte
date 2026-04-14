<script lang="ts">
  import { artifactPath, buildFallbackNostrUrl } from '$lib/ndk/artifacts';
  import type { CommunitySummary } from '$lib/ndk/groups';
  import type { HighlightSourceGroup } from './grouping';
  import HighlightCard from './HighlightCard.svelte';

  let {
    group,
    communities = [],
    showShareControl = false
  }: {
    group: HighlightSourceGroup;
    communities?: CommunitySummary[];
    showShareControl?: boolean;
  } = $props();

  const artifact = $derived(group.artifact);
  const leadHighlight = $derived(group.highlights[0]);
  const groupCountLabel = $derived(
    `${group.highlights.length} highlight${group.highlights.length === 1 ? '' : 's'}`
  );
  const artifactPageHref = $derived(
    artifact?.groupId && artifact.id ? artifactPath(artifact.groupId, artifact.id) : ''
  );
  const sourceTitle = $derived.by(() => {
    if (artifact?.title) return artifact.title;
    if (leadHighlight?.sourceUrl) return leadHighlight.sourceUrl.replace(/^https?:\/\//, '');
    if (leadHighlight?.artifactAddress) return 'Nostr article';
    if (leadHighlight?.eventReference) return 'Referenced event';
    return 'Unknown source';
  });
  const sourceHref = $derived.by(() => {
    if (artifact?.url) return artifact.url;
    if (leadHighlight?.sourceUrl) return leadHighlight.sourceUrl;
    if (leadHighlight?.artifactAddress) return buildFallbackNostrUrl(leadHighlight.artifactAddress);
    return '';
  });
  const sourceMeta = $derived.by(() => {
    const values: string[] = [];

    if (artifact?.source) values.push(artifact.source);
    if (artifact?.domain) values.push(artifact.domain);

    if (!artifact && leadHighlight?.artifactAddress) {
      values.push('nostr');
    }

    return [...new Set(values.filter(Boolean))];
  });
</script>

<section class="highlight-source-group">
  <header class="group-header">
    <div class="group-copy">
      <div class="group-title-row">
        {#if artifactPageHref}
          <a class="group-title" href={artifactPageHref}>{sourceTitle}</a>
        {:else if sourceHref}
          <a class="group-title" href={sourceHref} target="_blank" rel="noreferrer">{sourceTitle}</a>
        {:else}
          <p class="group-title">{sourceTitle}</p>
        {/if}
        <span class="group-count">{groupCountLabel}</span>
      </div>

      {#if sourceMeta.length > 0}
        <div class="group-meta">
          {#each sourceMeta as item (item)}
            <span>{item}</span>
          {/each}
        </div>
      {/if}
    </div>

    <div class="group-actions">
      {#if artifactPageHref}
        <a href={artifactPageHref}>Artifact page</a>
      {/if}
      {#if sourceHref}
        <a href={sourceHref} target="_blank" rel="noreferrer">Open source</a>
      {/if}
    </div>
  </header>

  <div class="group-stack">
    {#each group.highlights as highlight (highlight.eventId)}
      <HighlightCard {highlight} {artifact} {communities} {showShareControl} />
    {/each}
  </div>
</section>

<style>
  .highlight-source-group {
    display: grid;
    gap: 0.9rem;
    padding: 1rem;
    border: 1px solid var(--border);
    border-radius: 1.35rem;
    background:
      linear-gradient(180deg, color-mix(in srgb, var(--surface) 92%, white), var(--surface));
  }

  .group-header,
  .group-actions,
  .group-meta,
  .group-title-row {
    display: flex;
    gap: 0.55rem;
    align-items: center;
    flex-wrap: wrap;
  }

  .group-header {
    justify-content: space-between;
    align-items: start;
  }

  .group-copy {
    display: grid;
    gap: 0.45rem;
  }

  .group-title {
    margin: 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: 1.2rem;
    font-weight: 700;
    line-height: 1.2;
    text-decoration: none;
  }

  .group-title:hover {
    color: var(--accent);
  }

  .group-count,
  .group-meta span,
  .group-actions a {
    display: inline-flex;
    align-items: center;
    min-height: 1.9rem;
    padding: 0 0.65rem;
    border-radius: 999px;
    background: var(--surface-soft);
    color: var(--muted);
    font-size: 0.76rem;
    font-weight: 600;
    text-decoration: none;
  }

  .group-actions a:hover {
    color: var(--accent);
  }

  .group-stack {
    display: grid;
    gap: 0.8rem;
  }

  @media (max-width: 640px) {
    .group-header {
      grid-template-columns: 1fr;
    }
  }
</style>
