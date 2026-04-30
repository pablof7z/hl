/// Server-side helpers for the public `/podcast/<id>` SEO route.
///
/// Podcast highlights (kind:9802) tag their target with NIP-73 `i` values:
///   - `podcast:item:guid:<guid>` — preferred, episode-level GUID
///   - `podcast:guid:<guid>` — show-level GUID, used when no item GUID
///
/// We index the public route under a stable short id derived from the
/// guid so URLs stay both URL-safe and SEO-friendly. Format:
///   /podcast/<title-slug>-<id>
/// where `<id>` is the first 12 hex chars of `sha256(guid-prefix:guid)`.
/// 12 hex chars = 48 bits of entropy — enough to avoid collisions for the
/// foreseeable scale of podcast indexing on this app.

import { createHash } from 'node:crypto';
import type { NDKEvent, NDKFilter } from '@nostr-dev-kit/ndk';
import { fetchEventsForSsr } from '$lib/server/nostr';

export type PodcastIdComponents = {
  /// 12-char hex id used in public URLs.
  id: string;
  /// The full `i` tag value (`podcast:item:guid:…` / `podcast:guid:…`).
  iTagValue: string;
  /// Just the guid portion (sans `podcast:[item:]guid:` prefix).
  guid: string;
  /// Whether this is an episode-level (`item:guid`) or show-level (`guid`) id.
  scope: 'episode' | 'show';
};

const PODCAST_ITEM_PREFIX = 'podcast:item:guid:';
const PODCAST_PREFIX = 'podcast:guid:';

/// Hash a NIP-73 podcast `i` tag value to its public-URL id. The full
/// tag value (including prefix) is hashed so episode and show ids never
/// collide for the same underlying string.
export function podcastIdFromITag(iTagValue: string): string {
  const trimmed = iTagValue.trim();
  if (!trimmed) return '';
  return createHash('sha256').update(trimmed).digest('hex').slice(0, 12);
}

/// Convert a raw podcast `i` tag value into its components.
export function parsePodcastITag(iTagValue: string): PodcastIdComponents | undefined {
  const trimmed = iTagValue.trim();
  if (!trimmed) return undefined;

  if (trimmed.toLowerCase().startsWith(PODCAST_ITEM_PREFIX)) {
    const guid = trimmed.slice(PODCAST_ITEM_PREFIX.length);
    if (!guid) return undefined;
    return {
      id: podcastIdFromITag(trimmed),
      iTagValue: trimmed,
      guid,
      scope: 'episode'
    };
  }

  if (trimmed.toLowerCase().startsWith(PODCAST_PREFIX)) {
    const guid = trimmed.slice(PODCAST_PREFIX.length);
    if (!guid) return undefined;
    return {
      id: podcastIdFromITag(trimmed),
      iTagValue: trimmed,
      guid,
      scope: 'show'
    };
  }

  return undefined;
}

export type PodcastDetail = {
  components: PodcastIdComponents;
  /// Highlights that target this podcast/episode (kind:9802).
  highlights: NDKEvent[];
  /// Best-effort resolved metadata pulled from the highlights themselves.
  episodeTitle?: string;
  showTitle?: string;
  imageUrl?: string;
  audioUrl?: string;
  durationSeconds?: number;
  /// Inverted index: which `i` tag value (and thus which podcast id) the
  /// highlight set actually represents. Useful when the route hits the
  /// loader with a bare 12-char id and we need to fan out to all known
  /// podcast `i` tag values to find a match.
  resolvedITagValue?: string;
};

/// Look up podcast detail by its 12-char id. Strategy:
///
///   1. Sample recent kind:9802 highlights (limit ~400) carrying a
///      `i podcast:[item:]guid:…` tag — relays don't index by hash, so we
///      have to scan.
///   2. Find highlights whose `i` tag hashes to the requested id.
///   3. Pick the most-frequent `i` tag in the matching set as the
///      canonical `iTagValue` and re-fetch highlights filtered by it for
///      a complete view.
///
/// This is best-effort: when no matching highlights surface within the
/// sampling window, the caller should render an empty/404 state.
export async function fetchPodcastDetailById(id: string): Promise<PodcastDetail | undefined> {
  const trimmedId = id.trim().toLowerCase();
  if (!/^[0-9a-f]{12}$/.test(trimmedId)) return undefined;

  // Pull a generous slice of recent podcast highlights. The `#i` filter
  // doesn't accept prefixes, so we widen the net by kind alone and post-
  // filter on the tag value below.
  const sampleEvents = Array.from(
    (await fetchEventsForSsr(
      [{ kinds: [9802], limit: 400 }],
      `fetchPodcastDetail:sample(${trimmedId})`
    )) ?? []
  );

  // Group sampled highlights by their `i` tag value (when it's a podcast
  // tag) to find the canonical guid behind the requested hash id.
  const matchingITags = new Map<string, NDKEvent[]>();
  for (const event of sampleEvents) {
    for (const tag of event.getMatchingTags('i')) {
      const value = (tag[1] ?? '').trim();
      const components = parsePodcastITag(value);
      if (!components) continue;
      if (components.id !== trimmedId) continue;

      const existing = matchingITags.get(components.iTagValue) ?? [];
      existing.push(event);
      matchingITags.set(components.iTagValue, existing);
    }
  }

  if (matchingITags.size === 0) return undefined;

  // Largest bucket wins as the canonical `i` tag value.
  const [resolvedITagValue, sampledHighlights] = [...matchingITags.entries()].toSorted(
    ([, a], [, b]) => b.length - a.length
  )[0];

  const components = parsePodcastITag(resolvedITagValue);
  if (!components) return undefined;

  // Fetch a focused window of highlights for the resolved tag value to
  // surface ones that may have fallen outside the original sample.
  const focusedFilter: NDKFilter = {
    kinds: [9802],
    '#i': [resolvedITagValue],
    limit: 200
  } as NDKFilter;
  const focused = Array.from(
    (await fetchEventsForSsr([focusedFilter], `fetchPodcastDetail:focused(${resolvedITagValue})`)) ?? []
  );

  // Merge + dedupe by event id.
  const dedup = new Map<string, NDKEvent>();
  for (const event of [...focused, ...sampledHighlights]) {
    if (event.id) dedup.set(event.id, event);
  }
  const highlights = [...dedup.values()].sort(
    (left, right) => (right.created_at ?? 0) - (left.created_at ?? 0)
  );

  // Best-effort metadata derivation from the highlights themselves —
  // `r` (audio URL), `title` (episode title), `image` (cover), `start`
  // (max heard offset) when present. These are NOT reliable across
  // clients, so we keep them as decorative fallbacks; downstream UI
  // shows a neutral hero when nothing is found.
  let episodeTitle: string | undefined;
  let showTitle: string | undefined;
  let imageUrl: string | undefined;
  let audioUrl: string | undefined;
  let durationSeconds: number | undefined;

  for (const event of highlights) {
    if (!episodeTitle) {
      const candidate = event.tagValue('title') || event.tagValue('episode_title');
      if (candidate?.trim()) episodeTitle = candidate.trim();
    }
    if (!showTitle) {
      const candidate = event.tagValue('show_title') || event.tagValue('podcast');
      if (candidate?.trim()) showTitle = candidate.trim();
    }
    if (!imageUrl) {
      const candidate = event.tagValue('image') || event.tagValue('thumb');
      if (candidate?.trim()) imageUrl = candidate.trim();
    }
    if (!audioUrl) {
      const candidate = event.tagValue('r') || event.tagValue('url');
      if (candidate?.trim()) audioUrl = candidate.trim();
    }
    if (!durationSeconds) {
      const candidate = Number(event.tagValue('duration'));
      if (Number.isFinite(candidate) && candidate > 0) durationSeconds = candidate;
    }

    if (episodeTitle && showTitle && imageUrl && audioUrl && durationSeconds) break;
  }

  return {
    components,
    highlights,
    episodeTitle,
    showTitle,
    imageUrl,
    audioUrl,
    durationSeconds,
    resolvedITagValue
  };
}

/// Public reverse: when the highlight resolver wants to compute the
/// `/podcast/<slug>-<id>` href for a highlight whose `i` tag is a
/// podcast tag, share the same hash function.
export function podcastIdFromGuid(scope: 'episode' | 'show', guid: string): string {
  const tagValue = scope === 'episode' ? `${PODCAST_ITEM_PREFIX}${guid}` : `${PODCAST_PREFIX}${guid}`;
  return podcastIdFromITag(tagValue);
}
