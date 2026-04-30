<script lang="ts">
  import { browser } from '$app/environment';
  import { NDKKind } from '@nostr-dev-kit/ndk';
  import { ndk } from '$lib/ndk/client';
  import { GROUP_RELAY_URLS } from '$lib/ndk/config';
  import { buildJoinedRooms, groupIdFromEvent } from '$lib/ndk/groups';
  import {
    BOOKMARK_LIST_KIND,
    bookmarkListHasEventId,
    latestListEvent,
    setBookmarkEventIdPresence
  } from '$lib/ndk/lists';
  import { shareHighlightToRoom } from '$lib/ndk/highlights';

  /// Action bar for a public highlight page: bookmark the kind:9802 event
  /// (NIP-51 kind:10003 with an `e` tag) and add-to-room (kind:16 repost
  /// into one of the user's NIP-29 groups). Auth-gated — unsigned-in
  /// state collapses to inert / "sign in" hint.

  let {
    highlightEventId,
    highlightAuthorPubkey
  }: {
    highlightEventId: string;
    highlightAuthorPubkey: string;
  } = $props();

  let saving = $state(false);
  let bookmarkError = $state('');
  let showRoomPicker = $state(false);
  let selectedGroupId = $state('');
  let sharing = $state(false);
  let shareError = $state('');
  let shareStatus = $state('');

  const currentUser = $derived(ndk.$currentUser);
  const isReadOnly = $derived(Boolean(ndk.$sessions?.isReadOnly()));

  const bookmarkFeed = ndk.$subscribe(() => {
    if (!browser || !currentUser) return undefined;
    return {
      filters: [
        { kinds: [BOOKMARK_LIST_KIND], authors: [currentUser.pubkey], limit: 8 }
      ],
      closeOnEose: false
    };
  });
  const bookmarkListEvent = $derived(latestListEvent([...bookmarkFeed.events]));
  const isBookmarked = $derived(bookmarkListHasEventId(bookmarkListEvent, highlightEventId));

  const membershipFeed = ndk.$subscribe(() => {
    if (!browser || !currentUser) return undefined;
    return {
      filters: [
        {
          kinds: [NDKKind.GroupAdmins, NDKKind.GroupMembers],
          '#p': [currentUser.pubkey],
          limit: 128
        }
      ],
      relayUrls: GROUP_RELAY_URLS,
      closeOnEose: true
    };
  });
  const membershipGroupIds = $derived.by(() => {
    const ids = new Set<string>();
    for (const event of membershipFeed.events) {
      const id = groupIdFromEvent(event);
      if (id) ids.add(id);
    }
    return [...ids];
  });
  const metadataFeed = ndk.$subscribe(() => {
    if (!browser || !currentUser || membershipGroupIds.length === 0) return undefined;
    return {
      filters: [
        {
          kinds: [NDKKind.GroupMetadata],
          '#d': membershipGroupIds,
          limit: Math.max(membershipGroupIds.length * 2, 32)
        }
      ],
      relayUrls: GROUP_RELAY_URLS,
      closeOnEose: true
    };
  });
  const rooms = $derived.by(() => {
    if (!currentUser) return [];
    return buildJoinedRooms(
      currentUser.pubkey,
      [...metadataFeed.events],
      [...membershipFeed.events]
    );
  });

  $effect(() => {
    if (!selectedGroupId && rooms.length > 0) {
      selectedGroupId = rooms[0].id;
    }
  });

  async function toggleBookmark() {
    if (!currentUser || saving) return;
    saving = true;
    bookmarkError = '';
    try {
      await setBookmarkEventIdPresence(ndk, bookmarkListEvent, highlightEventId, !isBookmarked);
    } catch (err) {
      bookmarkError = err instanceof Error ? err.message : 'Could not update bookmark.';
    } finally {
      saving = false;
    }
  }

  async function shareToRoom() {
    if (!currentUser) {
      shareError = 'Sign in to add this to a room.';
      return;
    }
    if (isReadOnly) {
      shareError = 'Read-only sessions cannot publish.';
      return;
    }
    if (!selectedGroupId) {
      shareError = 'Pick a room first.';
      return;
    }
    sharing = true;
    shareError = '';
    shareStatus = '';
    try {
      const result = await shareHighlightToRoom(ndk, {
        groupId: selectedGroupId,
        highlight: { eventId: highlightEventId, pubkey: highlightAuthorPubkey }
      });
      shareStatus = result.existing
        ? 'Already in that room.'
        : 'Added to room.';
      setTimeout(() => {
        showRoomPicker = false;
        shareStatus = '';
      }, 1400);
    } catch (err) {
      shareError = err instanceof Error ? err.message : 'Could not share.';
    } finally {
      sharing = false;
    }
  }
</script>

<div class="actions">
  <button
    type="button"
    class="action-btn"
    class:active={isBookmarked}
    onclick={toggleBookmark}
    disabled={!currentUser || saving}
    aria-label={isBookmarked ? 'Remove bookmark' : 'Bookmark highlight'}
    title={currentUser ? (isBookmarked ? 'Remove bookmark' : 'Bookmark') : 'Sign in to bookmark'}
  >
    {#if isBookmarked}
      <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
        <path d="M6 3h12a1 1 0 0 1 1 1v17l-7-4-7 4V4a1 1 0 0 1 1-1Z" />
      </svg>
    {:else}
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" aria-hidden="true">
        <path d="M6 3h12a1 1 0 0 1 1 1v17l-7-4-7 4V4a1 1 0 0 1 1-1Z" />
      </svg>
    {/if}
    <span>{isBookmarked ? 'Saved' : 'Save'}</span>
  </button>

  <button
    type="button"
    class="action-btn"
    onclick={() => (showRoomPicker = !showRoomPicker)}
    disabled={!currentUser}
    aria-label="Add to a room"
    title={currentUser ? 'Add to a room' : 'Sign in to add to a room'}
  >
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" aria-hidden="true">
      <rect x="3" y="3" width="18" height="18" rx="2" />
      <path d="M12 8v8" />
      <path d="M8 12h8" />
    </svg>
    <span>Add to room</span>
  </button>

  {#if bookmarkError}
    <p class="error">{bookmarkError}</p>
  {/if}
</div>

{#if showRoomPicker}
  <div class="room-picker">
    {#if !currentUser}
      <p class="hint">
        <a href="/login">Sign in</a> to add this to one of your rooms.
      </p>
    {:else if rooms.length === 0}
      <p class="hint">
        You haven't joined any rooms yet. <a href="/discover">Browse rooms</a>.
      </p>
    {:else}
      <label class="picker-label">
        Pick a room
        <select bind:value={selectedGroupId} class="picker-select">
          {#each rooms as room (room.id)}
            <option value={room.id}>{room.name}</option>
          {/each}
        </select>
      </label>
      <div class="picker-actions">
        {#if shareStatus}
          <span class="status">{shareStatus}</span>
        {:else if shareError}
          <span class="error">{shareError}</span>
        {/if}
        <button
          type="button"
          class="picker-cancel"
          onclick={() => (showRoomPicker = false)}
          disabled={sharing}
        >
          Cancel
        </button>
        <button
          type="button"
          class="picker-confirm"
          onclick={shareToRoom}
          disabled={sharing || !selectedGroupId}
        >
          {sharing ? 'Adding…' : 'Add to room'}
        </button>
      </div>
    {/if}
  </div>
{/if}

<style>
  .actions {
    display: flex;
    gap: 0.6rem;
    flex-wrap: wrap;
    align-items: center;
    margin: 0.5rem 0 1.5rem 0;
    padding: 0.75rem 0;
    border-top: 1px solid var(--color-border, #e8d8cb);
    border-bottom: 1px solid var(--color-border, #e8d8cb);
  }

  .action-btn {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.45rem 0.85rem;
    background: transparent;
    border: 1px solid var(--color-border, #e8d8cb);
    border-radius: 999px;
    color: var(--color-ink, #1a1410);
    font-size: 0.88rem;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.15s;
  }

  .action-btn:hover:not(:disabled) {
    border-color: var(--color-accent, #d05a2d);
    color: var(--color-accent, #d05a2d);
  }

  .action-btn.active {
    background: var(--color-accent, #d05a2d);
    color: #fff8f2;
    border-color: var(--color-accent, #d05a2d);
  }

  .action-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .room-picker {
    margin: -0.75rem 0 1.5rem 0;
    padding: 1rem;
    background: rgba(0, 0, 0, 0.025);
    border: 1px solid var(--color-border, #e8d8cb);
    border-radius: 12px;
    display: grid;
    gap: 0.75rem;
  }

  .picker-label {
    display: grid;
    gap: 0.4rem;
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--color-muted, #695747);
  }

  .picker-select {
    padding: 0.5rem 0.7rem;
    border: 1px solid var(--color-border, #e8d8cb);
    border-radius: 8px;
    background: var(--color-paper, #fffaf3);
    font: inherit;
    color: inherit;
  }

  .picker-actions {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 0.6rem;
    flex-wrap: wrap;
  }

  .picker-cancel,
  .picker-confirm {
    border: none;
    border-radius: 999px;
    padding: 0.45rem 1rem;
    font-weight: 600;
    cursor: pointer;
    font-size: 0.85rem;
  }

  .picker-cancel {
    background: transparent;
    color: var(--color-muted, #695747);
  }

  .picker-confirm {
    background: var(--color-accent, #d05a2d);
    color: #fff8f2;
  }

  .picker-confirm:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .hint {
    margin: 0;
    color: var(--color-muted, #695747);
    font-size: 0.9rem;
  }

  .hint a {
    color: var(--color-accent, #d05a2d);
    text-decoration: underline;
  }

  .error {
    color: #b03a2e;
    font-size: 0.85rem;
  }

  .status {
    color: #2e7d4f;
    font-size: 0.85rem;
  }
</style>
