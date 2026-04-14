<script lang="ts">
  import type { PageProps } from './$types';
  import CommunityCard from '$lib/features/groups/CommunityCard.svelte';
  import { MIN_SEARCH_QUERY_LENGTH } from '$lib/search';

  let { data }: PageProps = $props();

  const hasResults = $derived(data.results.communities.length > 0 || data.results.articles.length > 0);
</script>

<svelte:head>
  <title>{data.results.query ? `Search: ${data.results.query} — Highlighter` : 'Search — Highlighter'}</title>
</svelte:head>

<section class="search-page">
  <header class="search-hero">
    <div class="search-copy">
      <p class="eyebrow">Search</p>
      <h1>Search communities and articles from the Highlighter relay.</h1>
      <p class="lede">
        Find public reading rooms by name or route, then jump straight into long-form Nostr pieces
        already indexed on the relay.
      </p>
    </div>
  </header>

  {#if data.results.query.length < MIN_SEARCH_QUERY_LENGTH}
    <section class="search-message">
      <p class="message-title">Type at least {MIN_SEARCH_QUERY_LENGTH} characters in the header search.</p>
      <p class="message-copy">
        Community names, route slugs, article titles, summaries, and article body text are all searchable.
      </p>
    </section>
  {:else if !hasResults}
    <section class="search-message">
      <p class="message-title">Nothing matched "{data.results.query}".</p>
      <p class="message-copy">
        Try a broader phrase, a community route slug, or a few words from the article title or body.
      </p>
    </section>
  {:else}
    {#if data.results.communities.length > 0}
      <section class="result-section">
        <div class="result-section-head">
          <div>
            <p class="eyebrow">Communities</p>
            <h2>{data.results.communities.length} public communit{data.results.communities.length === 1 ? 'y' : 'ies'}</h2>
          </div>
        </div>

        <div class="community-grid">
          {#each data.results.communities as community (community.id)}
            <CommunityCard {community} showRoute={true} />
          {/each}
        </div>
      </section>
    {/if}

    {#if data.results.articles.length > 0}
      <section class="result-section">
        <div class="result-section-head">
          <div>
            <p class="eyebrow">Articles</p>
            <h2>{data.results.articles.length} Nostr article{data.results.articles.length === 1 ? '' : 's'}</h2>
          </div>
        </div>

        <div class="article-results">
          {#each data.results.articles as article (article.id)}
            <a class="article-result-card" href={`/note/${encodeURIComponent(article.noteIdentifier)}`}>
              <div class="article-result-copy">
                <div class="article-result-meta">
                  <span>{article.publishedLabel}</span>
                  <span>By {article.authorName}</span>
                </div>

                <h3>{article.title}</h3>
                <p>{article.summary}</p>
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
    padding: 1.75rem 0 3rem;
  }

  .search-hero {
    display: grid;
    gap: 1rem;
  }

  .search-copy {
    max-width: 46rem;
  }

  .eyebrow {
    margin: 0 0 0.4rem;
    color: var(--accent);
    font-size: 0.8rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  h1,
  h2,
  h3 {
    margin: 0;
    color: var(--text-strong);
  }

  h1 {
    font-family: var(--font-serif);
    font-size: clamp(2rem, 4vw, 3.3rem);
    line-height: 1.03;
    letter-spacing: -0.04em;
    max-width: 16ch;
  }

  h2 {
    font-size: 1.5rem;
    line-height: 1.15;
  }

  h3 {
    font-size: 1.1rem;
    line-height: 1.3;
  }

  .lede,
  .message-copy {
    margin: 0.85rem 0 0;
    color: var(--muted);
    line-height: 1.65;
  }

  .search-message {
    max-width: 40rem;
    padding: 1.5rem;
    border: 1px solid var(--border);
    border-radius: 1.3rem;
    background: linear-gradient(180deg, rgba(255, 103, 25, 0.07), rgba(255, 255, 255, 0));
  }

  .message-title {
    margin: 0;
    color: var(--text-strong);
    font-size: 1.05rem;
    font-weight: 700;
  }

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

  .community-grid,
  .article-results {
    display: grid;
    gap: 1rem;
  }

  .community-grid {
    grid-template-columns: repeat(auto-fit, minmax(18rem, 1fr));
  }

  .article-result-card {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 1rem;
    align-items: start;
    padding: 1rem;
    border: 1px solid var(--border);
    border-radius: 1.3rem;
    background: var(--surface);
    color: inherit;
    text-decoration: none;
    transition: border-color 120ms ease, transform 120ms ease, box-shadow 120ms ease;
  }

  .article-result-card:hover,
  .article-result-card:focus-visible {
    border-color: rgba(255, 103, 25, 0.3);
    transform: translateY(-1px);
    box-shadow: 0 16px 40px rgba(17, 17, 17, 0.08);
  }

  .article-result-copy {
    display: grid;
    gap: 0.6rem;
    min-width: 0;
  }

  .article-result-copy p {
    margin: 0;
    color: var(--muted);
    line-height: 1.6;
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
    border-radius: 999px;
    background: var(--surface-soft);
    color: var(--muted);
    font-size: 0.78rem;
    font-weight: 600;
  }

  .article-result-card img {
    width: 8rem;
    height: 6rem;
    object-fit: cover;
    border-radius: 1rem;
  }

  @media (max-width: 760px) {
    .article-result-card {
      grid-template-columns: 1fr;
    }

    .article-result-card img {
      width: 100%;
      height: 11rem;
    }
  }
</style>
