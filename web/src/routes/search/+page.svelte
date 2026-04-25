<script lang="ts">
  import type { PageProps } from './$types';
  import RoomCard from '$lib/features/groups/RoomCard.svelte';
  import { MIN_SEARCH_QUERY_LENGTH } from '$lib/search';

  let { data }: PageProps = $props();

  type Tab = 'all' | 'articles' | 'rooms' | 'people' | 'highlights';
  let activeTab = $state<Tab>('all');

  const roomCount = $derived(data.results.rooms.length);
  const articleCount = $derived(data.results.articles.length);
  const profileCount = $derived(data.results.profiles.length);
  const highlightCount = $derived(data.results.highlights.length);
  const totalCount = $derived(roomCount + articleCount + profileCount + highlightCount);
  const hasResults = $derived(totalCount > 0);

  function formatDate(ts: number): string {
    if (!ts) return '';
    return new Intl.DateTimeFormat('en', {
      month: 'short',
      day: 'numeric',
      year: 'numeric'
    }).format(new Date(ts * 1000));
  }
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
    <p class="text-[19px] leading-[1.5] text-base-content/80 max-w-[52ch] m-0">Search articles, rooms, people, and highlights.</p>
  </header>

  {#if data.results.query.length < MIN_SEARCH_QUERY_LENGTH}
    <div class="bg-base-100 border border-base-300 rounded px-8 py-11 max-w-[40rem]">
      <p class="m-0 mb-2 text-[20px] font-medium text-base-content">Type at least {MIN_SEARCH_QUERY_LENGTH} characters in the header search.</p>
      <p class="m-0 text-[15px] leading-[1.6] text-base-content/80">
        Room names, article titles, people, highlighted excerpts, and article body text are all searchable.
      </p>
    </div>
  {:else if !hasResults}
    <div class="bg-base-100 border border-base-300 rounded px-8 py-11 max-w-[40rem]">
      <p class="m-0 mb-2 text-[20px] font-medium text-base-content">Nothing matched "{data.results.query}".</p>
      <p class="m-0 text-[15px] leading-[1.6] text-base-content/80">
        Try a broader phrase, a room route slug, or a few words from the article title or body.
      </p>
    </div>
  {:else}
    <p class="m-0 font-mono text-[11px] tracking-[0.12em] uppercase text-base-content/50">
      {totalCount} result{totalCount === 1 ? '' : 's'} · {articleCount} article{articleCount === 1 ? '' : 's'} · {roomCount} room{roomCount === 1 ? '' : 's'} · {profileCount} {profileCount === 1 ? 'person' : 'people'} · {highlightCount} highlight{highlightCount === 1 ? '' : 's'}
    </p>

    <div role="tablist" class="tabs tabs-border">
      <button
        role="tab"
        class="tab {activeTab === 'all' ? 'tab-active' : ''}"
        onclick={() => { activeTab = 'all'; }}
      >All</button>
      <button
        role="tab"
        class="tab {activeTab === 'articles' ? 'tab-active' : ''}"
        onclick={() => { activeTab = 'articles'; }}
      >Articles{articleCount > 0 ? ` (${articleCount})` : ''}</button>
      <button
        role="tab"
        class="tab {activeTab === 'rooms' ? 'tab-active' : ''}"
        onclick={() => { activeTab = 'rooms'; }}
      >Rooms{roomCount > 0 ? ` (${roomCount})` : ''}</button>
      <button
        role="tab"
        class="tab {activeTab === 'people' ? 'tab-active' : ''}"
        onclick={() => { activeTab = 'people'; }}
      >People{profileCount > 0 ? ` (${profileCount})` : ''}</button>
      <button
        role="tab"
        class="tab {activeTab === 'highlights' ? 'tab-active' : ''}"
        onclick={() => { activeTab = 'highlights'; }}
      >Highlights{highlightCount > 0 ? ` (${highlightCount})` : ''}</button>
    </div>

    {#if activeTab === 'all'}
      <div class="grid gap-10">
        {#if data.results.articles.length > 0}
          <section class="grid gap-4">
            <h2 class="font-serif font-normal text-[clamp(1.5rem,2.5vw,1.8rem)] leading-[1.1] tracking-[-0.015em] text-base-content m-0">
              Articles
            </h2>
            <div class="grid gap-0">
              {#each data.results.articles.slice(0, 4) as article (article.id)}
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
                    <img src={article.image} alt="" loading="lazy" class="w-24 h-[4.5rem] object-cover rounded" />
                  {/if}
                </a>
              {/each}
            </div>
            {#if data.results.articles.length > 4}
              <button class="btn btn-ghost btn-sm self-start" onclick={() => { activeTab = 'articles'; }}>
                View all {data.results.articles.length} articles
              </button>
            {/if}
          </section>
        {/if}

        {#if data.results.rooms.length > 0}
          <section class="grid gap-4">
            <h2 class="font-serif font-normal text-[clamp(1.5rem,2.5vw,1.8rem)] leading-[1.1] tracking-[-0.015em] text-base-content m-0">
              Rooms
            </h2>
            <div class="grid gap-4 [grid-template-columns:repeat(auto-fit,minmax(18rem,1fr))]">
              {#each data.results.rooms.slice(0, 4) as room (room.id)}
                <RoomCard {room} showRoute={true} />
              {/each}
            </div>
            {#if data.results.rooms.length > 4}
              <button class="btn btn-ghost btn-sm self-start" onclick={() => { activeTab = 'rooms'; }}>
                View all {data.results.rooms.length} rooms
              </button>
            {/if}
          </section>
        {/if}

        {#if data.results.profiles.length > 0}
          <section class="grid gap-4">
            <h2 class="font-serif font-normal text-[clamp(1.5rem,2.5vw,1.8rem)] leading-[1.1] tracking-[-0.015em] text-base-content m-0">
              People
            </h2>
            <div class="grid gap-0">
              {#each data.results.profiles.slice(0, 4) as profile (profile.pubkey)}
                <a
                  class="flex items-center gap-4 py-4 border-b border-base-300 first:border-t first:border-base-300 text-inherit no-underline transition-colors duration-[120ms] ease hover:bg-primary/[0.04] focus-visible:bg-primary/[0.04]"
                  href={`/profile/${encodeURIComponent(profile.npubBech32)}`}
                >
                  {#if profile.picture}
                    <img src={profile.picture} alt="" loading="lazy" class="w-8 h-8 rounded-full object-cover shrink-0" />
                  {:else}
                    <div class="w-8 h-8 rounded-full bg-base-200 shrink-0 grid place-items-center text-base-content/40 text-xs font-bold">
                      {profile.displayName.charAt(0).toUpperCase() || '?'}
                    </div>
                  {/if}
                  <div class="grid min-w-0 gap-0.5">
                    <span class="font-medium text-[0.95rem] leading-[1.3] text-base-content truncate">{profile.displayName}</span>
                    {#if profile.nip05}
                      <span class="text-[0.8rem] text-base-content/50 truncate">{profile.nip05}</span>
                    {/if}
                    {#if profile.bio}
                      <span class="text-[0.85rem] leading-[1.5] text-base-content/70 truncate">{profile.bio}</span>
                    {/if}
                  </div>
                </a>
              {/each}
            </div>
            {#if data.results.profiles.length > 4}
              <button class="btn btn-ghost btn-sm self-start" onclick={() => { activeTab = 'people'; }}>
                View all {data.results.profiles.length} people
              </button>
            {/if}
          </section>
        {/if}

        {#if data.results.highlights.length > 0}
          <section class="grid gap-4">
            <h2 class="font-serif font-normal text-[clamp(1.5rem,2.5vw,1.8rem)] leading-[1.1] tracking-[-0.015em] text-base-content m-0">
              Highlights
            </h2>
            <div class="grid gap-0">
              {#each data.results.highlights.slice(0, 4) as highlight (highlight.id)}
                <a
                  class="grid gap-2 py-5 border-b border-base-300 first:border-t first:border-base-300 text-inherit no-underline transition-colors duration-[120ms] ease hover:bg-primary/[0.04] focus-visible:bg-primary/[0.04]"
                  href={`/note/${encodeURIComponent(highlight.neventBech32)}`}
                >
                  <blockquote class="m-0 pl-4 border-l-2 border-primary/40 text-base-content/90 text-[0.95rem] leading-[1.65] italic">
                    {highlight.content}
                  </blockquote>
                  <div class="flex items-center gap-3">
                    {#if highlight.authorPicture}
                      <img src={highlight.authorPicture} alt="" loading="lazy" class="w-5 h-5 rounded-full object-cover" />
                    {/if}
                    <span class="text-[0.8rem] text-base-content/50">{highlight.authorName}</span>
                    {#if highlight.sourceLabel}
                      <span class="text-[0.8rem] text-base-content/40">{highlight.sourceLabel}</span>
                    {/if}
                    {#if highlight.createdAt}
                      <span class="text-[0.8rem] text-base-content/40 ml-auto">{formatDate(highlight.createdAt)}</span>
                    {/if}
                  </div>
                </a>
              {/each}
            </div>
            {#if data.results.highlights.length > 4}
              <button class="btn btn-ghost btn-sm self-start" onclick={() => { activeTab = 'highlights'; }}>
                View all {data.results.highlights.length} highlights
              </button>
            {/if}
          </section>
        {/if}
      </div>

    {:else if activeTab === 'articles'}
      {#if data.results.articles.length === 0}
        <p class="text-base-content/60 text-sm">No articles matched this search.</p>
      {:else}
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
                <img src={article.image} alt="" loading="lazy" class="w-24 h-[4.5rem] object-cover rounded" />
              {/if}
            </a>
          {/each}
        </div>
      {/if}

    {:else if activeTab === 'rooms'}
      {#if data.results.rooms.length === 0}
        <p class="text-base-content/60 text-sm">No rooms matched this search.</p>
      {:else}
        <div class="grid gap-4 [grid-template-columns:repeat(auto-fit,minmax(18rem,1fr))]">
          {#each data.results.rooms as room (room.id)}
            <RoomCard {room} showRoute={true} />
          {/each}
        </div>
      {/if}

    {:else if activeTab === 'people'}
      {#if data.results.profiles.length === 0}
        <p class="text-base-content/60 text-sm">No people matched this search.</p>
      {:else}
        <div class="grid gap-0">
          {#each data.results.profiles as profile (profile.pubkey)}
            <a
              class="flex items-center gap-4 py-4 border-b border-base-300 first:border-t first:border-base-300 text-inherit no-underline transition-colors duration-[120ms] ease hover:bg-primary/[0.04] focus-visible:bg-primary/[0.04]"
              href={`/profile/${encodeURIComponent(profile.npubBech32)}`}
            >
              {#if profile.picture}
                <img src={profile.picture} alt="" loading="lazy" class="w-8 h-8 rounded-full object-cover shrink-0" />
              {:else}
                <div class="w-8 h-8 rounded-full bg-base-200 shrink-0 grid place-items-center text-base-content/40 text-xs font-bold">
                  {profile.displayName.charAt(0).toUpperCase() || '?'}
                </div>
              {/if}
              <div class="grid min-w-0 gap-0.5">
                <span class="font-medium text-[0.95rem] leading-[1.3] text-base-content truncate">{profile.displayName}</span>
                {#if profile.nip05}
                  <span class="text-[0.8rem] text-base-content/50 truncate">{profile.nip05}</span>
                {/if}
                {#if profile.bio}
                  <p class="m-0 text-[0.85rem] leading-[1.5] text-base-content/70 line-clamp-2">{profile.bio}</p>
                {/if}
              </div>
            </a>
          {/each}
        </div>
      {/if}

    {:else if activeTab === 'highlights'}
      {#if data.results.highlights.length === 0}
        <p class="text-base-content/60 text-sm">No highlights matched this search.</p>
      {:else}
        <div class="grid gap-0">
          {#each data.results.highlights as highlight (highlight.id)}
            <a
              class="grid gap-2 py-5 border-b border-base-300 first:border-t first:border-base-300 text-inherit no-underline transition-colors duration-[120ms] ease hover:bg-primary/[0.04] focus-visible:bg-primary/[0.04]"
              href={`/note/${encodeURIComponent(highlight.neventBech32)}`}
            >
              <blockquote class="m-0 pl-4 border-l-2 border-primary/40 text-base-content/90 text-[0.95rem] leading-[1.65] italic">
                {highlight.content}
              </blockquote>
              <div class="flex items-center gap-3">
                {#if highlight.authorPicture}
                  <img src={highlight.authorPicture} alt="" loading="lazy" class="w-5 h-5 rounded-full object-cover" />
                {/if}
                <span class="text-[0.8rem] text-base-content/50">{highlight.authorName}</span>
                {#if highlight.sourceLabel}
                  <span class="text-[0.8rem] text-base-content/40">{highlight.sourceLabel}</span>
                {/if}
                {#if highlight.createdAt}
                  <span class="text-[0.8rem] text-base-content/40 ml-auto">{formatDate(highlight.createdAt)}</span>
                {/if}
              </div>
            </a>
          {/each}
        </div>
      {/if}
    {/if}
  {/if}
</section>
