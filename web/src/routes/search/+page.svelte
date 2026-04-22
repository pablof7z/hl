<script lang="ts">
  import type { PageProps } from './$types';
  import RoomCard from '$lib/features/groups/RoomCard.svelte';
  import { MIN_SEARCH_QUERY_LENGTH } from '$lib/search';

  let { data }: PageProps = $props();

  const communityCount = $derived(data.results.communities.length);
  const articleCount = $derived(data.results.articles.length);
  const totalCount = $derived(communityCount + articleCount);
  const hasResults = $derived(data.results.communities.length > 0 || data.results.articles.length > 0);
</script>

<svelte:head>
  <title>{data.results.query ? `Search: ${data.results.query} — Highlighter` : 'Search — Highlighter'}</title>
</svelte:head>

<section class="search-page">
  <header class="search-header">
    {#if data.results.query.length < MIN_SEARCH_QUERY_LENGTH}
      <h1 class="search-title">Search.</h1>
    {:else}
      <h1 class="search-title">Results for <em>"{data.results.query}"</em></h1>
    {/if}
    <p class="search-lead">Find rooms and articles across Highlighter.</p>
  </header>

  {#if data.results.query.length < MIN_SEARCH_QUERY_LENGTH}
    <div class="search-message">
      <p class="message-title">Type at least {MIN_SEARCH_QUERY_LENGTH} characters in the header search.</p>
      <p class="message-copy">
        Room names, route slugs, article titles, summaries, and article body text are all searchable.
      </p>
    </div>
  {:else if !hasResults}
    <div class="search-message">
      <p class="message-title">Nothing matched "{data.results.query}".</p>
      <p class="message-copy">
        Try a broader phrase, a room route slug, or a few words from the article title or body.
      </p>
    </div>
  {:else}
    <p class="search-summary">
      {totalCount} result{totalCount === 1 ? '' : 's'} · {communityCount} room{communityCount === 1
        ? ''
        : 's'} · {articleCount} article{articleCount === 1 ? '' : 's'}
    </p>

    {#if data.results.communities.length > 0}
      <section class="result-section">
        <div class="result-section-head">
          <h2 class="result-section-title">
            {data.results.communities.length} public room{data.results.communities.length === 1 ? '' : 's'}
          </h2>
        </div>

        <div class="community-grid">
          {#each data.results.communities as community (community.id)}
            <RoomCard {community} showRoute={true} />
          {/each}
        </div>
      </section>
    {/if}

    {#if data.results.articles.length > 0}
      <section class="result-section">
        <div class="result-section-head">
          <h2 class="result-section-title">
            {data.results.articles.length} Nostr article{data.results.articles.length === 1 ? '' : 's'}
          </h2>
        </div>

        <div class="article-results">
          {#each data.results.articles as article (article.id)}
            <a class="article-result-card" href={`/note/${encodeURIComponent(article.noteIdentifier)}`}>
              <div class="article-result-copy">
                <div class="article-result-meta">
                  <span>{article.publishedLabel}</span>
                  <span>By {article.authorName}</span>
                </div>

                <h3 class="article-result-title">{article.title}</h3>
                <p class="article-result-summary">{article.summary}</p>
              </div>

              {#if article.image}
                <img src={article.image} alt="" loading="lazy" />
              {/if}
            </a>
          {/each}
        </div>
      </section>
    {/if}
  {/if}
</section>

<style>
  .search-page {
    display: grid;
    gap: 2rem;
    padding: 56px 0 80px;
  }

  /* ── Header ── */

  .search-header {
    padding-bottom: 32px;
    border-bottom: 1px solid var(--rule);
    margin-bottom: 12px;
  }

  .search-title {
    font-family: var(--font-serif);
    font-weight: 400;
    font-size: clamp(44px, 6vw, 68px);
    line-height: 1.02;
    letter-spacing: -0.025em;
    color: var(--ink);
    margin: 0 0 14px;
  }

  .search-title em {
    font-style: italic;
    color: var(--brand-accent);
  }

  .search-lead {
    font-family: var(--font-serif);
    font-style: italic;
    font-size: 19px;
    line-height: 1.5;
    color: var(--ink-soft);
    max-width: 52ch;
    margin: 0;
  }

  /* ── Summary line ── */

  .search-summary {
    margin: 0;
    font-family: var(--font-mono);
    font-size: 11px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--ink-fade);
  }

  /* ── Empty / message states ── */

  .search-message {
    background: var(--surface);
    border: 1px solid var(--rule);
    border-radius: var(--radius);
    padding: 44px 32px;
    max-width: 40rem;
  }

  .message-title {
    margin: 0 0 8px;
    font-family: var(--font-serif);
    font-size: 20px;
    font-weight: 500;
    color: var(--ink);
  }

  .message-copy {
    margin: 0;
    font-family: var(--font-sans);
    font-size: 15px;
    line-height: 1.6;
    color: var(--ink-soft);
  }

  /* ── Result sections ── */

  .result-section {
    display: grid;
    gap: 1rem;
  }

  .result-section-head {
    display: flex;
    align-items: end;
    justify-content: space-between;
    gap: 1rem;
    flex-wrap: wrap;
  }

  .result-section-title {
    font-family: var(--font-serif);
    font-weight: 400;
    font-size: clamp(1.5rem, 2.5vw, 1.8rem);
    line-height: 1.1;
    letter-spacing: -0.015em;
    color: var(--ink);
    margin: 0;
  }

  /* ── Room grid ── */

  .community-grid {
    display: grid;
    gap: 1rem;
    grid-template-columns: repeat(auto-fit, minmax(18rem, 1fr));
  }

  /* ── Article results ── */

  .article-results {
    display: grid;
    gap: 0;
  }

  .article-result-card {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 1rem;
    align-items: start;
    padding: 1.25rem 0;
    border-bottom: 1px solid var(--rule);
    color: inherit;
    text-decoration: none;
    transition: background 120ms ease;
  }

  .article-result-card:first-child {
    border-top: 1px solid var(--rule);
  }

  .article-result-card:hover,
  .article-result-card:focus-visible {
    background: color-mix(in srgb, var(--brand-accent) 4%, transparent);
  }

  .article-result-copy {
    display: grid;
    gap: 0.5rem;
    min-width: 0;
  }

  .article-result-meta {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
  }

  .article-result-meta span {
    display: inline-flex;
    align-items: center;
    min-height: 1.8rem;
    padding: 0 0.65rem;
    border-radius: var(--radius-pill, 999px);
    background: var(--surface-muted, var(--surface));
    border: 1px solid var(--rule);
    color: var(--ink-fade);
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .article-result-title {
    font-family: var(--font-serif);
    font-weight: 500;
    font-size: 1.15rem;
    line-height: 1.25;
    letter-spacing: -0.01em;
    color: var(--ink);
    margin: 0;
  }

  .article-result-summary {
    margin: 0;
    font-family: var(--font-sans);
    font-size: 14px;
    line-height: 1.6;
    color: var(--ink-soft);
  }

  .article-result-card img {
    width: 6rem;
    height: 4.5rem;
    object-fit: cover;
    border-radius: var(--radius);
  }

  @media (max-width: 760px) {
    .article-result-card img {
      width: 4.5rem;
      height: 3.5rem;
    }
  }
</style>
