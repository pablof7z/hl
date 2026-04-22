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

<section class="grid gap-6">
  <header class="grid gap-[0.35rem]">
    <h2 class="m-0 font-serif text-base-content leading-[1.1] tracking-[-0.02em]" style="font-size: clamp(1.6rem, 3vw, 2.2rem);">
      Your For Later bookmarks
    </h2>
    <p class="m-0 text-base-content/50 leading-relaxed">Saved directly as standard NIP-51 bookmark tags on your Nostr identity.</p>
  </header>

  <section class="grid gap-[0.9rem]" style="grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));">
    <div class="grid gap-[0.35rem] p-4 border border-base-300 rounded-[1.1rem] bg-base-100">
      <p class="m-0 text-primary text-[0.8rem] font-bold tracking-[0.08em] uppercase">Saved items</p>
      <strong class="font-serif text-[2rem] leading-none text-base-content">{items.length}</strong>
      <span class="m-0 text-base-content/50 leading-relaxed">Public tags in your NIP-51 bookmark list.</span>
    </div>
    <div class="grid gap-[0.35rem] p-4 border border-base-300 rounded-[1.1rem] bg-base-100">
      <p class="m-0 text-primary text-[0.8rem] font-bold tracking-[0.08em] uppercase">Nostr refs</p>
      <strong class="font-serif text-[2rem] leading-none text-base-content">{nostrBookmarkCount}</strong>
      <span class="m-0 text-base-content/50 leading-relaxed">Address or event bookmarks.</span>
    </div>
    <div class="grid gap-[0.35rem] p-4 border border-base-300 rounded-[1.1rem] bg-base-100">
      <p class="m-0 text-primary text-[0.8rem] font-bold tracking-[0.08em] uppercase">URLs</p>
      <strong class="font-serif text-[2rem] leading-none text-base-content">{urlBookmarkCount}</strong>
      <span class="m-0 text-base-content/50 leading-relaxed">External links saved as r tags.</span>
    </div>
  </section>

  <SaveForLaterForm onSaved={upsertItem} />

  {#if rooms.length === 0}
    <div class="grid gap-[0.35rem] p-4 border border-base-300 rounded-[1.1rem] bg-base-100">
      <p class="m-0 text-base-content/50 leading-relaxed">Join or create a room to move saved items out of this queue.</p>
      <div class="flex flex-wrap gap-[0.55rem]">
        <a
          href="/discover"
          class="inline-flex items-center justify-center min-h-[2.6rem] px-[0.95rem] rounded-full border border-base-300 bg-base-200 text-base-content font-semibold no-underline"
        >
          Browse rooms
        </a>
        <a
          href="/r/create"
          class="inline-flex items-center justify-center min-h-[2.6rem] px-[0.95rem] rounded-full border border-primary bg-primary text-white font-semibold no-underline"
        >
          Create a room
        </a>
      </div>
    </div>
  {/if}

  {#if storageError}
    <p class="m-0 text-error leading-relaxed">{storageError}</p>
  {/if}

  {#if loadingItems}
    <p class="m-0 text-base-content/50 leading-relaxed">Loading your NIP-51 bookmark list...</p>
  {:else if items.length === 0}
    <section class="grid gap-[0.35rem] p-4 border border-base-300 rounded-[1.1rem] bg-base-100">
      <p class="m-0 text-base-content/50 leading-relaxed">No saved items yet.</p>
      <p class="m-0 text-base-content/50 leading-relaxed">Add a source above to start your NIP-51 bookmark list.</p>
    </section>
  {:else}
    <section class="grid gap-4">
      {#each items as item (item.bookmarkKey)}
        <ForLaterCard {item} {rooms} onRemoved={removeItem} />
      {/each}
    </section>
  {/if}
</section>
