<script lang="ts">
  import type { ArtifactRecord } from '$lib/ndk/artifacts';
  import { artifactPath } from '$lib/ndk/artifacts';
  import { ensureClientNdk, ndk } from '$lib/ndk/client';
  import {
    highlightPath,
    shareHighlightToCommunity,
    type HydratedHighlight,
    type HighlightShareRecord
  } from '$lib/ndk/highlights';
  import type { CommunitySummary } from '$lib/ndk/groups';

  let {
    highlight,
    artifact = undefined,
    communities = [],
    showShareControl = false
  }: {
    highlight: HydratedHighlight;
    artifact?: ArtifactRecord | undefined;
    communities?: CommunitySummary[];
    showShareControl?: boolean;
  } = $props();

  let optimisticShares = $state<HighlightShareRecord[]>([]);
  let selectedGroupId = $state('');
  let sharing = $state(false);
  let shareError = $state('');
  let shareStatus = $state('');

  const currentUser = $derived(ndk.$currentUser);
  const isReadOnly = $derived(Boolean(ndk.$sessions?.isReadOnly()));
  const allShares = $derived(
    [...highlight.shares, ...optimisticShares].toSorted(
      (left, right) => (right.createdAt ?? 0) - (left.createdAt ?? 0)
    )
  );
  const sharedGroupIds = $derived(new Set(allShares.map((share) => share.groupId)));
  const shareableCommunities = $derived(
    communities.filter((community) => !sharedGroupIds.has(community.id))
  );
  const canShareAgain = $derived(
    Boolean(showShareControl && currentUser && !isReadOnly && selectedGroupId && !sharing)
  );
  const sourceLabel = $derived(
    artifact ? `${artifact.title} · ${artifact.domain}` : highlight.sourceUrl.replace(/^https?:\/\//, '')
  );
  const primaryShare = $derived(allShares[0]);

  $effect(() => {
    if (!selectedGroupId && shareableCommunities.length > 0) {
      selectedGroupId = shareableCommunities[0].id;
    }
  });

  async function handleShareAgain() {
    if (!canShareAgain) return;

    sharing = true;
    shareError = '';
    shareStatus = '';

    try {
      await ensureClientNdk();

      const result = await shareHighlightToCommunity(ndk, {
        groupId: selectedGroupId,
        highlight
      });

      if (!result.existing) {
        optimisticShares = [result.share, ...optimisticShares];
      }

      shareStatus = result.existing
        ? 'That community already has this highlight.'
        : `Shared to ${selectedGroupId}.`;
      selectedGroupId = '';
    } catch (error) {
      shareError = error instanceof Error ? error.message : 'Could not share the highlight.';
    } finally {
      sharing = false;
    }
  }

  function shortPubkey(value: string): string {
    if (!value) return '';
    return `${value.slice(0, 8)}…${value.slice(-4)}`;
  }
</script>

<article class="highlight-card">
  {#if primaryShare}
    <div class="highlight-header">
      <a class="share-link" href={highlightPath(primaryShare.groupId, highlight.eventId)}>
        Public card
      </a>
    </div>
  {/if}

  <blockquote class="highlight-quote">
    <p>{highlight.quote}</p>
  </blockquote>

  {#if highlight.context}
    <p class="highlight-context">{highlight.context}</p>
  {/if}

  {#if highlight.note}
    <p class="highlight-note">{highlight.note}</p>
  {/if}

  <div class="highlight-meta">
    <span>{shortPubkey(highlight.pubkey)}</span>
    {#if sourceLabel}
      <span>{sourceLabel}</span>
    {/if}
    {#if artifact}
      <a href={artifactPath(artifact.groupId, artifact.id)}>Artifact</a>
    {/if}
    {#if highlight.shareCount > 0}
      <span>{highlight.shareCount} communit{highlight.shareCount === 1 ? 'y' : 'ies'}</span>
    {/if}
  </div>

  {#if showShareControl && communities.length > 0}
    <div class="share-again">
      <label>
        <span>Share again</span>
        <select bind:value={selectedGroupId} disabled={shareableCommunities.length === 0 || sharing}>
          {#if shareableCommunities.length === 0}
            <option value="">Already shared everywhere loaded here</option>
          {:else}
            {#each shareableCommunities as community (community.id)}
              <option value={community.id}>{community.name}</option>
            {/each}
          {/if}
        </select>
      </label>

      <button type="button" disabled={!canShareAgain} onclick={handleShareAgain}>
        {sharing ? 'Sharing…' : 'Share'}
      </button>
    </div>

    {#if shareError}
      <p class="error">{shareError}</p>
    {/if}

    {#if shareStatus}
      <p class="status">{shareStatus}</p>
    {/if}
  {/if}
</article>

<style>
  .highlight-card {
    display: grid;
    gap: 0.9rem;
    padding: 1.1rem 1.15rem 1.15rem;
    border: 1px solid var(--border);
    border-radius: 1.2rem;
    background: var(--surface);
  }

  .highlight-header,
  .highlight-meta,
  .share-again {
    display: flex;
    gap: 0.55rem;
    align-items: center;
    flex-wrap: wrap;
    justify-content: flex-start;
  }

  .share-link,
  .highlight-meta span,
  .highlight-meta a {
    display: inline-flex;
    align-items: center;
    min-height: 1.9rem;
    padding: 0 0.65rem;
    border-radius: 999px;
    background: var(--surface-soft);
    color: var(--muted);
    font-size: 0.76rem;
    font-weight: 600;
  }

  .share-link {
    color: var(--accent);
  }

  .highlight-quote {
    margin: 0;
    padding: 0 0 0 1rem;
    border-left: 2px solid var(--accent);
  }

  .highlight-quote p {
    margin: 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: 1.15rem;
    line-height: 1.55;
  }

  .highlight-context,
  .highlight-note,
  .error,
  .status {
    margin: 0;
    line-height: 1.6;
  }

  .highlight-context {
    color: var(--muted);
  }

  .highlight-note {
    color: var(--text);
  }

  .share-again {
    justify-content: flex-start;
  }

  .share-again label {
    display: grid;
    gap: 0.35rem;
    min-width: min(100%, 18rem);
  }

  .share-again label span {
    color: var(--text-strong);
    font-size: 0.82rem;
    font-weight: 700;
  }

  .share-again select {
    min-height: 2.5rem;
    border: 1px solid var(--border);
    border-radius: 0.9rem;
    background: white;
    color: var(--text);
    padding: 0 0.85rem;
  }

  .share-again button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: 2.5rem;
    padding: 0 0.95rem;
    border: 0;
    border-radius: 999px;
    background: var(--accent);
    color: white;
    font-weight: 700;
  }

  .share-again button:disabled {
    opacity: 0.55;
  }

  .error {
    color: #b42318;
    font-size: 0.88rem;
  }

  .status {
    color: #0f766e;
    font-size: 0.88rem;
  }
</style>
