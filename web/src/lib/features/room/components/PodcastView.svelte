<script lang="ts">
  import { onMount } from 'svelte';
  import { browser } from '$app/environment';
  import type { ArtifactRecord } from '$lib/ndk/artifacts';
  import type { PodcastArtifactData } from '$lib/features/podcasts/types';
  import { ensureClientNdk, ndk } from '$lib/ndk/client';
  import { buildArtifactHighlightFilters } from '$lib/ndk/highlights';
  import { formatPodcastClock, formatPodcastDuration, formatPodcastReleaseDate } from '$lib/features/podcasts/format';
  import { User } from '$lib/ndk/ui/user';

  let {
    artifact,
    podcast = undefined,
    roomMemberPubkeys,
    onBack
  }: {
    artifact: ArtifactRecord;
    podcast?: PodcastArtifactData | undefined;
    roomMemberPubkeys: string[];
    onBack: () => void;
  } = $props();

  let audioEl = $state<HTMLAudioElement | null>(null);
  let currentTime = $state(0);
  let audioDuration = $state<number | null>(null);

  onMount(() => {
    void ensureClientNdk();
  });

  const episodeTitle = $derived(podcast?.episodeTitle || artifact.title || 'Untitled');
  const showTitle = $derived(podcast?.showTitle || artifact.podcastShowTitle || artifact.author);
  const description = $derived(podcast?.description || artifact.description);
  const image = $derived(podcast?.image || artifact.image);
  const audioUrl = $derived(podcast?.audioUrl || artifact.audioUrl);
  const publishedAt = $derived(podcast?.publishedAt || artifact.publishedAt);
  const durationSeconds = $derived.by(() => {
    const candidate = audioDuration ?? podcast?.durationSeconds ?? artifact.durationSeconds;
    return typeof candidate === 'number' && Number.isFinite(candidate) && candidate >= 0 ? candidate : null;
  });
  const transcriptSegments = $derived(podcast?.transcriptSegments ?? []);
  const transcriptAvailable = $derived(transcriptSegments.length > 0);

  const highlightsSub = ndk.$subscribe(() => {
    if (!browser) return undefined;
    const filters = buildArtifactHighlightFilters([artifact], roomMemberPubkeys);
    if (filters.length === 0) return undefined;
    return { filters };
  });

  const highlightMarks = $derived.by(() => {
    return highlightsSub.events
      .map((event) => {
        const startRaw = event.tagValue('start');
        const start = startRaw ? Number(startRaw) : NaN;
        if (!Number.isFinite(start) || start < 0) return null;
        return {
          id: event.id,
          pubkey: event.pubkey,
          startSeconds: start,
          content: event.content
        };
      })
      .filter((mark): mark is { id: string; pubkey: string; startSeconds: number; content: string } => mark !== null)
      .toSorted((a, b) => a.startSeconds - b.startSeconds);
  });

  function seekTo(seconds: number) {
    if (!audioEl) return;
    audioEl.currentTime = Math.max(0, seconds);
    void audioEl.play();
  }

  function syncPlaybackState() {
    if (!audioEl) return;
    currentTime = audioEl.currentTime;
    if (Number.isFinite(audioEl.duration)) {
      audioDuration = audioEl.duration;
    }
  }

  function markerLeft(seconds: number): string {
    if (!durationSeconds || durationSeconds <= 0) return '0%';
    return `${(seconds / durationSeconds) * 100}%`;
  }
</script>

<div class="podcast-view">
  <div class="podcast-nav">
    <button class="back-btn" type="button" onclick={onBack}>
      ← Back to room
    </button>
  </div>

  <header class="podcast-hero">
    <div class="hero-cover-wrap">
      {#if image}
        <img class="hero-cover" src={image} alt="" width="200" height="200" />
      {:else}
        <div class="hero-cover hero-cover-placeholder">🎙</div>
      {/if}
    </div>

    <div class="hero-meta">
      <span class="podcast-kicker">PODCAST</span>
      <h1 class="podcast-title">{episodeTitle}</h1>
      {#if showTitle}
        <p class="podcast-author">{showTitle}</p>
      {/if}
      <div class="meta-row">
        {#if durationSeconds}
          <span>{formatPodcastDuration(durationSeconds)}</span>
        {/if}
        {#if publishedAt}
          <span>{formatPodcastReleaseDate(publishedAt)}</span>
        {/if}
        <span>
          {highlightMarks.length}
          clipped {highlightMarks.length === 1 ? 'moment' : 'moments'}
        </span>
      </div>
      {#if description}
        <p class="podcast-description">{description}</p>
      {/if}
    </div>
  </header>

  <section class="player-section">
    {#if audioUrl}
      <audio
        bind:this={audioEl}
        src={audioUrl}
        controls
        preload="metadata"
        class="audio-element"
        ontimeupdate={syncPlaybackState}
        onloadedmetadata={syncPlaybackState}
      ></audio>

      {#if durationSeconds && highlightMarks.length > 0}
        <div class="marker-track" role="presentation">
          {#each highlightMarks as mark (mark.id)}
            <button
              type="button"
              class="marker"
              style:left={markerLeft(mark.startSeconds)}
              aria-label={`Jump to ${formatPodcastClock(mark.startSeconds)}`}
              title={`Jump to ${formatPodcastClock(mark.startSeconds)}`}
              onclick={() => seekTo(mark.startSeconds)}
            ></button>
          {/each}
          {#if currentTime > 0}
            <div
              class="playhead"
              style:left={markerLeft(currentTime)}
              aria-hidden="true"
            ></div>
          {/if}
        </div>
      {/if}
    {:else}
      <div class="audio-unavailable">
        <p>No playable audio was exposed by this source.</p>
        {#if artifact.url}
          <a class="external-link" href={artifact.url} target="_blank" rel="noreferrer noopener">
            Open source ↗
          </a>
        {/if}
      </div>
    {/if}
  </section>

  <div class="podcast-body">
    <section class="timeline-section">
      <h2 class="section-title">Member Timestamps</h2>
      {#if highlightMarks.length === 0}
        <p class="empty-note">No clipped moments from this room yet.</p>
      {:else}
        <ul class="timeline-list">
          {#each highlightMarks as mark (mark.id)}
            <li class="timeline-item">
              <button
                type="button"
                class="timestamp-btn"
                onclick={() => seekTo(mark.startSeconds)}
                title={`Jump to ${formatPodcastClock(mark.startSeconds)}`}
              >
                {formatPodcastClock(mark.startSeconds)}
              </button>
              <div class="timeline-body">
                <User.Root {ndk} pubkey={mark.pubkey}>
                  <span class="stamp-author"><User.Name field="displayName" /></span>
                </User.Root>
                {#if mark.content}
                  <blockquote>{mark.content}</blockquote>
                {/if}
              </div>
            </li>
          {/each}
        </ul>
      {/if}
    </section>

    {#if transcriptAvailable}
      <section class="transcript-section">
        <h2 class="section-title">Transcript</h2>
        <div class="transcript-list">
          {#each transcriptSegments as segment (segment.id)}
            <button
              type="button"
              class="transcript-segment"
              disabled={segment.startSeconds == null}
              onclick={() => segment.startSeconds != null && seekTo(segment.startSeconds)}
            >
              <span class="transcript-time">
                {segment.startSeconds != null ? formatPodcastClock(segment.startSeconds) : '—'}
              </span>
              <span class="transcript-copy">
                {#if segment.speaker}
                  <span class="speaker">{segment.speaker}</span>
                {/if}
                <span>{segment.text}</span>
              </span>
            </button>
          {/each}
        </div>
      </section>
    {/if}
  </div>
</div>

<style>
  .podcast-view {
    display: flex;
    flex-direction: column;
    gap: 32px;
    padding-top: 24px;
    padding-bottom: 80px;
    padding-left: var(--container-px, 40px);
    padding-right: var(--container-px, 40px);
    max-width: var(--container-max, 1440px);
    margin: 0 auto;
  }

  .back-btn {
    font-family: var(--font-sans);
    font-size: 14px;
    font-weight: 500;
    color: var(--ink-soft);
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
    transition: color var(--transition);
  }

  .back-btn:hover {
    color: var(--brand-accent);
  }

  .back-btn:focus-visible {
    outline: 2px solid var(--brand-accent);
    outline-offset: 2px;
    border-radius: var(--radius);
  }

  .podcast-hero {
    display: flex;
    flex-direction: column;
    gap: 32px;
    align-items: flex-start;
  }

  .hero-cover-wrap {
    position: relative;
    flex-shrink: 0;
  }

  .hero-cover {
    width: 160px;
    height: 160px;
    border-radius: var(--radius);
    object-fit: cover;
    display: block;
  }

  .hero-cover-placeholder {
    background-color: var(--surface-muted);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 48px;
    border: 1px solid var(--rule);
  }

  .hero-meta {
    display: flex;
    flex-direction: column;
    gap: 12px;
    flex: 1;
    min-width: 0;
    padding-top: 4px;
  }

  .podcast-kicker {
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 500;
    color: var(--ink-soft);
    text-transform: uppercase;
    letter-spacing: 0.1em;
  }

  .podcast-title {
    font-family: var(--font-serif);
    font-weight: 400;
    font-size: clamp(28px, 4vw, 48px);
    color: var(--ink);
    line-height: 1.15;
    margin: 0;
  }

  .podcast-author {
    font-family: var(--font-sans);
    font-size: 15px;
    font-weight: 500;
    color: var(--ink-soft);
    margin: 0;
  }

  .meta-row {
    display: flex;
    flex-wrap: wrap;
    gap: 16px;
    font-family: var(--font-sans);
    font-size: 13px;
    color: var(--ink-fade);
  }

  .podcast-description {
    margin: 0;
    font-family: var(--font-serif);
    font-size: 16px;
    line-height: 1.6;
    color: var(--ink-soft);
  }

  .player-section {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .audio-element {
    width: 100%;
  }

  .marker-track {
    position: relative;
    height: 14px;
  }

  .marker {
    position: absolute;
    top: 0;
    width: 3px;
    height: 14px;
    transform: translateX(-50%);
    background-color: var(--brand-accent);
    border: none;
    border-radius: 2px;
    padding: 0;
    cursor: pointer;
    opacity: 0.7;
    transition: opacity 120ms ease-out, transform 120ms ease-out;
  }

  .marker:hover {
    opacity: 1;
    transform: translateX(-50%) scaleY(1.25);
  }

  .playhead {
    position: absolute;
    top: -2px;
    width: 2px;
    height: 18px;
    background-color: var(--ink);
    pointer-events: none;
  }

  .audio-unavailable {
    padding: 20px;
    border: 1px solid var(--rule);
    border-radius: var(--radius);
    background: var(--surface);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .audio-unavailable p {
    margin: 0;
    color: var(--ink-soft);
    font-family: var(--font-sans);
    font-size: 14px;
  }

  .external-link {
    align-self: flex-start;
    font-family: var(--font-sans);
    font-size: 14px;
    font-weight: 500;
    color: var(--brand-accent);
    text-decoration: none;
  }

  .external-link:hover {
    text-decoration: underline;
  }

  .podcast-body {
    display: grid;
    grid-template-columns: 1fr;
    gap: 40px;
    align-items: start;
  }

  .section-title {
    font-family: var(--font-sans);
    font-size: 11px;
    font-weight: 600;
    color: var(--ink-fade);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    margin: 0 0 14px;
  }

  .empty-note {
    margin: 0;
    color: var(--ink-fade);
    font-family: var(--font-sans);
    font-size: 13px;
  }

  .timeline-list {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .timeline-item {
    display: flex;
    gap: 14px;
    padding: 12px 16px;
    border-left: 3px solid var(--h-amber);
    background-color: var(--surface-warm);
    border-radius: 0 var(--radius) var(--radius) 0;
  }

  .timestamp-btn {
    flex-shrink: 0;
    font-family: var(--font-mono);
    font-size: 12px;
    font-weight: 600;
    color: var(--brand-accent);
    background: none;
    border: none;
    padding: 2px 0;
    cursor: pointer;
    align-self: flex-start;
  }

  .timestamp-btn:hover {
    text-decoration: underline;
  }

  .timeline-body {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }

  .stamp-author {
    font-family: var(--font-sans);
    font-size: 12px;
    font-weight: 600;
    color: var(--ink);
  }

  .timeline-body blockquote {
    margin: 0;
    font-family: var(--font-serif);
    font-style: italic;
    font-size: 14px;
    color: var(--ink-soft);
    line-height: 1.5;
  }

  .transcript-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-height: 560px;
    overflow-y: auto;
  }

  .transcript-segment {
    display: grid;
    grid-template-columns: 60px 1fr;
    gap: 12px;
    padding: 10px 12px;
    background: none;
    border: 1px solid transparent;
    border-radius: var(--radius);
    text-align: left;
    cursor: pointer;
    font-family: inherit;
    color: inherit;
  }

  .transcript-segment:hover:not([disabled]) {
    background: var(--surface);
    border-color: var(--rule);
  }

  .transcript-segment[disabled] {
    cursor: default;
  }

  .transcript-time {
    font-family: var(--font-mono);
    font-size: 11px;
    font-weight: 600;
    color: var(--ink-fade);
  }

  .transcript-copy {
    font-family: var(--font-serif);
    font-size: 14px;
    line-height: 1.55;
    color: var(--ink);
  }

  .speaker {
    display: block;
    font-family: var(--font-sans);
    font-size: 11px;
    font-weight: 700;
    color: var(--ink-soft);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    margin-bottom: 2px;
  }

  @media (min-width: 768px) {
    .podcast-hero {
      flex-direction: row;
    }

    .hero-cover {
      width: 200px;
      height: 200px;
    }

    .podcast-body {
      grid-template-columns: 3fr 2fr;
    }
  }
</style>
