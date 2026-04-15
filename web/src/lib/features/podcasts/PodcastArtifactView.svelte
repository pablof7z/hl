<script lang="ts">
  import HighlightCard from '$lib/features/highlights/HighlightCard.svelte';
  import DiscussionPanel from '$lib/features/discussions/DiscussionPanel.svelte';
  import type { DiscussionRootContext } from '$lib/features/discussions/discussion';
  import {
    formatPodcastClock,
    formatPodcastDuration,
    formatPodcastReleaseDate,
    parseTimeInput
  } from '$lib/features/podcasts/format';
  import type { PodcastArtifactData, PodcastTranscriptSegment } from '$lib/features/podcasts/types';
  import type { ArtifactRecord } from '$lib/ndk/artifacts';
  import { ensureClientNdk, ndk } from '$lib/ndk/client';
  import { publishAndShareHighlight, type HydratedHighlight } from '$lib/ndk/highlights';

  type SortMode = 'top' | 'newest' | 'timeline';

  let {
    artifact,
    community,
    podcast = undefined,
    highlights = [],
    savedForLater = false,
    savingForLater = false,
    forLaterMessage = '',
    forLaterError = '',
    onToggleForLater
  }: {
    artifact: ArtifactRecord;
    community: { id: string; name: string };
    podcast?: PodcastArtifactData | undefined;
    highlights?: HydratedHighlight[];
    savedForLater?: boolean;
    savingForLater?: boolean;
    forLaterMessage?: string;
    forLaterError?: string;
    onToggleForLater: () => void | Promise<void>;
  } = $props();

  let viewMode = $state<'listen' | 'discussion'>('listen');

  const podcastRootContext = $derived.by((): DiscussionRootContext => {
    if (artifact.referenceTagName === 'a') {
      return {
        type: 'artifact',
        artifactAddress: artifact.referenceTagValue,
        artifactKind: artifact.referenceKind
      };
    }
    return {
      type: 'share-thread',
      shareThreadEventId: artifact.shareEventId
    };
  });

  let audioEl = $state<HTMLAudioElement | null>(null);
  let timelineEl = $state<HTMLElement | null>(null);
  let audioDuration = $state<number | null>(null);
  let currentTime = $state(0);
  let isPlaying = $state(false);
  let sortMode = $state<SortMode>('top');
  let clipStart = $state<number | null>(null);
  let clipEnd = $state<number | null>(null);
  let dragAnchor = $state<number | null>(null);
  let draggingSelection = $state(false);
  let note = $state('');
  let publishing = $state(false);
  let publishError = $state('');
  let publishStatus = $state('');
  let transcriptNodes = $state<Array<HTMLElement | null>>([]);
  let clipMode = $state<'listen' | 'fine-tune' | 'preview'>('listen');
  let zoomedTimelineEl = $state<HTMLElement | null>(null);
  let draggedBoundary = $state<'start' | 'end' | null>(null);

  const currentUser = $derived(ndk.$currentUser);
  const isReadOnly = $derived(Boolean(ndk.$sessions?.isReadOnly()));
  const episodeTitle = $derived(podcast?.episodeTitle || artifact.title);
  const showTitle = $derived(podcast?.showTitle || artifact.podcastShowTitle || artifact.author);
  const description = $derived(podcast?.description || artifact.description);
  const image = $derived(podcast?.image || artifact.image);
  const audioUrl = $derived(podcast?.audioUrl || artifact.audioUrl);
  const transcriptUrl = $derived(podcast?.transcriptUrl || artifact.transcriptUrl);
  const durationSeconds = $derived.by(() => {
    const candidate = audioDuration ?? podcast?.durationSeconds ?? artifact.durationSeconds;
    return typeof candidate === 'number' && Number.isFinite(candidate) && candidate >= 0 ? candidate : null;
  });
  const publishedAt = $derived(podcast?.publishedAt || artifact.publishedAt);
  const playbackAvailable = $derived(Boolean(audioUrl));
  const transcriptSegments = $derived(podcast?.transcriptSegments ?? []);
  const transcriptAvailable = $derived(transcriptSegments.length > 0);
  const transcriptHasTiming = $derived(
    transcriptSegments.some((segment) => segment.startSeconds != null || segment.endSeconds != null)
  );
  const activeTranscriptIndex = $derived.by(() => {
    const segments = transcriptSegments;
    if (segments.length === 0) return -1;

    for (let index = 0; index < segments.length; index += 1) {
      const segment = segments[index];
      if (segment.startSeconds == null) continue;
      const end = segment.endSeconds ?? segment.startSeconds + 8;
      if (currentTime >= segment.startSeconds && currentTime < end) {
        return index;
      }
    }

    let fallbackIndex = -1;
    for (let index = 0; index < segments.length; index += 1) {
      if ((segments[index].startSeconds ?? Infinity) <= currentTime) {
        fallbackIndex = index;
      }
    }

    return fallbackIndex;
  });
  const clipRange = $derived.by(() => normalizeRange(clipStart, clipEnd));
  const hasPartialClip = $derived(clipStart != null && clipEnd == null);
  const hasCompleteClip = $derived(clipRange != null);
  const canPreviewClip = $derived(hasCompleteClip && playbackAvailable);
  const zoomedTimelineRange = $derived.by(() => {
    if (!clipRange) return null;
    const totalDuration = durationSeconds ?? 0;
    if (totalDuration <= 0) return null;
    const pad = 15;
    const start = Math.max(0, clipRange.start - pad);
    const end = Math.min(totalDuration, clipRange.end + pad);
    return { start, end, duration: end - start };
  });
  const clipDurationSeconds = $derived.by(() => {
    if (!clipRange) return 0;
    return clipRange.end - clipRange.start;
  });
  const selectedTranscriptSegments = $derived.by(() => {
    const range = clipRange;
    if (!range) return [];
    return transcriptSegments.filter((segment) => segmentOverlapsRange(segment, range.start, range.end));
  });
  const selectedTranscriptQuote = $derived.by(() =>
    selectedTranscriptSegments.map((segment) => segment.text).join(' ').trim()
  );
  const selectedTranscriptContext = $derived.by(() => {
    if (selectedTranscriptSegments.length === 0) return '';
    const firstIndex = transcriptSegments.findIndex((segment) => segment.id === selectedTranscriptSegments[0]?.id);
    const lastIndex = transcriptSegments.findIndex(
      (segment) => segment.id === selectedTranscriptSegments[selectedTranscriptSegments.length - 1]?.id
    );
    const before = firstIndex > 0 ? transcriptSegments[firstIndex - 1]?.text : '';
    const after =
      lastIndex >= 0 && lastIndex < transcriptSegments.length - 1
        ? transcriptSegments[lastIndex + 1]?.text
        : '';

    return [before, selectedTranscriptQuote, after].filter(Boolean).join(' ').trim();
  });
  const selectedTranscriptSpeaker = $derived.by(() => {
    const speakers = [...new Set(selectedTranscriptSegments.map((segment) => segment.speaker).filter(Boolean))];
    return speakers.length === 1 ? speakers[0] : '';
  });
  const selectedTranscriptIds = $derived(selectedTranscriptSegments.map((segment) => segment.id));
  const canPublishClip = $derived(
    Boolean(currentUser && clipRange && !publishing && !isReadOnly)
  );
  const clipMarkerHighlights = $derived(
    highlights.filter((highlight) => highlight.clipStartSeconds != null && Boolean(durationSeconds))
  );
  const sortedHighlights = $derived.by(() => {
    const items = [...highlights];

    if (sortMode === 'newest') {
      return items.toSorted((left, right) => (right.createdAt ?? 0) - (left.createdAt ?? 0));
    }

    if (sortMode === 'timeline') {
      return items.toSorted((left, right) => {
        const leftTime = left.clipStartSeconds ?? Number.POSITIVE_INFINITY;
        const rightTime = right.clipStartSeconds ?? Number.POSITIVE_INFINITY;
        if (leftTime !== rightTime) return leftTime - rightTime;
        return (right.createdAt ?? 0) - (left.createdAt ?? 0);
      });
    }

    return items.toSorted((left, right) => {
      if (right.shareCount !== left.shareCount) return right.shareCount - left.shareCount;
      const leftTime = left.clipStartSeconds ?? Number.POSITIVE_INFINITY;
      const rightTime = right.clipStartSeconds ?? Number.POSITIVE_INFINITY;
      if (leftTime !== rightTime) return leftTime - rightTime;
      return (right.createdAt ?? 0) - (left.createdAt ?? 0);
    });
  });
  const clippedPeopleCount = $derived(new Set(highlights.map((highlight) => highlight.pubkey)).size);
  const waveformBars = $derived(buildWaveformBars(`${artifact.id}:${episodeTitle}`, 64));

  $effect(() => {
    if (activeTranscriptIndex < 0 || !isPlaying) return;
    const node = transcriptNodes[activeTranscriptIndex];
    node?.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
  });

  $effect(() => {
    if (hasCompleteClip && clipMode === 'listen') {
      clipMode = 'fine-tune';
    }
  });

  $effect(() => {
    if (typeof window === 'undefined') return;

    function handleKeyDown(event: KeyboardEvent) {
      const target = event.target as HTMLElement | null;
      if (target?.tagName === 'INPUT' || target?.tagName === 'TEXTAREA') return;

      if (event.key === 'i' || event.key === 'I') {
        event.preventDefault();
        handleMarkIn();
        return;
      }

      if (event.key === 'o' || event.key === 'O') {
        event.preventDefault();
        handleMarkOut();
        return;
      }

      if (event.key === ' ') {
        event.preventDefault();
        togglePlayback();
        return;
      }

      if (event.key === 'Escape' && clipMode === 'preview') {
        event.preventDefault();
        stopPreview();
        return;
      }
    }

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  });

  $effect(() => {
    if (typeof window === 'undefined') return;

    function handlePointerMove(event: PointerEvent) {
      if (draggedBoundary && zoomedTimelineEl && clipRange) {
        const time = pointerToZoomedTime(event.clientX);
        if (draggedBoundary === 'start') {
          clipStart = Math.min(time, clipRange.end - 1);
        } else {
          clipEnd = Math.max(time, clipRange.start + 1);
        }
        return;
      }

      if (!draggingSelection || dragAnchor == null) return;
      clipStart = dragAnchor;
      clipEnd = pointerToTime(event.clientX);
    }

    function handlePointerUp(event: PointerEvent) {
      if (draggedBoundary) {
        draggedBoundary = null;
        return;
      }

      if (!draggingSelection || dragAnchor == null) return;
      clipStart = dragAnchor;
      clipEnd = pointerToTime(event.clientX);
      draggingSelection = false;
      dragAnchor = null;
    }

    window.addEventListener('pointermove', handlePointerMove);
    window.addEventListener('pointerup', handlePointerUp);

    return () => {
      window.removeEventListener('pointermove', handlePointerMove);
      window.removeEventListener('pointerup', handlePointerUp);
    };
  });

  function togglePlayback() {
    if (!audioEl || !playbackAvailable) return;

    if (isPlaying) {
      audioEl.pause();
      isPlaying = false;
      return;
    }

    void audioEl.play();
    isPlaying = true;
  }

  function handleTimelinePointerDown(event: PointerEvent) {
    if (!durationSeconds) return;
    const time = pointerToTime(event.clientX);
    dragAnchor = time;
    clipStart = time;
    clipEnd = time;
    draggingSelection = true;
  }

  function pointerToTime(clientX: number): number {
    const totalDuration = durationSeconds ?? 0;
    if (!timelineEl || totalDuration <= 0) return 0;

    const rect = timelineEl.getBoundingClientRect();
    const ratio = Math.min(1, Math.max(0, (clientX - rect.left) / rect.width));
    return ratio * totalDuration;
  }

  function seekToTime(seconds: number) {
    currentTime = Math.max(0, seconds);
    if (audioEl && playbackAvailable) {
      audioEl.currentTime = Math.max(0, seconds);
    }
  }

  function handleSegmentClick(segment: PodcastTranscriptSegment, index: number) {
    transcriptNodes[index]?.scrollIntoView({ block: 'nearest', behavior: 'smooth' });

    if (segment.startSeconds != null) {
      seekToTime(segment.startSeconds);
    }

    if (playbackAvailable || !transcriptHasTiming || segment.startSeconds == null) {
      return;
    }

    if (clipStart == null || clipEnd == null) {
      clipStart = segment.startSeconds;
      clipEnd = segment.endSeconds ?? segment.startSeconds + 8;
      return;
    }

    const nextStart = Math.min(clipStart, segment.startSeconds);
    const nextEnd = Math.max(clipEnd, segment.endSeconds ?? segment.startSeconds + 8);
    clipStart = nextStart;
    clipEnd = nextEnd;
  }

  function handleMarkIn() {
    if (!playbackAvailable) return;
    clipStart = currentTime;
    if (clipEnd != null && clipEnd <= currentTime) {
      clipEnd = null;
    }
  }

  function handleMarkOut() {
    if (!playbackAvailable) return;
    clipEnd = currentTime;
    if (clipStart == null) {
      clipStart = Math.max(0, currentTime - 30);
    }
    if (clipStart > currentTime) {
      clipStart = Math.max(0, currentTime - 30);
    }
  }

  function nudgeClipStart(delta: number) {
    if (clipStart == null) return;
    const totalDuration = durationSeconds ?? Infinity;
    clipStart = Math.max(0, Math.min(clipStart + delta, clipEnd != null ? clipEnd - 1 : totalDuration));
  }

  function nudgeClipEnd(delta: number) {
    if (clipEnd == null) return;
    const totalDuration = durationSeconds ?? Infinity;
    clipEnd = Math.min(totalDuration, Math.max(clipEnd + delta, clipStart != null ? clipStart + 1 : 0));
  }

  function handleTimeInputChange(boundary: 'start' | 'end', value: string) {
    const seconds = parseTimeInput(value);
    if (seconds == null) return;
    const totalDuration = durationSeconds ?? Infinity;
    if (boundary === 'start') {
      clipStart = Math.max(0, Math.min(seconds, clipEnd != null ? clipEnd - 1 : totalDuration));
    } else {
      clipEnd = Math.min(totalDuration, Math.max(seconds, clipStart != null ? clipStart + 1 : 0));
    }
  }

  function startPreview() {
    if (!canPreviewClip || !audioEl || !clipRange) return;
    clipMode = 'preview';
    audioEl.currentTime = clipRange.start;
    void audioEl.play();
    isPlaying = true;
  }

  function stopPreview() {
    clipMode = 'fine-tune';
    if (audioEl && isPlaying) {
      audioEl.pause();
      isPlaying = false;
    }
  }

  function pointerToZoomedTime(clientX: number): number {
    if (!zoomedTimelineEl || !zoomedTimelineRange) return 0;
    const rect = zoomedTimelineEl.getBoundingClientRect();
    const ratio = Math.min(1, Math.max(0, (clientX - rect.left) / rect.width));
    return zoomedTimelineRange.start + ratio * zoomedTimelineRange.duration;
  }

  function handleZoomedDragStart(event: PointerEvent, boundary: 'start' | 'end') {
    event.preventDefault();
    event.stopPropagation();
    draggedBoundary = boundary;
  }

  function zoomedSelectionLeft(): string {
    if (!clipRange || !zoomedTimelineRange) return '0%';
    return `${((clipRange.start - zoomedTimelineRange.start) / zoomedTimelineRange.duration) * 100}%`;
  }

  function zoomedSelectionWidth(): string {
    if (!clipRange || !zoomedTimelineRange) return '0%';
    return `${((clipRange.end - clipRange.start) / zoomedTimelineRange.duration) * 100}%`;
  }

  function zoomedMarkerLeft(seconds: number): string {
    if (!zoomedTimelineRange) return '0%';
    return `${((seconds - zoomedTimelineRange.start) / zoomedTimelineRange.duration) * 100}%`;
  }

  function clearClip() {
    clipStart = null;
    clipEnd = null;
    clipMode = 'listen';
    note = '';
    publishError = '';
    publishStatus = '';
  }

  async function publishClip() {
    if (!currentUser) {
      publishError = 'Sign in before clipping moments.';
      return;
    }

    if (isReadOnly) {
      publishError = 'Read-only sessions cannot publish highlight events.';
      return;
    }

    if (!clipRange) {
      publishError = 'Drag across the timeline or select timed transcript lines first.';
      return;
    }

    publishing = true;
    publishError = '';
    publishStatus = '';

    try {
      await ensureClientNdk();

      const result = await publishAndShareHighlight(ndk, {
        groupId: community.id,
        artifact,
        quote: selectedTranscriptQuote,
        context: selectedTranscriptContext,
        note,
        clip: {
          startTime: clipRange.start,
          endTime: clipRange.end,
          speaker: selectedTranscriptSpeaker,
          transcriptSegmentIds: selectedTranscriptIds
        }
      });

      publishStatus = result.shareExisting
        ? 'Clip saved. This community already had a share for it.'
        : 'Clip saved and shared to this community.';
      note = '';
      clipStart = null;
      clipEnd = null;
      clipMode = 'listen';
    } catch (error) {
      publishError = error instanceof Error ? error.message : 'Could not save that clip.';
    } finally {
      publishing = false;
    }
  }

  function markerLeft(seconds: number): string {
    if (!durationSeconds || durationSeconds <= 0) return '0%';
    return `${(seconds / durationSeconds) * 100}%`;
  }

  function selectionLeft(): string {
    if (!clipRange || !durationSeconds || durationSeconds <= 0) return '0%';
    return `${(clipRange.start / durationSeconds) * 100}%`;
  }

  function selectionWidth(): string {
    if (!clipRange || !durationSeconds || durationSeconds <= 0) return '0%';
    return `${((clipRange.end - clipRange.start) / durationSeconds) * 100}%`;
  }

  function syncPlaybackState() {
    if (!audioEl) return;
    currentTime = audioEl.currentTime;
    isPlaying = !audioEl.paused;
    audioDuration = Number.isFinite(audioEl.duration) ? audioEl.duration : audioDuration;

    if (clipMode === 'preview' && clipRange && currentTime >= clipRange.end) {
      audioEl.currentTime = clipRange.start;
    }
  }

  function buildWaveformBars(seed: string, count: number) {
    const values: number[] = [];
    let hash = 2166136261;

    for (const char of seed) {
      hash ^= char.charCodeAt(0);
      hash = Math.imul(hash, 16777619);
    }

    for (let index = 0; index < count; index += 1) {
      hash ^= index + 1;
      hash = Math.imul(hash, 16777619);
      values.push(18 + (Math.abs(hash) % 58));
    }

    return values;
  }

  function normalizeRange(start: number | null, end: number | null): { start: number; end: number } | null {
    if (start == null || end == null) return null;
    const normalizedStart = Math.min(start, end);
    const normalizedEnd = Math.max(start, end);
    if (normalizedEnd <= normalizedStart) return null;
    return { start: normalizedStart, end: normalizedEnd };
  }

  function segmentOverlapsRange(segment: PodcastTranscriptSegment, start: number, end: number): boolean {
    if (segment.startSeconds == null && segment.endSeconds == null) return false;
    const segmentStart = segment.startSeconds ?? segment.endSeconds ?? 0;
    const segmentEnd = segment.endSeconds ?? segment.startSeconds ?? segmentStart;
    return segmentEnd >= start && segmentStart <= end;
  }
</script>

<article class="podcast-page">
  <header class="podcast-hero">
    <div class="podcast-cover-shell">
      {#if image}
        <img class="podcast-cover" src={image} alt="" />
      {:else}
        <div class="podcast-cover fallback">🎙</div>
      {/if}
    </div>

    <div class="podcast-copy">
      <p class="eyebrow">Podcast Episode</p>
      <h1>{episodeTitle}</h1>
      {#if showTitle}
        <p class="show-title">{showTitle}</p>
      {/if}

      <div class="meta-row">
        {#if durationSeconds}
          <span>{formatPodcastDuration(durationSeconds)}</span>
        {/if}
        {#if publishedAt}
          <span>{formatPodcastReleaseDate(publishedAt)}</span>
        {/if}
        <span>{highlights.length} clipped moment{highlights.length === 1 ? '' : 's'}</span>
        {#if clippedPeopleCount > 0}
          <span>{clippedPeopleCount} people</span>
        {/if}
      </div>

      {#if description}
        <p class="description">{description}</p>
      {/if}

      <div class="actions">
        <a class="primary-link" href={artifact.url} target="_blank" rel="noreferrer">Open source</a>
        <a href={`/community/${community.id}`}>Back to {community.name}</a>
        <button
          type="button"
          class:active={viewMode === 'discussion'}
          onclick={() => (viewMode = viewMode === 'discussion' ? 'listen' : 'discussion')}
        >
          Discussion
        </button>
        <button type="button" class:active={savedForLater} disabled={savingForLater} onclick={onToggleForLater}>
          {savingForLater
            ? 'Updating…'
            : !currentUser
              ? 'Sign in to save'
              : savedForLater
                ? 'Saved to For Later'
                : 'Save to For Later'}
        </button>
      </div>

      {#if forLaterError}
        <p class="feedback error">{forLaterError}</p>
      {/if}

      {#if forLaterMessage}
        <p class="feedback">{forLaterMessage}</p>
      {/if}
    </div>
  </header>

  {#if playbackAvailable}
    <audio
      bind:this={audioEl}
      src={audioUrl}
      preload="metadata"
      ontimeupdate={syncPlaybackState}
      onloadedmetadata={syncPlaybackState}
      onplay={syncPlaybackState}
      onpause={syncPlaybackState}
      onended={syncPlaybackState}
    ></audio>
  {/if}

  {#if viewMode === 'listen'}
  <section class="player-shell">
    {#if clipMode === 'preview'}
      <div class="preview-badge">
        <span>Previewing clip</span>
        <button type="button" class="secondary-button" onclick={stopPreview}>Stop preview</button>
      </div>
    {/if}

    <div class="player-topline">
      <button type="button" class="play-button" disabled={!playbackAvailable} onclick={togglePlayback}>
        {isPlaying ? 'Pause' : 'Play'}
      </button>

      {#if playbackAvailable}
        <div class="mark-buttons">
          <button type="button" class="mark-button" onclick={handleMarkIn} title="Mark clip start (I)">
            Mark In
          </button>
          <button type="button" class="mark-button" onclick={handleMarkOut} title="Mark clip end (O)">
            Mark Out
          </button>
        </div>
      {/if}

      <div class="clock-row">
        <span>{formatPodcastClock(currentTime)}</span>
        <span>{durationSeconds ? formatPodcastClock(durationSeconds) : '0:00'}</span>
      </div>
    </div>

    <div
      class="timeline"
      data-podcast-timeline
      bind:this={timelineEl}
      onpointerdown={handleTimelinePointerDown}
      role="presentation"
    >
      <div class="waveform">
        {#each waveformBars as barHeight, index (`${barHeight}-${index}`)}
          <span class="bar" style={`height:${barHeight}%`}></span>
        {/each}
      </div>

      {#if clipRange}
        <div
          class="clip-selection"
          style={`left:${selectionLeft()};width:${selectionWidth()};`}
        ></div>
      {/if}

      {#each clipMarkerHighlights as highlight (highlight.eventId)}
        <button
          type="button"
          class="clip-marker"
          aria-label={`Jump to ${formatPodcastClock(highlight.clipStartSeconds ?? 0)}`}
          title={`Jump to ${formatPodcastClock(highlight.clipStartSeconds ?? 0)}`}
          style={`left:${markerLeft(highlight.clipStartSeconds ?? 0)};`}
          onclick={() => seekToTime(highlight.clipStartSeconds ?? 0)}
        ></button>
      {/each}

      {#if durationSeconds}
        <div class="playhead" style={`left:${markerLeft(currentTime)};`}></div>
      {/if}
    </div>

    {#if clipMode === 'fine-tune' || clipMode === 'preview'}
      {#if zoomedTimelineRange && clipRange && playbackAvailable}
        <div class="zoomed-timeline-shell">
          <div class="zoomed-timeline-label">
            <span>Clip region ({formatPodcastClock(zoomedTimelineRange.start)} – {formatPodcastClock(zoomedTimelineRange.end)})</span>
            <button type="button" class="link-button" onclick={() => { clipMode = 'listen'; }}>Back to full timeline</button>
          </div>
          <div
            class="zoomed-timeline"
            bind:this={zoomedTimelineEl}
            role="presentation"
          >
            <div
              class="clip-selection zoomed"
              style={`left:${zoomedSelectionLeft()};width:${zoomedSelectionWidth()};`}
            >
              <button
                type="button"
                class="drag-handle start"
                onpointerdown={(e) => handleZoomedDragStart(e, 'start')}
              ></button>
              <button
                type="button"
                class="drag-handle end"
                onpointerdown={(e) => handleZoomedDragStart(e, 'end')}
              ></button>
            </div>
            {#if durationSeconds}
              <div class="playhead" style={`left:${zoomedMarkerLeft(currentTime)};`}></div>
            {/if}
          </div>
        </div>
      {/if}

      {#if hasCompleteClip && clipRange}
        <div class="fine-tune-panel">
          <div class="fine-tune-row">
            <span class="fine-tune-label">Start</span>
            <button type="button" class="nudge-button" onclick={() => nudgeClipStart(-5)}>−5s</button>
            <button type="button" class="nudge-button" onclick={() => nudgeClipStart(-1)}>−1s</button>
            <input
              type="text"
              class="time-input"
              value={formatPodcastClock(clipRange.start)}
              onchange={(e) => handleTimeInputChange('start', e.currentTarget.value)}
            />
            <button type="button" class="nudge-button" onclick={() => nudgeClipStart(1)}>+1s</button>
            <button type="button" class="nudge-button" onclick={() => nudgeClipStart(5)}>+5s</button>
          </div>
          <div class="fine-tune-row">
            <span class="fine-tune-label">End</span>
            <button type="button" class="nudge-button" onclick={() => nudgeClipEnd(-5)}>−5s</button>
            <button type="button" class="nudge-button" onclick={() => nudgeClipEnd(-1)}>−1s</button>
            <input
              type="text"
              class="time-input"
              value={formatPodcastClock(clipRange.end)}
              onchange={(e) => handleTimeInputChange('end', e.currentTarget.value)}
            />
            <button type="button" class="nudge-button" onclick={() => nudgeClipEnd(1)}>+1s</button>
            <button type="button" class="nudge-button" onclick={() => nudgeClipEnd(5)}>+5s</button>
          </div>
          {#if canPreviewClip && clipMode !== 'preview'}
            <button type="button" class="preview-button" onclick={startPreview}>Preview clip</button>
          {/if}
        </div>
      {/if}
    {/if}

    <div class="player-help">
      {#if playbackAvailable && transcriptAvailable}
        <p>Drag across the timeline to define the clip. The transcript below fills the quote automatically.</p>
      {:else if playbackAvailable}
        <p>Drag across the timeline to define the clip. This source does not expose a transcript.</p>
      {:else if transcriptAvailable}
        <p>This source does not expose playable audio here. Select timed transcript lines to create a clip.</p>
      {:else}
        <p>{podcast?.audioRestrictedReason || 'This source does not expose playable audio or a transcript right now.'}</p>
      {/if}
    </div>
  </section>

  <section class="composer-shell">
    <div class="composer-copy">
      <p class="eyebrow">Create Clip</p>
      <h2>Capture the moment worth replaying.</h2>
      {#if clipRange}
        <p class="selection-label">
          Selected {formatPodcastClock(clipRange.start)}–{formatPodcastClock(clipRange.end)}
          ({formatPodcastDuration(clipDurationSeconds)})
        </p>
      {:else if hasPartialClip}
        <p class="selection-label">
          Clip start marked at {formatPodcastClock(clipStart ?? 0)} — mark end to continue
        </p>
      {:else}
        <p class="selection-label">No clip selected yet.</p>
      {/if}
    </div>

    <div class="composer-actions">
      <textarea
        bind:value={note}
        rows="3"
        maxlength="280"
        placeholder="Optional note about why this moment matters."
      ></textarea>

      {#if selectedTranscriptQuote}
        <blockquote class="quote-preview">{selectedTranscriptQuote}</blockquote>
      {/if}

      <div class="composer-buttons">
        <button class="primary-button" type="button" disabled={!canPublishClip} onclick={publishClip}>
          {publishing ? 'Saving…' : 'Save clip'}
        </button>
        <button class="secondary-button" type="button" onclick={clearClip}>Clear selection</button>
      </div>

      {#if publishError}
        <p class="feedback error">{publishError}</p>
      {/if}

      {#if publishStatus}
        <p class="feedback success">{publishStatus}</p>
      {/if}
    </div>
  </section>

  <section class="transcript-shell">
    <div class="section-head">
      <div>
        <p class="eyebrow">Transcript</p>
        <h2>{transcriptAvailable ? 'Follow the episode in text' : 'Transcript unavailable'}</h2>
      </div>
      {#if transcriptUrl}
        <a href={transcriptUrl} target="_blank" rel="noreferrer">Open transcript source</a>
      {/if}
    </div>

    {#if transcriptAvailable}
      <div class="transcript-list">
        {#each transcriptSegments as segment, index (segment.id)}
          <button
            type="button"
            class:active={index === activeTranscriptIndex}
            class:selected={selectedTranscriptIds.includes(segment.id)}
            class="transcript-segment"
            bind:this={transcriptNodes[index]}
            onclick={() => handleSegmentClick(segment, index)}
          >
            <div class="transcript-time">
              {#if segment.startSeconds != null}
                {formatPodcastClock(segment.startSeconds)}
              {:else}
                —
              {/if}
            </div>
            <div class="transcript-copy">
              {#if segment.speaker}
                <span class="speaker">{segment.speaker}</span>
              {/if}
              <span>{segment.text}</span>
            </div>
          </button>
        {/each}
      </div>
    {:else}
      <div class="unavailable-panel">
        <p>
          This episode metadata resolved, but no transcript was exposed by the page, feed, or linked transcript source.
        </p>
      </div>
    {/if}
  </section>

  <section class="clips-shell">
    <div class="section-head">
      <div>
        <p class="eyebrow">Clipped Moments</p>
        <h2>{highlights.length} moment{highlights.length === 1 ? '' : 's'} from this community</h2>
      </div>

      <div class="sort-group">
        <button type="button" class:active={sortMode === 'top'} onclick={() => (sortMode = 'top')}>Most clipped</button>
        <button type="button" class:active={sortMode === 'newest'} onclick={() => (sortMode = 'newest')}>Newest</button>
        <button type="button" class:active={sortMode === 'timeline'} onclick={() => (sortMode = 'timeline')}>Timeline</button>
      </div>
    </div>

    {#if sortedHighlights.length === 0}
      <div class="unavailable-panel">
        <p>No clipped moments shared here yet.</p>
        <p>Pick the first range from the timeline and it will show up here once it is shared.</p>
      </div>
    {:else}
      <div class="highlight-stack">
        {#each sortedHighlights as highlight (highlight.eventId)}
          <HighlightCard {highlight} {artifact} seekTo={seekToTime} groupId={community.id} showDiscussAction />
        {/each}
      </div>
    {/if}
  </section>
  {:else}
  <section class="podcast-discussion">
    <DiscussionPanel groupId={community.id} rootContext={podcastRootContext} />
  </section>
  {/if}
</article>

<style>
  .podcast-page {
    display: grid;
    gap: 1.4rem;
    padding: 2rem 0 3rem;
  }

  .podcast-hero,
  .player-shell,
  .composer-shell,
  .transcript-shell,
  .clips-shell {
    padding: 1.25rem;
    border: 1px solid var(--border);
    border-radius: 1.35rem;
    background:
      radial-gradient(circle at top left, rgba(255, 103, 25, 0.08), transparent 34%),
      var(--surface);
  }

  .podcast-hero {
    display: grid;
    grid-template-columns: minmax(220px, 260px) minmax(0, 1fr);
    gap: 1.3rem;
  }

  .podcast-cover-shell,
  .podcast-cover {
    width: 100%;
    aspect-ratio: 1;
    border-radius: 1.2rem;
  }

  .podcast-cover {
    object-fit: cover;
    background: linear-gradient(155deg, rgba(255, 103, 25, 0.18), rgba(15, 23, 42, 0.08));
  }

  .podcast-cover.fallback {
    display: grid;
    place-items: center;
    font-size: 3rem;
  }

  .podcast-copy {
    display: grid;
    align-content: start;
    gap: 0.8rem;
  }

  .eyebrow {
    margin: 0;
    color: var(--accent);
    font-size: 0.8rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  h1,
  h2 {
    margin: 0;
    color: var(--text-strong);
    font-family: var(--font-serif);
    letter-spacing: -0.02em;
  }

  h1 {
    font-size: clamp(2rem, 4vw, 3rem);
    line-height: 1.04;
  }

  h2 {
    font-size: 1.45rem;
    line-height: 1.12;
  }

  .show-title,
  .description,
  .selection-label,
  .player-help p,
  .feedback,
  .unavailable-panel p {
    margin: 0;
    color: var(--muted);
    line-height: 1.65;
  }

  .show-title {
    color: var(--text-strong);
    font-size: 1rem;
    font-weight: 700;
  }

  .meta-row,
  .actions,
  .clock-row,
  .composer-buttons,
  .sort-group,
  .section-head {
    display: flex;
    gap: 0.55rem;
    align-items: center;
    flex-wrap: wrap;
  }

  .meta-row span,
  .actions a,
  .actions button,
  .section-head a,
  .sort-group button,
  .primary-button,
  .secondary-button {
    display: inline-flex;
    align-items: center;
    min-height: 2rem;
    padding: 0 0.75rem;
    border-radius: 999px;
    background: var(--surface-soft);
    color: var(--text);
    font-size: 0.8rem;
    font-weight: 600;
    text-decoration: none;
    border: 0;
  }

  .actions button,
  .sort-group button,
  .primary-button,
  .secondary-button,
  .play-button,
  .clip-marker,
  .transcript-segment {
    cursor: pointer;
  }

  .primary-link,
  .actions button.active,
  .sort-group button.active,
  .primary-button,
  .play-button {
    background: var(--accent);
    color: white;
  }

  .player-shell,
  .composer-shell {
    display: grid;
    gap: 1rem;
  }

  .player-topline {
    display: flex;
    justify-content: space-between;
    gap: 1rem;
    align-items: center;
    flex-wrap: wrap;
  }

  .play-button {
    min-height: 3rem;
    padding: 0 1.2rem;
    border-radius: 999px;
    border: 0;
    font-weight: 700;
  }

  .play-button:disabled {
    opacity: 0.55;
    cursor: default;
  }

  .clock-row {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 0.82rem;
    font-weight: 600;
  }

  .timeline {
    position: relative;
    display: grid;
    align-items: end;
    min-height: 7.5rem;
    padding: 0.9rem 0.45rem 0.75rem;
    border-radius: 1.1rem;
    background: linear-gradient(180deg, rgba(255, 103, 25, 0.08), rgba(15, 23, 42, 0.04));
    overflow: hidden;
  }

  .waveform {
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: 1fr;
    gap: 0.2rem;
    align-items: end;
    height: 100%;
  }

  .bar {
    border-radius: 999px;
    background: color-mix(in srgb, var(--accent) 38%, white);
    min-height: 16%;
  }

  .clip-selection,
  .playhead,
  .clip-marker {
    position: absolute;
    top: 0;
    bottom: 0;
  }

  .clip-selection {
    background: rgba(255, 103, 25, 0.18);
    border-left: 2px solid var(--accent);
    border-right: 2px solid var(--accent);
    border-radius: 0.8rem;
    pointer-events: none;
  }

  .playhead {
    width: 2px;
    background: color-mix(in srgb, var(--text-strong) 74%, white);
    pointer-events: none;
  }

  .clip-marker {
    width: 0.5rem;
    border: 0;
    transform: translateX(-50%);
    background: linear-gradient(180deg, rgba(255, 103, 25, 0.8), rgba(255, 103, 25, 0));
  }

  .composer-shell {
    grid-template-columns: minmax(0, 0.95fr) minmax(0, 1.05fr);
  }

  .composer-copy,
  .composer-actions {
    display: grid;
    gap: 0.8rem;
    align-content: start;
  }

  textarea {
    width: 100%;
    min-height: 7rem;
    padding: 0.9rem 1rem;
    border: 1px solid var(--border);
    border-radius: 1rem;
    background: white;
    color: var(--text);
    resize: vertical;
  }

  .quote-preview {
    margin: 0;
    padding: 0.9rem 1rem;
    border-left: 2px solid var(--accent);
    border-radius: 0 1rem 1rem 0;
    background: color-mix(in srgb, var(--surface-soft) 78%, white);
    color: var(--text-strong);
    font-family: var(--font-serif);
    line-height: 1.6;
  }

  .secondary-button {
    background: var(--surface-soft);
  }

  .feedback.error {
    color: #b42318;
  }

  .feedback.success {
    color: #0f766e;
  }

  .transcript-shell,
  .clips-shell {
    display: grid;
    gap: 1rem;
  }

  .section-head {
    justify-content: space-between;
    align-items: end;
  }

  .transcript-list,
  .highlight-stack {
    display: grid;
    gap: 0.8rem;
  }

  .transcript-list {
    max-height: 34rem;
    overflow: auto;
    padding-right: 0.35rem;
  }

  .transcript-segment {
    display: grid;
    grid-template-columns: 5.5rem minmax(0, 1fr);
    gap: 0.9rem;
    padding: 0.9rem 1rem;
    border: 1px solid var(--border);
    border-radius: 1rem;
    background: white;
    color: inherit;
    text-align: left;
  }

  .transcript-segment.active {
    border-color: color-mix(in srgb, var(--accent) 50%, var(--border));
    background: color-mix(in srgb, white 85%, rgba(255, 103, 25, 0.12));
  }

  .transcript-segment.selected {
    box-shadow: inset 0 0 0 1px rgba(255, 103, 25, 0.28);
  }

  .transcript-time {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 0.82rem;
    font-weight: 700;
  }

  .transcript-copy {
    display: grid;
    gap: 0.25rem;
    color: var(--text);
    line-height: 1.6;
  }

  .speaker {
    color: var(--text-strong);
    font-size: 0.82rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .podcast-discussion {
    padding: 1.25rem;
    border: 1px solid var(--border);
    border-radius: 1.35rem;
    background:
      radial-gradient(circle at top left, rgba(255, 103, 25, 0.08), transparent 34%),
      var(--surface);
  }

  .unavailable-panel {
    display: grid;
    gap: 0.35rem;
    padding: 1rem 1.05rem;
    border: 1px solid var(--border);
    border-radius: 1rem;
    background: color-mix(in srgb, var(--surface-soft) 78%, white);
  }

  .preview-badge {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.55rem 1rem;
    border-radius: 999px;
    background: color-mix(in srgb, var(--accent) 14%, white);
    color: var(--accent);
    font-size: 0.82rem;
    font-weight: 700;
  }

  .mark-buttons {
    display: flex;
    gap: 0.4rem;
  }

  .mark-button {
    display: inline-flex;
    align-items: center;
    min-height: 2.4rem;
    padding: 0 0.85rem;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--surface-soft);
    color: var(--text);
    font-size: 0.8rem;
    font-weight: 700;
    cursor: pointer;
  }

  .mark-button:hover {
    border-color: var(--accent);
    color: var(--accent);
  }

  .zoomed-timeline-shell {
    display: grid;
    gap: 0.5rem;
  }

  .zoomed-timeline-label {
    display: flex;
    justify-content: space-between;
    align-items: center;
    color: var(--muted);
    font-size: 0.78rem;
    font-weight: 600;
  }

  .link-button {
    padding: 0;
    border: 0;
    background: none;
    color: var(--accent);
    font-size: 0.78rem;
    font-weight: 600;
    cursor: pointer;
    text-decoration: underline;
  }

  .zoomed-timeline {
    position: relative;
    min-height: 4rem;
    border-radius: 0.8rem;
    background: linear-gradient(180deg, rgba(255, 103, 25, 0.12), rgba(15, 23, 42, 0.06));
    overflow: hidden;
  }

  .clip-selection.zoomed {
    pointer-events: auto;
    display: flex;
    align-items: stretch;
    justify-content: space-between;
  }

  .drag-handle {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 12px;
    border: 0;
    padding: 0;
    background: var(--accent);
    cursor: col-resize;
    opacity: 0.7;
  }

  .drag-handle:hover {
    opacity: 1;
  }

  .drag-handle.start {
    left: -2px;
    border-radius: 0.4rem 0 0 0.4rem;
  }

  .drag-handle.end {
    right: -2px;
    border-radius: 0 0.4rem 0.4rem 0;
  }

  .fine-tune-panel {
    display: grid;
    gap: 0.6rem;
  }

  .fine-tune-row {
    display: flex;
    align-items: center;
    gap: 0.35rem;
  }

  .fine-tune-label {
    min-width: 3rem;
    color: var(--muted);
    font-size: 0.78rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }

  .nudge-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: 2rem;
    min-width: 2.8rem;
    padding: 0 0.5rem;
    border: 1px solid var(--border);
    border-radius: 0.55rem;
    background: white;
    color: var(--text);
    font-size: 0.78rem;
    font-weight: 600;
    cursor: pointer;
  }

  .nudge-button:hover {
    border-color: var(--accent);
    color: var(--accent);
  }

  .time-input {
    width: 5.5rem;
    min-height: 2rem;
    padding: 0 0.55rem;
    border: 1px solid var(--border);
    border-radius: 0.55rem;
    background: white;
    color: var(--text);
    font-family: var(--font-mono);
    font-size: 0.82rem;
    font-weight: 600;
    text-align: center;
  }

  .time-input:focus {
    border-color: var(--accent);
    outline: none;
  }

  .preview-button {
    display: inline-flex;
    align-items: center;
    justify-self: start;
    min-height: 2.2rem;
    padding: 0 1rem;
    border: 1px solid var(--accent);
    border-radius: 999px;
    background: white;
    color: var(--accent);
    font-size: 0.82rem;
    font-weight: 700;
    cursor: pointer;
  }

  .preview-button:hover {
    background: color-mix(in srgb, var(--accent) 8%, white);
  }

  @media (max-width: 900px) {
    .podcast-hero,
    .composer-shell {
      grid-template-columns: 1fr;
    }
  }

  @media (max-width: 640px) {
    .podcast-page {
      padding-top: 1.5rem;
    }

    .timeline {
      min-height: 6.5rem;
    }

    .transcript-segment {
      grid-template-columns: 1fr;
    }
  }
</style>
