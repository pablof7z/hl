<script lang="ts">
  import { browser } from '$app/environment';
  import RelayCard from '$lib/components/RelayCard.svelte';
  import { ndk } from '$lib/ndk/client';
  import {
    RELAY_FEED_LIST_KIND,
    latestListEvent,
    relayFeedHasUrl,
    relayUrlsFromEvent,
    setRelayFeedUrlPresence
  } from '$lib/ndk/lists';

  const currentUser = $derived(ndk.$currentUser);

  // ── My Relays (NDK relay feed list) ───────────────────────────
  const myRelaySet = ndk.$subscribe(() => {
    if (!browser || !currentUser) return undefined;
    return {
      filters: [{ kinds: [RELAY_FEED_LIST_KIND], authors: [currentUser.pubkey], limit: 20 }]
    };
  });
  const myRelayEvent = $derived(latestListEvent(myRelaySet.events));

  const myRelayUrls = $derived(relayUrlsFromEvent(myRelayEvent));

  async function removeRelay(relayUrl: string) {
    if (!currentUser) return;
    await setRelayFeedUrlPresence(ndk, myRelayEvent, relayUrl, false);
  }

  async function addRelay(relayUrl: string) {
    if (!currentUser) return;
    await setRelayFeedUrlPresence(ndk, myRelayEvent, relayUrl, true);
  }

  function isBookmarked(relayUrl: string): boolean {
    return relayFeedHasUrl(myRelayEvent, relayUrl);
  }

  async function toggleBookmark(relayUrl: string) {
    if (isBookmarked(relayUrl)) {
      await removeRelay(relayUrl);
    } else {
      await addRelay(relayUrl);
    }
  }

  // ── Network relay discovery (NDK relay feed list) ─────────────
  const networkRelaySets = ndk.$subscribe(() => {
    if (!browser) return undefined;
    return {
      filters: [{ kinds: [RELAY_FEED_LIST_KIND], limit: 100 }]
    };
  });

  const trendingRelays = $derived.by(() => {
    const counts = new Map<string, Set<string>>();
    for (const event of networkRelaySets.events) {
      if (currentUser && event.pubkey === currentUser.pubkey) continue;
      for (const url of relayUrlsFromEvent(event)) {
        const existing = counts.get(url);
        if (existing) {
          existing.add(event.pubkey);
        } else {
          counts.set(url, new Set([event.pubkey]));
        }
      }
    }
    return [...counts.entries()]
      .map(([url, users]) => ({ url, userCount: users.size }))
      .sort((a, b) => b.userCount - a.userCount)
      .slice(0, 20);
  });
</script>

<svelte:head>
  <title>Relays — Highlighter</title>
</svelte:head>

<div class="bookmarks-layout">
  <div class="bookmarks-main">
    {#if currentUser}
      <section class="bookmarks-section">
        <div class="bookmarks-section-header">
          <h2 class="bookmarks-section-title">My Relays</h2>
          <p class="bookmarks-section-desc">Relays you follow as magazines</p>
        </div>

        {#if myRelayUrls.length > 0}
          <div class="trending-grid">
            {#each myRelayUrls as relayUrl (relayUrl)}
              <RelayCard
                {relayUrl}
                onRemove={() => removeRelay(relayUrl)}
              />
            {/each}
          </div>
        {:else}
          <div class="bookmarks-empty">
            <p>No relays bookmarked yet</p>
            <p class="muted">Explore relays below and bookmark them to follow</p>
          </div>
        {/if}
      </section>
    {:else}
      <section class="bookmarks-section">
        <div class="bookmarks-section-header">
          <h2 class="bookmarks-section-title">My Relays</h2>
          <p class="bookmarks-section-desc">Log in to bookmark and follow relays</p>
        </div>
      </section>
    {/if}

    <section class="bookmarks-section">
      <div class="bookmarks-section-header">
        <h2 class="bookmarks-section-title">Relays Readers Are Exploring</h2>
        <p class="bookmarks-section-desc">Discover relays the community is reading from</p>
      </div>

      {#if trendingRelays.length > 0}
        <div class="trending-grid">
          {#each trendingRelays as { url, userCount } (url)}
            <RelayCard
              relayUrl={url}
              {userCount}
              bookmarked={isBookmarked(url)}
              showBookmarkToggle={!!currentUser}
              onToggleBookmark={() => toggleBookmark(url)}
            />
          {/each}
        </div>
      {:else if networkRelaySets.events.length > 0}
        <p class="muted">Analyzing which relays people follow...</p>
      {:else}
        <p class="muted">Discovering relays from the network...</p>
      {/if}
    </section>
  </div>
</div>
