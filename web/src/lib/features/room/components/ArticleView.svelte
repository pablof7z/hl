<script lang="ts">
  import { onMount } from 'svelte';
  import { browser } from '$app/environment';
  import type { ArtifactRecord } from '$lib/ndk/artifacts';
  import { parseNostrAddress } from '$lib/ndk/artifacts';
  import { ensureClientNdk, ndk } from '$lib/ndk/client';
  import { buildArtifactHighlightFilters } from '$lib/ndk/highlights';
  import ArticleMarkdown from '$lib/components/ArticleMarkdown.svelte';
  import { User } from '$lib/ndk/ui/user';

  let {
    artifact,
    roomMemberPubkeys,
    onBack
  }: {
    artifact: ArtifactRecord;
    roomMemberPubkeys: string[];
    onBack: () => void;
  } = $props();

  onMount(() => {
    void ensureClientNdk();
  });

  const nostrRef = $derived.by(() => {
    if (artifact.referenceTagName !== 'a') return undefined;
    const parsed = parseNostrAddress(artifact.referenceTagValue);
    if (!parsed || parsed.kind !== 30023) return undefined;
    return parsed;
  });

  const articleSub = ndk.$subscribe(() => {
    if (!browser || !nostrRef) return undefined;
    return {
      filters: [
        {
          kinds: [nostrRef.kind],
          authors: [nostrRef.pubkey],
          '#d': [nostrRef.identifier],
          limit: 1
        }
      ]
    };
  });

  const articleEvent = $derived(articleSub.events[0]);

  const highlightsSub = ndk.$subscribe(() => {
    if (!browser) return undefined;
    const filters = buildArtifactHighlightFilters([artifact], roomMemberPubkeys);
    if (filters.length === 0) return undefined;
    return { filters };
  });

  const highlightEvents = $derived(highlightsSub.events);

  const resolvedTitle = $derived(articleEvent?.tagValue('title') || artifact.title || 'Untitled');
  const resolvedAuthorPubkey = $derived(articleEvent?.pubkey || '');
  const resolvedCover = $derived(
    artifact.image || articleEvent?.tagValue('image') || ''
  );

  function handleShare() {
    if (typeof navigator !== 'undefined' && navigator.share) {
      void navigator.share({
        title: resolvedTitle,
        url: typeof window !== 'undefined' ? window.location.href : ''
      });
    }
  }

  function handleSaveForLater() {
    console.log('save for later:', artifact.id);
  }
</script>

<article class="article-view">
  <div class="article-nav">
    <button class="back-btn" type="button" onclick={onBack}>
      ← Back to room
    </button>
  </div>

  <header class="article-hero">
    {#if resolvedCover}
      <img class="hero-cover" src={resolvedCover} alt="" loading="eager" />
    {/if}

    <div class="hero-meta">
      <span class="article-kicker">ARTICLE</span>
      <h1 class="article-title">{resolvedTitle}</h1>
      {#if resolvedAuthorPubkey}
        <p class="article-author">
          <User.Root {ndk} pubkey={resolvedAuthorPubkey}>
            <User.Name field="displayName" />
          </User.Root>
        </p>
      {:else if artifact.author}
        <p class="article-author">{artifact.author}</p>
      {/if}
    </div>
  </header>

  <div class="article-body-layout">
    <div class="article-body">
      {#if nostrRef}
        {#if articleEvent}
          <ArticleMarkdown
            content={articleEvent.content}
            tags={articleEvent.tags}
            highlights={highlightEvents}
          />
        {:else}
          <p class="loading-note">Loading article…</p>
        {/if}
      {:else if artifact.url}
        <div class="external-source">
          <p>This artifact points to an external source.</p>
          <a class="external-link" href={artifact.url} target="_blank" rel="noreferrer noopener">
            Read at {artifact.domain || 'source'} ↗
          </a>
        </div>
      {:else}
        <p class="loading-note">No readable source is attached to this artifact.</p>
      {/if}

      <div class="article-footer">
        <button class="save-btn" type="button" onclick={handleSaveForLater}>
          Save for later
        </button>
        <button class="share-link" type="button" onclick={handleShare}>
          Share →
        </button>
      </div>
    </div>

    <aside class="article-margin">
      <div class="margin-card">
        <h3 class="margin-card-title">
          {highlightEvents.length}
          {highlightEvents.length === 1 ? 'highlight' : 'highlights'}
        </h3>
        {#if highlightEvents.length === 0}
          <p class="margin-empty">No highlights from this room yet.</p>
        {:else}
          <ul class="highlight-list">
            {#each highlightEvents.slice(0, 6) as h (h.id)}
              <li class="highlight-item">
                <User.Root {ndk} pubkey={h.pubkey}>
                  <span class="highlight-author"><User.Name field="displayName" /></span>
                </User.Root>
                <blockquote>{h.content}</blockquote>
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    </aside>
  </div>
</article>

<style>
  .article-view {
    display: flex;
    flex-direction: column;
    gap: 32px;
    padding-top: 24px;
    padding-bottom: 80px;
  }

  .article-nav {
    padding: 0;
  }

  .back-btn {
    font-family: var(--font-sans);
    font-size: 14px;
    font-weight: 500;
    color: var(--ink-soft);
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
    transition: color var(--transition);
  }

  .back-btn:hover {
    color: var(--brand-accent);
  }

  .back-btn:focus-visible {
    outline: 2px solid var(--brand-accent);
    outline-offset: 2px;
    border-radius: var(--radius);
  }

  .article-hero {
    display: flex;
    flex-direction: column;
    gap: 40px;
    align-items: flex-start;
  }

  .hero-cover {
    width: 100%;
    max-width: 100%;
    border-radius: var(--radius);
    object-fit: cover;
    aspect-ratio: 16/9;
    flex-shrink: 0;
  }

  .hero-meta {
    display: flex;
    flex-direction: column;
    gap: 12px;
    flex: 1;
    min-width: 0;
    padding-top: 8px;
  }

  .article-kicker {
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 500;
    color: var(--ink-soft);
    text-transform: uppercase;
    letter-spacing: 0.1em;
  }

  .article-title {
    font-family: var(--font-serif);
    font-weight: 400;
    font-size: clamp(32px, 5vw, 56px);
    color: var(--ink);
    line-height: 1.15;
    margin: 0;
  }

  .article-author {
    font-family: var(--font-sans);
    font-size: 15px;
    font-weight: 400;
    color: var(--ink-soft);
    margin: 0;
  }

  .article-body-layout {
    display: grid;
    grid-template-columns: 1fr;
    gap: 40px;
    align-items: start;
  }

  .article-body {
    max-width: 100%;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  .loading-note {
    font-family: var(--font-sans);
    color: var(--ink-fade);
    font-size: 14px;
    margin: 0;
  }

  .external-source {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 24px;
    border: 1px solid var(--rule);
    border-radius: var(--radius);
    background: var(--surface);
  }

  .external-source p {
    margin: 0;
    color: var(--ink-soft);
    font-family: var(--font-sans);
    font-size: 14px;
  }

  .external-link {
    align-self: flex-start;
    font-family: var(--font-sans);
    font-size: 14px;
    font-weight: 500;
    color: var(--brand-accent);
    text-decoration: none;
  }

  .external-link:hover {
    text-decoration: underline;
  }

  .article-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding-top: 24px;
    border-top: 1px solid var(--rule);
    margin-top: 8px;
  }

  .save-btn {
    font-family: var(--font-sans);
    font-size: 14px;
    font-weight: 500;
    color: var(--surface);
    background-color: var(--brand-accent);
    border: none;
    border-radius: var(--radius);
    padding: 10px 20px;
    cursor: pointer;
    transition: opacity var(--transition);
  }

  .save-btn:hover {
    opacity: 0.85;
  }

  .share-link {
    font-family: var(--font-sans);
    font-size: 14px;
    font-weight: 500;
    color: var(--brand-accent);
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
  }

  .share-link:hover {
    text-decoration: underline;
  }

  .article-margin {
    position: static;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .margin-card {
    background-color: var(--surface);
    border: 1px solid var(--rule);
    border-radius: var(--radius);
    padding: 14px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .margin-card-title {
    font-family: var(--font-sans);
    font-size: 11px;
    font-weight: 600;
    color: var(--ink-fade);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    margin: 0;
  }

  .margin-empty {
    margin: 0;
    font-family: var(--font-sans);
    font-size: 12px;
    color: var(--ink-fade);
  }

  .highlight-list {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .highlight-item {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding-bottom: 10px;
    border-bottom: 1px solid var(--rule-soft);
  }

  .highlight-item:last-child {
    border-bottom: none;
    padding-bottom: 0;
  }

  .highlight-author {
    font-family: var(--font-sans);
    font-size: 11px;
    font-weight: 600;
    color: var(--ink);
  }

  .highlight-item blockquote {
    margin: 0;
    font-family: var(--font-serif);
    font-style: italic;
    font-size: 13px;
    color: var(--ink-soft);
    line-height: 1.4;
  }

  @media (min-width: 768px) {
    .article-hero {
      flex-direction: row;
    }

    .hero-cover {
      width: 60%;
      max-width: 520px;
    }

    .article-body {
      max-width: 680px;
      margin: 0 auto;
    }

    .article-body-layout {
      grid-template-columns: 1fr 220px;
    }

    .article-margin {
      position: sticky;
      top: 24px;
    }
  }
</style>
