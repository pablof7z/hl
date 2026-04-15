<script lang="ts">
  import { browser } from '$app/environment';
  import { NDKKind } from '@nostr-dev-kit/ndk';
  import type { PageProps } from './$types';
  import { ndk } from '$lib/ndk/client';
  import { GROUP_RELAY_URLS } from '$lib/ndk/config';
  import {
    HIGHLIGHTER_ARTIFACT_SHARE_KIND,
    artifactFromEvent,
    artifactHighlightReferenceKey
  } from '$lib/ndk/artifacts';
  import ArtifactCard from '$lib/features/artifacts/ArtifactCard.svelte';
  import ArtifactForm from '$lib/features/artifacts/ArtifactForm.svelte';
  import ArtifactMiniCard from '$lib/features/groups/ArtifactMiniCard.svelte';
  import CommunityMembershipPanel from '$lib/features/groups/CommunityMembershipPanel.svelte';
  import FeaturedArtifactPanel from '$lib/features/groups/FeaturedArtifactPanel.svelte';
  import HighlightSourceGroup from '$lib/features/highlights/HighlightSourceGroup.svelte';
  import { groupHighlightsBySource } from '$lib/features/highlights/grouping';
  import {
    HIGHLIGHTER_HIGHLIGHT_REPOST_KIND,
    fetchHighlightsForShares,
    highlightCountsByArtifact,
    type HydratedHighlight
  } from '$lib/ndk/highlights';
  import { groupIdFromEvent, requestToJoinCommunity } from '$lib/ndk/groups';

  let { data }: PageProps = $props();
  const currentUser = $derived(ndk.$currentUser);
  const membershipFeed = ndk.$subscribe(() => {
    if (!browser || !currentUser?.pubkey) return undefined;

    return {
      filters: [{ kinds: [NDKKind.GroupAdmins, NDKKind.GroupMembers], '#p': [currentUser.pubkey], limit: 128 }],
      relayUrls: GROUP_RELAY_URLS,
      closeOnEose: true
    };
  });

  const artifactFeed = ndk.$subscribe(() => {
    if (!browser || !data.community) return undefined;

    return {
      filters: [{ kinds: [HIGHLIGHTER_ARTIFACT_SHARE_KIND], '#h': [data.community.id], limit: 64 }],
      relayUrls: GROUP_RELAY_URLS,
      closeOnEose: true
    };
  });

  const artifacts = $derived.by(() => {
    const latestById = new Map<string, ReturnType<typeof artifactFromEvent>>();

    for (const event of [...artifactFeed.events].toSorted((left, right) => (right.created_at ?? 0) - (left.created_at ?? 0))) {
      const artifact = artifactFromEvent(event);
      if (!artifact.id || latestById.has(artifact.id)) continue;
      latestById.set(artifact.id, artifact);
    }

    return [...latestById.values()];
  });
  const artifactsByReference = $derived(
    new Map(artifacts.map((artifact) => [artifactHighlightReferenceKey(artifact), artifact] as const))
  );
  const highlightShareFeed = ndk.$subscribe(() => {
    if (!browser || !data.community) return undefined;

    return {
      filters: [{ kinds: [HIGHLIGHTER_HIGHLIGHT_REPOST_KIND], '#h': [data.community.id], limit: 256 }],
      relayUrls: GROUP_RELAY_URLS,
      closeOnEose: true
    };
  });
  let communityHighlights = $state<HydratedHighlight[]>([]);

  $effect(() => {
    if (!browser) {
      communityHighlights = [];
      return;
    }

    const shareEvents = [...highlightShareFeed.events];
    if (shareEvents.length === 0) {
      communityHighlights = [];
      return;
    }

    let cancelled = false;

    void fetchHighlightsForShares(ndk, shareEvents).then((highlights) => {
      if (!cancelled) {
        communityHighlights = highlights;
      }
    });

    return () => {
      cancelled = true;
    };
  });
  const recentHighlightGroups = $derived(
    groupHighlightsBySource(communityHighlights, artifactsByReference).slice(0, 4)
  );
  const highlightCounts = $derived(highlightCountsByArtifact(communityHighlights));
  const featuredArtifact = $derived(
    artifacts
      .toSorted((left, right) => {
        const leftCount = highlightCounts.get(artifactHighlightReferenceKey(left)) ?? 0;
        const rightCount = highlightCounts.get(artifactHighlightReferenceKey(right)) ?? 0;

        if (rightCount !== leftCount) {
          return rightCount - leftCount;
        }

        return (right.createdAt ?? 0) - (left.createdAt ?? 0);
      })
      .at(0)
  );
  const featuredHighlight = $derived(
    featuredArtifact
      ? communityHighlights.find(
          (highlight) => highlight.sourceReferenceKey === artifactHighlightReferenceKey(featuredArtifact)
        )
      : undefined
  );
  const newlySharedArtifacts = $derived(artifacts.slice(0, 4));
  const conversationArtifacts = $derived(
    artifacts
      .filter(
        (artifact) =>
          (highlightCounts.get(artifactHighlightReferenceKey(artifact)) ?? 0) > 0 &&
          artifactHighlightReferenceKey(artifact) !==
            (featuredArtifact ? artifactHighlightReferenceKey(featuredArtifact) : '')
      )
      .toSorted((left, right) => {
        const leftCount = highlightCounts.get(artifactHighlightReferenceKey(left)) ?? 0;
        const rightCount = highlightCounts.get(artifactHighlightReferenceKey(right)) ?? 0;

        if (rightCount !== leftCount) {
          return rightCount - leftCount;
        }

        return (right.createdAt ?? 0) - (left.createdAt ?? 0);
      })
      .slice(0, 4)
  );
  const archiveArtifacts = $derived(
    featuredArtifact
      ? artifacts.filter(
          (artifact) => artifactHighlightReferenceKey(artifact) !== artifactHighlightReferenceKey(featuredArtifact)
        )
      : artifacts
  );
  const isMember = $derived.by(() => {
    if (!currentUser?.pubkey || !data.community) {
      return false;
    }

    return membershipFeed.events.some((event) => groupIdFromEvent(event) === data.community?.id);
  });
  const membershipReady = $derived(!currentUser || membershipFeed.eosed || isMember);
  const canShare = $derived(Boolean(currentUser && membershipReady && isMember));
  let joinPending = $state(false);
  let joinRequested = $state(false);
  let joinNotice = $state('');
  let joinError = $state('');
  let shareOpen = $state(false);

  $effect(() => {
    data.community?.id;
    joinPending = false;
    joinRequested = false;
    joinNotice = '';
    joinError = '';
  });

  $effect(() => {
    if (!isMember) {
      return;
    }

    joinPending = false;
    joinRequested = false;
    joinNotice = '';
    joinError = '';
  });

  function memberLabel(memberCount: number | null): string {
    if (memberCount === null) return 'Private membership';
    if (memberCount === 1) return '1 member';
    return `${memberCount} members`;
  }

  function accessLabel(access: 'open' | 'closed'): string {
    return access === 'open' ? 'Open to join' : 'Invite only';
  }

  function visibilityLabel(visibility: 'public' | 'private'): string {
    return visibility === 'public' ? 'Public preview' : 'Private inside';
  }

  function shelfLabel(count: number): string {
    return count === 1 ? '1 piece on the shelf' : `${count} pieces on the shelf`;
  }

  function passageLabel(count: number): string {
    return count === 1 ? '1 passage saved' : `${count} passages saved`;
  }

  function itemLabel(count: number, singular: string, plural = `${singular}s`): string {
    return `${count} ${count === 1 ? singular : plural}`;
  }

  function joinActionLabel(): string {
    if (!data.community) return 'Join this community';
    if (joinPending) return data.community.access === 'open' ? 'Joining...' : 'Sending...';
    if (joinRequested) return 'Request sent';
    return data.community.access === 'open' ? 'Join this community' : 'Request to join';
  }

  async function joinCommunity() {
    if (!data.community || !currentUser || !membershipReady || isMember || joinPending || joinRequested) {
      return;
    }

    joinPending = true;
    joinError = '';
    joinNotice = '';

    try {
      await requestToJoinCommunity(ndk, data.community.id);
      joinRequested = true;
      joinNotice =
        data.community.access === 'open'
          ? 'Join request sent. This page will update as soon as you are added.'
          : 'Request sent. A moderator can let you in when they are ready.';
    } catch (caught) {
      joinError = caught instanceof Error ? caught.message : 'Could not send the join request.';
    } finally {
      joinPending = false;
    }
  }
</script>

<svelte:head>
  <title>{data.community ? `${data.community.name} — Highlighter` : 'Community — Highlighter'}</title>
</svelte:head>

{#if data.missing || !data.community}
  <section class="community-missing">
    <h1>Community not found.</h1>
    <p>
      Nothing on the Highlighter relay currently resolves to <span>/community/{data.groupId}</span>.
      It may not exist yet, or its metadata has not propagated.
    </p>
    <div class="actions">
      <a href="/community">Browse communities</a>
      <a href="/community/create">Create a community</a>
    </div>
  </section>
{:else}
  <section class="community-page">
    {#if isMember}
      <header class="community-bar">
        <div class="community-bar-left">
          <div class="community-avatar-sm">
            {#if data.community.picture}
              <img src={data.community.picture} alt="" />
            {:else}
              <span>{data.community.name.trim().charAt(0).toUpperCase() || '#'}</span>
            {/if}
          </div>

          <div class="community-bar-info">
            <h1 class="community-bar-name">{data.community.name}</h1>
            <div class="community-bar-meta">
              <span>{memberLabel(data.community.memberCount)}</span>
              <span>{shelfLabel(artifacts.length)}</span>
              <span>{passageLabel(communityHighlights.length)}</span>
            </div>
          </div>
        </div>

        <div class="community-bar-actions">
          <button class="bar-share-btn" type="button" onclick={() => shareOpen = true}>Share something</button>
          <a class="bar-link" href="/community">All communities</a>
        </div>
      </header>
    {:else}
      <header class="community-hero">
        <div class="community-hero-top">
          <div class="community-identity">
            <div class="community-avatar">
              {#if data.community.picture}
                <img src={data.community.picture} alt="" />
              {:else}
                <span>{data.community.name.trim().charAt(0).toUpperCase() || '#'}</span>
              {/if}
            </div>

            <div class="community-copy">
              <h1>{data.community.name}</h1>
              <p class="community-about">
                {data.community.about || 'A reading room for pieces people want to keep passing around.'}
              </p>
              <div class="community-badges">
                <span>{accessLabel(data.community.access)}</span>
                <span>{visibilityLabel(data.community.visibility)}</span>
                <span>{memberLabel(data.community.memberCount)}</span>
              </div>

              <div class="community-glance">
                <span>{shelfLabel(artifacts.length)}</span>
                <span>{passageLabel(communityHighlights.length)}</span>
              </div>
            </div>
          </div>

          <CommunityMembershipPanel
            community={data.community}
            signedIn={Boolean(currentUser)}
            joined={isMember}
            checkingMembership={Boolean(currentUser && !membershipReady)}
            {joinPending}
            {joinRequested}
            {joinNotice}
            {joinError}
            onJoin={joinCommunity}
            onShare={() => shareOpen = true}
          />
        </div>
      </header>
    {/if}

    {#if artifacts.length === 0}
      <section class="empty-collection">
        <div class="empty-collection-copy">
          <p class="panel-label">The Collection Starts Here</p>
          <h2>Nothing has been shared into this room yet.</h2>
          <p>
            The home page is live and ready for the first article, book, podcast, or video that
            should anchor the conversation.
          </p>
        </div>

        {#if canShare}
          <div class="empty-collection-action">
            <button class="side-card-action" type="button" onclick={() => shareOpen = true}>
              Share the first source
            </button>
          </div>
        {:else if currentUser}
          <section class="side-card guest-card">
            <p class="panel-label">Join To Add</p>
            <h3>Join before you start the shelf.</h3>
            <p>
              Once you are in, you can bring the first article, book, podcast, or video into the room.
            </p>
            {#if membershipReady}
              <button
                class="side-card-action"
                type="button"
                disabled={joinPending || joinRequested}
                onclick={() => void joinCommunity()}
              >
                {joinActionLabel()}
              </button>
            {:else}
              <span class="side-card-note">Checking your membership...</span>
            {/if}
          </section>
        {:else}
          <section class="side-card guest-card">
            <p class="panel-label">Want To Add Something?</p>
            <h3>Set up a profile to join in.</h3>
            <p>Guests can browse the room. Members can start the shelf and keep it moving.</p>
            <a class="side-card-action" href="/onboarding">Set up a profile</a>
          </section>
        {/if}
      </section>
    {:else}
      <section class="featured-stage">
        {#if featuredArtifact}
            <FeaturedArtifactPanel
              artifact={featuredArtifact}
              highlight={featuredHighlight}
              communityName={data.community.name}
              highlightCount={highlightCounts.get(artifactHighlightReferenceKey(featuredArtifact)) ?? 0}
            />
          {/if}

        <aside class="featured-rail">
          <section class="side-card">
            <div class="side-card-header">
              <div>
                <p class="panel-label">New To The Collection</p>
                <h3>Fresh shares</h3>
              </div>
              <span>{itemLabel(newlySharedArtifacts.length, 'item')}</span>
            </div>

            <div class="mini-card-stack">
              {#each newlySharedArtifacts as artifact (artifact.id)}
                <ArtifactMiniCard
                  artifact={artifact}
                  highlightCount={highlightCounts.get(artifactHighlightReferenceKey(artifact)) ?? 0}
                />
              {/each}
            </div>
          </section>

          <section class="side-card">
            <div class="side-card-header">
              <div>
                <p class="panel-label">Conversation Cluster</p>
                <h3>Where people are lingering</h3>
              </div>
            </div>

            {#if conversationArtifacts.length === 0}
              <p class="side-card-empty">
                Once members start saving highlights, the most magnetic sources will surface here.
              </p>
            {:else}
              <div class="mini-card-stack">
                {#each conversationArtifacts as artifact (artifact.id)}
                  <ArtifactMiniCard
                    artifact={artifact}
                    highlightCount={highlightCounts.get(artifactHighlightReferenceKey(artifact)) ?? 0}
                  />
                {/each}
              </div>
            {/if}
          </section>

          {#if canShare}
            <section class="side-card share-card">
              <p class="panel-label">Add To The Collection</p>
              <h3>Bring in the next source.</h3>
              <p>Share a new source and give this community a stronger shelf to react to.</p>
              <button class="side-card-action" type="button" onclick={() => shareOpen = true}>Share something</button>
            </section>
          {:else if currentUser}
            <section class="side-card guest-card">
              <p class="panel-label">Join To Contribute</p>
              <h3>Join before you share your own picks.</h3>
              <p>Reading is public here. Adding to the shelf starts once this community lets you in.</p>
              {#if membershipReady}
                <button
                  class="side-card-action"
                  type="button"
                  disabled={joinPending || joinRequested}
                  onclick={() => void joinCommunity()}
                >
                  {joinActionLabel()}
                </button>
              {:else}
                <span class="side-card-note">Checking your membership...</span>
              {/if}
            </section>
          {:else}
            <section class="side-card guest-card">
              <p class="panel-label">Want To Add Something?</p>
              <h3>Set up a profile to join in.</h3>
              <p>Create a profile to join, save highlights, and add your own pieces to the shelf.</p>
              <a class="side-card-action" href="/onboarding">Set up a profile</a>
            </section>
          {/if}
        </aside>
      </section>

      {#if archiveArtifacts.length > 0}
        <section class="artifact-feed">
          <div class="artifact-feed-header">
            <div>
              <p class="panel-label">From The Shelf</p>
              <h2>Full community library</h2>
            </div>
            <span>{itemLabel(archiveArtifacts.length, 'item')}</span>
          </div>

          <div class="artifact-grid">
            {#each archiveArtifacts as artifact (artifact.id)}
              <ArtifactCard
                {artifact}
                highlightCount={highlightCounts.get(artifactHighlightReferenceKey(artifact)) ?? 0}
              />
            {/each}
          </div>
        </section>
      {/if}
    {/if}

    {#if canShare}
      <ArtifactForm groupId={data.community.id} bind:open={shareOpen} />
    {/if}

    <section class="highlight-feed">
        <div class="artifact-feed-header">
          <div>
            <h2>Recent community highlights</h2>
          </div>
        <span>{itemLabel(communityHighlights.length, 'highlight')}</span>
      </div>

      {#if recentHighlightGroups.length === 0}
        <div class="artifact-empty">
          <p>No highlights shared here yet.</p>
          <p>
            Once people start saving passages from the items shared in this community, they will
            show up here.
          </p>
        </div>
      {:else}
        <div class="highlight-groups">
          {#each recentHighlightGroups as group (group.referenceKey)}
            <HighlightSourceGroup {group} />
          {/each}
        </div>
      {/if}
    </section>
  </section>
{/if}

<style>
  .community-page,
  .community-missing {
    display: grid;
    gap: 1.8rem;
    padding: 2rem 0 3rem;
  }

  .community-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.75rem 1rem;
    border: 1px solid var(--border);
    border-radius: 1rem;
    background: var(--surface);
  }

  .community-bar-left {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    min-width: 0;
  }

  .community-avatar-sm {
    display: grid;
    place-items: center;
    width: 2.5rem;
    height: 2.5rem;
    border-radius: 0.75rem;
    background: linear-gradient(160deg, rgba(255, 103, 25, 0.16), rgba(255, 103, 25, 0.04));
    overflow: hidden;
    color: var(--accent);
    font-size: 0.9rem;
    font-weight: 700;
    flex-shrink: 0;
  }

  .community-avatar-sm img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .community-bar-info {
    min-width: 0;
  }

  .community-bar-name {
    margin: 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: 1.15rem;
    line-height: 1.2;
    letter-spacing: -0.02em;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .community-bar-meta {
    display: flex;
    gap: 0.35rem;
    flex-wrap: wrap;
    margin-top: 0.2rem;
  }

  .community-bar-meta span {
    color: var(--muted);
    font-size: 0.75rem;
    font-weight: 600;
  }

  .community-bar-meta span:not(:last-child)::after {
    content: '\00b7';
    margin-left: 0.35rem;
  }

  .community-bar-actions {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    flex-shrink: 0;
  }

  .bar-share-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: 2.1rem;
    padding: 0 0.85rem;
    border-radius: 999px;
    background: var(--accent);
    color: white;
    font-size: 0.85rem;
    font-weight: 700;
  }

  .bar-link {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: 2.1rem;
    padding: 0 0.85rem;
    border-radius: 999px;
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text);
    font-size: 0.85rem;
    font-weight: 600;
  }

  .community-hero {
    display: grid;
    gap: 1rem;
    padding: 1.5rem 0;
    background:
      radial-gradient(circle at top left, rgba(255, 103, 25, 0.12), transparent 38%),
      transparent;
  }

  .community-hero-top,
  .featured-stage,
  .featured-rail,
  .mini-card-stack,
  .empty-collection,
  .empty-collection-copy,
  .highlight-feed,
  .artifact-feed {
    display: grid;
    gap: 1rem;
  }

  .community-hero-top {
    grid-template-columns: minmax(0, 1fr) minmax(18rem, 20rem);
    gap: 1.5rem;
    align-items: start;
  }

  .community-identity {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 1rem;
    align-items: start;
  }

  .community-avatar {
    display: grid;
    place-items: center;
    width: 4.5rem;
    height: 4.5rem;
    border-radius: 1.35rem;
    background: linear-gradient(160deg, rgba(255, 103, 25, 0.16), rgba(255, 103, 25, 0.04));
    overflow: hidden;
    color: var(--accent);
    font-size: 1.35rem;
    font-weight: 700;
  }

  .community-avatar img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .panel-label {
    margin: 0;
    color: var(--accent);
    font-size: 0.8rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  h1 {
    margin: 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: clamp(2rem, 4vw, 3.1rem);
    line-height: 1.05;
    letter-spacing: -0.03em;
  }

  .community-about,
  .community-missing p {
    margin: 0.9rem 0 0;
    color: var(--muted);
    line-height: 1.65;
  }

  .community-badges,
  .community-glance,
  .actions {
    display: flex;
    gap: 0.45rem;
    flex-wrap: wrap;
  }

  .community-badges {
    margin-top: 1rem;
  }

  .community-glance {
    margin-top: 0.75rem;
  }

  .community-badges span,
  .community-glance span,
  .actions a {
    display: inline-flex;
    align-items: center;
    min-height: 2rem;
    padding: 0 0.75rem;
    border-radius: 999px;
    background: var(--surface-soft);
    color: var(--text);
    font-size: 0.8rem;
    font-weight: 600;
  }

  .actions a:last-child {
    background: var(--accent);
    color: white;
  }

  .artifact-empty,
  .empty-collection {
    padding: 1rem 1.1rem;
    border: 1px solid var(--border);
    border-radius: 1.1rem;
    background: var(--surface);
  }

  .side-card {
    padding: 1rem 0;
    border-bottom: 1px solid var(--border);
  }

  .side-card:first-child {
    padding-top: 0;
  }

  .side-card:last-child {
    border-bottom: none;
  }

  .side-card.share-card,
  .side-card.guest-card {
    padding: 1rem 1.1rem;
    border: none;
    border-radius: 1.1rem;
    background: color-mix(in srgb, var(--accent) 6%, var(--surface));
  }

  .empty-collection h2 {
    margin: 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: clamp(1.35rem, 3vw, 2rem);
    line-height: 1.1;
    letter-spacing: -0.02em;
  }

  .empty-collection p,
  .side-card p,
  .side-card-empty {
    margin: 0;
    color: var(--muted);
    line-height: 1.6;
  }

  .featured-stage {
    grid-template-columns: minmax(0, 1.8fr) minmax(280px, 0.95fr);
    align-items: start;
  }

  .side-card {
    display: grid;
    gap: 0.9rem;
  }

  .side-card-action {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: 2.5rem;
    width: fit-content;
    padding: 0 0.95rem;
    border-radius: 999px;
    border: 1px solid var(--accent);
    background: var(--accent);
    color: white;
    font-weight: 700;
  }

  button.side-card-action:disabled {
    cursor: default;
    opacity: 0.7;
  }

  .side-card-note {
    color: var(--muted);
    font-size: 0.92rem;
    line-height: 1.5;
  }

  .side-card-header {
    display: flex;
    align-items: end;
    justify-content: space-between;
    gap: 1rem;
  }

  .side-card-header h3,
  .side-card h3 {
    margin: 0.25rem 0 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: 1.3rem;
    line-height: 1.15;
    letter-spacing: -0.02em;
  }

  .side-card-header span {
    display: inline-flex;
    align-items: center;
    min-height: 1.9rem;
    padding: 0 0.65rem;
    border-radius: 999px;
    background: var(--surface-soft);
    color: var(--muted);
    font-size: 0.76rem;
    font-weight: 700;
  }

  .empty-collection {
    grid-template-columns: minmax(0, 1.1fr) minmax(280px, 0.95fr);
    gap: 1.2rem;
    background:
      radial-gradient(circle at top left, rgba(255, 103, 25, 0.08), transparent 36%),
      var(--surface);
  }

  .empty-collection-copy h2 {
    margin-top: 0.25rem;
  }

  .artifact-feed-header {
    display: flex;
    justify-content: space-between;
    align-items: end;
    gap: 1rem;
    flex-wrap: wrap;
  }

  .artifact-feed-header h2 {
    margin: 0.3rem 0 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: 1.6rem;
    line-height: 1.15;
    letter-spacing: -0.02em;
  }

  .artifact-feed-header span {
    display: inline-flex;
    align-items: center;
    min-height: 2rem;
    padding: 0 0.75rem;
    border-radius: 999px;
    background: var(--surface-soft);
    color: var(--muted);
    font-size: 0.8rem;
    font-weight: 600;
  }

  .artifact-grid {
    display: grid;
    gap: 0.95rem;
  }

  .highlight-groups {
    display: grid;
    gap: 0.95rem;
  }

  .artifact-empty {
    border-style: dashed;
    background: color-mix(in srgb, var(--surface) 80%, white);
  }

  .artifact-empty p {
    margin: 0;
    color: var(--muted);
    line-height: 1.6;
  }

  .artifact-empty p:first-child {
    color: var(--text-strong);
    font-weight: 700;
    margin-bottom: 0.35rem;
  }

  @media (max-width: 720px) {
    .community-page,
    .community-missing {
      padding-top: 1.5rem;
    }

    .community-bar {
      flex-wrap: wrap;
    }

    .community-bar-actions {
      width: 100%;
    }

    .bar-share-btn,
    .bar-link {
      flex: 1;
    }

    .community-hero-top,
    .featured-stage,
    .empty-collection {
      grid-template-columns: 1fr;
    }

    .community-identity {
      grid-template-columns: 1fr;
    }

    .community-avatar {
      width: 4rem;
      height: 4rem;
    }
  }
</style>
