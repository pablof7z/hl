<script lang="ts">
  import { browser } from '$app/environment';
  import { NDKKind } from '@nostr-dev-kit/ndk';
  import { ndk } from '$lib/ndk/client';
  import { GROUP_RELAY_URLS } from '$lib/ndk/config';
  import CommunityGrid from '$lib/features/groups/CommunityGrid.svelte';
  import { buildJoinedCommunities, groupIdFromEvent } from '$lib/ndk/groups';

  const currentUser = $derived(ndk.$currentUser);

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
  const publicCount = $derived(communities.filter((community) => community.visibility === 'public').length);
  const privateCount = $derived(communities.length - publicCount);
</script>

<svelte:head>
  <title>My Communities — Highlighter</title>
</svelte:head>

<section class="me-communities-page">
  <header class="page-copy">
    <h2>Your communities</h2>
  </header>

  <section class="me-summary">
    <div class="summary-card">
      <p class="summary-label">Joined</p>
      <strong>{communities.length}</strong>
      <span>Communities currently linked to your pubkey.</span>
    </div>
    <div class="summary-card">
      <p class="summary-label">Public</p>
      <strong>{publicCount}</strong>
      <span>Groups you can share outward without hiding their metadata.</span>
    </div>
    <div class="summary-card">
      <p class="summary-label">Private</p>
      <strong>{privateCount}</strong>
      <span>Communities that stay legible only to members.</span>
    </div>
  </section>

  <CommunityGrid
    communities={communities}
    joinedGroupIds={communities.map((community) => community.id)}
    defaultSort="name"
    searchPlaceholder="Search the communities you belong to"
    emptyLabel="No memberships loaded yet."
    emptyCopy="Join a public community or create your own."
    emptyCtaHref="/discover"
    emptyCtaLabel="Browse public communities"
  />
</section>

<style>
  .me-communities-page {
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

  .summary-card span {
    margin: 0;
    color: var(--muted);
    line-height: 1.65;
  }

  .me-summary {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 0.9rem;
  }

  .summary-card {
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
</style>
