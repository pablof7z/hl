import NDK, {
  NDKEvent,
  NDKRelaySet,
  getRelayListForUsers,
  type NDKEvent as NDKEventType,
  type NDKKind
} from '@nostr-dev-kit/ndk';
import type { ArtifactRecord } from '$lib/ndk/artifacts';
import { DEFAULT_RELAYS, HIGHLIGHTER_RELAY_URL } from '$lib/ndk/config';
import { buildCommunityRelaySet } from '$lib/ndk/groups';

export const HIGHLIGHTER_HIGHLIGHT_KIND = 9802 as NDKKind;
export const HIGHLIGHTER_HIGHLIGHT_REPOST_KIND = 16 as NDKKind;

export type HighlightRecord = {
  eventId: string;
  pubkey: string;
  quote: string;
  context: string;
  note: string;
  artifactAddress: string;
  sourceUrl: string;
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
  return `/g/${encodeURIComponent(groupId)}/e/${encodeURIComponent(highlightId)}`;
}

export function highlightFromEvent(event: NDKEventType): HighlightRecord {
  return {
    eventId: event.id,
    pubkey: event.pubkey,
    quote: cleanText(event.content),
    context: cleanText(event.tagValue('context')),
    note: cleanText(event.tagValue('comment')),
    artifactAddress: cleanText(event.tagValue('a')),
    sourceUrl: cleanText(event.tagValue('r')),
    createdAt: event.created_at ?? null
  };
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

export async function fetchHighlightsForShares(
  ndk: NDK,
  shareEvents: NDKEventType[]
): Promise<HydratedHighlight[]> {
  const shares = shareEvents
    .map((event) => highlightShareFromEvent(event))
    .filter((share): share is HighlightShareRecord => Boolean(share));

  const highlightIds = uniqueValues(shares.map((share) => share.highlightEventId));
  if (highlightIds.length === 0) {
    return [];
  }

  const relaySet = NDKRelaySet.fromRelayUrls(
    uniqueValues([...DEFAULT_RELAYS, HIGHLIGHTER_RELAY_URL, ...shares.map((share) => share.relayHint).filter(Boolean)]),
    ndk
  );
  const highlightEvents = Array.from(
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

  return hydrateHighlights(highlightEvents, shareEvents);
}

export function highlightCountsByArtifact(highlights: HydratedHighlight[]): Map<string, number> {
  const counts = new Map<string, number>();

  for (const highlight of highlights) {
    if (!highlight.artifactAddress) continue;
    counts.set(highlight.artifactAddress, (counts.get(highlight.artifactAddress) ?? 0) + 1);
  }

  return counts;
}

export async function publishAndShareHighlight(
  ndk: NDK,
  input: {
    groupId: string;
    artifact: ArtifactRecord;
    quote: string;
    context?: string;
    note?: string;
  }
): Promise<{
  highlight: HighlightRecord;
  share: HighlightShareRecord;
  shareExisting: boolean;
}> {
  const highlight = await publishCanonicalHighlight(ndk, input);
  const shared = await shareHighlightToCommunity(ndk, {
    groupId: input.groupId,
    highlight
  });

  return {
    highlight,
    share: shared.share,
    shareExisting: shared.existing
  };
}

export async function shareHighlightToCommunity(
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

  const relaySet = buildCommunityRelaySet(ndk);
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
    quote: string;
    context?: string;
    note?: string;
  }
): Promise<HighlightRecord> {
  if (!ndk.signer) {
    throw new Error('Connect a signer before creating highlights.');
  }

  const quote = cleanText(input.quote);
  if (!quote) {
    throw new Error('Enter the highlighted text first.');
  }

  const currentUser = ndk.activeUser ?? (await ndk.signer.user());
  const relaySet = await buildUserHighlightRelaySet(ndk, currentUser.pubkey);
  const event = new NDKEvent(ndk);

  event.kind = HIGHLIGHTER_HIGHLIGHT_KIND;
  event.content = quote;
  event.tags = [
    ['a', input.artifact.address],
    ['r', input.artifact.url]
  ];

  const context = cleanText(input.context);
  if (context && context !== quote) {
    event.tags.push(['context', context]);
  }

  const note = cleanText(input.note);
  if (note) {
    event.tags.push(['comment', note]);
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
  const relaySet = buildCommunityRelaySet(ndk);
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

async function buildUserHighlightRelaySet(ndk: NDK, pubkey: string): Promise<NDKRelaySet> {
  const relayLists = await getRelayListForUsers([pubkey], ndk, false, 1500);
  const relayList = relayLists.get(pubkey);
  const userRelayUrls = relayList?.writeRelayUrls.length
    ? relayList.writeRelayUrls
    : (relayList?.relays ?? []);
  const relayUrls = uniqueValues(
    userRelayUrls.length > 0
      ? [...userRelayUrls, HIGHLIGHTER_RELAY_URL]
      : [HIGHLIGHTER_RELAY_URL, ...DEFAULT_RELAYS]
  );

  return NDKRelaySet.fromRelayUrls(relayUrls, ndk);
}

function cleanText(value: string | undefined): string {
  return value?.trim() ?? '';
}

function uniqueValues(values: string[]): string[] {
  return [...new Set(values.map((value) => value.trim()).filter(Boolean))];
}
