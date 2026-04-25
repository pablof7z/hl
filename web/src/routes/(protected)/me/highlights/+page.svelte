<script lang="ts">
  import { browser } from '$app/environment';
  import { NDKKind } from '@nostr-dev-kit/ndk';
  import type { ArtifactRecord } from '$lib/ndk/artifacts';
  import HighlightSourceGroup from '$lib/features/highlights/HighlightSourceGroup.svelte';
  import { groupHighlightsBySource } from '$lib/features/highlights/grouping';
  import { buildFollowingFeedFilters, mergeFeed, type FeedItem } from '$lib/features/highlights/following-feed';
  import { fetchArtifactsByHighlightReferenceKeys } from '$lib/ndk/artifacts';
  import { ndk } from '$lib/ndk/client';
  import { DEFAULT_RELAYS, GROUP_RELAY_URLS } from '$lib/ndk/config';
  import {
    HIGHLIGHTER_HIGHLIGHT_KIND,
    HIGHLIGHTER_HIGHLIGHT_REPOST_KIND,
    hydrateHighlights,
    hydrateStandaloneHighlights,
    highlightFromEvent,
    resolveUserHighlightRelayUrls
  } from '$lib/ndk/highlights';
  import { buildJoinedRooms, groupIdFromEvent } from '$lib/ndk/groups';
  import HighlightCard from '$lib/features/highlights/HighlightCard.svelte';
  import ArticleCard from '$lib/components/ArticleCard.svelte';

  let activeTab = $state<'mine' | 'following'>('mine');

  // ── Mine tab state ──────────────────────────────────────────────────────

  const currentUser = $derived(ndk.$currentUser);
  let highlightRelayUrls = $state<string[]>(DEFAULT_RELAYS);
  let resolvingRelayList = $state(false);

  $effect(() => {
    if (!browser || !currentUser) {
      highlightRelayUrls = DEFAULT_RELAYS;
      resolvingRelayList = false;
      return;
    }

    let cancelled = false;
    resolvingRelayList = true;

    void resolveUserHighlightRelayUrls(ndk, currentUser.pubkey)
      .then((relayUrls) => {
        if (!cancelled) {
          highlightRelayUrls = relayUrls;
        }
      })
      .finally(() => {
        if (!cancelled) {
          resolvingRelayList = false;
        }
      });

    return () => {
      cancelled = true;
    };
  });

  const authoredHighlightFeed = ndk.$subscribe(() => {
    if (!browser || !currentUser) return undefined;

    return {
      filters: [{ kinds: [HIGHLIGHTER_HIGHLIGHT_KIND], authors: [currentUser.pubkey], limit: 96 }],
      relayUrls: highlightRelayUrls,
      closeOnEose: true
    };
  });

  const authoredShareFeed = ndk.$subscribe(() => {
    if (!browser || !currentUser) return undefined;

    return {
      filters: [{ kinds: [HIGHLIGHTER_HIGHLIGHT_REPOST_KIND], authors: [currentUser.pubkey], limit: 128 }],
      relayUrls: GROUP_RELAY_URLS,
      closeOnEose: true
    };
  });

  const highlights = $derived(
    hydrateHighlights([...authoredHighlightFeed.events], [...authoredShareFeed.events])
  );
  const membershipFeed = ndk.$subscribe(() => {
    if (!browser || !currentUser) return undefined;

    return {
      filters: [{ kinds: [NDKKind.GroupAdmins, NDKKind.GroupMembers], '#p': [currentUser.pubkey], limit: 128 }],
      relayUrls: GROUP_RELAY_URLS,
      closeOnEose: true
    };
  });
  const membershipGroupIds = $derived.by(() => {
    const ids = new Set<string>();

    for (const event of membershipFeed.events) {
      const groupId = groupIdFromEvent(event);
      if (groupId) ids.add(groupId);
    }

    return [...ids];
  });
  const metadataFeed = ndk.$subscribe(() => {
    if (!browser || membershipGroupIds.length === 0) return undefined;

    return {
      filters: [{ kinds: [NDKKind.GroupMetadata], '#d': membershipGroupIds, limit: Math.max(membershipGroupIds.length * 2, 32) }],
      relayUrls: GROUP_RELAY_URLS,
      closeOnEose: true
    };
  });
  const rooms = $derived(
    currentUser
      ? buildJoinedRooms(currentUser.pubkey, [...metadataFeed.events], [...membershipFeed.events])
      : []
  );

  let artifactsByReference = $state<Map<string, ArtifactRecord>>(new Map());
  let resolvingArtifacts = $state(false);
  const highlightGroups = $derived(groupHighlightsBySource(highlights, artifactsByReference));

  $effect(() => {
    if (!browser) {
      artifactsByReference = new Map();
      return;
    }

    const referenceKeys = [...new Set(highlights.map((highlight) => highlight.sourceReferenceKey).filter(Boolean))];
    if (referenceKeys.length === 0) {
      artifactsByReference = new Map();
      return;
    }

    let cancelled = false;
    resolvingArtifacts = true;

    void fetchArtifactsByHighlightReferenceKeys(ndk, referenceKeys)
      .then((artifacts) => {
        if (cancelled) return;
        artifactsByReference = artifacts;
      })
      .finally(() => {
        if (!cancelled) {
          resolvingArtifacts = false;
        }
      });

    return () => {
      cancelled = true;
    };
  });

  // ── Following tab state ─────────────────────────────────────────────────

  const followPubkeys = $derived.by<string[]>(() => {
    if (!browser || !currentUser) return [];
    return [...(ndk.$follows ?? [])].slice(0, 500);
  });

  let followingLimit = $state(60);

  const followingFeed = ndk.$subscribe(() => {
    if (!browser || followPubkeys.length === 0) return undefined;
    const filters = buildFollowingFeedFilters(followPubkeys).map((f) => ({
      ...f,
      limit: followingLimit
    }));
    return { filters, closeOnEose: true };
  });

  const followingItems = $derived<FeedItem[]>(
    followPubkeys.length > 0 ? mergeFeed([...followingFeed.events]) : []
  );

  function loadMoreFollowing() {
    followingLimit += 60;
  }

  // Build standalone HydratedHighlight from a FeedItem with kind:'highlight'
  // so we can pass it to HighlightCard without extra overhead.
  function hydratedFromFeedItem(item: FeedItem & { kind: 'highlight' }) {
    const record = highlightFromEvent(item.rawEvent);
    return { ...record, shares: [], shareCount: 0, latestSharedAt: null };
  }
</script>

<svelte:head>
  <title>My Highlights — Highlighter</title>
</svelte:head>

<section class="grid gap-6">
  <header class="grid gap-[0.35rem]">
    <h2 class="m-0 font-serif text-base-content leading-[1.1] tracking-[-0.02em]" style="font-size: clamp(1.6rem, 3vw, 2.2rem);">
      Highlights
    </h2>
  </header>

  <div role="tablist" class="tabs tabs-border">
    <button
      type="button"
      role="tab"
      class="tab"
      class:tab-active={activeTab === 'mine'}
      onclick={() => (activeTab = 'mine')}
    >
      Mine
    </button>
    <button
      type="button"
      role="tab"
      class="tab"
      class:tab-active={activeTab === 'following'}
      onclick={() => (activeTab = 'following')}
    >
      Following
    </button>
  </div>

  {#if activeTab === 'mine'}
    <section class="grid gap-[0.9rem]" style="grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));">
      <div class="grid gap-[0.4rem] p-4 border border-base-300 rounded-[1.1rem] bg-base-100">
        <p class="m-0 text-primary text-[0.8rem] font-bold tracking-[0.08em] uppercase">Saved highlights</p>
        <strong class="font-serif text-[2rem] leading-none text-base-content">{highlights.length}</strong>
        <span class="m-0 text-base-content/50 leading-relaxed">You can share the same highlight into more than one room.</span>
      </div>
      <div class="grid gap-[0.4rem] p-4 border border-base-300 rounded-[1.1rem] bg-base-100">
        <p class="m-0 text-primary text-[0.8rem] font-bold tracking-[0.08em] uppercase">Loaded rooms</p>
        <strong class="font-serif text-[2rem] leading-none text-base-content">{rooms.length}</strong>
        <span class="m-0 text-base-content/50 leading-relaxed">Available as share-again targets on each card.</span>
      </div>
      <div class="grid gap-[0.4rem] p-4 border border-base-300 rounded-[1.1rem] bg-base-100">
        <p class="m-0 text-primary text-[0.8rem] font-bold tracking-[0.08em] uppercase">Sources checked</p>
        <strong class="font-serif text-[2rem] leading-none text-base-content">{highlightRelayUrls.length}</strong>
        <span class="m-0 text-base-content/50 leading-relaxed">Loaded from the places where your highlights are stored, plus Highlighter's fallback.</span>
      </div>
    </section>

    {#if highlightGroups.length === 0}
      <section class="grid gap-[0.4rem] p-4 border border-base-300 rounded-[1.1rem] bg-base-100">
        <p class="m-0 font-bold text-base-content">
          {resolvingRelayList ? 'Looking for your highlights…' : 'No highlights found yet.'}
        </p>
        <p class="m-0 text-base-content/50 leading-relaxed">
          {#if resolvingRelayList}
            Checking the relays where your highlights live.
          {:else}
            Save a highlight from any source and it will show up here.
          {/if}
        </p>
      </section>
    {:else}
      <section class="grid gap-[0.9rem]">
        {#each highlightGroups as group (group.referenceKey)}
          <HighlightSourceGroup {group} {rooms} showShareControl={true} />
        {/each}
      </section>
    {/if}

    {#if resolvingArtifacts}
      <p class="m-0 text-base-content/50 leading-relaxed text-[0.88rem]">Resolving source details…</p>
    {/if}
  {:else}
    {#if followPubkeys.length === 0}
      <div class="grid gap-3 p-6 border border-base-300 rounded-[1.1rem] bg-base-100 text-center">
        <p class="m-0 font-bold text-base-content">Nothing here yet</p>
        <p class="m-0 text-base-content/50 leading-relaxed">
          Follow some authors to see their reads and highlights here.
        </p>
        <a href="/discover" class="btn btn-primary rounded-full w-fit mx-auto">Discover authors</a>
      </div>
    {:else if followingItems.length === 0 && followingFeed.eosed}
      <div class="grid gap-3 p-6 border border-base-300 rounded-[1.1rem] bg-base-100 text-center">
        <p class="m-0 font-bold text-base-content">Nothing from your follows yet</p>
        <p class="m-0 text-base-content/50 leading-relaxed">
          Highlights and articles from people you follow will appear here as they publish.
        </p>
      </div>
    {:else if followingItems.length === 0}
      <div class="grid gap-3 p-6 border border-base-300 rounded-[1.1rem] bg-base-100">
        <p class="m-0 text-base-content/50 leading-relaxed text-[0.88rem]">Loading feed from your follows…</p>
      </div>
    {:else}
      <section class="grid gap-[0.9rem]">
        {#each followingItems as item (item.eventId)}
          {#if item.kind === 'highlight'}
            <div class="border border-base-300 rounded-[1.1rem] bg-base-100 p-4 grid gap-3">
              <div class="flex items-center gap-2">
                <span class="badge badge-primary badge-sm">Highlight</span>
              </div>
              <HighlightCard highlight={hydratedFromFeedItem(item)} />
            </div>
          {:else}
            <div class="border border-base-300 rounded-[1.1rem] bg-base-100 p-4 grid gap-3">
              <div class="flex items-center gap-2">
                <span class="badge badge-ghost badge-sm">Read</span>
              </div>
              <ArticleCard event={item.rawEvent} showAuthor={true} />
            </div>
          {/if}
        {/each}
      </section>

      <div class="flex justify-center pt-2">
        <button
          type="button"
          class="btn btn-ghost rounded-full"
          onclick={loadMoreFollowing}
        >
          Load more
        </button>
      </div>
    {/if}
  {/if}
</section>
