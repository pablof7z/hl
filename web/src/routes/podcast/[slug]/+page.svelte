<script lang="ts">
  import type { PageProps } from './$types';
  import { NDKEvent, type NostrEvent, type NDKUserProfile } from '@nostr-dev-kit/ndk';
  import { ndk } from '$lib/ndk/client';
  import { displayName, avatarUrl, cleanText } from '$lib/ndk/format';
  import { relativeTime } from '$lib/utils/time';
  import { pause, playEpisode, podcastPlayer, resume } from '$lib/features/podcasts/playerStore.svelte';

  /// Public podcast episode detail. Mirrors iOS PodcastListeningView at
  /// a high level: hero header (show + episode + duration), play pill
  /// hooked into the persistent global player, and a list of "clipped
  /// moments" (kind:9802 highlights with `i podcast:[item:]guid:<guid>`).

  let { data }: PageProps = $props();

  const podcast = $derived(data.podcast);
  const highlights = $derived(
    (data.highlights ?? []).map((raw: NostrEvent) => new NDKEvent(ndk, raw))
  );
  const profiles = $derived((data.profiles ?? {}) as Record<string, NDKUserProfile>);
  const missing = $derived(data.missing === true);

  const playerState = $derived(podcastPlayer.snapshot());
  const isCurrentEpisode = $derived(playerState.episode?.id === podcast?.id);
  const isPlaying = $derived(isCurrentEpisode && playerState.playing);

  function formatDuration(seconds: number | null | undefined): string {
    if (!seconds || !Number.isFinite(seconds)) return '';
    const total = Math.floor(seconds);
    const h = Math.floor(total / 3600);
    const m = Math.floor((total % 3600) / 60);
    const s = total % 60;
    return h > 0
      ? `${h}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`
      : `${m}:${s.toString().padStart(2, '0')}`;
  }

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

  function togglePlayback() {
    if (!podcast) return;
    if (isCurrentEpisode) {
      if (playerState.playing) {
        pause();
      } else {
        resume();
      }
      return;
    }
    if (!podcast.audioUrl) return;
    playEpisode({
      id: podcast.id,
      title: podcast.episodeTitle || podcast.showTitle || 'Podcast episode',
      showTitle: podcast.showTitle,
      imageUrl: podcast.imageUrl,
      audioUrl: podcast.audioUrl,
      durationSeconds: podcast.durationSeconds ?? null,
      detailHref: `/podcast/${podcast.id}`
    });
  }

  function clipStart(event: NDKEvent): number | null {
    const raw = event.tagValue('start');
    if (!raw) return null;
    const value = Number(raw);
    return Number.isFinite(value) && value >= 0 ? value : null;
  }

  function formatClock(seconds: number): string {
    const total = Math.floor(seconds);
    const h = Math.floor(total / 3600);
    const m = Math.floor((total % 3600) / 60);
    const s = total % 60;
    return h > 0
      ? `${h}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`
      : `${m}:${s.toString().padStart(2, '0')}`;
  }
</script>

{#if missing}
  <section class="missing">
    <h1>This podcast page is not available right now.</h1>
    <p>We could not find any highlights tied to this episode id. It may not have synced yet.</p>
  </section>
{:else if podcast}
  <article class="podcast-page">
    <header class="hero">
      <div class="cover-shell">
        {#if podcast.imageUrl}
          <img class="cover" src={podcast.imageUrl} alt="" />
        {:else}
          <div class="cover cover--fallback" aria-hidden="true">🎙</div>
        {/if}
      </div>
      <div class="meta">
        {#if podcast.showTitle}
          <p class="show">{podcast.showTitle.toUpperCase()}</p>
        {/if}
        <h1>{podcast.episodeTitle || podcast.showTitle || 'Podcast episode'}</h1>
        <p class="badges">
          {#if podcast.durationSeconds}
            <span class="badge">{formatDuration(podcast.durationSeconds)}</span>
          {/if}
          <span class="badge"
            >{highlights.length} clipped moment{highlights.length === 1 ? '' : 's'}</span
          >
          {#if podcast.scope === 'show'}
            <span class="badge">Show-level</span>
          {/if}
        </p>
      </div>
    </header>

    <section class="player">
      {#if podcast.audioUrl}
        <button type="button" class="play-pill" class:active={isCurrentEpisode} onclick={togglePlayback}>
          <span class="play-icon" aria-hidden="true">{isPlaying ? '⏸' : '▶'}</span>
          <span class="play-text">
            {isPlaying ? 'Pause' : isCurrentEpisode ? 'Resume' : 'Play episode'}
          </span>
          {#if isCurrentEpisode && playerState.duration}
            <span class="play-progress">
              {formatClock(playerState.position)} / {formatClock(playerState.duration)}
            </span>
          {/if}
        </button>
      {:else}
        <p class="player-empty">No playable audio surface for this episode.</p>
      {/if}
    </section>

    <section class="passages">
      <h2>Clipped moments</h2>
      {#if highlights.length === 0}
        <p class="empty">No highlights yet for this episode.</p>
      {:else}
        <ul>
          {#each highlights as event (event.id)}
            <li class="passage">
              {#if clipStart(event) !== null}
                <span class="clip-time">{formatClock(clipStart(event)!)}</span>
              {/if}
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
{/if}

<style>
  .podcast-page {
    max-width: 760px;
    margin: 2rem auto 3rem;
    padding: 1.5rem;
    background: var(--color-paper, #fffaf3);
    border: 1px solid var(--color-border, #e8d8cb);
    border-radius: 24px;
    display: grid;
    gap: 1.5rem;
  }

  .hero {
    display: grid;
    grid-template-columns: minmax(120px, 160px) minmax(0, 1fr);
    gap: 1.25rem;
  }

  .cover-shell {
    width: 100%;
    aspect-ratio: 1;
    border-radius: 12px;
    overflow: hidden;
    background: var(--color-accent-soft, #f0c7b5);
  }

  .cover {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .cover--fallback {
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 2.5rem;
  }

  .meta {
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 0.5rem;
  }

  .show {
    margin: 0;
    font-size: 0.78rem;
    font-weight: 700;
    letter-spacing: 0.06em;
    color: var(--color-muted, #695747);
  }

  h1 {
    font-family: 'Fraunces', Georgia, serif;
    font-weight: 600;
    font-size: clamp(1.4rem, 2.8vw, 1.85rem);
    line-height: 1.2;
    margin: 0;
    color: var(--color-ink, #1a1410);
  }

  .badges {
    margin: 0;
    display: flex;
    gap: 0.4rem;
    flex-wrap: wrap;
  }

  .badge {
    font-size: 0.72rem;
    font-weight: 600;
    padding: 0.22rem 0.6rem;
    border-radius: 999px;
    background: rgba(0, 0, 0, 0.05);
    color: var(--color-ink, #1a1410);
  }

  .player {
    display: flex;
    align-items: center;
    gap: 1rem;
  }

  .play-pill {
    display: inline-flex;
    align-items: center;
    gap: 0.65rem;
    padding: 0.55rem 1rem;
    border-radius: 999px;
    border: 1px solid var(--color-border, #e8d8cb);
    background: var(--color-paper, #fffaf3);
    cursor: pointer;
    color: var(--color-ink, #1a1410);
    font-weight: 600;
    transition: background 0.15s, border-color 0.15s;
  }

  .play-pill:hover {
    border-color: var(--color-accent, #d05a2d);
    color: var(--color-accent, #d05a2d);
  }

  .play-pill.active {
    background: var(--color-accent, #d05a2d);
    color: #fff8f2;
    border-color: var(--color-accent, #d05a2d);
  }

  .play-icon {
    font-size: 1rem;
  }

  .play-progress {
    font-family: 'JetBrains Mono', ui-monospace, monospace;
    font-size: 0.78rem;
    opacity: 0.85;
  }

  .player-empty {
    margin: 0;
    color: var(--color-muted, #695747);
    font-size: 0.9rem;
  }

  .passages h2 {
    font-family: 'Fraunces', Georgia, serif;
    font-weight: 600;
    font-size: 1.25rem;
    margin: 0 0 0.85rem 0;
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
    position: relative;
  }

  .clip-time {
    display: inline-block;
    font-family: 'JetBrains Mono', ui-monospace, monospace;
    font-size: 0.78rem;
    color: var(--color-accent, #d05a2d);
    font-weight: 700;
    margin-bottom: 0.4rem;
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

  .missing {
    max-width: 600px;
    margin: 6rem auto;
    padding: 0 2rem;
    text-align: center;
  }

  .missing h1 {
    font-family: 'Fraunces', Georgia, serif;
    font-weight: 600;
  }

  .missing p {
    color: var(--color-muted, #695747);
  }
</style>
