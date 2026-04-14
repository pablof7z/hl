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

  const currentUser = $derived(ndk.$currentUser);

  const myBookmarkList = ndk.$subscribe(() => {
    if (!browser || !currentUser) return undefined;
    return {
      filters: [{ kinds: [10003], authors: [currentUser.pubkey], limit: 1 }]
    };
  });

  const myBookmarkedAddresses = $derived.by(() => {
    const bookmarkEvent = myBookmarkList.events[0];
    if (!bookmarkEvent) return [];
    return bookmarkEvent.tags
      .filter((tag) => tag[0] === 'a' && tag[1]?.startsWith('30023:'))
      .map((tag) => tag[1]);
  });

  const myArticles = ndk.$subscribe(() => {
    if (!browser || myBookmarkedAddresses.length === 0) return undefined;
    const filters = myBookmarkedAddresses.map((addr) => {
      const [kind, pubkey, identifier] = addr.split(':');
      return {
        kinds: [Number(kind)],
        authors: [pubkey],
        '#d': [identifier]
      } as import('@nostr-dev-kit/ndk').NDKFilter;
    });
    return { filters };
  });

  const networkBookmarks = ndk.$subscribe(() => {
    if (!browser) return undefined;
    return {
      filters: [{ kinds: [10003], limit: 100 }]
    };
  });

  const trendingArticleAddresses = $derived.by(() => {
    const counts = new Map<string, { count: number; pubkeys: Set<string> }>();
    for (const bookmarkEvent of networkBookmarks.events) {
      if (currentUser && bookmarkEvent.pubkey === currentUser.pubkey) continue;
      for (const tag of bookmarkEvent.tags) {
        if (tag[0] === 'a' && tag[1]?.startsWith('30023:')) {
          const addr = tag[1];
          const existing = counts.get(addr);
          if (existing) {
            existing.count++;
            existing.pubkeys.add(bookmarkEvent.pubkey);
          } else {
            counts.set(addr, { count: 1, pubkeys: new Set([bookmarkEvent.pubkey]) });
          }
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
    const bookmarkEvent = myBookmarkList.events[0];
    if (!bookmarkEvent) return;
    const updated = new NDKEvent(ndk);
    updated.kind = 10003;
    updated.tags = bookmarkEvent.tags.filter(
      (tag) => !(tag[0] === 'a' && tag[1] === articleAddress)
    );
    await updated.publish();
  }
</script>

<svelte:head>
  <title>Bookmarks — Highlighter</title>
</svelte:head>

<div class="bookmarks-layout">
  <div class="bookmarks-main">
    {#if currentUser}
      <section class="bookmarks-section">
        <div class="bookmarks-section-header">
          <h2 class="bookmarks-section-title">My Reading List</h2>
          <p class="bookmarks-section-desc">Articles you've saved for later</p>
        </div>

        {#if orderedMyArticles.length > 0}
          <div class="article-feed">
            {#each orderedMyArticles as event (event.id)}
              <div class="bookmark-feed-item">
                <ArticleCard {event} showAuthor />
                <button
                  class="bookmark-remove-btn"
                  title="Remove from reading list"
                  onclick={() => removeBookmark(event.tagId())}
                >
                  <BookmarkIcon size={16} filled />
                </button>
              </div>
            {/each}
          </div>
        {:else if myBookmarkedAddresses.length > 0}
          <p class="muted">Loading your saved articles...</p>
        {:else}
          <div class="bookmarks-empty">
            <div class="bookmarks-empty-icon">
              <BookmarkIcon size={32} />
            </div>
            <p>Your reading list is empty</p>
            <p class="muted">Bookmark articles to save them here for later</p>
          </div>
        {/if}
      </section>
    {:else}
      <section class="bookmarks-section">
        <div class="bookmarks-section-header">
          <h2 class="bookmarks-section-title">My Reading List</h2>
          <p class="bookmarks-section-desc">Log in to save and manage your bookmarks</p>
        </div>
      </section>
    {/if}

    <section class="bookmarks-section">
      <div class="bookmarks-section-header">
        <h2 class="bookmarks-section-title">What Readers Are Saving</h2>
        <p class="bookmarks-section-desc">Discover articles the community finds worth keeping</p>
      </div>

      {#if orderedTrending.length > 0}
        <div class="trending-grid">
          {#each orderedTrending as { article, saveCount } (article.id)}
            <a class="trending-card" href={`/note/${article.encode()}`}>
              {#if articleImageUrl(article.rawEvent())}
                <img
                  class="trending-card-image"
                  src={articleImageUrl(article.rawEvent())}
                  alt=""
                  loading="lazy"
                />
              {:else}
                <div class="trending-card-image-placeholder"></div>
              {/if}
              <div class="trending-card-body">
                <h3 class="trending-card-title">{articleTitle(article.rawEvent())}</h3>
                <p class="trending-card-summary">{articleSummary(article.rawEvent(), 120)}</p>
                <div class="trending-card-meta">
                  <StoryAuthor
                    {ndk}
                    pubkey={article.pubkey}
                    avatarClass="article-author-avatar article-author-avatar-compact"
                    compact
                  />
                  <span class="trending-save-count">
                    <BookmarkIcon size={14} filled />
                    {saveCount} {saveCount === 1 ? 'save' : 'saves'}
                  </span>
                </div>
              </div>
            </a>
          {/each}
        </div>
      {:else if networkBookmarks.events.length > 0}
        <p class="muted">Analyzing what people are saving...</p>
      {:else}
        <p class="muted">Discovering bookmarks from the network...</p>
      {/if}
    </section>
  </div>
</div>

<style>
  .bookmarks-layout {
    max-width: var(--page-width);
  }

  .bookmarks-main {
    display: grid;
    gap: 3.5rem;
  }

  .bookmarks-section {
    display: grid;
    gap: 1.5rem;
  }

  .bookmarks-section-header {
    display: grid;
    gap: 0.35rem;
  }

  .bookmarks-section-title {
    margin: 0;
    font-family: var(--font-serif);
    font-size: clamp(1.6rem, 3.5vw, 2.2rem);
    font-weight: 700;
    color: var(--text-strong);
    letter-spacing: -0.02em;
    line-height: 1.1;
  }

  .bookmarks-section-desc {
    margin: 0;
    color: var(--muted);
    font-size: 0.95rem;
  }

  /* ── my reading list feed ─────────────────────────────────── */

  .article-feed {
    max-width: var(--content-width);
    display: grid;
  }

  .bookmark-feed-item {
    position: relative;
  }

  .bookmark-remove-btn {
    position: absolute;
    top: 1.5rem;
    right: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 2rem;
    height: 2rem;
    padding: 0;
    border: none;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--accent);
    cursor: pointer;
    opacity: 0;
    transition: opacity 160ms ease, color 160ms ease, background 160ms ease;
  }

  .bookmark-feed-item:hover .bookmark-remove-btn {
    opacity: 1;
  }

  .bookmark-remove-btn:hover {
    background: var(--pale-red);
    color: var(--pale-red-text);
  }

  /* ── empty state ──────────────────────────────────────────── */

  .bookmarks-empty {
    display: grid;
    gap: 0.5rem;
    justify-items: center;
    padding: 3rem 1rem;
    border: 1px dashed var(--border);
    border-radius: var(--radius-md);
    text-align: center;
  }

  .bookmarks-empty-icon {
    color: var(--border);
    margin-bottom: 0.25rem;
  }

  .bookmarks-empty p {
    margin: 0;
  }

  /* ── trending grid ────────────────────────────────────────── */

  .trending-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 20rem), 1fr));
    gap: 1.5rem;
  }

  .trending-card {
    display: grid;
    gap: 0;
    border: 1px solid var(--border-light);
    border-radius: var(--radius-md);
    overflow: hidden;
    color: inherit;
    text-decoration: none;
    transition: border-color 200ms ease, box-shadow 200ms ease;
  }

  .trending-card:hover {
    border-color: var(--border);
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.06);
  }

  .trending-card-image {
    width: 100%;
    aspect-ratio: 16 / 9;
    object-fit: cover;
  }

  .trending-card-image-placeholder {
    width: 100%;
    aspect-ratio: 16 / 9;
    background: linear-gradient(135deg, var(--surface-soft) 0%, var(--border-light) 100%);
  }

  .trending-card-body {
    display: grid;
    gap: 0.6rem;
    padding: 1.1rem 1.25rem 1.25rem;
  }

  .trending-card-title {
    margin: 0;
    font-family: var(--font-serif);
    font-size: 1.15rem;
    font-weight: 700;
    color: var(--text-strong);
    line-height: 1.25;
    letter-spacing: -0.01em;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    transition: color 160ms ease;
  }

  .trending-card:hover .trending-card-title {
    color: var(--accent);
  }

  .trending-card-summary {
    margin: 0;
    color: var(--muted);
    font-size: 0.88rem;
    line-height: 1.5;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .trending-card-meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    padding-top: 0.35rem;
  }

  .trending-save-count {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    color: var(--accent);
    font-size: 0.8rem;
    font-weight: 600;
  }

  /* ── responsive ───────────────────────────────────────────── */

  @media (max-width: 720px) {
    .trending-grid {
      grid-template-columns: 1fr;
    }

    .bookmark-remove-btn {
      opacity: 1;
    }
  }
</style>
