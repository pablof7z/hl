<script lang="ts">
  import type { PageProps } from './$types';
  import { page } from '$app/state';
  import { browser } from '$app/environment';
  import { createFetchUser } from '@nostr-dev-kit/svelte';
  import { NDKEvent, NDKKind, type NostrEvent, type NDKFilter, nip19 } from '@nostr-dev-kit/ndk';
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
    return { filters: [{ kinds: [10003], authors: [pubkey], limit: 1 }] };
  });

  const bookmarkedAddresses = $derived.by(() => {
    const bookmarkEvent = userBookmarkList.events[0];
    if (!bookmarkEvent) return [];
    return bookmarkEvent.tags
      .filter((tag) => tag[0] === 'a' && tag[1]?.startsWith('30023:'))
      .map((tag) => tag[1]);
  });

  const bookmarkedArticles = ndk.$subscribe(() => {
    if (!browser || bookmarkedAddresses.length === 0) return undefined;
    const filters = bookmarkedAddresses.map((addr) => {
      const [kind, pubkey, identifier] = addr.split(':');
      return { kinds: [Number(kind)], authors: [pubkey], '#d': [identifier] } as NDKFilter;
    });
    return { filters };
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
  <section class="profile-container">
    <h1>We could not find that profile</h1>
    <p class="muted" style="margin: 0;">Try a different profile link or come back in a moment.</p>
  </section>
{:else}
  <section class="profile-container">
    {#if bannerUrl}
      <div class="profile-banner">
        <img src={bannerUrl} alt="" class="profile-banner-img" />
      </div>
    {/if}

    <div class="profile-header">
      <User.Root {ndk} pubkey={pubkey} profile={profile}>
        <User.Avatar class="profile-avatar author-avatar-centered" />
      </User.Root>

      <h1>{name}</h1>
      <p class="profile-bio">{bio}</p>

      <div class="profile-meta-row">
        {#if nip05}
          <span class="muted">{nip05}</span>
        {/if}
        <span class="muted">{storyCountLabel}</span>
        {#if latestDateLabel}
          <span class="muted">Latest: {latestDateLabel}</span>
        {/if}
        {#if website}
          <a class="profile-website-link" href={website} target="_blank" rel="noopener noreferrer">
            {websiteLabel(website)}
          </a>
        {/if}
      </div>

      {#if nipF1CustomFields.length > 0}
        <div class="definition-list profile-custom-fields">
          {#each nipF1CustomFields as field (field.key)}
            <div class="definition-row">
              <span>{field.key}</span>
              <p>{field.value}</p>
            </div>
          {/each}
        </div>
      {/if}

      {#if nipF1Music}
        <div class="profile-music">
          <audio bind:this={audioEl} src={nipF1Music} loop muted></audio>
          <button class="profile-music-btn" type="button" onclick={toggleMusic}>
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

      <div class="profile-actions">
        {#if pubkey && ndk.$currentUser && ndk.$currentUser.pubkey !== pubkey}
          <button
            class="follow-btn"
            class:following={ndk.$follows.has(pubkey)}
            onclick={() => ndk.$follows.has(pubkey) ? ndk.$follows.remove(pubkey) : ndk.$follows.add(pubkey)}
          >
            {ndk.$follows.has(pubkey) ? 'Following' : 'Follow'}
          </button>
        {/if}
        {#if isOwnProfile}
          <a href="/profile/edit" class="button-secondary profile-edit-btn">Edit profile</a>
        {/if}
      </div>
    </div>
  </section>

  <nav class="profile-tabs">
    <button
      class="profile-tab"
      class:active={activeTab === 'writing'}
      onclick={() => activeTab = 'writing'}
    >Writing</button>
    <button
      class="profile-tab"
      class:active={activeTab === 'highlights'}
      onclick={() => activeTab = 'highlights'}
    >Highlights</button>
    <button
      class="profile-tab"
      class:active={activeTab === 'bookmarks'}
      onclick={() => activeTab = 'bookmarks'}
    >Bookmarks</button>
  </nav>

  {#if activeTab === 'writing'}
    {#if articles.length > 0}
      <section class="article-feed profile-feed">
        {#each articles as event (event.id)}
          <ArticleCard {event} />
        {/each}
      </section>
    {:else}
      <section class="profile-container">
        <p class="muted" style="margin: 0;">No long-form articles loaded for this author yet.</p>
      </section>
    {/if}
  {:else if activeTab === 'highlights'}
    {#if sortedHighlights.length > 0}
      <section class="profile-feed profile-highlights">
        {#each sortedHighlights as highlight (highlight.id)}
          {@const source = highlightSourceLink(highlight)}
          <div class="highlight-item">
            <blockquote class="highlight-quote">
              {noteExcerpt(highlight.content, 400)}
            </blockquote>
            {#if source}
              {#if source.href.startsWith('/')}
                <a class="highlight-source" href={source.href}>{source.label}</a>
              {:else}
                <a class="highlight-source" href={source.href} target="_blank" rel="noopener noreferrer">{source.label}</a>
              {/if}
            {/if}
          </div>
        {/each}
      </section>
    {:else}
      <section class="profile-container">
        <p class="muted" style="margin: 0;">No highlights yet.</p>
      </section>
    {/if}
  {:else if activeTab === 'bookmarks'}
    {#if orderedBookmarks.length > 0}
      <section class="article-feed profile-feed">
        {#each orderedBookmarks as event (event.id)}
          <ArticleCard {event} showAuthor />
        {/each}
      </section>
    {:else if bookmarkedAddresses.length > 0}
      <section class="profile-container">
        <p class="muted" style="margin: 0;">Loading bookmarked articles...</p>
      </section>
    {:else}
      <section class="profile-container">
        <p class="muted" style="margin: 0;">No bookmarked articles yet.</p>
      </section>
    {/if}
  {/if}
{/if}

<style>
  .profile-container {
    max-width: var(--content-width);
    margin: 0 auto;
    border-radius: var(--radius-md);
    padding: 0 0 1rem;
  }

  .profile-banner {
    width: 100%;
    aspect-ratio: 4 / 1;
    overflow: hidden;
    border-radius: var(--radius-md) var(--radius-md) 0 0;
  }

  .profile-banner-img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .profile-banner + .profile-header {
    margin-top: -2.5rem;
  }

  .profile-header {
    display: grid;
    justify-items: center;
    gap: 0.75rem;
    text-align: center;
    padding: 0 1rem 1.5rem;
  }

  :global(.author-avatar-centered) {
    width: 5rem;
    height: 5rem;
    border: 3px solid var(--canvas);
    position: relative;
    z-index: 1;
  }

  .profile-bio {
    margin: 0;
    color: inherit;
    font-size: 1.02rem;
    max-width: 48ch;
  }

  .profile-meta-row {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 0.75rem;
    font-size: 0.85rem;
  }

  .profile-custom-fields {
    width: 100%;
    max-width: 24rem;
  }

  .profile-music {
    display: flex;
    justify-content: center;
  }

  .profile-music-btn {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.35rem 0.85rem;
    border: 1px solid var(--border);
    border-radius: 9999px;
    background: var(--surface-soft);
    color: var(--muted);
    font-size: 0.8rem;
    cursor: pointer;
    transition: color 120ms, border-color 120ms;
  }

  .profile-music-btn:hover {
    color: var(--text-strong);
    border-color: var(--text);
  }

  .profile-music-btn svg {
    width: 1rem;
    height: 1rem;
  }

  .profile-actions {
    display: flex;
    gap: 0.75rem;
    align-items: center;
  }

  .profile-edit-btn {
    font-size: 0.88rem;
    text-decoration: none;
  }

  .profile-website-link {
    color: var(--accent);
  }

  .profile-website-link:hover {
    color: var(--accent-hover);
  }

  .follow-btn {
    padding: 0.4rem 1.2rem;
    border-radius: 999px;
    border: 1px solid var(--accent);
    background: var(--accent);
    color: white;
    font-size: 0.9rem;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.15s, color 0.15s;
  }

  .follow-btn.following {
    background: transparent;
    color: var(--accent);
  }

  .follow-btn:hover {
    background: var(--accent-hover);
    border-color: var(--accent-hover);
    color: white;
  }

  /* ── tabs ──────────────────────────────────────────────────── */

  .profile-tabs {
    display: flex;
    justify-content: center;
    gap: 0;
    max-width: var(--content-width);
    margin: 0 auto;
    border-bottom: 1px solid var(--border-light);
  }

  .profile-tab {
    padding: 0.65rem 1.5rem;
    border: none;
    border-bottom: 2px solid transparent;
    background: none;
    color: var(--muted);
    font-size: 0.9rem;
    font-weight: 500;
    cursor: pointer;
    transition: color 160ms ease, border-color 160ms ease;
  }

  .profile-tab:hover {
    color: var(--text);
  }

  .profile-tab.active {
    color: var(--text-strong);
    border-bottom-color: var(--accent);
  }

  /* ── highlights tab ──────────────────────────────────────── */

  .profile-highlights {
    display: grid;
    gap: 0;
  }

  .highlight-item {
    display: grid;
    gap: 0.4rem;
    padding: 1.25rem 0;
    border-bottom: 1px solid var(--border-light);
  }

  .highlight-item:first-child {
    padding-top: 0;
  }

  .highlight-item:last-child {
    border-bottom: none;
  }

  .highlight-quote {
    margin: 0;
    padding: 0.75rem 1rem;
    border-left: 3px solid rgba(31, 108, 159, 0.35);
    border-radius: 0 var(--radius-sm) var(--radius-sm) 0;
    background: var(--pale-blue);
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: 0.95rem;
    line-height: 1.6;
  }

  .highlight-source {
    font-size: 0.78rem;
    padding-left: 1rem;
    color: var(--accent);
    text-decoration: none;
  }

  .highlight-source:hover {
    color: var(--accent-hover);
    text-decoration: underline;
  }

  .profile-feed {
    max-width: var(--content-width);
    margin: 0 auto;
  }

  .article-feed {
    display: grid;
  }
</style>
