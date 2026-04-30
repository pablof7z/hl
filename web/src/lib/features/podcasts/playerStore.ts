import { browser } from '$app/environment';

/// Module-level singleton podcast player. Survives client-side
/// navigation because Svelte's module graph is shared across routes
/// — the `<audio>` element is owned by the root layout, and views just
/// dispatch `playEpisode` / `pause` / `resume` / `seek` against this
/// store. Reactivity is handled with Svelte 5's `$state` runes inside
/// the wrapping object.
///
/// Position is autosaved every 5s to localStorage (per episode), and on
/// boot we rehydrate the most recent episode (paused) so the mini
/// player has somewhere to pick up from.

export type PodcastEpisode = {
  /// Stable id used as the localStorage key (the same 12-char hash that
  /// powers `/podcast/<id>` routes — guarantees position scoping is
  /// shared with the canonical URL).
  id: string;
  title: string;
  showTitle?: string;
  imageUrl?: string;
  audioUrl: string;
  durationSeconds?: number | null;
  /// Where the mini-player's "tap title" affordance routes to.
  detailHref?: string;
};

type PersistedEntry = {
  episode: PodcastEpisode;
  position: number;
  updatedAt: number;
};

const LS_KEY = 'highlighter:podcast-player:v1';
const POSITION_AUTOSAVE_MS = 5000;

type PlayerState = {
  episode: PodcastEpisode | null;
  position: number;
  duration: number | null;
  playing: boolean;
  /// `true` from `playEpisode`/`resume` until the actual `<audio>` reports
  /// `play`. Lets the UI show pressed-state without flicker.
  pendingPlay: boolean;
  /// User explicitly closed the mini-player. We keep state but UI hides.
  hidden: boolean;
};

function defaultState(): PlayerState {
  return {
    episode: null,
    position: 0,
    duration: null,
    playing: false,
    pendingPlay: false,
    hidden: false
  };
}

let audioElement: HTMLAudioElement | null = null;
let lastPersistedAt = 0;

const state = $state<PlayerState>(defaultState());

if (browser) {
  rehydrate();
}

function rehydrate(): void {
  try {
    const raw = window.localStorage.getItem(LS_KEY);
    if (!raw) return;
    const parsed = JSON.parse(raw) as { recent?: Record<string, PersistedEntry> };
    const recent = parsed?.recent ?? {};
    const entries = Object.values(recent).filter(isPersistedEntry);
    if (entries.length === 0) return;

    const mostRecent = entries.reduce((best, entry) =>
      entry.updatedAt > best.updatedAt ? entry : best
    );

    state.episode = mostRecent.episode;
    state.position = mostRecent.position;
    state.duration = mostRecent.episode.durationSeconds ?? null;
    state.playing = false;
    state.pendingPlay = false;
    state.hidden = false;
  } catch {
    // Ignore corrupted state — start clean.
  }
}

function isPersistedEntry(value: unknown): value is PersistedEntry {
  if (!value || typeof value !== 'object') return false;
  const entry = value as Record<string, unknown>;
  if (!entry.episode || typeof entry.episode !== 'object') return false;
  if (typeof entry.position !== 'number') return false;
  if (typeof entry.updatedAt !== 'number') return false;
  return true;
}

function persistPosition(force = false): void {
  if (!browser || !state.episode) return;
  const now = Date.now();
  if (!force && now - lastPersistedAt < POSITION_AUTOSAVE_MS) return;
  lastPersistedAt = now;

  try {
    const raw = window.localStorage.getItem(LS_KEY);
    const parsed = raw ? (JSON.parse(raw) as { recent?: Record<string, PersistedEntry> }) : {};
    const recent = parsed.recent ?? {};
    recent[state.episode.id] = {
      episode: state.episode,
      position: state.position,
      updatedAt: now
    };
    // Cap to the 24 most recent episodes — prevents the localStorage
    // entry from growing unbounded over time.
    const trimmed = Object.entries(recent)
      .toSorted(([, a], [, b]) => b.updatedAt - a.updatedAt)
      .slice(0, 24);
    window.localStorage.setItem(LS_KEY, JSON.stringify({ recent: Object.fromEntries(trimmed) }));
  } catch {
    // localStorage may be disabled or full — non-fatal.
  }
}

/// Start (or restart) playback for an episode. If the episode is the
/// same as the currently-loaded one, just resumes. Otherwise we tear
/// down and reload the audio source.
export function playEpisode(episode: PodcastEpisode): void {
  state.hidden = false;
  if (state.episode?.id === episode.id && audioElement?.src === episode.audioUrl) {
    resume();
    return;
  }

  const prior = readPersistedPosition(episode.id);
  state.episode = episode;
  state.position = prior ?? 0;
  state.duration = episode.durationSeconds ?? null;
  state.playing = false;
  state.pendingPlay = true;

  if (audioElement) {
    applySrcAndPlay();
  }
}

function readPersistedPosition(episodeId: string): number | null {
  if (!browser) return null;
  try {
    const raw = window.localStorage.getItem(LS_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as { recent?: Record<string, PersistedEntry> };
    const entry = parsed.recent?.[episodeId];
    return typeof entry?.position === 'number' ? entry.position : null;
  } catch {
    return null;
  }
}

export function pause(): void {
  state.pendingPlay = false;
  if (audioElement && !audioElement.paused) {
    audioElement.pause();
  }
  state.playing = false;
  persistPosition(true);
}

export function resume(): void {
  if (!state.episode) return;
  state.hidden = false;
  state.pendingPlay = true;
  if (audioElement) {
    if (audioElement.src !== state.episode.audioUrl) {
      applySrcAndPlay();
    } else {
      void audioElement.play().catch(() => {
        state.pendingPlay = false;
      });
    }
  }
}

export function seek(seconds: number): void {
  state.position = Math.max(0, seconds);
  if (audioElement) {
    audioElement.currentTime = state.position;
  }
  persistPosition(true);
}

export function close(): void {
  pause();
  state.hidden = true;
}

export function show(): void {
  state.hidden = false;
}

/// Called by the root layout's hidden `<audio>` once it's mounted so the
/// store can drive playback directly.
export function attachAudioElement(el: HTMLAudioElement | null): void {
  audioElement = el;
  if (!el) return;
  if (state.episode && state.position > 0) {
    // Restore position on reattach. Audio metadata may not be ready
    // yet so guard against that — `seekTo` runs on `loadedmetadata`.
    if (el.readyState >= 1) {
      el.currentTime = state.position;
    }
  }
  if (state.episode && state.pendingPlay) {
    applySrcAndPlay();
  }
}

function applySrcAndPlay(): void {
  if (!audioElement || !state.episode) return;
  const desiredSrc = state.episode.audioUrl;
  if (audioElement.src !== desiredSrc) {
    audioElement.src = desiredSrc;
  }
  // Wait for metadata before seeking when restoring position.
  const playWhenReady = () => {
    if (!audioElement || !state.episode) return;
    if (state.position > 0) {
      try {
        audioElement.currentTime = state.position;
      } catch {
        // Some browsers throw if duration unknown — ignore and let it default to 0.
      }
    }
    audioElement.play().catch(() => {
      state.pendingPlay = false;
    });
  };
  if (audioElement.readyState >= 1) {
    playWhenReady();
  } else {
    audioElement.addEventListener('loadedmetadata', playWhenReady, { once: true });
  }
}

/// Wired up by the root layout's audio element bindings.
export function reportTimeUpdate(): void {
  if (!audioElement) return;
  state.position = audioElement.currentTime;
  if (Number.isFinite(audioElement.duration) && audioElement.duration > 0) {
    state.duration = audioElement.duration;
  }
  persistPosition();
}
export function reportPlay(): void {
  state.playing = true;
  state.pendingPlay = false;
}
export function reportPause(): void {
  state.playing = false;
}
export function reportEnded(): void {
  state.playing = false;
  state.pendingPlay = false;
  state.position = 0;
  persistPosition(true);
}
export function reportLoadedMetadata(): void {
  if (!audioElement) return;
  if (Number.isFinite(audioElement.duration) && audioElement.duration > 0) {
    state.duration = audioElement.duration;
  }
}

/// Runtime snapshot accessor for components that don't want to expose
/// the underlying `$state` directly. Reading individual fields off this
/// is reactive since the underlying state is reactive.
export const podcastPlayer = {
  snapshot(): PlayerState {
    return state;
  },
  /// Convenience: the resolved audio element (for binding currentTime
  /// from outside, if a feature wants a stronger handle). Most callers
  /// should use the helper functions above.
  audioElement: () => audioElement
};
