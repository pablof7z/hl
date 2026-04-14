<script lang="ts">
  import { browser } from '$app/environment';
  import { page } from '$app/state';
  import { onMount } from 'svelte';
  import { MIN_SEARCH_QUERY_LENGTH, type SearchResponse } from '$lib/search';

  const SEARCH_RESULT_LIMIT = 4;

  let rootEl = $state<HTMLElement | null>(null);
  let query = $state(page.url.searchParams.get('q') ?? '');
  let open = $state(false);
  let loading = $state(false);
  let lastFetchedQuery = $state('');
  let results = $state<SearchResponse>(emptySearch(''));

  let debounceHandle: ReturnType<typeof setTimeout> | undefined;
  let controller: AbortController | undefined;
  let requestVersion = 0;

  const trimmedQuery = $derived(query.trim());
  const hasResults = $derived(results.communities.length > 0 || results.articles.length > 0);
  const showEmptyState = $derived(
    trimmedQuery.length >= MIN_SEARCH_QUERY_LENGTH &&
      !loading &&
      lastFetchedQuery === trimmedQuery &&
      !hasResults
  );
  const showDropdown = $derived(
    open &&
      trimmedQuery.length >= MIN_SEARCH_QUERY_LENGTH &&
      (loading || hasResults || lastFetchedQuery === trimmedQuery)
  );

  $effect(() => {
    if (page.url.pathname !== '/search' || open) {
      return;
    }

    query = page.url.searchParams.get('q') ?? '';
  });

  $effect(() => {
    if (!browser || !open) {
      return;
    }

    const searchQuery = trimmedQuery;

    if (searchQuery.length < MIN_SEARCH_QUERY_LENGTH) {
      controller?.abort();
      loading = false;
      lastFetchedQuery = '';
      results = emptySearch(searchQuery);
      return;
    }

    debounceHandle = setTimeout(() => {
      void fetchResults(searchQuery);
    }, 180);

    return () => {
      if (debounceHandle) {
        clearTimeout(debounceHandle);
      }
    };
  });

  onMount(() => {
    const handlePointerDown = (event: PointerEvent) => {
      if (rootEl?.contains(event.target as Node)) {
        return;
      }

      open = false;
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        open = false;
      }
    };

    document.addEventListener('pointerdown', handlePointerDown);
    document.addEventListener('keydown', handleKeyDown);

    return () => {
      document.removeEventListener('pointerdown', handlePointerDown);
      document.removeEventListener('keydown', handleKeyDown);
    };
  });

  async function fetchResults(searchQuery: string): Promise<void> {
    controller?.abort();
    controller = new AbortController();

    const currentVersion = ++requestVersion;
    loading = true;

    try {
      const response = await fetch(
        `/api/search?q=${encodeURIComponent(searchQuery)}&limit=${SEARCH_RESULT_LIMIT}`,
        {
          signal: controller.signal
        }
      );

      if (!response.ok) {
        throw new Error(`Search failed with ${response.status}`);
      }

      const payload = (await response.json()) as SearchResponse;

      if (currentVersion !== requestVersion) {
        return;
      }

      results = payload;
      lastFetchedQuery = searchQuery;
    } catch (error) {
      if (error instanceof DOMException && error.name === 'AbortError') {
        return;
      }

      console.error('Header search request failed', error);

      if (currentVersion !== requestVersion) {
        return;
      }

      results = emptySearch(searchQuery);
      lastFetchedQuery = searchQuery;
    } finally {
      if (currentVersion === requestVersion) {
        loading = false;
      }
    }
  }

  function emptySearch(searchQuery: string): SearchResponse {
    return {
      query: searchQuery,
      communities: [],
      articles: []
    };
  }

  function closeDropdown(): void {
    open = false;
  }
</script>

<form class="header-search" role="search" method="GET" action="/search" bind:this={rootEl}>
  <label class="header-search-input">
    <input
      bind:value={query}
      type="search"
      name="q"
      placeholder="Search communities and articles"
      autocomplete="off"
      spellcheck="false"
      aria-label="Search communities and articles"
      onfocus={() => {
        open = true;
      }}
    />
  </label>

  <button type="submit">Search</button>

  {#if showDropdown}
    <div class="header-search-dropdown">
      {#if loading}
        <p class="search-status">Searching the Highlighter relay...</p>
      {:else if showEmptyState}
        <p class="search-status">No communities or articles matched "{trimmedQuery}".</p>
      {:else}
        {#if results.communities.length > 0}
          <section class="search-section">
            <div class="search-section-head">
              <span>Communities</span>
              <a href={`/search?q=${encodeURIComponent(trimmedQuery)}`} onclick={closeDropdown}>View all</a>
            </div>

            <div class="search-result-list">
              {#each results.communities as community (community.id)}
                <a class="search-result-row" href={`/community/${community.id}`} onclick={closeDropdown}>
                  <div class="search-result-copy">
                    <strong>{community.name}</strong>
                    <span>{community.about || `/community/${community.id}`}</span>
                  </div>
                  <small>/community/{community.id}</small>
                </a>
              {/each}
            </div>
          </section>
        {/if}

        {#if results.articles.length > 0}
          <section class="search-section">
            <div class="search-section-head">
              <span>Articles</span>
              <a href={`/search?q=${encodeURIComponent(trimmedQuery)}`} onclick={closeDropdown}>View all</a>
            </div>

            <div class="search-result-list">
              {#each results.articles as article (article.id)}
                <a
                  class="search-result-row search-result-row-article"
                  href={`/note/${encodeURIComponent(article.noteIdentifier)}`}
                  onclick={closeDropdown}
                >
                  <div class="search-result-copy">
                    <strong>{article.title}</strong>
                    <span>{article.summary}</span>
                  </div>
                  <small>By {article.authorName}</small>
                </a>
              {/each}
            </div>
          </section>
        {/if}
      {/if}
    </div>
  {/if}
</form>

<style>
  .header-search {
    position: relative;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 0.65rem;
    width: 100%;
  }

  .header-search-input {
    display: block;
  }

  .header-search input {
    width: 100%;
    min-height: 2.9rem;
    padding: 0 1rem;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--surface);
    color: var(--text);
    font: inherit;
  }

  .header-search input:focus {
    outline: none;
    border-color: rgba(255, 103, 25, 0.4);
    box-shadow: 0 0 0 3px rgba(255, 103, 25, 0.12);
  }

  .header-search button {
    min-height: 2.9rem;
    padding: 0 1rem;
    border: 1px solid var(--accent);
    border-radius: 999px;
    background: var(--accent);
    color: white;
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }

  .header-search button:hover,
  .header-search button:focus-visible {
    background: var(--accent-hover);
    border-color: var(--accent-hover);
  }

  .header-search-dropdown {
    position: absolute;
    top: calc(100% + 0.55rem);
    left: 0;
    right: 0;
    display: grid;
    gap: 0.9rem;
    padding: 1rem;
    border: 1px solid var(--border);
    border-radius: 1.25rem;
    background: color-mix(in srgb, var(--surface) 92%, white);
    box-shadow: 0 24px 60px rgba(17, 17, 17, 0.12);
    backdrop-filter: blur(10px);
    max-height: min(70vh, 34rem);
    overflow-y: auto;
    z-index: 30;
  }

  .search-status {
    margin: 0;
    color: var(--muted);
    line-height: 1.6;
  }

  .search-section {
    display: grid;
    gap: 0.65rem;
  }

  .search-section-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
  }

  .search-section-head span,
  .search-section-head a {
    color: var(--muted);
    font-size: 0.76rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .search-section-head a:hover,
  .search-section-head a:focus-visible {
    color: var(--accent);
  }

  .search-result-list {
    display: grid;
    gap: 0.5rem;
  }

  .search-result-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 0.8rem;
    align-items: start;
    padding: 0.85rem 0.95rem;
    border: 1px solid var(--border);
    border-radius: 1rem;
    background: var(--surface);
    color: inherit;
    text-decoration: none;
    transition: border-color 120ms ease, transform 120ms ease, box-shadow 120ms ease;
  }

  .search-result-row:hover,
  .search-result-row:focus-visible {
    border-color: rgba(255, 103, 25, 0.3);
    transform: translateY(-1px);
    box-shadow: 0 12px 30px rgba(17, 17, 17, 0.08);
  }

  .search-result-copy {
    display: grid;
    gap: 0.25rem;
    min-width: 0;
  }

  .search-result-copy strong {
    color: var(--text-strong);
    font-size: 0.97rem;
    line-height: 1.25;
  }

  .search-result-copy span,
  .search-result-row small {
    color: var(--muted);
    line-height: 1.45;
  }

  .search-result-row small {
    font-size: 0.8rem;
    text-align: right;
  }

  .search-result-row-article small {
    max-width: 8rem;
  }

  @media (max-width: 700px) {
    .header-search {
      grid-template-columns: 1fr;
    }

    .header-search button {
      width: 100%;
    }

    .search-result-row {
      grid-template-columns: 1fr;
    }

    .search-result-row small {
      text-align: left;
    }
  }
</style>
