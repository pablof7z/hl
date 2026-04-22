<script lang="ts">
  import { browser } from '$app/environment';
  import { NDKEvent } from '@nostr-dev-kit/ndk';
  import StoryAuthor from '$lib/components/StoryAuthor.svelte';
  import ArticleCard from '$lib/components/ArticleCard.svelte';
  import BookmarkIcon from '$lib/components/BookmarkIcon.svelte';
  import { ndk } from '$lib/ndk/client';
  import {
    articleImageUrl,
    articleSummary,
    articleTitle
  } from '$lib/ndk/format';
  import {
    BOOKMARK_LIST_KIND,
    bookmarkAddressFilters,
    bookmarkAddressesFromEvent,
    latestListEvent,
    setBookmarkAddressPresence
  } from '$lib/ndk/lists';

  const currentUser = $derived(ndk.$currentUser);

  const myBookmarkList = ndk.$subscribe(() => {
    if (!browser || !currentUser) return undefined;
    return {
      filters: [{ kinds: [BOOKMARK_LIST_KIND], authors: [currentUser.pubkey], limit: 20 }]
    };
  });
  const myBookmarkListEvent = $derived(latestListEvent(myBookmarkList.events));

  const myBookmarkedAddresses = $derived.by(() => {
    return bookmarkAddressesFromEvent(myBookmarkListEvent, '30023:');
  });
  const myBookmarkFilters = $derived(bookmarkAddressFilters(myBookmarkedAddresses));

  const myArticles = ndk.$subscribe(() => {
    if (!browser || myBookmarkFilters.length === 0) return undefined;
    return { filters: myBookmarkFilters };
  });

  const networkBookmarks = ndk.$subscribe(() => {
    if (!browser) return undefined;
    return {
      filters: [{ kinds: [BOOKMARK_LIST_KIND], limit: 100 }]
    };
  });

  const trendingArticleAddresses = $derived.by(() => {
    const counts = new Map<string, { count: number; pubkeys: Set<string> }>();
    for (const bookmarkEvent of networkBookmarks.events) {
      if (currentUser && bookmarkEvent.pubkey === currentUser.pubkey) continue;
      for (const addr of bookmarkAddressesFromEvent(bookmarkEvent, '30023:')) {
        const existing = counts.get(addr);
        if (existing) {
          existing.count++;
          existing.pubkeys.add(bookmarkEvent.pubkey);
        } else {
          counts.set(addr, { count: 1, pubkeys: new Set([bookmarkEvent.pubkey]) });
        }
      }
    }
    return [...counts.entries()]
      .sort((a, b) => b[1].count - a[1].count)
      .slice(0, 20);
  });

  const trendingArticles = ndk.$subscribe(() => {
    if (!browser || trendingArticleAddresses.length === 0) return undefined;
    const filters = trendingArticleAddresses.map(([addr]) => {
      const [kind, pubkey, identifier] = addr.split(':');
      return {
        kinds: [Number(kind)],
        authors: [pubkey],
        '#d': [identifier]
      } as import('@nostr-dev-kit/ndk').NDKFilter;
    });
    return { filters };
  });

  const trendingArticleLookup = $derived.by(() => {
    const lookup = new Map<string, NDKEvent>();
    for (const article of trendingArticles.events) {
      lookup.set(article.tagId(), article);
    }
    return lookup;
  });

  const orderedTrending = $derived.by(() => {
    return trendingArticleAddresses
      .map(([addr, data]) => ({
        article: trendingArticleLookup.get(addr),
        saveCount: data.count
      }))
      .filter((item): item is typeof item & { article: NDKEvent } => Boolean(item.article));
  });

  const myArticleLookup = $derived.by(() => {
    const lookup = new Map<string, NDKEvent>();
    for (const article of myArticles.events) {
      lookup.set(article.tagId(), article);
    }
    return lookup;
  });

  const orderedMyArticles = $derived.by(() => {
    return myBookmarkedAddresses
      .map((addr) => myArticleLookup.get(addr))
      .filter((article): article is NDKEvent => Boolean(article));
  });

  async function removeBookmark(articleAddress: string) {
    if (!currentUser) return;
    await setBookmarkAddressPresence(ndk, myBookmarkListEvent, articleAddress, false);
  }
</script>

<svelte:head>
  <title>Bookmarks — Highlighter</title>
</svelte:head>

<div class="max-w-[var(--page-width)]">
  <div class="grid gap-14">
    {#if currentUser}
      <section class="grid gap-6">
        <div class="grid gap-1.5">
          <h2 class="m-0 font-serif text-[clamp(1.6rem,3.5vw,2.2rem)] font-bold text-base-content tracking-tight leading-[1.1]">My Reading List</h2>
          <p class="m-0 text-base-content/50 text-[0.95rem]">Articles you've saved for later</p>
        </div>

        {#if orderedMyArticles.length > 0}
          <div class="max-w-[var(--content-width)] grid">
            {#each orderedMyArticles as event (event.id)}
              <div class="relative group">
                <ArticleCard {event} showAuthor />
                <button
                  class="absolute top-6 right-0 inline-flex items-center justify-center w-8 h-8 p-0 border-none rounded bg-transparent text-primary cursor-pointer opacity-0 group-hover:opacity-100 sm:opacity-100 transition-opacity duration-[160ms] ease hover:bg-error/10 hover:text-error"
                  title="Remove from reading list"
                  onclick={() => removeBookmark(event.tagId())}
                >
                  <BookmarkIcon size={16} filled />
                </button>
              </div>
            {/each}
          </div>
        {:else if myBookmarkedAddresses.length > 0}
          <p class="text-base-content/50">Loading your saved articles...</p>
        {:else}
          <div class="grid gap-2 justify-items-center py-12 px-4 border border-dashed border-base-300 rounded-box text-center">
            <div class="text-base-300 mb-1">
              <BookmarkIcon size={32} />
            </div>
            <p class="m-0">Your reading list is empty</p>
            <p class="m-0 text-base-content/50">Bookmark articles to save them here for later</p>
          </div>
        {/if}
      </section>
    {:else}
      <section class="grid gap-6">
        <div class="grid gap-1.5">
          <h2 class="m-0 font-serif text-[clamp(1.6rem,3.5vw,2.2rem)] font-bold text-base-content tracking-tight leading-[1.1]">My Reading List</h2>
          <p class="m-0 text-base-content/50 text-[0.95rem]">Log in to save and manage your bookmarks</p>
        </div>
      </section>
    {/if}

    <section class="grid gap-6">
      <div class="grid gap-1.5">
        <h2 class="m-0 font-serif text-[clamp(1.6rem,3.5vw,2.2rem)] font-bold text-base-content tracking-tight leading-[1.1]">What Readers Are Saving</h2>
        <p class="m-0 text-base-content/50 text-[0.95rem]">Discover articles readers find worth keeping</p>
      </div>

      {#if orderedTrending.length > 0}
        <div class="grid gap-6 [grid-template-columns:repeat(auto-fill,minmax(min(100%,20rem),1fr))]">
          {#each orderedTrending as { article, saveCount } (article.id)}
            <a class="grid gap-0 border border-base-300 rounded-box overflow-hidden text-inherit no-underline transition-[border-color,box-shadow] duration-200 ease hover:border-base-300 hover:shadow-[0_4px_20px_rgba(0,0,0,0.06)] group" href={`/note/${article.encode()}`}>
              {#if articleImageUrl(article.rawEvent())}
                <img
                  class="w-full aspect-video object-cover"
                  src={articleImageUrl(article.rawEvent())}
                  alt=""
                  loading="lazy"
                />
              {:else}
                <div class="w-full aspect-video bg-gradient-to-br from-base-200 to-base-300"></div>
              {/if}
              <div class="grid gap-2.5 px-5 pt-[1.1rem] pb-5">
                <h3 class="m-0 font-serif text-[1.15rem] font-bold text-base-content leading-[1.25] tracking-[-0.01em] line-clamp-2 transition-colors duration-[160ms] ease group-hover:text-primary">{articleTitle(article.rawEvent())}</h3>
                <p class="m-0 text-base-content/50 text-[0.88rem] leading-[1.5] line-clamp-2">{articleSummary(article.rawEvent(), 120)}</p>
                <div class="flex flex-wrap items-center justify-between gap-2 pt-[0.35rem]">
                  <StoryAuthor
                    {ndk}
                    pubkey={article.pubkey}
                    avatarClass="article-author-avatar article-author-avatar-compact"
                    compact
                  />
                  <span class="inline-flex items-center gap-1 text-primary text-[0.8rem] font-semibold">
                    <BookmarkIcon size={14} filled />
                    {saveCount} {saveCount === 1 ? 'save' : 'saves'}
                  </span>
                </div>
              </div>
            </a>
          {/each}
        </div>
      {:else if networkBookmarks.events.length > 0}
        <p class="text-base-content/50">Analyzing what people are saving...</p>
      {:else}
        <p class="text-base-content/50">Discovering bookmarks from the network...</p>
      {/if}
    </section>
  </div>
</div>
