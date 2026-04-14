<script lang="ts">
  import type { ArtifactRecord } from '$lib/ndk/artifacts';
  import { artifactPath, buildFallbackNostrUrl } from '$lib/ndk/artifacts';
  import { ensureClientNdk, ndk } from '$lib/ndk/client';
  import { User } from '$lib/ndk/ui/user';
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
  const primaryShare = $derived(allShares[0]);
  const excerptSegments = $derived(buildExcerptSegments(highlight.context, highlight.quote));
  const createdLabel = $derived(
    highlight.createdAt
      ? new Intl.DateTimeFormat('en', {
          month: 'short',
          day: 'numeric',
          year: 'numeric'
        }).format(new Date(highlight.createdAt * 1000))
      : ''
  );
  const artifactPageHref = $derived(
    artifact?.groupId && artifact.id ? artifactPath(artifact.groupId, artifact.id) : ''
  );
  const sourceTitle = $derived.by(() => {
    if (artifact?.title) return artifact.title;
    if (highlight.sourceUrl) return highlight.sourceUrl.replace(/^https?:\/\//, '');
    if (highlight.artifactAddress) return 'Nostr article';
    if (highlight.eventReference) return 'Referenced event';
    return 'Unknown source';
  });
  const sourceHref = $derived.by(() => {
    if (artifact?.url) return artifact.url;
    if (highlight.sourceUrl) return highlight.sourceUrl;
    if (highlight.artifactAddress) return buildFallbackNostrUrl(highlight.artifactAddress);
    return '';
  });
  const sourceMeta = $derived.by(() => {
    const values: string[] = [];

    if (artifact?.source) values.push(artifact.source);
    if (artifact?.domain) values.push(artifact.domain);
    if (!artifact && highlight.artifactAddress) values.push('nostr');

    return [...new Set(values.filter(Boolean))];
  });

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

      const selectedCommunity =
        communities.find((community) => community.id === selectedGroupId)?.name ?? selectedGroupId;
      shareStatus = result.existing
        ? `${selectedCommunity} already has this highlight.`
        : `Shared to ${selectedCommunity}.`;
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

  function buildExcerptSegments(context: string, quote: string): Array<{ text: string; marked: boolean }> {
    const normalizedContext = compactWhitespace(context);
    const normalizedQuote = compactWhitespace(quote);

    if (!normalizedContext && !normalizedQuote) {
      return [];
    }

    if (!normalizedContext || normalizedContext.toLocaleLowerCase() === normalizedQuote.toLocaleLowerCase()) {
      return [{ text: normalizedQuote || normalizedContext, marked: true }];
    }

    const matchIndex = normalizedContext.toLocaleLowerCase().indexOf(normalizedQuote.toLocaleLowerCase());
    if (normalizedQuote && matchIndex >= 0) {
      return [
        { text: normalizedContext.slice(0, matchIndex), marked: false },
        {
          text: normalizedContext.slice(matchIndex, matchIndex + normalizedQuote.length),
          marked: true
        },
        {
          text: normalizedContext.slice(matchIndex + normalizedQuote.length),
          marked: false
        }
      ].filter((segment) => segment.text.length > 0);
    }

    if (!normalizedQuote) {
      return [{ text: normalizedContext, marked: false }];
    }

    return [
      { text: normalizedContext, marked: false },
      { text: ' … ', marked: false },
      { text: normalizedQuote, marked: true }
    ];
  }

  function compactWhitespace(value: string): string {
    return value.trim().replace(/\s+/g, ' ');
  }
</script>

<article class="highlight-card">
  <div class="highlight-header">
    <div class="highlight-byline">
      <User.Root {ndk} pubkey={highlight.pubkey}>
        <a class="author-link" href={`/profile/${highlight.pubkey}`}>
          <User.Name fallback={shortPubkey(highlight.pubkey)} />
        </a>
      </User.Root>

      {#if createdLabel}
        <span class="created-at">{createdLabel}</span>
      {/if}
    </div>

    {#if primaryShare}
      <a class="share-link" href={highlightPath(primaryShare.groupId, highlight.eventId)}>
        Public card
      </a>
    {/if}
  </div>

  <blockquote class="highlight-excerpt">
    <p>
      {#each excerptSegments as segment (segment.text)}
        {#if segment.marked}
          <mark>{segment.text}</mark>
        {:else}
          {segment.text}
        {/if}
      {/each}
    </p>
  </blockquote>

  {#if highlight.note}
    <p class="highlight-note">{highlight.note}</p>
  {/if}

  <div class="highlight-footer">
    <div class="highlight-source">
      {#if artifactPageHref}
        <a class="artifact-link" href={artifactPageHref}>{sourceTitle}</a>
      {:else if sourceHref}
        <a class="artifact-link" href={sourceHref} target="_blank" rel="noreferrer">{sourceTitle}</a>
      {:else}
        <span class="artifact-link static">{sourceTitle}</span>
      {/if}

      {#each sourceMeta as item (item)}
        <span class="meta-chip">{item}</span>
      {/each}

      {#if highlight.shareCount > 0}
        <span class="meta-chip">
          {highlight.shareCount} communit{highlight.shareCount === 1 ? 'y' : 'ies'}
        </span>
      {/if}
    </div>

    {#if sourceHref}
      <a class="source-link" href={sourceHref} target="_blank" rel="noreferrer">Open source</a>
    {/if}
  </div>

  {#if showShareControl && communities.length > 0}
    <div class="share-again">
      <select bind:value={selectedGroupId} disabled={shareableCommunities.length === 0 || sharing}>
        {#if shareableCommunities.length === 0}
          <option value="">Already shared everywhere loaded here</option>
        {:else}
          {#each shareableCommunities as community (community.id)}
            <option value={community.id}>{community.name}</option>
          {/each}
        {/if}
      </select>

      <button type="button" disabled={!canShareAgain} onclick={handleShareAgain}>
        {sharing ? 'Sharing…' : 'Share to community'}
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
    background: color-mix(in srgb, var(--surface) 90%, white);
  }

  .highlight-header,
  .highlight-footer,
  .highlight-source,
  .share-again {
    display: flex;
    gap: 0.55rem;
    align-items: center;
    flex-wrap: wrap;
    justify-content: flex-start;
  }

  .highlight-header {
    justify-content: space-between;
  }

  .highlight-byline {
    display: flex;
    gap: 0.5rem;
    align-items: baseline;
    flex-wrap: wrap;
  }

  .author-link {
    color: var(--text-strong);
    font-size: 0.84rem;
    font-weight: 700;
    text-decoration: none;
  }

  .author-link:hover {
    color: var(--accent);
  }

  .created-at,
  .share-link,
  .meta-chip,
  .source-link {
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

  .share-link {
    color: var(--accent);
  }

  .highlight-excerpt {
    margin: 0;
    padding: 0 0 0 1rem;
    border-left: 2px solid var(--accent);
  }

  .highlight-excerpt p {
    margin: 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: 1.15rem;
    line-height: 1.55;
  }

  .highlight-excerpt mark {
    background: color-mix(in srgb, var(--accent) 18%, white);
    color: inherit;
    padding: 0.06em 0.14em;
    border-radius: 0.2em;
  }

  .highlight-note,
  .error,
  .status {
    margin: 0;
    line-height: 1.6;
  }

  .highlight-note {
    color: var(--text);
  }

  .highlight-footer {
    justify-content: space-between;
    align-items: start;
  }

  .highlight-source {
    justify-content: flex-start;
  }

  .artifact-link {
    color: var(--text-strong);
    font-size: 0.86rem;
    font-weight: 700;
    text-decoration: none;
  }

  .artifact-link:hover {
    color: var(--accent);
  }

  .artifact-link.static {
    pointer-events: none;
  }

  .share-again select {
    min-height: 2.5rem;
    border: 1px solid var(--border);
    border-radius: 0.9rem;
    background: white;
    color: var(--text);
    padding: 0 0.85rem;
    min-width: min(100%, 16rem);
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

  @media (max-width: 640px) {
    .highlight-footer {
      flex-direction: column;
      align-items: stretch;
    }

    .share-again {
      flex-direction: column;
      align-items: stretch;
    }

    .share-again button,
    .share-again select {
      width: 100%;
    }
  }
</style>
