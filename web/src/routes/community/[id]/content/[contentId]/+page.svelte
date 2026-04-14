<script lang="ts">
  import { browser } from '$app/environment';
  import { NDKEvent, NDKKind } from '@nostr-dev-kit/ndk';
  import type { PageProps } from './$types';
  import ArticleMarkdown from '$lib/components/ArticleMarkdown.svelte';
  import HighlightPopover from '$lib/components/HighlightPopover.svelte';
  import HighlightCard from '$lib/features/highlights/HighlightCard.svelte';
  import HighlightForm from '$lib/features/highlights/HighlightForm.svelte';
  import { ndk } from '$lib/ndk/client';
  import {
    articlePublishedAt,
    articleReadTimeMinutes,
    articleSummary,
    displayNip05,
    displayName,
    formatDisplayDate
  } from '$lib/ndk/format';
  import {
    buildArtifactHighlightFilters,
    highlightReferenceKey,
    hydrateStandaloneHighlights,
    type HydratedHighlight
  } from '$lib/ndk/highlights';
  import { DEFAULT_RELAYS, GROUP_RELAY_URLS } from '$lib/ndk/config';
  import { artifactHighlightReferenceKey } from '$lib/ndk/artifacts';
  import { User } from '$lib/ndk/ui/user';

  let { data }: PageProps = $props();
  let articleContentEl = $state<HTMLElement | null>(null);
  const currentUser = $derived(ndk.$currentUser);
  const articleEvent = $derived(data.articleEvent ? new NDKEvent(ndk, data.articleEvent) : undefined);
  const articleAuthorPubkey = $derived(data.articleAuthorPubkey || articleEvent?.pubkey || '');
  const articleAuthorIdentifier = $derived(
    data.articleAuthorIdentifier || data.articleAuthorNpub || articleAuthorPubkey || 'author'
  );
  const articleAuthorName = $derived(
    displayName(
      data.articleProfile,
      articleAuthorPubkey ? `${articleAuthorPubkey.slice(0, 8)}...` : 'Author'
    )
  );
  const articleAuthorIdentity = $derived.by(() => {
    const nip05 = displayNip05(data.articleProfile);
    return nip05 && nip05 !== articleAuthorName ? nip05 : '';
  });

  const groupAdminFeed = ndk.$subscribe(() => {
    if (!browser || !data.community) return undefined;

    return {
      filters: [{ kinds: [NDKKind.GroupAdmins], '#d': [data.community.id], limit: 1 }],
      relayUrls: GROUP_RELAY_URLS,
      closeOnEose: true
    };
  });

  const groupMemberFeed = ndk.$subscribe(() => {
    if (!browser || !data.community) return undefined;

    return {
      filters: [{ kinds: [NDKKind.GroupMembers], '#d': [data.community.id], limit: 1 }],
      relayUrls: GROUP_RELAY_URLS,
      closeOnEose: true
    };
  });

  function latestEvent(events: NDKEvent[]): NDKEvent | undefined {
    return [...events].sort((left, right) => (right.created_at ?? 0) - (left.created_at ?? 0))[0];
  }

  function uniquePubkeys(event: NDKEvent | undefined): string[] {
    return [...new Set((event?.getMatchingTags('p').map((tag) => tag[1]).filter(Boolean) ?? []).map((value) => value.trim()))];
  }

  const memberPubkeys = $derived.by(() => {
    const groupAdmins = uniquePubkeys(latestEvent(groupAdminFeed.events));
    const groupMembers = uniquePubkeys(latestEvent(groupMemberFeed.events));

    return [...new Set([...(data.community?.adminPubkeys ?? []), ...groupAdmins, ...groupMembers])];
  });
  const highlightFeed = ndk.$subscribe(() => {
    if (!browser || !data.community || !data.artifact) return undefined;

    const filters = buildArtifactHighlightFilters([data.artifact], memberPubkeys, 120);
    if (filters.length === 0) return undefined;

    return {
      filters,
      relayUrls: DEFAULT_RELAYS,
      closeOnEose: true
    };
  });
  const communityHighlights = $derived<HydratedHighlight[]>(
    hydrateStandaloneHighlights([...highlightFeed.events])
  );
  const artifactReferenceKey = $derived(
    data.artifact ? artifactHighlightReferenceKey(data.artifact) : ''
  );
  const artifactHighlights = $derived(
    data.artifact
      ? communityHighlights.filter((highlight) => highlight.sourceReferenceKey === artifactReferenceKey)
      : []
  );
  const artifactHighlightEvents = $derived(
    data.artifact
      ? highlightFeed.events.filter(
          (highlight) =>
            highlightReferenceKey({
              artifactAddress: highlight.tagValue('a'),
              eventReference: highlight.tagValue('e'),
              sourceUrl: highlight.tagValue('r')
            }) === artifactReferenceKey
        )
      : []
  );
</script>

<svelte:head>
  <title>{data.artifact ? `${data.artifact.title} — Highlighter` : 'Artifact — Highlighter'}</title>
</svelte:head>

{#if data.missing || !data.community || !data.artifact}
  <section class="artifact-missing">
    <p class="eyebrow">Artifact</p>
    <h1>Artifact not found.</h1>
    <p>
      Nothing currently resolves to <span>/community/{data.groupId}/content/{data.contentId}</span>.
      Share the URL into this community first, then come back here.
    </p>
    <div class="actions">
      <a href={`/community/${data.groupId}`}>Back to community</a>
      <a href="/community">Browse communities</a>
    </div>
  </section>
{:else}
  <article class="artifact-page">
    <header class="artifact-hero">
      <div class="artifact-hero-media">
        {#if data.artifact.image}
          <img src={data.artifact.image} alt="" />
        {:else}
          <div class="artifact-fallback">
            <span>{data.artifact.domain.charAt(0).toUpperCase() || '#'}</span>
          </div>
        {/if}
      </div>

      <div class="artifact-hero-copy">
        <p class="eyebrow">Artifact</p>
        <h1>{data.artifact.title}</h1>
        <div class="artifact-badges">
          <span>{data.artifact.source}</span>
          <span>{data.artifact.domain}</span>
          {#if data.artifact.author}
            <span>{data.artifact.author}</span>
          {/if}
        </div>

        {#if data.artifact.note}
          <p class="artifact-note">{data.artifact.note}</p>
        {/if}

        <div class="artifact-actions">
          <a href={data.artifact.url} target="_blank" rel="noreferrer">Open source</a>
          <a href={`/community/${data.community.id}`}>Back to {data.community.name}</a>
          <a href={`/community/${data.community.id}/content/${data.artifact.id}/discussion`}>
            Discussion
          </a>
        </div>
      </div>
    </header>

    <section class="artifact-panels">
      <div class="artifact-panel">
        <p class="panel-label">Route</p>
        <p class="panel-value">/community/{data.community.id}/content/{data.artifact.id}</p>
      </div>

      <div class="artifact-panel">
        <p class="panel-label">Artifact ID</p>
        <p class="panel-value mono">{data.artifact.id}</p>
      </div>

      <div class="artifact-panel">
        <p class="panel-label">Source Reference</p>
        <p class="panel-value mono">{data.artifact.referenceKey}</p>
      </div>
    </section>

    <section class="artifact-next">
      <p class="panel-label">{articleEvent ? 'Reading Surface' : 'What lands here next'}</p>
      <p>
        {#if articleEvent}
          This route now renders the underlying Nostr article in full, and group members can select
          text directly from the source to create highlights.
        {:else}
          Artifact-level discussion hangs off this same coordinate on the discussion route, while
          canonical highlights resolve back here through their `a` tag.
        {/if}
      </p>
    </section>

    {#if articleEvent}
      <section class="artifact-reader">
        <div class="artifact-reader-header">
          <p class="panel-label">Article</p>
          <p class="artifact-reader-hint">
            {#if currentUser}
              Select text in the article to create a highlight.
            {:else}
              Sign in, then select text in the article to create a highlight.
            {/if}
          </p>
        </div>

        <article class="article-container artifact-reader-article">
          <div class="article-byline">
            <User.Root {ndk} pubkey={articleAuthorPubkey} profile={data.articleProfile}>
              <a class="article-author-link" href={`/profile/${articleAuthorIdentifier}`}>
                <User.Avatar class="article-author-avatar" />
              </a>
              <div class="article-author-copy">
                <div class="feed-meta">
                  <a class="article-author-name" href={`/profile/${articleAuthorIdentifier}`}>
                    {articleAuthorName}
                  </a>
                  <span>{formatDisplayDate(articlePublishedAt(articleEvent.rawEvent()))}</span>
                  <span>{articleReadTimeMinutes(articleEvent.content)} min read</span>
                </div>
                {#if articleAuthorIdentity}
                  <div class="feed-meta">
                    <span class="article-author-handle">{articleAuthorIdentity}</span>
                  </div>
                {/if}
              </div>
            </User.Root>
          </div>

          <p class="lede" style="margin: 0;">
            {articleSummary(articleEvent.rawEvent(), 320)}
          </p>

          <div bind:this={articleContentEl}>
            <ArticleMarkdown
              content={articleEvent.content}
              tags={articleEvent.tags}
              highlights={artifactHighlightEvents}
            />
          </div>
        </article>
      </section>
    {:else if currentUser}
      <HighlightForm artifact={data.artifact} groupId={data.community.id} />
    {:else}
      <section class="artifact-next">
        <p class="panel-label">Create Highlight</p>
        <p>Sign in to save a canonical highlight and repost it into this community.</p>
      </section>
    {/if}

    <section class="artifact-highlights">
      <div class="artifact-highlights-header">
        <div>
          <p class="panel-label">Highlights</p>
          <h2>What this community pulled out</h2>
        </div>
        <span>{artifactHighlights.length} item{artifactHighlights.length === 1 ? '' : 's'}</span>
      </div>

      {#if artifactHighlights.length === 0}
        <div class="artifact-empty">
          <p>No member highlights yet.</p>
          <p>
            Save the first excerpt from this source and it will appear here once a group member
            publishes the highlight.
          </p>
        </div>
      {:else}
        <div class="highlight-stack">
          {#each artifactHighlights as highlight (highlight.eventId)}
            <HighlightCard {highlight} artifact={data.artifact} />
          {/each}
        </div>
      {/if}
    </section>

    {#if articleEvent}
      <HighlightPopover articleEvent={articleEvent} containerEl={articleContentEl} />
    {/if}
  </article>
{/if}

<style>
  .artifact-page,
  .artifact-missing {
    display: grid;
    gap: 1.5rem;
    padding: 2rem 0 3rem;
  }

  .artifact-hero {
    display: grid;
    grid-template-columns: minmax(180px, 240px) minmax(0, 1fr);
    gap: 1.4rem;
    padding: 1.35rem;
    border: 1px solid var(--border);
    border-radius: 1.45rem;
    background:
      radial-gradient(circle at top left, rgba(255, 103, 25, 0.1), transparent 38%),
      var(--surface);
  }

  .artifact-hero-media,
  .artifact-hero-media img,
  .artifact-fallback {
    width: 100%;
    aspect-ratio: 4 / 5;
    border-radius: 1.1rem;
  }

  .artifact-hero-media {
    overflow: hidden;
    background: linear-gradient(160deg, rgba(255, 103, 25, 0.12), rgba(255, 103, 25, 0.04));
  }

  .artifact-hero-media img {
    object-fit: cover;
  }

  .artifact-fallback {
    display: grid;
    place-items: center;
    color: var(--accent);
    font-size: 2rem;
    font-weight: 700;
  }

  .artifact-hero-copy {
    display: grid;
    align-content: start;
    gap: 0.8rem;
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
    margin: 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: clamp(2rem, 4vw, 3rem);
    line-height: 1.05;
    letter-spacing: -0.03em;
  }

  .artifact-badges,
  .artifact-actions,
  .actions {
    display: flex;
    gap: 0.45rem;
    flex-wrap: wrap;
  }

  .artifact-badges span,
  .artifact-actions a,
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

  .artifact-actions a:first-child,
  .actions a:last-child {
    background: var(--accent);
    color: white;
  }

  .artifact-note,
  .artifact-next p,
  .artifact-missing p {
    margin: 0;
    color: var(--muted);
    line-height: 1.65;
  }

  .artifact-panels {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 0.85rem;
  }

  .artifact-panel,
  .artifact-next,
  .artifact-empty {
    padding: 1rem 1.1rem;
    border: 1px solid var(--border);
    border-radius: 1.1rem;
    background: var(--surface);
  }

  .panel-value,
  .mono,
  .artifact-missing span {
    margin: 0.55rem 0 0;
    color: var(--text-strong);
    font-family: var(--font-mono);
    line-height: 1.55;
    overflow-wrap: anywhere;
  }

  .artifact-highlights {
    display: grid;
    gap: 1rem;
  }

  .artifact-reader {
    display: grid;
    gap: 0.85rem;
  }

  .artifact-reader-header {
    display: flex;
    align-items: end;
    justify-content: space-between;
    gap: 1rem;
    flex-wrap: wrap;
  }

  .artifact-reader-hint {
    margin: 0;
    color: var(--muted);
    line-height: 1.6;
  }

  .artifact-reader-article {
    max-width: none;
    margin: 0;
    padding: 1.25rem 1.35rem 1.5rem;
    border: 1px solid var(--border);
    border-radius: 1.25rem;
    background: var(--surface);
  }

  .artifact-highlights-header {
    display: flex;
    align-items: end;
    justify-content: space-between;
    gap: 1rem;
    flex-wrap: wrap;
  }

  .artifact-highlights-header h2 {
    margin: 0.3rem 0 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: 1.55rem;
    line-height: 1.15;
    letter-spacing: -0.02em;
  }

  .artifact-highlights-header span {
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

  .highlight-stack {
    display: grid;
    gap: 0.85rem;
  }

  .artifact-empty p {
    margin: 0;
    color: var(--muted);
    line-height: 1.6;
  }

  @media (max-width: 760px) {
    .artifact-page,
    .artifact-missing {
      padding-top: 1.5rem;
    }

    .artifact-hero {
      grid-template-columns: 1fr;
    }

    .artifact-hero-media,
    .artifact-hero-media img,
    .artifact-fallback {
      aspect-ratio: 16 / 9;
    }
  }
</style>
