<script lang="ts">
  import { browser } from '$app/environment';
  import { NDKEvent } from '@nostr-dev-kit/ndk';
  import StoryAuthor from '$lib/components/StoryAuthor.svelte';
  import { ndk } from '$lib/ndk/client';
  import { articleTitle, articleSummary, noteExcerpt } from '$lib/ndk/format';

  // Subscribe to recent highlights; meta-subscription resolves the articles they point to
  const highlightedArticles = ndk.$metaSubscribe(() => {
    if (!browser) return undefined;
    return {
      filters: [{ kinds: [9802], limit: 100 }],
      sort: 'unique-authors'
    };
  });

  function articleHighlights(article: NDKEvent): NDKEvent[] {
    return highlightedArticles.eventsTagging(article);
  }

  function bestQuote(article: NDKEvent): string {
    const highlights = articleHighlights(article);
    if (!highlights.length) return '';

    const contentCounts = new Map<string, number>();
    for (const h of highlights) {
      const content = h.content.trim();
      if (!content) continue;
      contentCounts.set(content, (contentCounts.get(content) ?? 0) + 1);
    }

    let quote = '';
    let bestCount = 0;
    for (const [content, count] of contentCounts) {
      if (count > bestCount || (count === bestCount && content.length > quote.length)) {
        quote = content;
        bestCount = count;
      }
    }
    return quote || highlights[0].content.trim();
  }

  function uniqueHighlighters(article: NDKEvent): number {
    return new Set(articleHighlights(article).map((h) => h.pubkey)).size;
  }

  function sampleQuotes(article: NDKEvent, max: number): string[] {
    const seen = new Set<string>();
    const quotes: string[] = [];
    for (const h of articleHighlights(article)) {
      const content = h.content.trim();
      if (!content || seen.has(content)) continue;
      seen.add(content);
      quotes.push(content);
      if (quotes.length >= max) break;
    }
    return quotes;
  }

  const entries = $derived(highlightedArticles.events);
  const heroEntry = $derived(entries[0] ?? null);
  const gridEntries = $derived(entries.slice(1));
  const sidebarEntries = $derived(entries.slice(0, 8));
</script>

<div class="hl-page">
  {#if heroEntry}
    <section class="hl-hero">
      <a class="hl-hero-card" href={`/note/${heroEntry.encode()}`}>
        <blockquote class="hl-hero-quote">
          {noteExcerpt(bestQuote(heroEntry), 500)}
        </blockquote>
        <div class="hl-hero-info">
          <span class="hl-hero-title">{articleTitle(heroEntry.rawEvent())}</span>
          <div class="hl-hero-byline">
            <StoryAuthor
              {ndk}
              pubkey={heroEntry.pubkey}
              avatarClass="article-author-avatar article-author-avatar-compact"
              compact
            />
            <span class="hl-badge">
              {articleHighlights(heroEntry).length} highlight{articleHighlights(heroEntry).length === 1 ? '' : 's'}
              · {uniqueHighlighters(heroEntry)} reader{uniqueHighlighters(heroEntry) === 1 ? '' : 's'}
            </span>
          </div>
        </div>
      </a>
    </section>
  {/if}

  <div class="hl-body">
    <div class="hl-main">
      {#if entries.length === 0}
        <p class="muted">No highlights yet.</p>
      {:else}
        <div class="hl-grid">
          {#each gridEntries as article, i (article.tagId())}
            {@const isWide = i % 3 === 0}
            {@const quotes = sampleQuotes(article, isWide ? 3 : 2)}
            {@const hlCount = articleHighlights(article).length}
            <a
              class="hl-card"
              class:hl-card-wide={isWide}
              href={`/note/${article.encode()}`}
            >
              <div class="hl-card-header">
                <span class="hl-card-title">{articleTitle(article.rawEvent())}</span>
                <p class="hl-card-summary">{articleSummary(article.rawEvent(), 120)}</p>
                <div class="hl-card-byline">
                  <StoryAuthor
                    {ndk}
                    pubkey={article.pubkey}
                    avatarClass="article-author-avatar article-author-avatar-compact"
                    compact
                  />
                </div>
              </div>

              <div class="hl-card-quotes">
                {#each quotes as quote}
                  <blockquote class="hl-card-quote">
                    {noteExcerpt(quote, isWide ? 280 : 160)}
                  </blockquote>
                {/each}
              </div>

              <div class="hl-card-footer">
                <span class="hl-badge">
                  {hlCount} highlight{hlCount === 1 ? '' : 's'}
                </span>
                <span class="hl-readers">
                  {uniqueHighlighters(article)} reader{uniqueHighlighters(article) === 1 ? '' : 's'}
                </span>
              </div>
            </a>
          {/each}
        </div>
      {/if}
    </div>

    {#if sidebarEntries.length > 0}
      <aside class="hl-sidebar">
        <span class="hl-sidebar-heading">Most highlighted</span>
        <div class="hl-sidebar-list">
          {#each sidebarEntries as article (article.tagId())}
            <a class="hl-sidebar-item" href={`/note/${article.encode()}`}>
              <blockquote class="hl-sidebar-quote">
                {noteExcerpt(bestQuote(article), 100)}
              </blockquote>
              <span class="hl-sidebar-title">{articleTitle(article.rawEvent())}</span>
              <div class="hl-sidebar-meta">
                <span class="hl-sidebar-author">
                  <StoryAuthor
                    {ndk}
                    pubkey={article.pubkey}
                    avatarClass="article-author-avatar article-author-avatar-compact"
                    compact
                  />
                </span>
                <span class="hl-sidebar-dot">·</span>
                <span class="hl-sidebar-count">{articleHighlights(article).length} highlights</span>
              </div>
            </a>
          {/each}
        </div>
      </aside>
    {/if}
  </div>
</div>

<style>
  .hl-page {
    display: grid;
    gap: 2.5rem;
  }

  .hl-body {
    display: grid;
    grid-template-columns: 1fr 20rem;
    gap: 3rem;
    align-items: start;
  }

  .hl-main {
    min-width: 0;
  }

  /* ── hero ───────────────────────────────────────────────────── */

  .hl-hero-card {
    display: grid;
    gap: 1.25rem;
    padding: 2.25rem 2.5rem;
    background: var(--pale-blue);
    border-left: 4px solid rgba(31, 108, 159, 0.45);
    border-radius: 0 var(--radius-md) var(--radius-md) 0;
    color: inherit;
    text-decoration: none;
    transition: border-left-color 200ms ease;
  }

  .hl-hero-card:hover {
    border-left-color: var(--accent);
  }

  .hl-hero-quote {
    margin: 0;
    font-family: var(--font-serif);
    font-size: clamp(1.3rem, 2.2vw, 1.85rem);
    font-weight: 400;
    line-height: 1.5;
    color: var(--text-strong);
  }

  .hl-hero-info {
    display: grid;
    gap: 0.65rem;
  }

  .hl-hero-title {
    font-size: 1.05rem;
    font-weight: 600;
    color: var(--accent);
    transition: color 160ms ease;
  }

  .hl-hero-card:hover .hl-hero-title {
    color: var(--accent-hover);
  }

  .hl-hero-byline {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.75rem;
  }

  /* ── badge ──────────────────────────────────────────────────── */

  .hl-badge {
    display: inline-flex;
    align-items: center;
    padding: 0.2rem 0.55rem;
    border-radius: 9999px;
    background: rgba(31, 108, 159, 0.1);
    color: var(--pale-blue-text);
    font-size: 0.72rem;
    font-weight: 600;
    letter-spacing: 0.03em;
    white-space: nowrap;
  }

  .hl-readers {
    font-size: 0.78rem;
    color: var(--muted);
  }

  /* ── grid ───────────────────────────────────────────────────── */

  .hl-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1.25rem;
  }

  .hl-card {
    display: grid;
    gap: 1rem;
    padding: 1.25rem;
    border: 1px solid var(--border-light);
    border-radius: var(--radius-md);
    color: inherit;
    text-decoration: none;
    transition: border-color 200ms ease, box-shadow 200ms ease;
  }

  .hl-card:hover {
    border-color: var(--border);
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.06);
  }

  .hl-card-wide {
    grid-column: 1 / -1;
  }

  .hl-card-header {
    display: grid;
    gap: 0.4rem;
  }

  .hl-card-title {
    font-family: var(--font-serif);
    font-size: 1.1rem;
    font-weight: 700;
    color: var(--text-strong);
    line-height: 1.25;
    letter-spacing: -0.01em;
    transition: color 160ms ease;
  }

  .hl-card:hover .hl-card-title {
    color: var(--accent);
  }

  .hl-card-summary {
    margin: 0;
    color: var(--muted);
    font-size: 0.85rem;
    line-height: 1.5;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .hl-card-byline {
    padding-top: 0.25rem;
  }

  .hl-card-quotes {
    display: grid;
    gap: 0.6rem;
  }

  .hl-card-quote {
    margin: 0;
    padding: 0.65rem 0.85rem;
    border-left: 3px solid rgba(31, 108, 159, 0.3);
    border-radius: 0 var(--radius-sm) var(--radius-sm) 0;
    background: var(--pale-blue);
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: 0.88rem;
    line-height: 1.55;
  }

  .hl-card-footer {
    display: flex;
    align-items: center;
    gap: 0.6rem;
  }

  /* ── sidebar ────────────────────────────────────────────────── */

  .hl-sidebar {
    position: sticky;
    top: 5rem;
    display: grid;
    gap: 1rem;
  }

  .hl-sidebar-heading {
    font-family: var(--font-sans);
    font-size: 0.72rem;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--muted);
  }

  .hl-sidebar-list {
    display: grid;
  }

  .hl-sidebar-item {
    display: grid;
    gap: 0.4rem;
    padding: 0.85rem 0;
    border-bottom: 1px solid var(--border-light);
    color: inherit;
    text-decoration: none;
  }

  .hl-sidebar-item:first-child {
    padding-top: 0;
  }

  .hl-sidebar-item:last-child {
    border-bottom: none;
  }

  .hl-sidebar-quote {
    margin: 0;
    padding: 0.45rem 0.6rem;
    border-left: 2px solid rgba(31, 108, 159, 0.3);
    background: var(--surface-soft);
    color: var(--text);
    font-family: var(--font-serif);
    font-size: 0.8rem;
    line-height: 1.45;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .hl-sidebar-title {
    font-size: 0.82rem;
    font-weight: 600;
    color: var(--text-strong);
    line-height: 1.3;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    transition: color 160ms ease;
  }

  .hl-sidebar-item:hover .hl-sidebar-title {
    color: var(--accent);
  }

  .hl-sidebar-meta {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    font-size: 0.72rem;
    color: var(--muted);
  }

  .hl-sidebar-author {
    min-width: 0;
  }

  .hl-sidebar-author :global(.registry-user-avatar),
  .hl-sidebar-author :global(.story-author-handle) {
    display: none;
  }

  .hl-sidebar-author :global(.story-author-link) {
    gap: 0;
  }

  .hl-sidebar-author :global(.story-author-name) {
    font-size: 0.72rem;
    font-weight: 500;
    color: var(--muted);
  }

  .hl-sidebar-dot {
    color: var(--border);
  }

  .hl-sidebar-count {
    white-space: nowrap;
  }

  /* ── responsive ─────────────────────────────────────────────── */

  @media (max-width: 900px) {
    .hl-body {
      grid-template-columns: 1fr;
    }

    .hl-sidebar {
      position: static;
    }
  }

  @media (max-width: 720px) {
    .hl-grid {
      grid-template-columns: 1fr;
    }

    .hl-card-wide {
      grid-column: auto;
    }

    .hl-hero-card {
      padding: 1.5rem;
    }

    .hl-hero-quote {
      font-size: 1.2rem;
    }
  }
</style>
