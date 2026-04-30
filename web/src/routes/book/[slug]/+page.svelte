<script lang="ts">
  import type { PageProps } from './$types';
  import { NDKEvent, type NostrEvent, type NDKUserProfile } from '@nostr-dev-kit/ndk';
  import { ndk } from '$lib/ndk/client';
  import { displayName, avatarUrl, cleanText } from '$lib/ndk/format';
  import { relativeTime } from '$lib/utils/time';

  /// Public book detail page. Mirrors the iOS BookView layout — blurred
  /// hero with the cover floating in front, title + author info, an
  /// expandable description, and a list of "passages" (kind:9802
  /// highlights with `i isbn:<isbn>`) below.

  let { data }: PageProps = $props();

  let descriptionExpanded = $state(false);

  const book = $derived(data.book);
  const highlights = $derived(
    (data.highlights ?? []).map((raw: NostrEvent) => new NDKEvent(ndk, raw))
  );
  const profiles = $derived((data.profiles ?? {}) as Record<string, NDKUserProfile>);
  const description = $derived(cleanText(book?.description ?? ''));
  const isLongDescription = $derived(description.length > 280);
  const visibleDescription = $derived(
    !isLongDescription || descriptionExpanded
      ? description
      : `${description.slice(0, 280).trimEnd()}…`
  );

  function highlightContent(event: NDKEvent): string {
    return cleanText(event.content);
  }
  function highlightNote(event: NDKEvent): string {
    return cleanText(event.tagValue('comment'));
  }
  function authorName(pubkey: string): string {
    return displayName(profiles[pubkey], `${pubkey.slice(0, 8)}…`);
  }
  function authorAvatar(pubkey: string): string | undefined {
    const profile = profiles[pubkey];
    return profile ? avatarUrl(profile) : undefined;
  }
</script>

<article class="book-page">
  <header class="hero">
    <div class="hero-bg" style={book?.coverUrl ? `--hero-bg-image: url(${book.coverUrl});` : ''}></div>
    <div class="hero-overlay"></div>
    <div class="hero-cover-shell">
      {#if book?.coverUrl}
        <img class="hero-cover" src={book.coverUrl} alt="" />
      {:else}
        <div class="hero-cover hero-cover--fallback" aria-hidden="true">📖</div>
      {/if}
    </div>
  </header>

  <section class="info">
    <h1>{book?.title || `ISBN ${book?.isbn13}`}</h1>
    {#if book?.author}
      <p class="author">{book.author.toUpperCase()}</p>
    {/if}
    <p class="isbn">ISBN {book?.isbn13}</p>
  </section>

  {#if description}
    <section class="description">
      <p>{visibleDescription}</p>
      {#if isLongDescription}
        <button type="button" onclick={() => (descriptionExpanded = !descriptionExpanded)}>
          {descriptionExpanded ? 'Show less' : 'Show more'}
        </button>
      {/if}
    </section>
  {/if}

  <section class="passages">
    <h2>Passages</h2>
    {#if highlights.length === 0}
      <p class="empty">No highlights yet from this book.</p>
    {:else}
      <ul>
        {#each highlights as event (event.id)}
          <li class="passage">
            <blockquote>{highlightContent(event)}</blockquote>
            {#if highlightNote(event)}
              <p class="passage-note">{highlightNote(event)}</p>
            {/if}
            <footer class="passage-byline">
              {#if authorAvatar(event.pubkey)}
                <img class="avatar" src={authorAvatar(event.pubkey)} alt="" />
              {:else}
                <span class="avatar avatar-fallback" aria-hidden="true">
                  {authorName(event.pubkey).slice(0, 1).toUpperCase()}
                </span>
              {/if}
              <span class="passage-author">{authorName(event.pubkey)}</span>
              {#if event.created_at}
                <span class="passage-time">· {relativeTime(event.created_at)}</span>
              {/if}
            </footer>
          </li>
        {/each}
      </ul>
    {/if}
  </section>
</article>

<style>
  .book-page {
    max-width: 760px;
    margin: 0 auto 3rem;
    background: var(--color-paper, #fffaf3);
    border: 1px solid var(--color-border, #e8d8cb);
    border-radius: 24px;
    overflow: hidden;
  }

  .hero {
    position: relative;
    height: 300px;
    overflow: hidden;
    display: flex;
    align-items: flex-end;
    justify-content: center;
  }

  .hero-bg {
    position: absolute;
    inset: 0;
    background-image: var(--hero-bg-image, none);
    background-size: cover;
    background-position: center;
    filter: blur(28px);
    transform: scale(1.2);
  }

  .hero-overlay {
    position: absolute;
    inset: 0;
    background: linear-gradient(
      155deg,
      rgba(0, 0, 0, 0.45),
      rgba(0, 0, 0, 0.6)
    );
  }

  .hero-cover-shell {
    position: relative;
    z-index: 1;
    margin-bottom: 24px;
    transform: perspective(800px) rotateY(-2deg);
    box-shadow:
      -6px 10px 20px rgba(0, 0, 0, 0.35),
      0 4px 12px rgba(0, 0, 0, 0.25);
    border-radius: 4px;
    background: var(--color-border, #e8d8cb);
  }

  .hero-cover {
    display: block;
    width: 130px;
    height: 195px;
    object-fit: cover;
    border-radius: 4px;
  }

  .hero-cover--fallback {
    display: flex;
    width: 130px;
    height: 195px;
    align-items: center;
    justify-content: center;
    font-size: 3rem;
    background: var(--color-accent-soft, #f0c7b5);
    color: var(--color-accent, #d05a2d);
    border-radius: 4px;
  }

  .info {
    padding: 1.5rem 1.75rem 1rem;
  }

  h1 {
    font-family: 'Fraunces', Georgia, serif;
    font-weight: 600;
    font-size: clamp(1.5rem, 3vw, 2rem);
    line-height: 1.2;
    margin: 0;
    color: var(--color-ink, #1a1410);
  }

  .author {
    margin: 0.6rem 0 0;
    color: var(--color-muted, #695747);
    font-size: 0.78rem;
    font-weight: 700;
    letter-spacing: 0.06em;
  }

  .isbn {
    margin: 0.4rem 0 0;
    color: var(--color-muted, #695747);
    font-family: 'JetBrains Mono', ui-monospace, monospace;
    font-size: 0.78rem;
  }

  .description {
    padding: 0 1.75rem 1rem;
    color: var(--color-ink, #1a1410);
    line-height: 1.6;
    font-family: 'Fraunces', Georgia, serif;
  }

  .description p {
    margin: 0;
  }

  .description button {
    margin-top: 0.4rem;
    background: none;
    border: none;
    padding: 0;
    color: var(--color-accent, #d05a2d);
    font-weight: 600;
    cursor: pointer;
    font-size: 0.9rem;
  }

  .passages {
    padding: 1rem 1.75rem 2rem;
    border-top: 1px solid var(--color-border, #e8d8cb);
  }

  .passages h2 {
    font-family: 'Fraunces', Georgia, serif;
    font-weight: 600;
    font-size: 1.25rem;
    margin: 0 0 1rem 0;
  }

  .passages ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 1rem;
  }

  .passage {
    padding: 1rem;
    border: 1px solid var(--color-border, #e8d8cb);
    border-radius: 14px;
    background: rgba(0, 0, 0, 0.02);
  }

  .passage blockquote {
    margin: 0;
    font-family: 'Fraunces', Georgia, serif;
    font-style: italic;
    font-weight: 500;
    line-height: 1.5;
    font-size: 1.05rem;
    color: var(--color-ink, #1a1410);
    border-left: 3px solid var(--color-accent, #d05a2d);
    padding-left: 0.75rem;
  }

  .passage-note {
    margin: 0.6rem 0 0;
    color: var(--color-muted, #695747);
    font-family: 'Fraunces', Georgia, serif;
    line-height: 1.5;
  }

  .passage-byline {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-top: 0.75rem;
    font-size: 0.85rem;
    color: var(--color-muted, #695747);
  }

  .avatar {
    width: 24px;
    height: 24px;
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
    font-size: 0.7rem;
  }

  .passage-author {
    color: var(--color-ink, #1a1410);
    font-weight: 600;
  }

  .empty {
    color: var(--color-muted, #695747);
    margin: 0;
  }
</style>
