<script lang="ts">
  import type { PageProps } from './$types';
  import { page } from '$app/state';
  import { browser } from '$app/environment';
  import { createFetchEvent, createFetchUser } from '@nostr-dev-kit/svelte';
  import { NDKEvent, type NostrEvent } from '@nostr-dev-kit/ndk';
  import { ndk } from '$lib/ndk/client';
  import { profileIdentifier } from '$lib/ndk/format';
  import { safeUserIdentifier } from '$lib/ndk/user';
  import { mergeUniqueEvents } from '$lib/ndk/events';
  import { targetReferences, buildReferenceFilters } from '$lib/features/articles/comments';
  import ArticleView from '$lib/features/articles/ArticleView.svelte';

  let { data }: PageProps = $props();

  const routeIdentifier = $derived(page.params.id || '');
  const seedEvent = $derived(data.event ? new NDKEvent(ndk, data.event) : undefined);
  const fetchedEvent = createFetchEvent(ndk, () => ({
    bech32: routeIdentifier,
    opts: { closeOnEose: true }
  }));
  const event = $derived(fetchedEvent.event ?? seedEvent);
  const isArticle = $derived(event?.kind === 30023);
  const authorPubkey = $derived(event?.pubkey ?? data.authorPubkey ?? '');
  const author = createFetchUser(ndk, () => authorPubkey || data.authorNpub || '');
  const authorProfile = $derived(author.profile ?? data.profile);
  const authorLinkIdentifier = $derived(
    profileIdentifier(
      authorProfile,
      data.authorIdentifier ||
        safeUserIdentifier(author, data.authorNpub || authorPubkey || 'author')
    )
  );

  // Live comment subscription
  const seedComments = $derived(
    (data.comments ?? []).map((comment: NostrEvent) => new NDKEvent(ndk, comment))
  );
  const seedHighlights = $derived(
    (data.highlights ?? []).map((highlight: NostrEvent) => new NDKEvent(ndk, highlight))
  );

  const liveComments = ndk.$subscribe(() => {
    if (!browser || !event || event.kind !== 30023) return undefined;
    const filters = buildReferenceFilters(targetReferences(event), [1111], {
      addressTag: 'A',
      idTag: 'E',
      limit: 120
    });
    return filters.length > 0 ? { filters } : undefined;
  });

  const liveHighlights = ndk.$subscribe(() => {
    if (!browser || !event || event.kind !== 30023) return undefined;
    const filters = buildReferenceFilters(targetReferences(event), [9802], {
      addressTag: 'a',
      idTag: 'e',
      limit: 80
    });
    return filters.length > 0 ? { filters } : undefined;
  });

  const commentEvents = $derived(
    mergeUniqueEvents(
      liveComments.events.filter((comment) => comment.kind === 1111),
      seedComments
    )
  );
  const highlightEvents = $derived(
    mergeUniqueEvents(
      liveHighlights.events.filter((highlight) => highlight.kind === 9802),
      seedHighlights
    )
  );

  const missing = $derived(!event && (browser ? !fetchedEvent.loading : data.missing));
</script>

{#if missing}
  <section class="article-container">
    <h1>{browser && fetchedEvent.loading ? 'Loading this post...' : 'This post is not available right now'}</h1>
    <p class="muted" style="margin: 0;">
      {browser && fetchedEvent.loading
        ? 'Trying to load it directly from relays.'
        : 'It may have moved, been deleted, or not synced yet.'}
    </p>
  </section>
{:else if event}
  {#if isArticle}
    <ArticleView
      {event}
      {authorPubkey}
      {authorProfile}
      {authorLinkIdentifier}
      {commentEvents}
      {highlightEvents}
    />
  {:else}
    <ArticleView
      {event}
      {authorPubkey}
      {authorProfile}
      {authorLinkIdentifier}
      highlightEvents={[]}
    />
  {/if}
{/if}

<style>
  .article-container {
    max-width: var(--content-width);
    margin: 0 auto;
    display: grid;
    gap: 1.35rem;
  }
</style>
