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
  import { buildJoinedCommunities, groupIdFromEvent } from '$lib/ndk/groups';

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
  const communities = $derived(
    currentUser
      ? buildJoinedCommunities(currentUser.pubkey, [...metadataFeed.events], [...membershipFeed.events])
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

<section class="me-highlights-page">
  <header class="page-copy">
    <p class="eyebrow">Highlights</p>
    <h2>Everything you highlight lives here.</h2>
    <p>
      Everything you highlight lives here, even after you share it across different communities.
    </p>
  </header>

  <section class="me-summary">
    <div class="summary-card">
      <p class="summary-label">Saved highlights</p>
      <strong>{highlights.length}</strong>
      <span>You can share the same highlight into more than one community.</span>
    </div>
    <div class="summary-card">
      <p class="summary-label">Loaded communities</p>
      <strong>{communities.length}</strong>
      <span>Available as share-again targets on each card.</span>
    </div>
    <div class="summary-card">
      <p class="summary-label">Sources checked</p>
      <strong>{highlightRelayUrls.length}</strong>
      <span>Loaded from the places where your highlights are stored, plus Highlighter’s fallback.</span>
    </div>
  </section>

  {#if highlightGroups.length === 0}
    <section class="empty-state">
      <p>{resolvingRelayList ? 'Looking for your highlights…' : 'No highlights found yet.'}</p>
      <p>
        {#if resolvingRelayList}
          Highlighter is checking the places where your highlights live.
        {:else}
          Share an artifact into a community, save an excerpt, and it will appear here as your
          permanent entry in your vault.
        {/if}
      </p>
    </section>
  {:else}
    <section class="highlight-groups">
      {#each highlightGroups as group (group.referenceKey)}
        <HighlightSourceGroup {group} {communities} showShareControl={true} />
      {/each}
    </section>
  {/if}

  {#if resolvingArtifacts}
    <p class="loading">Resolving artifact metadata…</p>
  {/if}
</section>

<style>
  .me-highlights-page {
    display: grid;
    gap: 1.5rem;
  }

  .page-copy {
    display: grid;
    gap: 0.35rem;
  }

  .eyebrow,
  .summary-label {
    margin: 0;
    color: var(--accent);
    font-size: 0.8rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  h2 {
    margin: 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: clamp(1.6rem, 3vw, 2.2rem);
    line-height: 1.1;
    letter-spacing: -0.02em;
  }

  .page-copy p:last-child,
  .summary-card span,
  .empty-state p,
  .loading {
    margin: 0;
    color: var(--muted);
    line-height: 1.65;
  }

  .me-summary {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 0.9rem;
  }

  .summary-card,
  .empty-state {
    display: grid;
    gap: 0.4rem;
    padding: 1rem 1.1rem;
    border: 1px solid var(--border);
    border-radius: 1.1rem;
    background: var(--surface);
  }

  .summary-card strong {
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: 2rem;
    line-height: 1;
  }

  .highlight-groups {
    display: grid;
    gap: 0.9rem;
  }

  .empty-state p:first-child {
    color: var(--text-strong);
    font-weight: 700;
  }

  .loading {
    font-size: 0.88rem;
  }
</style>
