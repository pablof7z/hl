<script lang="ts">
  import { browser } from '$app/environment';
  import { NDKEvent } from '@nostr-dev-kit/ndk';
  import StoryAuthor from '$lib/components/StoryAuthor.svelte';
  import { User } from '$lib/ndk/ui/user';
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

<div class="grid gap-10">
  {#if heroEntry}
    <section>
      <a class="grid gap-5 px-10 py-9 bg-[var(--pale-blue)] border-l-4 border-l-[rgba(31,108,159,0.45)] rounded-r-box text-inherit no-underline transition-[border-left-color] duration-200 hover:border-l-[var(--accent)]" href={`/note/${heroEntry.encode()}`}>
        <blockquote class="m-0 font-serif text-[clamp(1.3rem,2.2vw,1.85rem)] font-normal leading-[1.5] text-[var(--text-strong)]">
          {noteExcerpt(bestQuote(heroEntry), 500)}
        </blockquote>
        <div class="grid gap-[0.65rem]">
          <span class="text-[1.05rem] font-semibold text-[var(--accent)] transition-colors duration-[160ms]">
            {articleTitle(heroEntry.rawEvent())}
          </span>
          <div class="flex flex-wrap items-center gap-3">
            <StoryAuthor
              {ndk}
              pubkey={heroEntry.pubkey}
              avatarClass="article-author-avatar article-author-avatar-compact"
              compact
            />
            <span class="inline-flex items-center px-[0.55rem] py-[0.2rem] rounded-full bg-[rgba(31,108,159,0.1)] text-[var(--pale-blue-text)] text-[0.72rem] font-semibold tracking-[0.03em] whitespace-nowrap">
              {articleHighlights(heroEntry).length} highlight{articleHighlights(heroEntry).length === 1 ? '' : 's'}
              · {uniqueHighlighters(heroEntry)} reader{uniqueHighlighters(heroEntry) === 1 ? '' : 's'}
            </span>
          </div>
        </div>
      </a>
    </section>
  {/if}

  <div class="grid grid-cols-[1fr_20rem] gap-12 items-start max-[900px]:grid-cols-1">
    <div class="min-w-0">
      {#if entries.length === 0}
        <p class="text-base-content/50">No highlights yet.</p>
      {:else}
        <div class="grid grid-cols-2 gap-5 max-[720px]:grid-cols-1">
          {#each gridEntries as article, i (article.tagId())}
            {@const isWide = i % 3 === 0}
            {@const quotes = sampleQuotes(article, isWide ? 3 : 2)}
            {@const hlCount = articleHighlights(article).length}
            <a
              class="grid gap-4 p-5 border border-[var(--border-light)] rounded-box text-inherit no-underline transition-[border-color,box-shadow] duration-200 hover:border-base-300 hover:shadow-[0_4px_20px_rgba(0,0,0,0.06)] {isWide ? 'col-span-full max-[720px]:col-auto' : ''}"
              href={`/note/${article.encode()}`}
            >
              <div class="grid gap-[0.4rem]">
                <span class="font-serif text-[1.1rem] font-bold text-[var(--text-strong)] leading-[1.25] tracking-[-0.01em] transition-colors duration-[160ms]">
                  {articleTitle(article.rawEvent())}
                </span>
                <p class="m-0 text-base-content/50 text-[0.85rem] leading-[1.5] line-clamp-2">
                  {articleSummary(article.rawEvent(), 120)}
                </p>
                <div class="pt-1">
                  <StoryAuthor
                    {ndk}
                    pubkey={article.pubkey}
                    avatarClass="article-author-avatar article-author-avatar-compact"
                    compact
                  />
                </div>
              </div>

              <div class="grid gap-[0.6rem]">
                {#each quotes as quote}
                  <blockquote class="m-0 px-[0.85rem] py-[0.65rem] border-l-[3px] border-l-[rgba(31,108,159,0.3)] rounded-r bg-[var(--pale-blue)] text-[var(--text-strong)] font-serif text-[0.88rem] leading-[1.55]">
                    {noteExcerpt(quote, isWide ? 280 : 160)}
                  </blockquote>
                {/each}
              </div>

              <div class="flex items-center gap-[0.6rem]">
                <span class="inline-flex items-center px-[0.55rem] py-[0.2rem] rounded-full bg-[rgba(31,108,159,0.1)] text-[var(--pale-blue-text)] text-[0.72rem] font-semibold tracking-[0.03em] whitespace-nowrap">
                  {hlCount} highlight{hlCount === 1 ? '' : 's'}
                </span>
                <span class="text-[0.78rem] text-base-content/50">
                  {uniqueHighlighters(article)} reader{uniqueHighlighters(article) === 1 ? '' : 's'}
                </span>
              </div>
            </a>
          {/each}
        </div>
      {/if}
    </div>

    {#if sidebarEntries.length > 0}
      <aside class="sticky top-20 grid gap-4 max-[900px]:static">
        <span class="font-sans text-[0.72rem] font-semibold tracking-[0.08em] uppercase text-base-content/50">Most highlighted</span>
        <div class="grid">
          {#each sidebarEntries as article (article.tagId())}
            <a class="grid gap-[0.4rem] py-[0.85rem] border-b border-b-[var(--border-light)] text-inherit no-underline first:pt-0 last:border-b-0" href={`/note/${article.encode()}`}>
              <blockquote class="m-0 px-[0.6rem] py-[0.45rem] border-l-2 border-l-[rgba(31,108,159,0.3)] bg-[var(--surface-soft)] text-[var(--text)] font-serif text-[0.8rem] leading-[1.45] line-clamp-2">
                {noteExcerpt(bestQuote(article), 100)}
              </blockquote>
              <span class="text-[0.82rem] font-semibold text-[var(--text-strong)] leading-[1.3] overflow-hidden whitespace-nowrap text-ellipsis transition-colors duration-[160ms]">
                {articleTitle(article.rawEvent())}
              </span>
              <div class="flex items-center gap-[0.3rem] text-[0.72rem] text-base-content/50">
                <span class="min-w-0">
                  <User.Root {ndk} pubkey={article.pubkey}>
                    <User.Name field="displayName" class="text-[0.72rem] font-medium text-base-content/50" />
                  </User.Root>
                </span>
                <span class="text-base-300">·</span>
                <span class="whitespace-nowrap">{articleHighlights(article).length} highlights</span>
              </div>
            </a>
          {/each}
        </div>
      </aside>
    {/if}
  </div>
</div>
