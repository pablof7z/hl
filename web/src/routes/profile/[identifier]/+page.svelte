<script lang="ts">
  import type { PageProps } from './$types';
  import { page } from '$app/state';
  import { browser } from '$app/environment';
  import { createFetchUser } from '@nostr-dev-kit/svelte';
  import { NDKEvent, NDKKind, type NostrEvent, nip19 } from '@nostr-dev-kit/ndk';
  import { User } from '$lib/ndk/ui/user';
  import {
    articlePublishedAt,
    cleanText,
    displayNip05,
    displayName,
    formatDisplayDate,
    noteExcerpt
  } from '$lib/ndk/format';
  import ArticleCard from '$lib/components/ArticleCard.svelte';
  import { ndk } from '$lib/ndk/client';
  import {
    BOOKMARK_LIST_KIND,
    bookmarkAddressFilters,
    bookmarkAddressesFromEvent,
    latestListEvent
  } from '$lib/ndk/lists';
  import { safeUserPubkey } from '$lib/ndk/user';

  type Tab = 'writing' | 'highlights' | 'bookmarks';
  let activeTab: Tab = $state('writing');

  let { data }: PageProps = $props();
  const routeIdentifier = $derived(page.params.identifier || data.identifier || '');
  const user = createFetchUser(ndk, () => routeIdentifier || data.npub || data.pubkey || '');
  const profile = $derived(user.profile ?? data.profile);
  const pubkey = $derived(data.pubkey || safeUserPubkey(user));
  const isOwnProfile = $derived(Boolean(pubkey && ndk.$currentUser && ndk.$currentUser.pubkey === pubkey));

  // ── NIP-F1 subscription ────────────────────────────────────────
  const nipF1Sub = ndk.$subscribe(() => {
    if (!browser || !pubkey) return undefined;
    return {
      filters: [{ kinds: [19999 as NDKKind], authors: [pubkey], limit: 1 }]
    };
  });

  const nipF1Event = $derived(nipF1Sub.events[0] ?? null);
  const nipF1Tags = $derived(nipF1Event?.tags ?? []);
  const nipF1BgColor = $derived(nipF1Tags.find((t) => t[0] === 'background-color')?.[1] ?? '');
  const nipF1FgColor = $derived(nipF1Tags.find((t) => t[0] === 'foreground-color')?.[1] ?? '');
  const nipF1Music = $derived(nipF1Tags.find((t) => t[0] === 'background-music')?.[1] ?? '');
  const nipF1PriorityKinds = $derived.by(() => {
    const raw = nipF1Tags.find((t) => t[0] === 'priority_kinds')?.[1];
    return raw ? raw.split(',').map(Number).filter((n) => !isNaN(n)) : [];
  });
  const nipF1CustomFields = $derived(
    nipF1Tags
      .filter((t) => t[0] === 'custom' && t[1] && t[2])
      .map((t) => ({ key: t[1], value: t[2] }))
  );

  // ── articles subscription using priority kinds if set ──────────
  const articleKinds = $derived(nipF1PriorityKinds.length > 0 ? nipF1PriorityKinds : [30023]);

  const liveArticles = ndk.$subscribe(() => {
    if (!browser || !pubkey) return undefined;

    return {
      filters: [{ kinds: articleKinds, authors: [pubkey], limit: 12 }]
    };
  });

  const seedArticles = $derived(
    (data.seedArticles ?? []).map((event: NostrEvent) => new NDKEvent(ndk, event))
  );

  const articles = $derived(liveArticles.events.length > 0 ? liveArticles.events : seedArticles);

  // ── highlights subscription ──────────────────────────────────
  const userHighlights = ndk.$subscribe(() => {
    if (!browser || !pubkey || activeTab !== 'highlights') return undefined;
    return { filters: [{ kinds: [9802], authors: [pubkey], limit: 50 }] };
  });

  const sortedHighlights = $derived(
    userHighlights.events.toSorted((a, b) => (b.created_at ?? 0) - (a.created_at ?? 0))
  );

  // ── bookmarks subscription ──────────────────────────────────
  const userBookmarkList = ndk.$subscribe(() => {
    if (!browser || !pubkey || activeTab !== 'bookmarks') return undefined;
    return { filters: [{ kinds: [BOOKMARK_LIST_KIND], authors: [pubkey], limit: 20 }] };
  });
  const userBookmarkListEvent = $derived(latestListEvent(userBookmarkList.events));

  const bookmarkedAddresses = $derived.by(() => {
    return bookmarkAddressesFromEvent(userBookmarkListEvent, '30023:');
  });
  const bookmarkFilters = $derived(bookmarkAddressFilters(bookmarkedAddresses));

  const bookmarkedArticles = ndk.$subscribe(() => {
    if (!browser || bookmarkFilters.length === 0) return undefined;
    return { filters: bookmarkFilters };
  });

  const bookmarkedArticleLookup = $derived.by(() => {
    const lookup = new Map<string, NDKEvent>();
    for (const article of bookmarkedArticles.events) {
      lookup.set(article.tagId(), article);
    }
    return lookup;
  });

  const orderedBookmarks = $derived.by(() => {
    return bookmarkedAddresses
      .map((addr) => bookmarkedArticleLookup.get(addr))
      .filter((article): article is NDKEvent => Boolean(article));
  });

  const missing = $derived(!pubkey && data.missing && user.$loaded);
  const name = $derived(pubkey ? displayName(profile, 'Author') : 'Author');
  const bio = $derived.by(() => {
    const candidate = cleanText(profile?.about) || cleanText(profile?.bio);
    if (!candidate || candidate === '~' || candidate === '-' || candidate === '_') {
      return 'Recent writing collected here.';
    }
    return candidate;
  });
  const nip05 = $derived(displayNip05(profile));
  const bannerUrl = $derived(cleanText(profile?.banner));
  const website = $derived(cleanText(typeof profile?.website === 'string' ? profile.website : ''));
  const storyCountLabel = $derived(`${articles.length} ${articles.length === 1 ? 'story' : 'stories'}`);
  const latestDateLabel = $derived(
    articles[0] ? formatDisplayDate(articlePublishedAt(articles[0].rawEvent())) : ''
  );

  // ── music player state ─────────────────────────────────────────
  let musicPlaying = $state(false);
  let audioEl: HTMLAudioElement | null = $state(null);

  function toggleMusic() {
    if (!audioEl) return;
    if (musicPlaying) {
      audioEl.pause();
      musicPlaying = false;
    } else {
      audioEl.muted = false;
      void audioEl.play();
      musicPlaying = true;
    }
  }

  function websiteLabel(url: string): string {
    try {
      return new URL(url).hostname.replace(/^www\./, '');
    } catch {
      return url;
    }
  }

  // ── highlight source link helpers ─────────────────────────────
  function highlightSourceLink(highlight: NDKEvent): { href: string; label: string } | null {
    const aTag = highlight.tags.find((t) => t[0] === 'a')?.[1]?.trim();
    if (aTag) {
      const parts = aTag.split(':');
      if (parts.length >= 3) {
        const [kindStr, hPubkey, identifier] = parts;
        const kind = Number(kindStr);
        if (!isNaN(kind) && hPubkey && identifier) {
          try {
            const naddr = nip19.naddrEncode({ kind, pubkey: hPubkey, identifier });
            const label = identifier.replace(/-/g, ' ');
            return { href: `/note/${naddr}`, label };
          } catch {
            // fall through to r tag
          }
        }
      }
    }
    const rTag = highlight.tags.find((t) => t[0] === 'r')?.[1]?.trim();
    if (rTag) {
      try {
        const hostname = new URL(rTag).hostname.replace(/^www\./, '');
        return { href: rTag, label: hostname };
      } catch {
        return { href: rTag, label: rTag };
      }
    }
    return null;
  }

  // ── Apply NIP-F1 colors to the whole page ──────────────────────
  $effect(() => {
    if (!browser) return;
    if (nipF1BgColor) {
      document.body.style.setProperty('background-color', nipF1BgColor);
    } else {
      document.body.style.removeProperty('background-color');
    }
    if (nipF1FgColor) {
      document.body.style.setProperty('color', nipF1FgColor);
    } else {
      document.body.style.removeProperty('color');
    }
    return () => {
      document.body.style.removeProperty('background-color');
      document.body.style.removeProperty('color');
    };
  });
</script>

{#if missing}
  <section class="max-w-[var(--content-width)] mx-auto rounded-box pb-4">
    <h1>We could not find that profile</h1>
    <p class="text-base-content/50 m-0">Try a different profile link or come back in a moment.</p>
  </section>
{:else}
  <section class="max-w-[var(--content-width)] mx-auto rounded-box pb-4">
    {#if bannerUrl}
      <div class="w-full aspect-[4/1] overflow-hidden rounded-t-box">
        <img src={bannerUrl} alt="" class="w-full h-full object-cover" />
      </div>
    {/if}

    <div class="grid justify-items-center gap-3 text-center px-4 pb-6" class:-mt-10={!!bannerUrl}>
      <User.Root {ndk} pubkey={pubkey} profile={profile}>
        <User.Avatar class="profile-avatar author-avatar-centered" />
      </User.Root>

      <h1>{name}</h1>
      <p class="m-0 text-[1.02rem] max-w-[48ch]">{bio}</p>

      <div class="flex flex-wrap justify-center gap-3 text-[0.85rem]">
        {#if nip05}
          <span class="text-base-content/50">{nip05}</span>
        {/if}
        <span class="text-base-content/50">{storyCountLabel}</span>
        {#if latestDateLabel}
          <span class="text-base-content/50">Latest: {latestDateLabel}</span>
        {/if}
        {#if website}
          <a class="text-primary hover:text-primary/80" href={website} target="_blank" rel="noopener noreferrer">
            {websiteLabel(website)}
          </a>
        {/if}
      </div>

      {#if nipF1CustomFields.length > 0}
        <div class="definition-list w-full max-w-[24rem]">
          {#each nipF1CustomFields as field (field.key)}
            <div class="definition-row">
              <span>{field.key}</span>
              <p>{field.value}</p>
            </div>
          {/each}
        </div>
      {/if}

      {#if nipF1Music}
        <div class="flex justify-center">
          <audio bind:this={audioEl} src={nipF1Music} loop muted></audio>
          <button
            class="inline-flex items-center gap-1.5 px-[0.85rem] py-[0.35rem] border border-base-300 rounded-full bg-base-200 text-base-content/50 text-[0.8rem] cursor-pointer transition-colors hover:text-base-content hover:border-base-content [&_svg]:w-4 [&_svg]:h-4"
            type="button"
            onclick={toggleMusic}
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
              {#if musicPlaying}
                <rect x="6" y="4" width="4" height="16" /><rect x="14" y="4" width="4" height="16" />
              {:else}
                <polygon points="5 3 19 12 5 21 5 3" />
              {/if}
            </svg>
            {musicPlaying ? 'Pause music' : 'Play music'}
          </button>
        </div>
      {/if}

      <div class="flex gap-3 items-center">
        {#if pubkey && ndk.$currentUser && ndk.$currentUser.pubkey !== pubkey}
          <button
            class={ndk.$follows.has(pubkey) ? 'btn btn-outline btn-primary rounded-full' : 'btn btn-primary rounded-full'}
            onclick={() => ndk.$follows.has(pubkey) ? ndk.$follows.remove(pubkey) : ndk.$follows.add(pubkey)}
          >
            {ndk.$follows.has(pubkey) ? 'Following' : 'Follow'}
          </button>
        {/if}
        {#if isOwnProfile}
          <a href="/profile/edit" class="btn text-[0.88rem] no-underline">Edit profile</a>
        {/if}
      </div>
    </div>
  </section>

  <nav class="flex justify-center max-w-[var(--content-width)] mx-auto border-b border-base-300">
    <button
      class="px-6 py-[0.65rem] border-none border-b-2 bg-transparent text-[0.9rem] font-medium cursor-pointer transition-colors {activeTab === 'writing' ? 'text-base-content border-b-primary' : 'text-base-content/50 border-b-transparent hover:text-base-content'}"
      onclick={() => activeTab = 'writing'}
    >Writing</button>
    <button
      class="px-6 py-[0.65rem] border-none border-b-2 bg-transparent text-[0.9rem] font-medium cursor-pointer transition-colors {activeTab === 'highlights' ? 'text-base-content border-b-primary' : 'text-base-content/50 border-b-transparent hover:text-base-content'}"
      onclick={() => activeTab = 'highlights'}
    >Highlights</button>
    <button
      class="px-6 py-[0.65rem] border-none border-b-2 bg-transparent text-[0.9rem] font-medium cursor-pointer transition-colors {activeTab === 'bookmarks' ? 'text-base-content border-b-primary' : 'text-base-content/50 border-b-transparent hover:text-base-content'}"
      onclick={() => activeTab = 'bookmarks'}
    >Bookmarks</button>
  </nav>

  {#if activeTab === 'writing'}
    {#if articles.length > 0}
      <section class="grid max-w-[var(--content-width)] mx-auto">
        {#each articles as event (event.id)}
          <ArticleCard {event} />
        {/each}
      </section>
    {:else}
      <section class="max-w-[var(--content-width)] mx-auto rounded-box pb-4">
        <p class="text-base-content/50 m-0">No long-form articles loaded for this author yet.</p>
      </section>
    {/if}
  {:else if activeTab === 'highlights'}
    {#if sortedHighlights.length > 0}
      <section class="grid max-w-[var(--content-width)] mx-auto">
        {#each sortedHighlights as highlight (highlight.id)}
          {@const source = highlightSourceLink(highlight)}
          <div class="grid gap-1.5 py-5 border-b border-base-300 first:pt-0 last:border-b-0">
            <blockquote class="m-0 py-3 px-4 border-l-[3px] border-l-[rgba(31,108,159,0.35)] rounded-r font-serif text-[0.95rem] leading-relaxed text-base-content" style="background: var(--pale-blue)">
              {noteExcerpt(highlight.content, 400)}
            </blockquote>
            {#if source}
              {#if source.href.startsWith('/')}
                <a class="text-[0.78rem] pl-4 text-primary no-underline hover:text-primary/80 hover:underline" href={source.href}>{source.label}</a>
              {:else}
                <a class="text-[0.78rem] pl-4 text-primary no-underline hover:text-primary/80 hover:underline" href={source.href} target="_blank" rel="noopener noreferrer">{source.label}</a>
              {/if}
            {/if}
          </div>
        {/each}
      </section>
    {:else}
      <section class="max-w-[var(--content-width)] mx-auto rounded-box pb-4">
        <p class="text-base-content/50 m-0">No highlights yet.</p>
      </section>
    {/if}
  {:else if activeTab === 'bookmarks'}
    {#if orderedBookmarks.length > 0}
      <section class="grid max-w-[var(--content-width)] mx-auto">
        {#each orderedBookmarks as event (event.id)}
          <ArticleCard {event} showAuthor />
        {/each}
      </section>
    {:else if bookmarkedAddresses.length > 0}
      <section class="max-w-[var(--content-width)] mx-auto rounded-box pb-4">
        <p class="text-base-content/50 m-0">Loading bookmarked articles...</p>
      </section>
    {:else}
      <section class="max-w-[var(--content-width)] mx-auto rounded-box pb-4">
        <p class="text-base-content/50 m-0">No bookmarked articles yet.</p>
      </section>
    {/if}
  {/if}
{/if}

<style>
  :global(.author-avatar-centered) {
    width: 5rem;
    height: 5rem;
    border: 3px solid var(--canvas);
    position: relative;
    z-index: 1;
  }
</style>
