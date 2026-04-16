<script lang="ts">
  import { ensureClientNdk, ndk } from '$lib/ndk/client';
  import { buildFallbackNostrUrl, publishArtifact } from '$lib/ndk/artifacts';
  import type { CommunitySummary } from '$lib/ndk/groups';
  import {
    removeForLaterArtifact,
    type ForLaterItem
  } from './vault';

  let {
    item,
    communities = [],
    onRemoved = undefined
  }: {
    item: ForLaterItem;
    communities?: CommunitySummary[];
    onRemoved?: ((id: string) => void) | undefined;
  } = $props();

  let selectedGroupId = $state('');
  let sharing = $state(false);
  let removing = $state(false);
  let actionError = $state('');
  let statusMessage = $state('');

  const currentUser = $derived(ndk.$currentUser);
  const isReadOnly = $derived(Boolean(ndk.$sessions?.isReadOnly()));
  const sourceHref = $derived(
    item.url || (item.bookmarkTagName === 'a' ? buildFallbackNostrUrl(item.bookmarkTagValue) : '')
  );
  const bookmarkLabel = $derived(`${item.bookmarkTagName} tag`);
  const canShare = $derived(
    Boolean(currentUser && !isReadOnly && selectedGroupId && !sharing && communities.length > 0)
  );

  $effect(() => {
    if (!selectedGroupId && communities.length > 0) {
      selectedGroupId = communities[0].id;
    }
  });

  async function handleMoveToCommunity() {
    if (!canShare) {
      return;
    }

    sharing = true;
    actionError = '';
    statusMessage = '';

    try {
      await ensureClientNdk();

      const result = await publishArtifact(ndk, {
        groupId: selectedGroupId,
        preview: item,
        note: ''
      });

      const communityName =
        communities.find((community) => community.id === selectedGroupId)?.name ?? selectedGroupId;
      statusMessage = result.existing
        ? `${communityName} already had this source.`
        : `Shared into ${communityName}.`;
    } catch (error) {
      actionError = error instanceof Error ? error.message : 'Could not move this item yet.';
    } finally {
      sharing = false;
    }
  }

  async function handleRemove() {
    removing = true;
    actionError = '';
    statusMessage = '';

    try {
      await removeForLaterArtifact(item);
      onRemoved?.(item.bookmarkKey);
    } catch (error) {
      actionError = error instanceof Error ? error.message : 'Could not remove this item.';
    } finally {
      removing = false;
    }
  }
</script>

<article class="for-later-card">
  <div class="card-media">
    {#if item.image}
      <img src={item.image} alt="" loading="lazy" />
    {:else}
      <div class="card-fallback">
        <span>{item.domain.charAt(0).toUpperCase() || '#'}</span>
      </div>
    {/if}
  </div>

  <div class="card-copy">
    <div class="card-topline">
      <div class="card-tags">
        <span class="meta-chip">{bookmarkLabel}</span>
        <span class="meta-chip">{item.source}</span>
        <span class="meta-chip">{item.domain}</span>
      </div>

      <div class="card-links">
        {#if sourceHref}
          <a href={sourceHref} target="_blank" rel="noreferrer">Open source</a>
        {/if}
      </div>
    </div>

    <div class="card-body">
      <h2>{item.title}</h2>
      {#if item.author}
        <p class="author">{item.author}</p>
      {/if}
      {#if item.description}
        <p class="description">{item.description}</p>
      {/if}
    </div>

    <div class="card-share">
      <div class="card-section-header">
        <span>Actions</span>
      </div>

      <div class="card-actions card-actions-share">
        {#if communities.length > 0}
          <select bind:value={selectedGroupId} disabled={sharing}>
            {#each communities as community (community.id)}
              <option value={community.id}>{community.name}</option>
            {/each}
          </select>

          <button type="button" class="primary" disabled={!canShare} onclick={handleMoveToCommunity}>
            {sharing ? 'Sharing…' : 'Share to community'}
          </button>
        {/if}
        <button type="button" class="ghost" disabled={removing} onclick={handleRemove}>
          {removing ? 'Removing…' : 'Remove bookmark'}
        </button>
      </div>
    </div>

    {#if actionError}
      <p class="feedback error">{actionError}</p>
    {/if}

    {#if statusMessage}
      <p class="feedback status">{statusMessage}</p>
    {/if}
  </div>
</article>

<style>
  .for-later-card {
    display: grid;
    grid-template-columns: minmax(160px, 220px) minmax(0, 1fr);
    gap: 1.1rem;
    padding: 1.1rem;
    border: 1px solid var(--border);
    border-radius: 1.35rem;
    background:
      linear-gradient(180deg, color-mix(in srgb, var(--surface) 92%, white), var(--surface));
  }

  .card-media {
    width: 100%;
    aspect-ratio: 4 / 5;
    overflow: hidden;
    border-radius: 1rem;
    background: linear-gradient(160deg, rgba(255, 103, 25, 0.12), rgba(255, 103, 25, 0.04));
  }

  .card-media img,
  .card-fallback {
    width: 100%;
    height: 100%;
  }

  .card-media img {
    object-fit: cover;
  }

  .card-fallback {
    display: grid;
    place-items: center;
    color: var(--accent);
    font-size: 1.65rem;
    font-weight: 700;
  }

  .card-copy,
  .card-body,
  .card-share {
    display: grid;
    gap: 0.7rem;
  }

  .card-topline,
  .card-links,
  .card-tags,
  .card-actions,
  .card-actions-share,
  .card-section-header {
    display: flex;
    gap: 0.55rem;
    flex-wrap: wrap;
    align-items: center;
  }

  .card-topline {
    justify-content: space-between;
    align-items: start;
  }

  .meta-chip,
  .card-links a {
    display: inline-flex;
    align-items: center;
    min-height: 1.85rem;
    padding: 0 0.65rem;
    border-radius: 999px;
    background: var(--surface-soft);
    color: var(--muted);
    font-size: 0.74rem;
    font-weight: 700;
    text-decoration: none;
  }

  .card-links a:hover {
    color: var(--accent);
  }

  h2 {
    margin: 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: clamp(1.3rem, 2vw, 1.7rem);
    line-height: 1.15;
    letter-spacing: -0.02em;
  }

  .author,
  .description,
  .feedback {
    margin: 0;
    color: var(--muted);
    line-height: 1.6;
  }

  .card-section-header span:first-child {
    color: var(--text-strong);
    font-size: 0.88rem;
    font-weight: 700;
  }

  button,
  select {
    font: inherit;
  }

  select {
    width: 100%;
    min-width: min(22rem, 100%);
    border: 1px solid var(--border);
    border-radius: 0.95rem;
    background: white;
    color: var(--text);
    padding: 0.8rem 0.9rem;
  }

  button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: 2.8rem;
    padding: 0 1rem;
    border-radius: 999px;
    border: 1px solid var(--border);
    cursor: pointer;
    font-weight: 700;
  }

  button.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: white;
  }

  button.ghost {
    background: var(--surface);
    color: var(--text);
  }

  button:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .feedback.error {
    color: #b42318;
  }

  .feedback.status {
    color: var(--muted);
  }

  @media (max-width: 820px) {
    .for-later-card {
      grid-template-columns: 1fr;
    }

    .card-media {
      aspect-ratio: 16 / 9;
    }
  }
</style>
