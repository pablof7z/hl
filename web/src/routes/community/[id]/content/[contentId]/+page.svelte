<script lang="ts">
  import { browser } from '$app/environment';
  import { createFetchEvent, createFetchUser } from '@nostr-dev-kit/svelte';
  import { NDKEvent } from '@nostr-dev-kit/ndk';
  import type { PageProps } from './$types';
  import PodcastArtifactView from '$lib/features/podcasts/PodcastArtifactView.svelte';
  import HighlightForm from '$lib/features/highlights/HighlightForm.svelte';
  import {
    getForLaterArtifact,
    removeForLaterArtifact,
    saveForLaterArtifact
  } from '$lib/features/vault/vault';
  import { ndk } from '$lib/ndk/client';
  import { profileIdentifier } from '$lib/ndk/format';
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
  import type { DiscussionRootContext } from '$lib/features/discussions/discussion';
  import ArticleView from '$lib/features/articles/ArticleView.svelte';

  let { data }: PageProps = $props();
  const currentUser = $derived(ndk.$currentUser);

  // Resolve article event
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

  // Resolve author
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

  // Highlight share events for community
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
      if (!cancelled) fetchedHighlightEvents = events as NDKEvent[];
    });
    return () => { cancelled = true; };
  });

  const artifactReferenceKey = $derived(
    data.artifact ? artifactHighlightReferenceKey(data.artifact) : ''
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

  // Hydrated highlights for podcast view
  const communityHighlights = $derived<HydratedHighlight[]>(
    hydrateHighlights(fetchedHighlightEvents, [...highlightShareFeed.events])
  );
  const artifactHighlights = $derived(
    data.artifact
      ? communityHighlights.filter((highlight) => highlight.sourceReferenceKey === artifactReferenceKey)
      : []
  );

  // Discussion root context
  const artifactRootContext = $derived.by((): DiscussionRootContext | null => {
    if (!data.artifact) return null;
    if (data.artifact.referenceTagName === 'a') {
      return {
        type: 'artifact',
        artifactAddress: data.artifact.referenceTagValue,
        artifactKind: data.artifact.referenceKind
      };
    }
    return {
      type: 'share-thread',
      shareThreadEventId: data.artifact.shareEventId
    };
  });

  // Save for Later
  let savingForLater = $state(false);
  let savedForLater = $state(false);
  let forLaterMessage = $state('');
  let forLaterError = $state('');

  $effect(() => {
    if (!browser || !data.artifact || !currentUser) {
      savedForLater = false;
      return;
    }
    let cancelled = false;
    void getForLaterArtifact(data.artifact.id)
      .then((item) => { if (!cancelled) savedForLater = Boolean(item); })
      .catch(() => { if (!cancelled) savedForLater = false; });
    return () => { cancelled = true; };
  });

  async function toggleForLater() {
    if (!data.artifact || !data.community || savingForLater) return;
    if (!currentUser) {
      forLaterError = 'Sign in to save this source to your private For Later list.';
      forLaterMessage = '';
      return;
    }
    savingForLater = true;
    forLaterMessage = '';
    forLaterError = '';
    try {
      if (savedForLater) {
        await removeForLaterArtifact(data.artifact.id);
        savedForLater = false;
        forLaterMessage = 'Removed from your private For Later list.';
        return;
      }
      const result = await saveForLaterArtifact({
        artifact: data.artifact,
        communityIds: [data.community.id],
        sharedRoutes: [{ groupId: data.community.id, artifactId: data.artifact.id }]
      });
      savedForLater = true;
      forLaterMessage = result.existing
        ? 'Already saved in your private For Later list.'
        : 'Saved to your private For Later list.';
    } catch (error) {
      forLaterError =
        error instanceof Error ? error.message : 'Could not update your private For Later list.';
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
{:else if articleEvent && artifactRootContext}
  <ArticleView
    event={articleEvent}
    authorPubkey={articleAuthorPubkey}
    authorProfile={articleProfile}
    authorLinkIdentifier={articleAuthorIdentifier}
    highlightEvents={artifactHighlightEvents}
    communityContext={{
      groupId: data.community.id,
      communityName: data.community.name,
      communityUrl: `/community/${data.community.id}`,
      artifact: data.artifact,
      rootContext: artifactRootContext
    }}
  />
{:else if articlePending}
  <section class="artifact-status">
    <p class="panel-label">Loading Article</p>
    <p>Resolving the original Nostr article so you can read and highlight it here.</p>
  </section>
{:else if articleUnavailable}
  <section class="artifact-status">
    <p class="panel-label">Article Unavailable</p>
    <p>
      This share points to a Nostr article, but the event could not be loaded right now.
      Refresh in a moment.
    </p>
  </section>
{:else if !isNostrArticleArtifact}
  <!-- Non-Nostr-article artifact: show highlight form or sign-in prompt -->
  <section class="artifact-status">
    <div class="artifact-header">
      <h1>{data.artifact.title}</h1>
      <div class="actions">
        <a class="button" href={data.artifact.url} target="_blank" rel="noreferrer">Open source</a>
        <a class="button-secondary" href={`/community/${data.community.id}`}>Back to {data.community.name}</a>
      </div>
    </div>
    {#if currentUser}
      <HighlightForm artifact={data.artifact} groupId={data.community.id} />
    {:else}
      <p class="muted">Sign in to save a highlight and share it into this community.</p>
    {/if}
  </section>
{/if}

<style>
  .artifact-missing,
  .artifact-status {
    display: grid;
    gap: 1.5rem;
    padding: 2rem 0 3rem;
  }

  .artifact-missing h1 {
    margin: 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: clamp(2rem, 4vw, 3rem);
    line-height: 1.05;
    letter-spacing: -0.03em;
  }

  .artifact-missing p,
  .artifact-status p {
    margin: 0;
    color: var(--muted);
    line-height: 1.65;
  }

  .artifact-missing span {
    color: var(--text-strong);
    font-family: var(--font-mono);
    overflow-wrap: anywhere;
  }

  .panel-label {
    margin: 0;
    color: var(--accent);
    font-size: 0.8rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .artifact-header {
    display: grid;
    gap: 1rem;
  }

  .artifact-header h1 {
    margin: 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    font-size: clamp(2rem, 4vw, 3rem);
    line-height: 1.05;
    letter-spacing: -0.03em;
  }
</style>
