<script lang="ts">
  import { browser } from '$app/environment';
  import { page } from '$app/state';
  import { onMount, tick } from 'svelte';
  import { MIN_SEARCH_QUERY_LENGTH, type SearchResponse } from '$lib/search';

  const SEARCH_RESULT_LIMIT = 4;

  let rootEl = $state<HTMLElement | null>(null);
  let inputEl = $state<HTMLInputElement | null>(null);
  let query = $state(page.url.searchParams.get('q') ?? '');
  let expanded = $state(false);
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
      expanded &&
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

      collapse();
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        collapse();
      }
    };

    document.addEventListener('pointerdown', handlePointerDown);
    document.addEventListener('keydown', handleKeyDown);

    return () => {
      document.removeEventListener('pointerdown', handlePointerDown);
      document.removeEventListener('keydown', handleKeyDown);
    };
  });

  async function expand(): Promise<void> {
    expanded = true;
    open = true;
    await tick();
    inputEl?.focus();
  }

  function collapse(): void {
    open = false;
    expanded = false;
  }

  function closeDropdown(): void {
    collapse();
  }

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
</script>

<div class="header-search" bind:this={rootEl}>
  {#if expanded}
    <form class="search-expanded" role="search" method="GET" action="/search">
      <label class="input input-bordered input-sm flex items-center gap-2 flex-1 rounded-full">
        <svg class="opacity-50" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/></svg>
        <input
          bind:this={inputEl}
          bind:value={query}
          type="search"
          name="q"
          class="grow"
          placeholder="Search circles and articles…"
          autocomplete="off"
          spellcheck="false"
          aria-label="Search circles and articles"
          onfocus={() => { open = true; }}
        />
      </label>
      <button type="button" class="btn btn-ghost btn-circle btn-sm" onclick={collapse} aria-label="Close search">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 6 6 18"/><path d="m6 6 12 12"/></svg>
      </button>
    </form>
  {:else}
    <button type="button" class="btn btn-ghost btn-circle btn-sm" onclick={expand} aria-label="Open search">
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/></svg>
    </button>
  {/if}

  {#if showDropdown}
    <ul class="menu menu-sm search-dropdown rounded-box bg-base-100 shadow-lg border border-base-300">
      {#if loading}
        <li class="menu-title text-base-content/50"><span>Searching…</span></li>
      {:else if showEmptyState}
        <li class="menu-title text-base-content/50"><span>No results for "{trimmedQuery}"</span></li>
      {:else}
        {#if results.communities.length > 0}
          <li class="menu-title"><span>Circles</span></li>
          {#each results.communities as community (community.id)}
            <li>
              <a href={`/community/${community.id}`} onclick={closeDropdown}>
                <span class="flex-1 truncate font-medium">{community.name}</span>
                {#if community.about}
                  <span class="text-xs text-base-content/50 truncate max-w-[12rem]">{community.about}</span>
                {/if}
              </a>
            </li>
          {/each}
        {/if}

        {#if results.articles.length > 0}
          <li class="menu-title"><span>Articles</span></li>
          {#each results.articles as article (article.id)}
            <li>
              <a href={`/note/${encodeURIComponent(article.noteIdentifier)}`} onclick={closeDropdown}>
                {#if article.image}
                  <img class="w-7 h-7 rounded object-cover shrink-0" src={article.image} alt="" loading="lazy" />
                {/if}
                <span class="flex-1 truncate font-medium">{article.title}</span>
                <span class="text-xs text-base-content/50 shrink-0">by {article.authorName}</span>
              </a>
            </li>
          {/each}
        {/if}

        <li class="mt-1 border-t border-base-300">
          <a
            class="justify-center text-primary font-semibold text-xs"
            href={`/search?q=${encodeURIComponent(trimmedQuery)}`}
            onclick={closeDropdown}
          >
            View all results
          </a>
        </li>
      {/if}
    </ul>
  {/if}
</div>

<style>
  .header-search {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: flex-end;
  }

  .search-expanded {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    width: 100%;
  }

  .search-dropdown {
    position: absolute;
    top: calc(100% + 0.4rem);
    left: 0;
    right: 0;
    z-index: 30;
    max-height: min(70vh, 28rem);
    overflow-y: auto;
  }

  @media (max-width: 700px) {
    .search-dropdown {
      left: -2rem;
      right: -2rem;
    }
  }
</style>
