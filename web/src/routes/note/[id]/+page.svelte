<script lang="ts">
  import type { PageProps } from './$types';
  import { page } from '$app/state';
  import { createFetchEvent, createFetchUser } from '@nostr-dev-kit/svelte';
  import { NDKEvent, type NostrEvent } from '@nostr-dev-kit/ndk';
  import { ndk } from '$lib/ndk/client';
  import { profileIdentifier } from '$lib/ndk/format';
  import { safeUserIdentifier } from '$lib/ndk/user';
  import ArticleView from '$lib/features/articles/ArticleView.svelte';

  let { data }: PageProps = $props();

  const routeIdentifier = $derived(page.params.id || '');
  const seedEvent = $derived(data.event ? new NDKEvent(ndk, data.event) : undefined);
  const fetchedEvent = createFetchEvent(ndk, () => ({
    bech32: routeIdentifier,
    opts: { closeOnEose: true }
  }));
  const event = $derived(fetchedEvent.event ?? seedEvent);

  const authorPubkey = $derived(event?.pubkey ?? data.authorPubkey ?? '');
  const author = createFetchUser(ndk, () => authorPubkey || data.authorNpub || '');
  const authorProfile = $derived(author.profile ?? data.profile);
  const authorLinkIdentifier = $derived(
    profileIdentifier(
      authorProfile,
      data.authorIdentifier || safeUserIdentifier(author, data.authorNpub || authorPubkey || 'author')
    )
  );

  const seedComments = $derived(
    (data.comments ?? []).map((c: NostrEvent) => new NDKEvent(ndk, c))
  );
  const seedHighlights = $derived(
    (data.highlights ?? []).map((h: NostrEvent) => new NDKEvent(ndk, h))
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
  <ArticleView
    {event}
    {authorPubkey}
    {authorProfile}
    {authorLinkIdentifier}
    {seedComments}
    {seedHighlights}
  />
{/if}

<style>
  .article-container {
    max-width: var(--content-width);
    margin: 0 auto;
    display: grid;
    gap: 1.35rem;
  }
</style>
