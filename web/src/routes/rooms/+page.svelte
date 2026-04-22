<script lang="ts">
  import { browser } from '$app/environment';
  import { NDKKind } from '@nostr-dev-kit/ndk';
  import { ndk } from '$lib/ndk/client';
  import { GROUP_RELAY_URLS } from '$lib/ndk/config';
  import {
    buildJoinedCommunities,
    groupIdFromEvent,
    type CommunitySummary
  } from '$lib/ndk/groups';

  const currentUser = $derived(ndk.$currentUser);
  const signedIn = $derived(Boolean(currentUser));

  // Subscribe to user's admin/member records across NIP-29 relays
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
      const groupId = groupIdFromEvent(event);
      if (groupId) ids.add(groupId);
    }
    return [...ids];
  });

  // Fetch metadata for those groups
  const metadataFeed = ndk.$subscribe(() => {
    if (!browser || membershipGroupIds.length === 0) return undefined;
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

  const rooms: CommunitySummary[] = $derived(
    currentUser
      ? buildJoinedCommunities(
          currentUser.pubkey,
          [...metadataFeed.events],
          [...membershipFeed.events]
        )
      : []
  );

  const loading = $derived(signedIn && !membershipFeed.eosed);
</script>

<svelte:head>
  <title>Your rooms · Highlighter</title>
</svelte:head>

<section class="rooms-page">
  <header class="rooms-header">
    <div class="rooms-header-top">
      <div class="rooms-kicker">— your rooms</div>
      {#if signedIn}
        <a href="/r/create" class="btn-create">+ Create a room</a>
      {/if}
    </div>
    <h1 class="rooms-title">Your <em>rooms.</em></h1>
    <p class="rooms-lead">
      Rooms you're a member of — small, invitation-only reading groups.
    </p>
  </header>

  {#if !signedIn}
    <div class="empty">
      <p>Log in to see your rooms.</p>
    </div>
  {:else if loading}
    <div class="empty">
      <p>Loading your rooms…</p>
    </div>
  {:else if rooms.length === 0}
    <div class="empty">
      <h2>You're not in any rooms yet.</h2>
      <p>Rooms are closed by default. Either bring one of your own, or find a public one to read along with.</p>
      <div class="empty-actions">
        <a href="/discover" class="btn-primary">Discover rooms</a>
        <a href="/onboarding" class="btn-secondary">Bring a room</a>
      </div>
    </div>
  {:else}
    <div class="rooms-grid">
      {#each rooms as room (room.id)}
        <a href="/r/{room.id}" class="room-card">
          <div class="rc-kicker">
            {#if room.visibility === 'private'}Private{:else}Public{/if} ·
            {room.memberCount ?? '?'} members
          </div>
          <h3 class="rc-name">{room.name}</h3>
          {#if room.about}
            <p class="rc-about">{room.about}</p>
          {/if}
          <div class="rc-foot">Open →</div>
        </a>
      {/each}
    </div>
  {/if}
</section>

<style>
  .rooms-page {
    padding: 56px 0 80px;
  }

  .rooms-header {
    padding-bottom: 32px;
    border-bottom: 1px solid var(--rule);
    margin-bottom: 44px;
  }

  .rooms-header-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 14px;
  }

  .rooms-header-top .rooms-kicker {
    margin-bottom: 0;
  }

  .btn-create {
    display: inline-block;
    padding: 7px 16px;
    font-family: var(--font-sans);
    font-size: 13px;
    font-weight: 500;
    text-decoration: none;
    border-radius: var(--radius);
    background: var(--ink);
    color: var(--surface);
    transition: background 200ms ease;
  }

  .btn-create:hover {
    background: var(--brand-accent);
  }

  .rooms-kicker {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.22em;
    text-transform: uppercase;
    color: var(--brand-accent);
    margin-bottom: 14px;
  }

  .rooms-title {
    font-family: var(--font-serif);
    font-weight: 400;
    font-size: clamp(44px, 6vw, 68px);
    line-height: 1.02;
    letter-spacing: -0.025em;
    color: var(--ink);
    margin: 0 0 14px;
  }

  .rooms-title em {
    font-style: italic;
    color: var(--brand-accent);
  }

  .rooms-lead {
    font-family: var(--font-serif);
    font-style: italic;
    font-size: 19px;
    line-height: 1.5;
    color: var(--ink-soft);
    max-width: 52ch;
    margin: 0;
  }

  .empty {
    background: var(--surface);
    border: 1px solid var(--rule);
    border-radius: var(--radius);
    padding: 44px 32px;
    text-align: center;
  }

  .empty h2 {
    font-family: var(--font-serif);
    font-size: 26px;
    font-weight: 500;
    color: var(--ink);
    margin: 0 0 8px;
  }

  .empty p {
    font-family: var(--font-sans);
    color: var(--ink-soft);
    font-size: 15px;
    margin: 0 auto;
    max-width: 44ch;
  }

  .empty-actions {
    display: flex;
    gap: 12px;
    justify-content: center;
    margin-top: 24px;
    flex-wrap: wrap;
  }

  .btn-primary,
  .btn-secondary {
    display: inline-block;
    padding: 10px 20px;
    font-family: var(--font-sans);
    font-size: 13px;
    font-weight: 500;
    text-decoration: none;
    border-radius: var(--radius);
    transition: all 200ms ease;
  }

  .btn-primary {
    background: var(--ink);
    color: var(--surface);
  }

  .btn-primary:hover {
    background: var(--brand-accent);
  }

  .btn-secondary {
    background: var(--surface);
    color: var(--ink-soft);
    border: 1px solid var(--rule);
  }

  .btn-secondary:hover {
    border-color: var(--brand-accent);
    color: var(--brand-accent);
  }

  .rooms-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 16px;
  }

  .room-card {
    background: var(--surface);
    border: 1px solid var(--rule);
    border-radius: var(--radius);
    padding: 22px 24px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    text-decoration: none;
    color: inherit;
    transition: border-color 200ms ease, transform 200ms ease;
  }

  .room-card:hover {
    border-color: var(--brand-accent);
    transform: translateY(-2px);
  }

  .rc-kicker {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--ink-fade);
  }

  .rc-name {
    font-family: var(--font-serif);
    font-weight: 500;
    font-size: 24px;
    line-height: 1.1;
    color: var(--ink);
    margin: 0;
    letter-spacing: -0.01em;
  }

  .rc-about {
    font-family: var(--font-sans);
    font-size: 13.5px;
    line-height: 1.5;
    color: var(--ink-soft);
    margin: 0;
    flex: 1;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .rc-foot {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--brand-accent);
    padding-top: 10px;
    border-top: 1px dotted var(--rule);
    margin-top: auto;
  }
</style>
