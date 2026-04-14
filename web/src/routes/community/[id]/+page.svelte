<script lang="ts">
  import { browser } from '$app/environment';
  import type { PageProps } from './$types';
  import { ndk } from '$lib/ndk/client';
  import { GROUP_RELAY_URLS } from '$lib/ndk/config';
  import { HIGHLIGHTER_ARTIFACT_KIND, artifactFromEvent } from '$lib/ndk/artifacts';
  import ArtifactCard from '$lib/features/artifacts/ArtifactCard.svelte';
  import ArtifactForm from '$lib/features/artifacts/ArtifactForm.svelte';
  import ArtifactMiniCard from '$lib/features/groups/ArtifactMiniCard.svelte';
  import FeaturedArtifactPanel from '$lib/features/groups/FeaturedArtifactPanel.svelte';
  import HighlightCard from '$lib/features/highlights/HighlightCard.svelte';
  import {
    HIGHLIGHTER_HIGHLIGHT_REPOST_KIND,
    fetchHighlightsForShares,
    highlightCountsByArtifact,
    type HydratedHighlight
  } from '$lib/ndk/highlights';

  let { data }: PageProps = $props();
  const currentUser = $derived(ndk.$currentUser);

  const artifactFeed = ndk.$subscribe(() => {
    if (!browser || !data.community) return undefined;

    return {
      filters: [{ kinds: [HIGHLIGHTER_ARTIFACT_KIND], '#h': [data.community.id], limit: 32 }],
      relayUrls: GROUP_RELAY_URLS,
      closeOnEose: true
    };
  });

  const artifacts = $derived(
    artifactFeed.events
      .toSorted((left, right) => (right.created_at ?? 0) - (left.created_at ?? 0))
      .map((event) => artifactFromEvent(event))
  );
  const artifactsByAddress = $derived(new Map(artifacts.map((artifact) => [artifact.address, artifact] as const)));

  const highlightShareFeed = ndk.$subscribe(() => {
    if (!browser || !data.community) return undefined;

    return {
      filters: [{ kinds: [HIGHLIGHTER_HIGHLIGHT_REPOST_KIND], '#h': [data.community.id], limit: 128 }],
      relayUrls: GROUP_RELAY_URLS,
      closeOnEose: true
    };
  });

  let communityHighlights = $state<HydratedHighlight[]>([]);
  let resolvingHighlights = $state(false);

  $effect(() => {
    if (!browser || !data.community) {
      communityHighlights = [];
      return;
    }

    const shareEvents = [...highlightShareFeed.events];
    if (shareEvents.length === 0) {
      communityHighlights = [];
      return;
    }

    let cancelled = false;
    resolvingHighlights = true;

    void fetchHighlightsForShares(ndk, shareEvents)
      .then((highlights) => {
        if (cancelled) return;
        communityHighlights = highlights;
      })
      .finally(() => {
        if (!cancelled) {
          resolvingHighlights = false;
        }
      });

    return () => {
      cancelled = true;
    };
  });

  const recentHighlights = $derived(communityHighlights.slice(0, 6));
  const highlightCounts = $derived(highlightCountsByArtifact(communityHighlights));
  const featuredArtifact = $derived(
    artifacts
      .toSorted((left, right) => {
        const leftCount = highlightCounts.get(left.address) ?? 0;
        const rightCount = highlightCounts.get(right.address) ?? 0;

        if (rightCount !== leftCount) {
          return rightCount - leftCount;
        }

        return (right.createdAt ?? 0) - (left.createdAt ?? 0);
      })
      .at(0)
  );
  const featuredHighlight = $derived(
    featuredArtifact
      ? communityHighlights.find((highlight) => highlight.artifactAddress === featuredArtifact.address)
      : undefined
  );
  const newlySharedArtifacts = $derived(artifacts.slice(0, 4));
  const conversationArtifacts = $derived(
    artifacts
      .filter(
        (artifact) =>
          (highlightCounts.get(artifact.address) ?? 0) > 0 &&
          artifact.address !== featuredArtifact?.address
      )
      .toSorted((left, right) => {
        const leftCount = highlightCounts.get(left.address) ?? 0;
        const rightCount = highlightCounts.get(right.address) ?? 0;

        if (rightCount !== leftCount) {
          return rightCount - leftCount;
        }

        return (right.createdAt ?? 0) - (left.createdAt ?? 0);
      })
      .slice(0, 4)
  );
  const archiveArtifacts = $derived(
    featuredArtifact
      ? artifacts.filter((artifact) => artifact.address !== featuredArtifact.address)
      : artifacts
  );

  function memberLabel(memberCount: number | null): string {
    if (memberCount === null) return 'Private membership';
    if (memberCount === 1) return '1 member';
    return `${memberCount} members`;
  }

  function itemLabel(count: number, singular: string, plural = `${singular}s`): string {
    return `${count} ${count === 1 ? singular : plural}`;
  }
</script>

<svelte:head>
  <title>{data.community ? `${data.community.name} — Highlighter` : 'Community — Highlighter'}</title>
</svelte:head>

{#if data.missing || !data.community}
  <section class="community-missing">
    <p class="eyebrow">Community</p>
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
            <p class="eyebrow">Community</p>
            <h1>{data.community.name}</h1>
            <p class="community-about">
              {data.community.about || 'A calm private collection of artifacts, highlights, and conversation, all anchored to one relay-backed community.'}
            </p>
            <div class="community-badges">
              <span>{data.community.visibility}</span>
              <span>{data.community.access}</span>
              <span>{memberLabel(data.community.memberCount)}</span>
              <span>{data.community.adminPubkeys.length || 1} admin{data.community.adminPubkeys.length === 1 ? '' : 's'}</span>
            </div>
          </div>
        </div>

        <div class="community-actions">
          <a href="/community">All communities</a>
          {#if currentUser}
            <a href="#share-artifact">Share new content</a>
          {:else}
            <a href="/discover">Preview more groups</a>
          {/if}
        </div>
      </div>

      <div class="community-summary-strip">
        <section class="summary-card">
          <p class="panel-label">Collection</p>
          <strong>{itemLabel(artifacts.length, 'artifact')}</strong>
          <span>Shared directly into this community’s shelf.</span>
        </section>

        <section class="summary-card">
          <p class="panel-label">Highlights</p>
          <strong>{itemLabel(communityHighlights.length, 'shared highlight')}</strong>
          <span>Portable `kind:9802` highlights reposted here.</span>
        </section>

        <section class="summary-card">
          <p class="panel-label">Relay</p>
          <strong>{data.community.relayUrl.replace(/^wss?:\/\//, '')}</strong>
          <span>The community’s home and routing layer.</span>
        </section>
      </div>
    </header>

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

        {#if currentUser}
          <div id="share-artifact">
            <ArtifactForm groupId={data.community.id} />
          </div>
        {:else}
          <section class="side-card guest-card">
            <p class="panel-label">Preview</p>
            <h3>Sign in to add the first piece.</h3>
            <p>Guests can read the shape of the collection, but sharing starts after login.</p>
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
            highlightCount={highlightCounts.get(featuredArtifact.address) ?? 0}
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
              {#each newlySharedArtifacts as artifact (artifact.address)}
                <ArtifactMiniCard artifact={artifact} highlightCount={highlightCounts.get(artifact.address) ?? 0} />
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
                Once members start saving highlights, the most magnetic artifacts will surface here.
              </p>
            {:else}
              <div class="mini-card-stack">
                {#each conversationArtifacts as artifact (artifact.address)}
                  <ArtifactMiniCard artifact={artifact} highlightCount={highlightCounts.get(artifact.address) ?? 0} />
                {/each}
              </div>
            {/if}
          </section>

          {#if currentUser}
            <section class="side-card share-card">
              <p class="panel-label">Add To The Collection</p>
              <h3>Bring in the next artifact.</h3>
              <p>Share a new source and give this community a stronger shelf to react to.</p>
              <a href="#share-artifact">Open the share form</a>
            </section>
          {:else}
            <section class="side-card guest-card">
              <p class="panel-label">Preview</p>
              <h3>Want to add something?</h3>
              <p>Sign in to share an artifact, save highlights, and join the conversation.</p>
            </section>
          {/if}
        </aside>
      </section>

      <section class="composer-row" id="share-artifact">
        {#if currentUser}
          <ArtifactForm groupId={data.community.id} />
        {:else}
          <section class="side-card guest-card">
            <p class="panel-label">Share Into This Community</p>
            <h3>Sign in to contribute.</h3>
            <p>Guests can browse the collection. Members can keep feeding it.</p>
          </section>
        {/if}
      </section>

      {#if archiveArtifacts.length > 0}
        <section class="artifact-feed">
          <div class="artifact-feed-header">
            <div>
              <p class="panel-label">From The Shelf</p>
              <h2>Full community library</h2>
            </div>
            <span>{itemLabel(archiveArtifacts.length, 'artifact')}</span>
          </div>

          <div class="artifact-grid">
            {#each archiveArtifacts as artifact (artifact.address)}
              <ArtifactCard {artifact} highlightCount={highlightCounts.get(artifact.address) ?? 0} />
            {/each}
          </div>
        </section>
      {/if}
    {/if}

    <section class="highlight-feed">
      <div class="artifact-feed-header">
        <div>
          <p class="panel-label">What Caught Our Eye</p>
          <h2>Recent community highlights</h2>
        </div>
        <span>{itemLabel(recentHighlights.length, 'share')}</span>
      </div>

      {#if recentHighlights.length === 0}
        <div class="artifact-empty">
          <p>No highlight reposts yet.</p>
          <p>
            Save a highlight on any artifact in this community and it will show up here after the
            repost lands.
          </p>
        </div>
      {:else}
        <div class="highlight-grid">
          {#each recentHighlights as highlight (highlight.eventId)}
            <HighlightCard highlight={highlight} artifact={artifactsByAddress.get(highlight.artifactAddress)} />
          {/each}
        </div>
      {/if}

      {#if resolvingHighlights}
        <p class="highlight-loading">Refreshing highlight reposts…</p>
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

  .community-hero {
    display: grid;
    gap: 1.25rem;
    padding: 1.5rem;
    border: 1px solid var(--border);
    border-radius: 1.5rem;
    background:
      radial-gradient(circle at top left, rgba(255, 103, 25, 0.12), transparent 38%),
      linear-gradient(180deg, rgba(255, 255, 255, 0.96), rgba(255, 255, 255, 1));
  }

  .community-hero-top,
  .community-summary-strip,
  .featured-stage,
  .featured-rail,
  .mini-card-stack,
  .empty-collection,
  .empty-collection-copy,
  .composer-row,
  .highlight-feed,
  .artifact-feed {
    display: grid;
    gap: 1rem;
  }

  .community-hero-top {
    grid-template-columns: minmax(0, 1fr) auto;
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

  .eyebrow,
  .panel-label {
    margin: 0;
    color: var(--accent);
    font-size: 0.8rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  h1 {
    margin: 0.35rem 0 0;
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
  .actions,
  .community-actions {
    display: flex;
    gap: 0.45rem;
    flex-wrap: wrap;
  }

  .community-badges {
    margin-top: 1rem;
  }

  .community-badges span,
  .community-actions a,
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

  .community-actions a:last-child,
  .actions a:last-child {
    background: var(--accent);
    color: white;
  }

  .community-summary-strip {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: 0.85rem;
  }

  .summary-card,
  .artifact-empty,
  .side-card,
  .empty-collection {
    padding: 1rem 1.1rem;
    border: 1px solid var(--border);
    border-radius: 1.1rem;
    background: var(--surface);
  }

  .summary-card strong,
  .empty-collection h2 {
    margin: 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: clamp(1.35rem, 3vw, 2rem);
    line-height: 1.1;
    letter-spacing: -0.02em;
  }

  .summary-card span,
  .empty-collection p,
  .side-card p,
  .side-card-empty,
  .highlight-loading {
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

  .share-card a {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: 2.5rem;
    width: fit-content;
    padding: 0 0.95rem;
    border-radius: 999px;
    background: var(--accent);
    color: white;
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

  .highlight-grid {
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

  .highlight-loading {
    font-size: 0.88rem;
  }

  @media (max-width: 720px) {
    .community-page,
    .community-missing {
      padding-top: 1.5rem;
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
