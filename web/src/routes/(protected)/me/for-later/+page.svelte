<script lang="ts">
  import { browser } from '$app/environment';
  import { NDKKind } from '@nostr-dev-kit/ndk';
  import { ndk } from '$lib/ndk/client';
  import ForLaterCard from '$lib/features/vault/ForLaterCard.svelte';
  import SaveForLaterForm from '$lib/features/vault/SaveForLaterForm.svelte';
  import { listForLaterArtifacts, type ForLaterItem } from '$lib/features/vault/vault';
  import { GROUP_RELAY_URLS } from '$lib/ndk/config';
  import { buildJoinedCommunities, groupIdFromEvent } from '$lib/ndk/groups';

  const currentUser = $derived(ndk.$currentUser);
  let items = $state<ForLaterItem[]>([]);
  let loadingItems = $state(false);
  let storageError = $state('');

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
  const readyCount = $derived(
    items.filter((item) => item.communityIds.length === 0 && item.teaser.trim().length > 0).length
  );
  const needsTeaserCount = $derived(
    items.filter((item) => item.communityIds.length === 0 && item.teaser.trim().length === 0).length
  );
  const sharedCount = $derived(items.filter((item) => item.communityIds.length > 0).length);

  $effect(() => {
    if (!browser) {
      items = [];
      return;
    }

    let cancelled = false;
    loadingItems = true;
    storageError = '';

    void listForLaterArtifacts()
      .then((savedItems) => {
        if (!cancelled) {
          items = savedItems;
        }
      })
      .catch((error) => {
        if (!cancelled) {
          storageError =
            error instanceof Error ? error.message : 'Could not load your local For Later queue.';
        }
      })
      .finally(() => {
        if (!cancelled) {
          loadingItems = false;
        }
      });

    return () => {
      cancelled = true;
    };
  });

  function upsertItem(nextItem: ForLaterItem) {
    items = [nextItem, ...items.filter((item) => item.id !== nextItem.id)].toSorted(
      (left, right) => right.savedAt - left.savedAt
    );
  }

  function removeItem(id: string) {
    items = items.filter((item) => item.id !== id);
  }
</script>

<svelte:head>
  <title>For Later — Highlighter</title>
</svelte:head>

<section class="for-later-page">
  <header class="page-copy">
    <p class="eyebrow">For Later</p>
    <h2>Keep sources local until you know what to do with them.</h2>
    <p>
      Save articles, books, podcasts, and other sources privately. Add a teaser now or later,
      then move them into a community when the timing is right.
    </p>
  </header>

  <section class="me-summary">
    <div class="summary-card">
      <p class="summary-label">Saved items</p>
      <strong>{items.length}</strong>
      <span>Private to this browser for MVP.</span>
    </div>
    <div class="summary-card">
      <p class="summary-label">Ready to share</p>
      <strong>{readyCount}</strong>
      <span>Saved items with a teaser and no community yet.</span>
    </div>
    <div class="summary-card">
      <p class="summary-label">Needs teaser</p>
      <strong>{needsTeaserCount}</strong>
      <span>Saved privately, but still waiting for framing.</span>
    </div>
    <div class="summary-card">
      <p class="summary-label">Already shared</p>
      <strong>{sharedCount}</strong>
      <span>Items you are still tracking after they reached at least one community.</span>
    </div>
  </section>

  <SaveForLaterForm onSaved={upsertItem} />

  {#if communities.length === 0}
    <div class="community-note">
      <p>Join or create a community to unlock the “Move to community” action for saved items.</p>
      <div class="community-actions">
        <a href="/discover">Browse communities</a>
        <a href="/community/create">Create a community</a>
      </div>
    </div>
  {/if}

  {#if storageError}
    <p class="feedback error">{storageError}</p>
  {/if}

  {#if loadingItems}
    <p class="feedback">Loading your local queue…</p>
  {:else if items.length === 0}
    <section class="empty-state">
      <p>No saved items yet.</p>
      <p>Preview a source above or save something from a source page to start your queue.</p>
    </section>
  {:else}
    <section class="for-later-list">
      {#each items as item (item.id)}
        <ForLaterCard {item} {communities} onChanged={upsertItem} onRemoved={removeItem} />
      {/each}
    </section>
  {/if}
</section>

<style>
  .for-later-page {
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
  .feedback,
  .empty-state p,
  .community-note p {
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
  .empty-state,
  .community-note {
    display: grid;
    gap: 0.35rem;
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

  .community-actions {
    display: flex;
    gap: 0.55rem;
    flex-wrap: wrap;
  }

  .community-actions a {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: 2.6rem;
    padding: 0 0.95rem;
    border-radius: 999px;
    border: 1px solid var(--border);
    background: var(--surface-soft);
    color: var(--text);
    font-weight: 600;
    text-decoration: none;
  }

  .community-actions a:last-child {
    background: var(--accent);
    border-color: var(--accent);
    color: white;
  }

  .for-later-list {
    display: grid;
    gap: 1rem;
  }

  .feedback.error {
    color: #b42318;
  }
</style>
