<script lang="ts">
  import { browser } from '$app/environment';
  import { createFetchEvent, createFetchUser } from '@nostr-dev-kit/svelte';
  import { NDKEvent } from '@nostr-dev-kit/ndk';
  import type { PageProps } from './$types';
  import ArticleMarkdown from '$lib/components/ArticleMarkdown.svelte';
  import HighlightPopover from '$lib/components/HighlightPopover.svelte';
  import HighlightCard from '$lib/features/highlights/HighlightCard.svelte';
  import HighlightForm from '$lib/features/highlights/HighlightForm.svelte';
  import PodcastArtifactView from '$lib/features/podcasts/PodcastArtifactView.svelte';
  import {
    getForLaterArtifact,
    removeForLaterArtifact,
    saveForLaterArtifact
  } from '$lib/features/vault/vault';
  import { ndk } from '$lib/ndk/client';
  import {
    articlePublishedAt,
    articleReadTimeMinutes,
    articleSummary,
    displayNip05,
    displayName,
    formatDisplayDate,
    profileIdentifier
  } from '$lib/ndk/format';
  import {
    fetchHighlightEventsForShares,
    highlightReferenceKey,
    HIGHLIGHTER_HIGHLIGHT_REPOST_KIND,
    hydrateHighlights,
    type HydratedHighlight
  } from '$lib/ndk/highlights';
  import { GROUP_RELAY_URLS } from '$lib/ndk/config';
  import { artifactHighlightReferenceKey, naddrFromAddress } from '$lib/ndk/artifacts';
  import { safeUserIdentifier } from '$lib/ndk/user';
  import { User } from '$lib/ndk/ui/user';

  let { data }: PageProps = $props();
  let articleContentEl = $state<HTMLElement | null>(null);
  const currentUser = $derived(ndk.$currentUser);
  const seedArticleEvent = $derived(data.articleEvent ? new NDKEvent(ndk, data.articleEvent) : undefined);
  const articleBech32 = $derived(
    data.artifact?.referenceTagName === 'a' && data.artifact.referenceKind === '30023'
      ? naddrFromAddress(data.artifact.referenceTagValue) ?? ''
      : ''
  );
  const fetchedArticleEvent = createFetchEvent(ndk, () =>
    articleBech32
      ? { bech32: articleBech32, opts: { closeOnEose: true } }
      : { ids: ['0000000000000000000000000000000000000000000000000000000000000000'], opts: { closeOnEose: true } }
  );
  const articleEvent = $derived(fetchedArticleEvent.event ?? seedArticleEvent);
  const isNostrArticleArtifact = $derived(Boolean(articleBech32));
  const articlePending = $derived(
    isNostrArticleArtifact && !articleEvent && (browser ? fetchedArticleEvent.loading : true)
  );
  const articleUnavailable = $derived(
    isNostrArticleArtifact && !articleEvent && browser && !fetchedArticleEvent.loading
  );
  const articleAuthorPubkey = $derived(articleEvent?.pubkey || data.articleAuthorPubkey || '');
  const articleAuthor = createFetchUser(ndk, () => articleAuthorPubkey || data.articleAuthorNpub || '');
  const articleProfile = $derived(articleAuthor.profile ?? data.articleProfile);
  const articleAuthorIdentifier = $derived(
    profileIdentifier(
      articleProfile,
      data.articleAuthorIdentifier ||
        safeUserIdentifier(articleAuthor, data.articleAuthorNpub || articleAuthorPubkey || 'author')
    )
  );
  const articleAuthorName = $derived(
    displayName(
      articleProfile,
      articleAuthorPubkey ? `${articleAuthorPubkey.slice(0, 8)}...` : 'Author'
    )
  );
  const articleAuthorIdentity = $derived.by(() => {
    const nip05 = displayNip05(articleProfile);
    return nip05 && nip05 !== articleAuthorName ? nip05 : '';
  });

  const highlightShareFeed = ndk.$subscribe(() => {
    if (!browser || !data.community) return undefined;

    return {
      filters: [{ kinds: [HIGHLIGHTER_HIGHLIGHT_REPOST_KIND], '#h': [data.community.id], limit: 256 }],
      relayUrls: GROUP_RELAY_URLS,
      closeOnEose: true
    };
  });
  let fetchedHighlightEvents = $state<NDKEvent[]>([]);

  $effect(() => {
    if (!browser) {
      fetchedHighlightEvents = [];
      return;
    }

    const shareEvents = [...highlightShareFeed.events];
    if (shareEvents.length === 0) {
      fetchedHighlightEvents = [];
      return;
    }

    let cancelled = false;

    void fetchHighlightEventsForShares(ndk, shareEvents).then((events) => {
      if (!cancelled) {
        fetchedHighlightEvents = events as NDKEvent[];
      }
    });

    return () => {
      cancelled = true;
    };
  });
  const communityHighlights = $derived<HydratedHighlight[]>(
    hydrateHighlights(fetchedHighlightEvents, [...highlightShareFeed.events])
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
      ? fetchedHighlightEvents.filter(
          (highlight) =>
            highlightReferenceKey({
              artifactAddress: highlight.tagValue('a'),
              eventReference: highlight.tagValue('e'),
              sourceUrl: highlight.tagValue('r')
            }) === artifactReferenceKey
        )
      : []
  );
  let savingForLater = $state(false);
  let savedForLater = $state(false);
  let forLaterMessage = $state('');
  let forLaterError = $state('');

  $effect(() => {
    if (!browser || !data.artifact) {
      savedForLater = false;
      return;
    }

    let cancelled = false;

    void getForLaterArtifact(data.artifact.id)
      .then((item) => {
        if (!cancelled) {
          savedForLater = Boolean(item);
        }
      })
      .catch(() => {
        if (!cancelled) {
          savedForLater = false;
        }
      });

    return () => {
      cancelled = true;
    };
  });

  async function toggleForLater() {
    if (!data.artifact || !data.community || savingForLater) {
      return;
    }

    savingForLater = true;
    forLaterMessage = '';
    forLaterError = '';

    try {
      if (savedForLater) {
        await removeForLaterArtifact(data.artifact.id);
        savedForLater = false;
        forLaterMessage = 'Removed from For Later.';
        return;
      }

      const result = await saveForLaterArtifact({
        artifact: data.artifact,
        communityIds: [data.community.id],
        sharedRoutes: [{ groupId: data.community.id, artifactId: data.artifact.id }]
      });

      savedForLater = true;
      forLaterMessage = result.existing ? 'Already saved in For Later.' : 'Saved to For Later.';
    } catch (error) {
      forLaterError =
        error instanceof Error ? error.message : 'Could not update your For Later queue.';
    } finally {
      savingForLater = false;
    }
  }
</script>

<svelte:head>
  <title>{data.artifact ? `${data.artifact.title} — Highlighter` : 'Source — Highlighter'}</title>
</svelte:head>

{#if data.missing || !data.community || !data.artifact}
  <section class="artifact-missing">
    <h1>Source not found.</h1>
    <p>
      Nothing currently resolves to <span>/community/{data.groupId}/content/{data.contentId}</span>.
      Share the URL into this community first, then come back here.
    </p>
    <div class="actions">
      <a href={`/community/${data.groupId}`}>Back to community</a>
      <a href="/community">Browse communities</a>
    </div>
  </section>
{:else if data.artifact.source === 'podcast'}
  <PodcastArtifactView
    artifact={data.artifact}
    community={{ id: data.community.id, name: data.community.name }}
    podcast={data.podcast}
    highlights={artifactHighlights}
    {savedForLater}
    {savingForLater}
    {forLaterMessage}
    {forLaterError}
    onToggleForLater={toggleForLater}
  />
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
          <button type="button" class:active={savedForLater} disabled={savingForLater} onclick={toggleForLater}>
            {savingForLater
              ? 'Updating…'
              : savedForLater
                ? 'Saved to For Later'
                : 'Save to For Later'}
          </button>
        </div>

        {#if forLaterError}
          <p class="artifact-feedback error">{forLaterError}</p>
        {/if}

        {#if forLaterMessage}
          <p class="artifact-feedback">{forLaterMessage}</p>
        {/if}
      </div>
    </header>

    {#if articleEvent}
      <section class="artifact-reader">
        <article class="article-container artifact-reader-article">
          <div class="article-byline">
            <User.Root {ndk} pubkey={articleAuthorPubkey} profile={articleProfile}>
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
    {:else if articlePending}
      <section class="artifact-next">
        <p class="panel-label">Loading Article</p>
        <p>Resolving the original Nostr article so you can read and highlight it here.</p>
      </section>
    {:else if articleUnavailable}
      <section class="artifact-next">
        <p class="panel-label">Article Unavailable</p>
        <p>
          This share points to a Nostr article, but the event could not be loaded right now. Use
          the source link above and refresh in a moment.
        </p>
      </section>
    {:else if currentUser}
      <HighlightForm artifact={data.artifact} groupId={data.community.id} />
    {:else}
      <section class="artifact-next">
        <p class="panel-label">Create Highlight</p>
        <p>Sign in to save a highlight and share it into this community.</p>
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
          <p>No highlights shared here yet.</p>
          <p>
            Save the first excerpt from this source and it will appear here once someone shares it
            into this community.
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
      <HighlightPopover
        articleEvent={articleEvent}
        containerEl={articleContentEl}
        groupId={data.community.id}
        artifact={data.artifact}
      />
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
  .artifact-actions button,
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

  .artifact-actions button {
    border: 0;
    cursor: pointer;
  }

  .artifact-actions a:first-child,
  .artifact-actions button.active,
  .actions a:last-child {
    background: var(--accent);
    color: white;
  }

  .artifact-note,
  .artifact-feedback,
  .artifact-next p,
  .artifact-missing p {
    margin: 0;
    color: var(--muted);
    line-height: 1.65;
  }

  .artifact-feedback.error {
    color: #b42318;
  }

  .artifact-next,
  .artifact-empty {
    padding: 1rem 1.1rem;
    border: 1px solid var(--border);
    border-radius: 1.1rem;
    background: var(--surface);
  }

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
