<script lang="ts">
  import { browser } from '$app/environment';
  import { NDKKind } from '@nostr-dev-kit/ndk';
  import type { ArtifactRecord } from '$lib/ndk/artifacts';
  import HighlightSourceGroup from '$lib/features/highlights/HighlightSourceGroup.svelte';
  import { groupHighlightsBySource } from '$lib/features/highlights/grouping';
  import { fetchArtifactsByHighlightReferenceKeys } from '$lib/ndk/artifacts';
  import { ndk } from '$lib/ndk/client';
  import { DEFAULT_RELAYS, GROUP_RELAY_URLS } from '$lib/ndk/config';
  import {
    HIGHLIGHTER_HIGHLIGHT_KIND,
    HIGHLIGHTER_HIGHLIGHT_REPOST_KIND,
    hydrateHighlights,
    resolveUserHighlightRelayUrls
  } from '$lib/ndk/highlights';
  import { buildJoinedRooms, groupIdFromEvent } from '$lib/ndk/groups';

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
</script>

<svelte:head>
  <title>My Highlights — Highlighter</title>
</svelte:head>

<section class="grid gap-6">
  <header class="grid gap-[0.35rem]">
    <h2 class="m-0 font-serif text-base-content leading-[1.1] tracking-[-0.02em]" style="font-size: clamp(1.6rem, 3vw, 2.2rem);">
      Your highlights
    </h2>
  </header>

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
</section>
