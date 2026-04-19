<script lang="ts">
  let {
    duration,
    currentTime = '0:00',
    highlightSpans = []
  }: {
    duration: string;
    currentTime?: string;
    highlightSpans?: Array<{ start: string; end: string; colorIndex: number; memberName: string }>;
  } = $props();

  let playing = $state(false);
  let scrubPercent = $state(0);
  let hoveredSpan = $state<string | null>(null);

  const TINT_VARS = [
    'var(--h-amber)',
    'var(--h-sage)',
    'var(--h-blue)',
    'var(--h-rose)',
    'var(--h-lilac)',
    'var(--h-amber-l)'
  ] as const;

  const WAVEFORM_HEIGHTS = [60, 85, 72, 100, 68, 90, 75, 95];

  function parseDuration(str: string): number {
    const parts = str.split(':').map(Number);
    if (parts.length === 3) return parts[0] * 3600 + parts[1] * 60 + parts[2];
    if (parts.length === 2) return parts[0] * 60 + parts[1];
    return 0;
  }

  const totalSeconds = $derived(parseDuration(duration));

  function spanToPercent(timeStr: string): number {
    const secs = parseDuration(timeStr);
    return totalSeconds > 0 ? (secs / totalSeconds) * 100 : 0;
  }

  function getMemberColor(colorIndex: number): string {
    return TINT_VARS[((colorIndex - 1) % 6 + 6) % 6];
  }

  function togglePlay() {
    playing = !playing;
    console.log('podcast player:', playing ? 'play' : 'pause');
  }

  function handleScrubClick(e: MouseEvent) {
    const target = e.currentTarget as HTMLElement;
    const rect = target.getBoundingClientRect();
    scrubPercent = ((e.clientX - rect.left) / rect.width) * 100;
    console.log('seek to:', scrubPercent.toFixed(1) + '%');
  }

  function handleVolumeChange(e: Event) {
    const val = (e.target as HTMLInputElement).value;
    console.log('volume:', val);
  }
</script>

<div class="podcast-player">
  <!-- Waveform visualisation -->
  <div class="waveform" aria-hidden="true">
    {#each WAVEFORM_HEIGHTS as h, i (i)}
      <div
        class="waveform-bar"
        class:playing
        style:height="{h}%"
        style:animation-delay="{i * 80}ms"
      ></div>
    {/each}
  </div>

  <!-- Controls row -->
  <div class="controls-row">
    <button
      class="play-btn"
      type="button"
      onclick={togglePlay}
      aria-label={playing ? 'Pause' : 'Play'}
    >
      {#if playing}
        <span class="play-icon" aria-hidden="true">⏸</span>
      {:else}
        <span class="play-icon" aria-hidden="true">▶</span>
      {/if}
    </button>

    <div class="time-scrub">
      <span class="time-display">{currentTime} / {duration}</span>

      <!-- Scrubber track -->
      <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
      <div
        class="scrubber-track"
        onclick={handleScrubClick}
        role="slider"
        aria-label="Seek position"
        aria-valuenow={scrubPercent}
        aria-valuemin={0}
        aria-valuemax={100}
        tabindex="0"
      >
        <div class="scrubber-fill" style:width="{scrubPercent}%"></div>

        <!-- Highlight spans -->
        {#each highlightSpans as span, i (i)}
          {@const startPct = spanToPercent(span.start)}
          {@const endPct = spanToPercent(span.end)}
          {@const widthPct = Math.max(endPct - startPct, 1.5)}
          {@const color = getMemberColor(span.colorIndex)}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="highlight-span"
            style:left="{startPct}%"
            style:width="{widthPct}%"
            style:background={color}
            onmouseenter={() => (hoveredSpan = `${span.memberName} highlighted at ${span.start}`)}
            onmouseleave={() => (hoveredSpan = null)}
          ></div>
        {/each}

        {#if hoveredSpan}
          <div class="span-tooltip">{hoveredSpan}</div>
        {/if}
      </div>
    </div>

    <!-- Volume -->
    <div class="volume-control">
      <span class="volume-icon" aria-hidden="true">🔊</span>
      <input
        type="range"
        min="0"
        max="100"
        value="80"
        class="volume-slider"
        aria-label="Volume"
        oninput={handleVolumeChange}
      />
    </div>
  </div>
</div>

<style>
  .podcast-player {
    display: flex;
    flex-direction: column;
    gap: 12px;
    background-color: var(--surface);
    border: 1px solid var(--rule);
    border-radius: var(--radius);
    padding: 20px 24px;
  }

  .waveform {
    display: flex;
    align-items: flex-end;
    gap: 4px;
    height: 48px;
  }

  .waveform-bar {
    width: 4px;
    background: linear-gradient(to top, var(--brand-accent), var(--h-amber));
    border-radius: 2px;
    opacity: 0.7;
    transition: opacity 200ms ease-out;
    flex-shrink: 0;
  }

  .waveform-bar.playing {
    animation: pulse 0.8s ease-in-out infinite alternate;
    opacity: 1;
  }

  @keyframes pulse {
    from { opacity: 0.5; transform: scaleY(0.7); }
    to   { opacity: 1;   transform: scaleY(1); }
  }

  .controls-row {
    display: flex;
    align-items: center;
    gap: 16px;
  }

  .play-btn {
    width: 56px;
    height: 56px;
    border-radius: 50%;
    background-color: var(--brand-accent);
    border: none;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    transition: opacity var(--transition);
  }

  .play-btn:hover {
    opacity: 0.85;
  }

  .play-icon {
    font-size: 20px;
    color: var(--surface);
    line-height: 1;
  }

  .time-scrub {
    display: flex;
    flex-direction: column;
    gap: 6px;
    flex: 1;
    min-width: 0;
  }

  .time-display {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--ink-soft);
  }

  .scrubber-track {
    position: relative;
    height: 6px;
    background-color: var(--surface-muted);
    border-radius: 3px;
    cursor: pointer;
    overflow: visible;
  }

  .scrubber-fill {
    position: absolute;
    left: 0;
    top: 0;
    height: 100%;
    background-color: var(--brand-accent);
    border-radius: 3px;
    pointer-events: none;
  }

  .highlight-span {
    position: absolute;
    top: -1px;
    height: 8px;
    border-radius: 2px;
    opacity: 0.6;
    cursor: pointer;
    z-index: 1;
  }

  .highlight-span:hover {
    opacity: 1;
  }

  .span-tooltip {
    position: absolute;
    bottom: 14px;
    left: 50%;
    transform: translateX(-50%);
    background-color: var(--surface);
    border: 1px solid var(--brand-accent);
    border-radius: var(--radius);
    padding: 4px 8px;
    font-family: var(--font-serif);
    font-style: italic;
    font-size: 12px;
    color: var(--ink);
    white-space: nowrap;
    pointer-events: none;
    z-index: 10;
  }

  .volume-control {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }

  .volume-icon {
    font-size: 14px;
  }

  .volume-slider {
    width: 80px;
    accent-color: var(--brand-accent);
  }
</style>
