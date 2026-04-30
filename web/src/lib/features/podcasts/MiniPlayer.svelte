<script lang="ts">
  import {
    close,
    pause,
    podcastPlayer,
    resume,
    seek
  } from './playerStore';

  /// Sticky bottom bar that appears whenever the global podcast player
  /// has an active episode. Hidden when the user closes it explicitly
  /// or when no episode is loaded.
  ///
  /// Mirrors the iOS MiniPlayerView affordance: artwork + title +
  /// play/pause + scrub bar + close. Tapping the title nav-routes back
  /// to the canonical episode page (`/podcast/<id>` — same id used by
  /// the playerStore).

  const playerState = $derived(podcastPlayer.snapshot());
  const visible = $derived(Boolean(playerState.episode) && !playerState.hidden);

  function clock(seconds: number): string {
    if (!Number.isFinite(seconds) || seconds < 0) return '0:00';
    const total = Math.floor(seconds);
    const h = Math.floor(total / 3600);
    const m = Math.floor((total % 3600) / 60);
    const s = total % 60;
    return h > 0
      ? `${h}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`
      : `${m}:${s.toString().padStart(2, '0')}`;
  }

  function handleScrub(event: Event) {
    const target = event.currentTarget as HTMLInputElement;
    const value = Number(target.value);
    if (!Number.isFinite(value)) return;
    seek(value);
  }

  function handleToggle() {
    if (playerState.playing) {
      pause();
    } else {
      resume();
    }
  }

  // Range needs an upper bound. Default to 1 (so the slider thumb has
  // somewhere to live) and switch to the real duration once metadata
  // resolves.
  const max = $derived(
    playerState.duration && playerState.duration > 0 ? playerState.duration : 1
  );
  const detailHref = $derived(
    playerState.episode?.detailHref ?? (playerState.episode ? `/podcast/${playerState.episode.id}` : '')
  );
</script>

{#if visible && playerState.episode}
  <div class="mini-player" role="region" aria-label="Podcast player">
    <a class="title-link" href={detailHref}>
      {#if playerState.episode.imageUrl}
        <img class="cover" src={playerState.episode.imageUrl} alt="" />
      {:else}
        <span class="cover cover--fallback" aria-hidden="true">🎙</span>
      {/if}
      <span class="meta">
        <span class="title">{playerState.episode.title}</span>
        {#if playerState.episode.showTitle}
          <span class="show">{playerState.episode.showTitle}</span>
        {/if}
      </span>
    </a>

    <button
      type="button"
      class="play"
      onclick={handleToggle}
      aria-label={playerState.playing ? 'Pause' : 'Play'}
    >
      {playerState.playing ? '⏸' : '▶'}
    </button>

    <div class="scrub">
      <span class="time">{clock(playerState.position)}</span>
      <input
        type="range"
        min="0"
        max={max}
        step="1"
        value={Math.min(playerState.position, max)}
        oninput={handleScrub}
        aria-label="Seek"
      />
      <span class="time">{playerState.duration ? clock(playerState.duration) : '—'}</span>
    </div>

    <button type="button" class="close" onclick={close} aria-label="Close mini player">×</button>
  </div>
{/if}

<style>
  .mini-player {
    position: fixed;
    bottom: 0;
    left: 0;
    right: 0;
    z-index: 50;
    display: grid;
    grid-template-columns: minmax(0, 1.4fr) auto minmax(0, 1.6fr) auto;
    align-items: center;
    gap: 0.85rem;
    padding: 0.7rem 1rem;
    background: var(--color-paper, #fffaf3);
    border-top: 1px solid var(--color-border, #e8d8cb);
    box-shadow: 0 -2px 12px rgba(0, 0, 0, 0.06);
  }

  .title-link {
    display: flex;
    align-items: center;
    gap: 0.65rem;
    text-decoration: none;
    color: inherit;
    min-width: 0;
  }

  .cover {
    width: 36px;
    height: 36px;
    border-radius: 6px;
    object-fit: cover;
    flex-shrink: 0;
    background: var(--color-accent-soft, #f0c7b5);
  }

  .cover--fallback {
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 1.1rem;
    color: var(--color-accent, #d05a2d);
  }

  .meta {
    display: flex;
    flex-direction: column;
    min-width: 0;
    line-height: 1.25;
  }

  .title {
    font-weight: 600;
    color: var(--color-ink, #1a1410);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: 0.92rem;
  }

  .show {
    color: var(--color-muted, #695747);
    font-size: 0.78rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .play {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 40px;
    height: 40px;
    border-radius: 50%;
    border: none;
    background: var(--color-accent, #d05a2d);
    color: #fff8f2;
    cursor: pointer;
    font-size: 1.1rem;
    flex-shrink: 0;
  }

  .play:hover {
    filter: brightness(1.05);
  }

  .scrub {
    display: flex;
    align-items: center;
    gap: 0.55rem;
    color: var(--color-muted, #695747);
    font-family: 'JetBrains Mono', ui-monospace, monospace;
    font-size: 0.74rem;
    min-width: 0;
  }

  .scrub input[type='range'] {
    flex: 1;
    min-width: 0;
    accent-color: var(--color-accent, #d05a2d);
  }

  .time {
    flex-shrink: 0;
  }

  .close {
    border: none;
    background: transparent;
    color: var(--color-muted, #695747);
    cursor: pointer;
    font-size: 1.4rem;
    line-height: 1;
    padding: 0.25rem 0.45rem;
    border-radius: 8px;
    flex-shrink: 0;
  }

  .close:hover {
    background: rgba(0, 0, 0, 0.04);
    color: var(--color-ink, #1a1410);
  }

  @media (max-width: 640px) {
    .mini-player {
      grid-template-columns: minmax(0, 1fr) auto auto;
      grid-template-rows: auto auto;
      row-gap: 0.4rem;
    }

    .scrub {
      grid-column: 1 / -1;
    }
  }
</style>
