import NDK, {
  NDKEvent,
  NDKHighlight,
  NDKKind,
  NDKRelaySet,
  getRelayListForUsers,
  type NDKFilter,
  type NDKEvent as NDKEventType,
  type NDKKind as NDKKindType
} from '@nostr-dev-kit/ndk';
import type { ArtifactRecord } from '$lib/ndk/artifacts';
import { artifactHighlightReferenceKey } from '$lib/ndk/artifacts';
import { DEFAULT_RELAYS, HIGHLIGHTER_RELAY_URL } from '$lib/ndk/config';
import { buildRoomRelaySet } from '$lib/ndk/groups';

export const HIGHLIGHTER_HIGHLIGHT_KIND = NDKKind.Highlight as NDKKindType;
export const HIGHLIGHTER_HIGHLIGHT_REPOST_KIND = NDKKind.GenericRepost as NDKKindType;

export type HighlightRecord = {
  eventId: string;
  pubkey: string;
  quote: string;
  context: string;
  note: string;
  artifactAddress: string;
  eventReference: string;
  sourceUrl: string;
  sourceReferenceKey: string;
  clipStartSeconds: number | null;
  clipEndSeconds: number | null;
  clipSpeaker: string;
  clipTranscriptSegmentIds: string[];
  createdAt: number | null;
};

export type HighlightShareRecord = {
  repostEventId: string;
  highlightEventId: string;
  highlightAuthorPubkey: string;
  groupId: string;
  relayHint: string;
  pubkey: string;
  createdAt: number | null;
};

export type HydratedHighlight = HighlightRecord & {
  shares: HighlightShareRecord[];
  shareCount: number;
  latestSharedAt: number | null;
};

export function highlightPath(groupId: string, highlightId: string): string {
  return `/r/${encodeURIComponent(groupId)}/e/${encodeURIComponent(highlightId)}`;
}

export function highlightFromEvent(event: NDKEventType): HighlightRecord {
  const highlight = NDKHighlight.from(event);
  const artifactAddress = cleanText(highlight.tagValue('a'));
  const eventReference = cleanText(highlight.tagValue('e'));
  const sourceUrl = cleanText(highlight.url);

  return {
    eventId: highlight.id,
    pubkey: highlight.pubkey,
    quote: cleanText(highlight.content),
    context: cleanText(highlight.context),
    note: cleanText(highlight.tagValue('comment')),
    artifactAddress,
    eventReference,
    sourceUrl,
    sourceReferenceKey: highlightReferenceKey({
      artifactAddress,
      eventReference,
      sourceUrl
    }),
    clipStartSeconds: numericTagValue(highlight, 'start'),
    clipEndSeconds: numericTagValue(highlight, 'end'),
    clipSpeaker: cleanText(highlight.tagValue('speaker')),
    clipTranscriptSegmentIds: uniqueValues(highlight.getMatchingTags('segment').map((tag) => tag[1] ?? '')),
    createdAt: highlight.created_at ?? null
  };
}

export function highlightReferenceKey(input: {
  artifactAddress?: string;
  eventReference?: string;
  sourceUrl?: string;
}): string {
  const artifactAddress = cleanText(input.artifactAddress);
  if (artifactAddress) return `a:${artifactAddress}`;

  const eventReference = cleanText(input.eventReference);
  if (eventReference) return `e:${eventReference}`;

  const sourceUrl = cleanText(input.sourceUrl);
  if (sourceUrl) return `r:${sourceUrl}`;

  return '';
}

export function buildArtifactHighlightFilters(
  artifacts: Array<Pick<ArtifactRecord, 'highlightTagName' | 'highlightTagValue'>>,
  authors: string[],
  limit = 160
): NDKFilter[] {
  const normalizedAuthors = uniqueValues(authors);
  if (normalizedAuthors.length === 0 || artifacts.length === 0) {
    return [];
  }

  const aValues = uniqueValues(
    artifacts
      .filter((artifact) => artifact.highlightTagName === 'a')
      .map((artifact) => artifact.highlightTagValue)
  );
  const eValues = uniqueValues(
    artifacts
      .filter((artifact) => artifact.highlightTagName === 'e')
      .map((artifact) => artifact.highlightTagValue)
  );
  const rValues = uniqueValues(
    artifacts
      .filter((artifact) => artifact.highlightTagName === 'r')
      .map((artifact) => artifact.highlightTagValue)
  );
  const filters: NDKFilter[] = [];

  if (aValues.length > 0) {
    filters.push({
      kinds: [HIGHLIGHTER_HIGHLIGHT_KIND],
      authors: normalizedAuthors,
      '#a': aValues,
      limit
    } as NDKFilter);
  }

  if (eValues.length > 0) {
    filters.push({
      kinds: [HIGHLIGHTER_HIGHLIGHT_KIND],
      authors: normalizedAuthors,
      '#e': eValues,
      limit
    } as NDKFilter);
  }

  if (rValues.length > 0) {
    filters.push({
      kinds: [HIGHLIGHTER_HIGHLIGHT_KIND],
      authors: normalizedAuthors,
      '#r': rValues,
      limit
    } as NDKFilter);
  }

  return filters;
}

export function highlightShareFromEvent(event: NDKEventType): HighlightShareRecord | undefined {
  const eTag = event.getMatchingTags('e')[0];
  const highlightEventId = cleanText(eTag?.[1]);
  const groupId = cleanText(event.tagValue('h'));

  if (!highlightEventId || !groupId) {
    return undefined;
  }

  return {
    repostEventId: event.id,
    highlightEventId,
    highlightAuthorPubkey: cleanText(event.tagValue('p')),
    groupId,
    relayHint: cleanText(eTag?.[2]),
    pubkey: event.pubkey,
    createdAt: event.created_at ?? null
  };
}

export function extractHighlightIdsFromShares(shareEvents: NDKEventType[]): string[] {
  return uniqueValues(
    shareEvents
      .map((event) => highlightShareFromEvent(event)?.highlightEventId ?? '')
      .filter(Boolean)
  );
}

export function hydrateHighlights(
  highlightEvents: NDKEventType[],
  shareEvents: NDKEventType[]
): HydratedHighlight[] {
  const sharesByHighlightId = new Map<string, HighlightShareRecord[]>();

  for (const event of shareEvents) {
    const share = highlightShareFromEvent(event);
    if (!share) continue;

    const shares = sharesByHighlightId.get(share.highlightEventId) ?? [];
    shares.push(share);
    sharesByHighlightId.set(share.highlightEventId, shares);
  }

  return highlightEvents
    .map((event) => {
      const highlight = highlightFromEvent(event);
      const shares = (sharesByHighlightId.get(highlight.eventId) ?? []).toSorted(
        (left, right) => (right.createdAt ?? 0) - (left.createdAt ?? 0)
      );

      return {
        ...highlight,
        shares,
        shareCount: shares.length,
        latestSharedAt: shares[0]?.createdAt ?? null
      };
    })
    .toSorted(
      (left, right) =>
        (right.latestSharedAt ?? right.createdAt ?? 0) - (left.latestSharedAt ?? left.createdAt ?? 0)
    );
}

export function hydrateStandaloneHighlights(highlightEvents: NDKEventType[]): HydratedHighlight[] {
  return highlightEvents
    .map((event) => {
      const highlight = highlightFromEvent(event);

      return {
        ...highlight,
        shares: [],
        shareCount: 0,
        latestSharedAt: null
      };
    })
    .toSorted((left, right) => (right.createdAt ?? 0) - (left.createdAt ?? 0));
}

export async function fetchHighlightsForShares(
  ndk: NDK,
  shareEvents: NDKEventType[]
): Promise<HydratedHighlight[]> {
  const highlightEvents = await fetchHighlightEventsForShares(ndk, shareEvents);
  return hydrateHighlights(highlightEvents, shareEvents);
}

export async function fetchHighlightEventsForShares(
  ndk: NDK,
  shareEvents: NDKEventType[]
): Promise<NDKEventType[]> {
  const shares = shareEvents
    .map((event) => highlightShareFromEvent(event))
    .filter((share): share is HighlightShareRecord => Boolean(share));

  const highlightIds = uniqueValues(shares.map((share) => share.highlightEventId));
  if (highlightIds.length === 0) {
    return [];
  }

  const relayUrls = await resolveHighlightFetchRelayUrls(ndk, shares);
  const relaySet = NDKRelaySet.fromRelayUrls(
    relayUrls,
    ndk
  );
  return Array.from(
    (await ndk.fetchEvents(
      {
        kinds: [HIGHLIGHTER_HIGHLIGHT_KIND],
        ids: highlightIds,
        limit: highlightIds.length
      },
      { closeOnEose: true },
      relaySet
    )) ?? []
  );
}

export function highlightCountsByArtifact(
  highlights: HydratedHighlight[] | HighlightRecord[]
): Map<string, number> {
  const counts = new Map<string, number>();

  for (const highlight of highlights) {
    if (!highlight.sourceReferenceKey) continue;
    counts.set(highlight.sourceReferenceKey, (counts.get(highlight.sourceReferenceKey) ?? 0) + 1);
  }

  return counts;
}

export async function publishAndShareHighlight(
  ndk: NDK,
  input: {
    groupId: string;
    artifact: ArtifactRecord;
    quote?: string;
    context?: string;
    note?: string;
    clip?: {
      startTime: number;
      endTime: number;
      speaker?: string;
      transcriptSegmentIds?: string[];
    };
  }
): Promise<{
  highlight: HighlightRecord;
  share: HighlightShareRecord;
  shareExisting: boolean;
}> {
  const highlight = await publishCanonicalHighlight(ndk, input);
  const shared = await shareHighlightToRoom(ndk, {
    groupId: input.groupId,
    highlight
  });

  return {
    highlight,
    share: shared.share,
    shareExisting: shared.existing
  };
}

export async function shareHighlightToRoom(
  ndk: NDK,
  input: {
    groupId: string;
    highlight: Pick<HighlightRecord, 'eventId' | 'pubkey'>;
  }
): Promise<{ share: HighlightShareRecord; existing: boolean }> {
  if (!ndk.signer) {
    throw new Error('Connect a signer before sharing highlights.');
  }

  const existing = await findExistingHighlightShare(ndk, input.groupId, input.highlight.eventId);
  if (existing) {
    return { share: existing, existing: true };
  }

  const relaySet = buildRoomRelaySet(ndk);
  const event = new NDKEvent(ndk);
  event.kind = HIGHLIGHTER_HIGHLIGHT_REPOST_KIND;
  event.content = '';
  event.tags = [
    ['e', input.highlight.eventId, HIGHLIGHTER_RELAY_URL],
    ['k', String(HIGHLIGHTER_HIGHLIGHT_KIND)],
    ['p', input.highlight.pubkey],
    ['h', input.groupId]
  ];

  await event.sign();
  await event.publish(relaySet);

  const share = highlightShareFromEvent(event);
  if (!share) {
    throw new Error('Highlight share was published without the expected repost tags.');
  }

  return { share, existing: false };
}

async function publishCanonicalHighlight(
  ndk: NDK,
  input: {
    artifact: ArtifactRecord;
    quote?: string;
    context?: string;
    note?: string;
    clip?: {
      startTime: number;
      endTime: number;
      speaker?: string;
      transcriptSegmentIds?: string[];
    };
  }
): Promise<HighlightRecord> {
  if (!ndk.signer) {
    throw new Error('Connect a signer before creating highlights.');
  }

  const quote = cleanText(input.quote);
  const clip = normalizeClip(input.clip);
  if (!quote && !clip) {
    throw new Error('Enter the highlighted text first.');
  }

  const currentUser = ndk.activeUser ?? (await ndk.signer.user());
  const relaySet = await buildUserHighlightRelaySet(ndk, currentUser.pubkey);
  const event = new NDKHighlight(ndk);

  event.content = quote || buildClipFallbackQuote(clip);
  event.tags = [];

  if (input.artifact.highlightTagName === 'r' && input.artifact.url) {
    event.article = input.artifact.url;
  } else {
    event.tags = [[input.artifact.highlightTagName, input.artifact.highlightTagValue]];
  }

  const context = cleanText(input.context);
  event.context = context && context !== event.content ? context : undefined;

  const note = cleanText(input.note);
  if (note) {
    event.removeTag('comment');
    event.tags.push(['comment', note]);
  }

  if (clip) {
    event.tags.push(['start', clip.startTime.toFixed(3)]);
    event.tags.push(['end', clip.endTime.toFixed(3)]);

    if (clip.speaker) {
      event.tags.push(['speaker', clip.speaker]);
    }

    for (const segmentId of clip.transcriptSegmentIds) {
      event.tags.push(['segment', segmentId]);
    }
  }

  await event.sign();
  await event.publish(relaySet);

  return highlightFromEvent(event);
}

async function findExistingHighlightShare(
  ndk: NDK,
  groupId: string,
  highlightEventId: string
): Promise<HighlightShareRecord | undefined> {
  const relaySet = buildRoomRelaySet(ndk);
  const events = Array.from(
    (await ndk.fetchEvents(
      {
        kinds: [HIGHLIGHTER_HIGHLIGHT_REPOST_KIND],
        '#h': [groupId],
        '#e': [highlightEventId],
        limit: 10
      },
      { closeOnEose: true },
      relaySet
    )) ?? []
  ).sort((left, right) => (right.created_at ?? 0) - (left.created_at ?? 0));

  const existing = events[0];
  return existing ? highlightShareFromEvent(existing) : undefined;
}

export async function resolveUserHighlightRelayUrls(ndk: NDK, pubkey: string): Promise<string[]> {
  const relayLists = await getRelayListForUsers([pubkey], ndk, false, 1500);
  const relayList = relayLists.get(pubkey);
  const userRelayUrls = relayList?.writeRelayUrls.length
    ? relayList.writeRelayUrls
    : (relayList?.relays ?? []);

  return uniqueValues(
    userRelayUrls.length > 0
      ? [...userRelayUrls, HIGHLIGHTER_RELAY_URL]
      : [HIGHLIGHTER_RELAY_URL, ...DEFAULT_RELAYS]
  );
}

async function buildUserHighlightRelaySet(ndk: NDK, pubkey: string): Promise<NDKRelaySet> {
  return NDKRelaySet.fromRelayUrls(await resolveUserHighlightRelayUrls(ndk, pubkey), ndk);
}

async function resolveHighlightFetchRelayUrls(
  ndk: NDK,
  shares: HighlightShareRecord[]
): Promise<string[]> {
  const authorPubkeys = uniqueValues(shares.map((share) => share.highlightAuthorPubkey).filter(Boolean));
  const relayLists = authorPubkeys.length > 0 ? await getRelayListForUsers(authorPubkeys, ndk, false, 1500) : new Map();
  const authorRelayUrls = authorPubkeys.flatMap((pubkey) => {
    const relayList = relayLists.get(pubkey);
    return relayList?.writeRelayUrls.length
      ? relayList.writeRelayUrls
      : (relayList?.relays ?? []);
  });

  return uniqueValues([
    HIGHLIGHTER_RELAY_URL,
    ...DEFAULT_RELAYS,
    ...shares.map((share) => share.relayHint).filter(Boolean),
    ...authorRelayUrls
  ]);
}

export function artifactReferenceKey(artifact: Pick<ArtifactRecord, 'highlightTagName' | 'highlightTagValue'>): string {
  return artifactHighlightReferenceKey(artifact);
}

function cleanText(value: string | undefined): string {
  return value?.trim() ?? '';
}

function uniqueValues(values: string[]): string[] {
  return [...new Set(values.map((value) => value.trim()).filter(Boolean))];
}

function numericTagValue(event: Pick<NDKEventType, 'tagValue'>, tagName: string): number | null {
  const rawValue = cleanText(event.tagValue(tagName));
  if (!rawValue) return null;

  const parsed = Number(rawValue);
  return Number.isFinite(parsed) && parsed >= 0 ? parsed : null;
}

function normalizeClip(
  clip:
    | {
        startTime: number;
        endTime: number;
        speaker?: string;
        transcriptSegmentIds?: string[];
      }
    | undefined
): {
  startTime: number;
  endTime: number;
  speaker: string;
  transcriptSegmentIds: string[];
} | null {
  if (!clip) return null;

  const startTime = Number(clip.startTime);
  const endTime = Number(clip.endTime);
  if (!Number.isFinite(startTime) || !Number.isFinite(endTime)) {
    return null;
  }

  const normalizedStart = Math.max(0, Math.min(startTime, endTime));
  const normalizedEnd = Math.max(0, Math.max(startTime, endTime));
  if (normalizedEnd <= normalizedStart) {
    return null;
  }

  return {
    startTime: normalizedStart,
    endTime: normalizedEnd,
    speaker: cleanText(clip.speaker),
    transcriptSegmentIds: uniqueValues(clip.transcriptSegmentIds ?? [])
  };
}

function buildClipFallbackQuote(
  clip:
    | {
        startTime: number;
        endTime: number;
      }
    | null
): string {
  if (!clip) return '';
  return `Clip ${formatClipTime(clip.startTime)}-${formatClipTime(clip.endTime)}`;
}

function formatClipTime(value: number): string {
  const totalSeconds = Math.max(0, Math.round(value));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;

  if (hours > 0) {
    return `${hours}:${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`;
  }

  return `${minutes}:${String(seconds).padStart(2, '0')}`;
}
