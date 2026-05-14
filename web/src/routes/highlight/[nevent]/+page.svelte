<script lang="ts">
  import type { PageProps } from './$types';
  import { page } from '$app/state';
  import { browser } from '$app/environment';
  import { createFetchEvent } from '@nostr-dev-kit/svelte';
  import { NDKEvent, type NostrEvent } from '@nostr-dev-kit/ndk';
  import { ndk } from '$lib/ndk/client';
  import { displayName, avatarUrl, cleanText } from '$lib/ndk/format';
  import { profileIdentifier } from '$lib/ndk/format';
  import HighlightComments from './HighlightComments.svelte';
  import HighlightActions from './HighlightActions.svelte';

  let { data }: PageProps = $props();

  const routeIdentifier = $derived(page.params.nevent || '');
  const seedEvent = $derived(data.event ? new NDKEvent(ndk, data.event) : undefined);
  const fetchedEvent = createFetchEvent(ndk, () => ({
    bech32: routeIdentifier,
    opts: { closeOnEose: true }
  }));
  const event = $derived(fetchedEvent.event ?? seedEvent);

  const quote = $derived(cleanText(event?.content ?? ''));
  const note = $derived(cleanText(event?.tagValue('comment') ?? ''));
  const pageImageUrl = $derived(data.pageImageUrl ?? '');
  const source = $derived(data.source);

  const authorPubkey = $derived(event?.pubkey ?? data.authorPubkey ?? '');
  const profile = $derived(data.profile);
  const authorName = $derived(displayName(profile, 'A reader'));
  const authorAvatar = $derived(profile ? avatarUrl(profile) : '');
  const authorLinkIdentifier = $derived(
    profileIdentifier(profile, data.authorIdentifier || data.authorNpub || authorPubkey || 'author')
  );

  const seedComments = $derived(
    (data.comments ?? []).map((c: NostrEvent) => new NDKEvent(ndk, c))
  );

  const missing = $derived(!event && (browser ? !fetchedEvent.loading : data.missing));

  function relativeDate(seconds: number | null | undefined): string {
    if (!seconds) return '';
    const delta = Date.now() / 1000 - seconds;
    if (delta < 60) return 'just now';
    if (delta < 3600) return `${Math.floor(delta / 60)}m ago`;
    if (delta < 86400) return `${Math.floor(delta / 3600)}h ago`;
    if (delta < 86400 * 7) return `${Math.floor(delta / 86400)}d ago`;
    if (delta < 86400 * 30) return `${Math.floor(delta / (86400 * 7))}w ago`;
    return `${Math.floor(delta / (86400 * 30))}mo ago`;
  }

  const createdLabel = $derived(relativeDate(event?.created_at ?? null));
</script>

{#if missing}
  <section class="missing">
    <h1>{browser && fetchedEvent.loading ? 'Loading this highlight…' : 'This highlight is not available right now'}</h1>
    <p>It may not have synced yet. Try again in a moment.</p>
  </section>
{:else if event}
  <article class="highlight-page">
    <div class="badge">HIGHLIGHT</div>

    {#if source}
      <a class="source-card" href={source.href}>
        {#if source.coverUrl}
          <img class="source-cover" src={source.coverUrl} alt="" />
        {:else}
          <div class="source-cover source-cover--fallback" aria-hidden="true">
            {source.kind === 'book'
              ? '📖'
              : source.kind === 'article'
                ? '📄'
                : source.kind === 'podcast'
                  ? '🎙'
                  : '🌐'}
          </div>
        {/if}
        <div class="source-meta">
          <div class="source-kind">{source.kind.toUpperCase()}</div>
          <div class="source-title">{source.title}</div>
          {#if source.author}
            <div class="source-author">{source.author}</div>
          {/if}
        </div>
        <div class="source-arrow" aria-hidden="true">→</div>
      </a>
    {/if}

    <div class="byline">
      <a class="author-link" href="/p/{authorLinkIdentifier}">
        {#if authorAvatar}
          <img class="avatar" src={authorAvatar} alt="" />
        {:else}
          <span class="avatar avatar-fallback">{authorName.slice(0, 1).toUpperCase()}</span>
        {/if}
        <div class="byline-text">
          <div class="byline-name">{authorName}</div>
          {#if createdLabel}
            <div class="byline-time">{createdLabel}</div>
          {/if}
        </div>
      </a>
    </div>

    {#if event.id}
      <HighlightActions highlightEventId={event.id} highlightAuthorPubkey={authorPubkey} />
    {/if}

    {#if pageImageUrl}
      <div class="page-photo">
        <img src={pageImageUrl} alt="Highlighted page" loading="lazy" />
      </div>
    {/if}

    <blockquote class="quote">
      <span class="open-quote" aria-hidden="true">“</span>
      <p class="quote-text">{quote}</p>
      <span class="close-quote" aria-hidden="true">”</span>
    </blockquote>

    {#if note}
      <p class="note">{note}</p>
    {/if}

    {#if event.id}
      <section class="comments-section">
        <h2 class="comments-heading">Discussion</h2>
        <HighlightComments highlightEventId={event.id} {seedComments} />
      </section>
    {/if}
  </article>
{/if}

<style>
  .highlight-page {
    max-width: 760px;
    margin: 3rem auto;
    padding: 2.5rem 2.5rem 3rem;
    background: var(--color-paper, #fffaf3);
    border: 1px solid var(--color-border, #e8d8cb);
    border-radius: 24px;
    position: relative;
    overflow: hidden;
  }

  .highlight-page::before {
    content: '';
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 6px;
    background: var(--color-accent, #d05a2d);
  }

  .badge {
    display: inline-block;
    background: var(--color-accent, #d05a2d);
    color: #fff8f2;
    font-size: 0.7rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    padding: 0.3rem 0.75rem;
    border-radius: 999px;
    margin-bottom: 1.5rem;
  }

  /* Source card (book / article / web) */

  .source-card {
    display: flex;
    align-items: center;
    gap: 0.9rem;
    padding: 0.85rem 1rem;
    border-radius: 14px;
    background: rgba(0, 0, 0, 0.025);
    border: 1px solid var(--color-border, #e8d8cb);
    text-decoration: none;
    color: inherit;
    transition: background 0.15s;
    margin-bottom: 1.5rem;
  }

  .source-card:hover {
    background: rgba(0, 0, 0, 0.04);
  }

  .source-cover {
    width: 56px;
    height: 56px;
    border-radius: 6px;
    object-fit: cover;
    flex-shrink: 0;
    background: var(--color-border, #e8d8cb);
  }

  .source-cover--fallback {
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 1.5rem;
    background: var(--color-border, #e8d8cb);
  }

  .source-meta {
    flex: 1;
    min-width: 0;
  }

  .source-kind {
    font-size: 0.7rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    color: var(--color-muted, #695747);
    margin-bottom: 0.15rem;
  }

  .source-title {
    font-family: 'Inter', system-ui, sans-serif;
    font-weight: 600;
    font-size: 1.05rem;
    color: var(--color-ink, #1a1410);
    line-height: 1.25;
    overflow: hidden;
    text-overflow: ellipsis;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }

  .source-author {
    font-size: 0.85rem;
    color: var(--color-muted, #695747);
    margin-top: 0.15rem;
  }

  .source-arrow {
    color: var(--color-muted, #695747);
    font-size: 1.1rem;
    flex-shrink: 0;
  }

  /* Byline */

  .byline {
    margin-bottom: 1.5rem;
  }

  .author-link {
    display: inline-flex;
    align-items: center;
    gap: 0.7rem;
    text-decoration: none;
    color: inherit;
  }

  .avatar {
    width: 40px;
    height: 40px;
    border-radius: 50%;
    object-fit: cover;
    background: var(--color-accent, #d05a2d);
  }

  .avatar-fallback {
    display: flex;
    align-items: center;
    justify-content: center;
    color: #fff8f2;
    font-weight: 700;
  }

  .byline-text {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
  }

  .byline-name {
    font-weight: 600;
    color: var(--color-ink, #1a1410);
    font-size: 0.95rem;
  }

  .byline-time {
    font-size: 0.8rem;
    color: var(--color-muted, #695747);
  }

  /* Page photo */

  .page-photo {
    margin-bottom: 1.75rem;
    border-radius: 14px;
    overflow: hidden;
    border: 1px solid var(--color-border, #e8d8cb);
  }

  .page-photo img {
    display: block;
    width: 100%;
    height: auto;
  }

  /* Quote */

  .quote {
    margin: 0 0 1.25rem 0;
    padding: 0 0 0 0.5rem;
    position: relative;
  }

  .open-quote,
  .close-quote {
    font-family: 'Inter', system-ui, sans-serif;
    font-size: 5rem;
    line-height: 0.5;
    color: var(--color-accent, #d05a2d);
    opacity: 0.25;
    position: absolute;
    pointer-events: none;
  }

  .open-quote {
    top: 0.4em;
    left: -0.6em;
  }

  .close-quote {
    bottom: -0.4em;
    right: -0.2em;
  }

  .quote-text {
    font-family: 'Inter', system-ui, sans-serif;
    font-style: italic;
    font-weight: 500;
    font-size: clamp(1.4rem, 2.4vw, 1.85rem);
    line-height: 1.4;
    color: var(--color-ink, #1a1410);
    margin: 0;
    white-space: pre-wrap;
  }

  .note {
    margin: 0 0 1.5rem 0;
    font-family: 'Inter', system-ui, sans-serif;
    font-size: 1rem;
    line-height: 1.55;
    color: var(--color-muted, #695747);
    border-left: 3px solid var(--color-border, #e8d8cb);
    padding: 0.4rem 0 0.4rem 1rem;
  }

  /* Comments section */

  .comments-section {
    margin-top: 2.5rem;
    padding-top: 1.75rem;
    border-top: 1px solid var(--color-border, #e8d8cb);
  }

  .comments-heading {
    font-family: 'Inter', system-ui, sans-serif;
    font-weight: 600;
    font-size: 1.25rem;
    color: var(--color-ink, #1a1410);
    margin: 0 0 1rem 0;
  }

  /* Missing state */

  .missing {
    max-width: 600px;
    margin: 6rem auto;
    padding: 0 2rem;
    text-align: center;
  }

  .missing h1 {
    font-family: 'Inter', system-ui, sans-serif;
    font-weight: 600;
  }

  .missing p {
    color: var(--color-muted, #695747);
  }
</style>
