<script lang="ts">
  import { browser } from '$app/environment';
  import { NDKKind } from '@nostr-dev-kit/ndk';
  import { ndk } from '$lib/ndk/client';
  import ForLaterCard from '$lib/features/vault/ForLaterCard.svelte';
  import SaveForLaterForm from '$lib/features/vault/SaveForLaterForm.svelte';
  import { listForLaterArtifacts, type ForLaterItem } from '$lib/features/vault/vault';
  import { GROUP_RELAY_URLS } from '$lib/ndk/config';
  import { buildJoinedRooms, groupIdFromEvent } from '$lib/ndk/groups';

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

  const rooms = $derived(
    currentUser
      ? buildJoinedRooms(currentUser.pubkey, [...metadataFeed.events], [...membershipFeed.events])
      : []
  );
  const nostrBookmarkCount = $derived(items.filter((item) => item.bookmarkTagName !== 'r').length);
  const urlBookmarkCount = $derived(items.filter((item) => item.bookmarkTagName === 'r').length);

  $effect(() => {
    if (!browser || !currentUser) {
      items = [];
      loadingItems = false;
      storageError = '';
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
            error instanceof Error ? error.message : 'Could not load your For Later bookmarks.';
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
    items = [nextItem, ...items.filter((item) => item.bookmarkKey !== nextItem.bookmarkKey)];
  }

  function removeItem(id: string) {
    items = items.filter((item) => item.bookmarkKey !== id);
  }
</script>

<svelte:head>
  <title>For Later — Highlighter</title>
</svelte:head>

<section class="for-later-page">
  <header class="page-copy">
    <h2>Your For Later bookmarks</h2>
    <p>Saved directly as standard NIP-51 bookmark tags on your Nostr identity.</p>
  </header>

  <section class="me-summary">
    <div class="summary-card">
      <p class="summary-label">Saved items</p>
      <strong>{items.length}</strong>
      <span>Public tags in your NIP-51 bookmark list.</span>
    </div>
    <div class="summary-card">
      <p class="summary-label">Nostr refs</p>
      <strong>{nostrBookmarkCount}</strong>
      <span>Address or event bookmarks.</span>
    </div>
    <div class="summary-card">
      <p class="summary-label">URLs</p>
      <strong>{urlBookmarkCount}</strong>
      <span>External links saved as r tags.</span>
    </div>
  </section>

  <SaveForLaterForm onSaved={upsertItem} />

  {#if rooms.length === 0}
    <div class="room-note">
      <p>Join or create a room to move saved items out of this queue.</p>
      <div class="room-actions">
        <a href="/discover">Browse rooms</a>
        <a href="/r/create">Create a room</a>
      </div>
    </div>
  {/if}

  {#if storageError}
    <p class="feedback error">{storageError}</p>
  {/if}

  {#if loadingItems}
    <p class="feedback">Loading your NIP-51 bookmark list...</p>
  {:else if items.length === 0}
    <section class="empty-state">
      <p>No saved items yet.</p>
      <p>Add a source above to start your NIP-51 bookmark list.</p>
    </section>
  {:else}
    <section class="for-later-list">
      {#each items as item (item.bookmarkKey)}
        <ForLaterCard {item} {rooms} onRemoved={removeItem} />
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
  .room-note p {
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
  .room-note {
    display: grid;
    gap: 0.35rem;
    padding: 1rem 1.1rem;
    border: 1px solid var(--color-base-300);
    border-radius: 1.1rem;
    background: var(--surface);
  }

  .summary-card strong {
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: 2rem;
    line-height: 1;
  }

  .room-actions {
    display: flex;
    gap: 0.55rem;
    flex-wrap: wrap;
  }

  .room-actions a {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: 2.6rem;
    padding: 0 0.95rem;
    border-radius: 999px;
    border: 1px solid var(--color-base-300);
    background: var(--surface-soft);
    color: var(--text);
    font-weight: 600;
    text-decoration: none;
  }

  .room-actions a:last-child {
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
