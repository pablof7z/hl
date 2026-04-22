<script lang="ts">
  import type { PageProps } from './$types';
  import RoomCard from '$lib/features/groups/RoomCard.svelte';
  import { MIN_SEARCH_QUERY_LENGTH } from '$lib/search';

  let { data }: PageProps = $props();

  const roomCount = $derived(data.results.rooms.length);
  const articleCount = $derived(data.results.articles.length);
  const totalCount = $derived(roomCount + articleCount);
  const hasResults = $derived(data.results.rooms.length > 0 || data.results.articles.length > 0);
</script>

<svelte:head>
  <title>{data.results.query ? `Search: ${data.results.query} — Highlighter` : 'Search — Highlighter'}</title>
</svelte:head>

<section class="grid gap-8 pt-14 pb-20">
  <header class="pb-8 border-b border-base-300 mb-3">
    {#if data.results.query.length < MIN_SEARCH_QUERY_LENGTH}
      <h1 class="font-serif font-normal text-[clamp(44px,6vw,68px)] leading-[1.02] tracking-[-0.025em] text-base-content m-0 mb-3.5">Search.</h1>
    {:else}
      <h1 class="font-serif font-normal text-[clamp(44px,6vw,68px)] leading-[1.02] tracking-[-0.025em] text-base-content m-0 mb-3.5">Results for <em class="italic text-primary">"{data.results.query}"</em></h1>
    {/if}
    <p class="font-serif italic text-[19px] leading-[1.5] text-base-content/80 max-w-[52ch] m-0">Find rooms and articles across Highlighter.</p>
  </header>

  {#if data.results.query.length < MIN_SEARCH_QUERY_LENGTH}
    <div class="bg-base-100 border border-base-300 rounded px-8 py-11 max-w-[40rem]">
      <p class="m-0 mb-2 font-serif text-[20px] font-medium text-base-content">Type at least {MIN_SEARCH_QUERY_LENGTH} characters in the header search.</p>
      <p class="m-0 text-[15px] leading-[1.6] text-base-content/80">
        Room names, route slugs, article titles, summaries, and article body text are all searchable.
      </p>
    </div>
  {:else if !hasResults}
    <div class="bg-base-100 border border-base-300 rounded px-8 py-11 max-w-[40rem]">
      <p class="m-0 mb-2 font-serif text-[20px] font-medium text-base-content">Nothing matched "{data.results.query}".</p>
      <p class="m-0 text-[15px] leading-[1.6] text-base-content/80">
        Try a broader phrase, a room route slug, or a few words from the article title or body.
      </p>
    </div>
  {:else}
    <p class="m-0 font-mono text-[11px] tracking-[0.12em] uppercase text-base-content/50">
      {totalCount} result{totalCount === 1 ? '' : 's'} · {roomCount} room{roomCount === 1
        ? ''
        : 's'} · {articleCount} article{articleCount === 1 ? '' : 's'}
    </p>

    {#if data.results.rooms.length > 0}
      <section class="grid gap-4">
        <div class="flex items-end justify-between gap-4 flex-wrap">
          <h2 class="font-serif font-normal text-[clamp(1.5rem,2.5vw,1.8rem)] leading-[1.1] tracking-[-0.015em] text-base-content m-0">
            {data.results.rooms.length} public room{data.results.rooms.length === 1 ? '' : 's'}
          </h2>
        </div>

        <div class="grid gap-4 [grid-template-columns:repeat(auto-fit,minmax(18rem,1fr))]">
          {#each data.results.rooms as room (room.id)}
            <RoomCard {room} showRoute={true} />
          {/each}
        </div>
      </section>
    {/if}

    {#if data.results.articles.length > 0}
      <section class="grid gap-4">
        <div class="flex items-end justify-between gap-4 flex-wrap">
          <h2 class="font-serif font-normal text-[clamp(1.5rem,2.5vw,1.8rem)] leading-[1.1] tracking-[-0.015em] text-base-content m-0">
            {data.results.articles.length} Nostr article{data.results.articles.length === 1 ? '' : 's'}
          </h2>
        </div>

        <div class="grid gap-0">
          {#each data.results.articles as article (article.id)}
            <a
              class="grid [grid-template-columns:minmax(0,1fr)_auto] gap-4 items-start py-5 border-b border-base-300 first:border-t first:border-base-300 text-inherit no-underline transition-colors duration-[120ms] ease hover:bg-primary/[0.04] focus-visible:bg-primary/[0.04]"
              href={`/note/${encodeURIComponent(article.noteIdentifier)}`}
            >
              <div class="grid gap-2 min-w-0">
                <div class="flex flex-wrap gap-2">
                  <span class="inline-flex items-center min-h-[1.8rem] px-[0.65rem] rounded-full bg-base-200 border border-base-300 text-base-content/50 font-mono text-[10px] tracking-[0.08em] uppercase">{article.publishedLabel}</span>
                  <span class="inline-flex items-center min-h-[1.8rem] px-[0.65rem] rounded-full bg-base-200 border border-base-300 text-base-content/50 font-mono text-[10px] tracking-[0.08em] uppercase">By {article.authorName}</span>
                </div>

                <h3 class="font-serif font-medium text-[1.15rem] leading-[1.25] tracking-[-0.01em] text-base-content m-0">{article.title}</h3>
                <p class="m-0 text-sm leading-[1.6] text-base-content/80">{article.summary}</p>
              </div>

              {#if article.image}
                <img src={article.image} alt="" loading="lazy" class="w-24 h-[4.5rem] sm:w-24 sm:h-[4.5rem] object-cover rounded" />
              {/if}
            </a>
          {/each}
        </div>
      </section>
    {/if}
  {/if}
</section>
