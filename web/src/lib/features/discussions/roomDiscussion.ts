import NDK, {
  NDKEvent,
  NDKKind,
  type NDKEvent as NDKEventType,
  type NDKFilter
} from '@nostr-dev-kit/ndk';
import { buildRoomRelaySet } from '$lib/ndk/groups';
import {
  type ArtifactPreview,
  type ArtifactSource
} from '$lib/ndk/artifacts';
import { decodeHtmlEntities } from '$lib/utils/html';

export const ROOM_DISCUSSION_KIND = NDKKind.Thread as number;
export const DISCUSSION_MARKER_TAG: readonly [string, string] = ['t', 'discussion'];

export type RoomDiscussionAttachment = {
  title: string;
  author: string;
  image: string;
  summary: string;
  url: string;
  source: ArtifactSource;
  referenceTagName: 'a' | 'e' | 'i';
  referenceTagValue: string;
  referenceKind: string;
  artifactId: string;
};

export type RoomDiscussionRecord = {
  id: string;
  eventId: string;
  groupId: string;
  pubkey: string;
  title: string;
  body: string;
  summary: string;
  createdAt: number;
  attachment?: RoomDiscussionAttachment;
};

/**
 * True iff this kind:11 thread was posted as a free-form discussion rather
 * than an artifact share. Distinguisher: the `['t', 'discussion']` marker.
 */
export function isDiscussionThread(event: Pick<NDKEventType, 'getMatchingTags'>): boolean {
  return event.getMatchingTags('t').some((tag) => tag[1] === 'discussion');
}

export function discussionFromEvent(event: NDKEventType): RoomDiscussionRecord {
  const title = decodeHtmlEntities(event.tagValue('title') ?? '').trim();
  const body = (event.content ?? '').trim();
  const groupId = event.tagValue('h')?.trim() ?? '';
  const slug = event.tagValue('d')?.trim() || event.id;
  const summary = decodeHtmlEntities(event.tagValue('summary') ?? '').trim();
  const attachment = readAttachment(event);

  return {
    id: slug,
    eventId: event.id,
    groupId,
    pubkey: event.pubkey,
    title: title || 'Untitled discussion',
    body,
    summary,
    createdAt: event.created_at ?? 0,
    attachment
  };
}

function readAttachment(event: NDKEventType): RoomDiscussionAttachment | undefined {
  const aTag = event.tagValue('a')?.trim() ?? '';
  const eTag = event.tagValue('e')?.trim() ?? '';
  const iTag = event.getMatchingTags('i')[0];
  const rTag = event.tagValue('r')?.trim() ?? '';
  const hasAny = Boolean(aTag || eTag || iTag?.[1] || rTag);
  if (!hasAny) return undefined;

  const referenceTagName: 'a' | 'e' | 'i' = aTag ? 'a' : eTag ? 'e' : 'i';
  const referenceTagValue = aTag || eTag || iTag?.[1]?.trim() || rTag;
  const referenceKind = event.tagValue('k')?.trim() ?? '';
  const source = parseSource(event.tagValue('source')) ?? 'web';

  return {
    title: decodeHtmlEntities(event.tagValue('attachment_title') ?? event.tagValue('title') ?? '').trim(),
    author: decodeHtmlEntities(event.tagValue('author') ?? '').trim(),
    image: (event.tagValue('image') ?? '').trim(),
    summary: decodeHtmlEntities(event.tagValue('attachment_summary') ?? '').trim(),
    url: rTag || (iTag?.[2]?.trim() ?? ''),
    source,
    referenceTagName,
    referenceTagValue,
    referenceKind,
    artifactId: event.tagValue('attachment_id')?.trim() ?? ''
  };
}

function parseSource(value: string | undefined): ArtifactSource | undefined {
  if (
    value === 'article' ||
    value === 'book' ||
    value === 'podcast' ||
    value === 'video' ||
    value === 'paper' ||
    value === 'web'
  ) {
    return value;
  }
  return undefined;
}

export function discussionPath(groupId: string, discussionSlug: string): string {
  return `/r/${encodeURIComponent(groupId)}/d/${encodeURIComponent(discussionSlug)}`;
}

export function discussionsFilterForGroup(groupId: string, limit = 64): NDKFilter {
  return {
    kinds: [ROOM_DISCUSSION_KIND],
    '#h': [groupId],
    '#t': ['discussion'],
    limit
  } as NDKFilter;
}

export async function publishRoomDiscussion(
  ndk: NDK,
  input: {
    groupId: string;
    title: string;
    body: string;
    summary?: string;
    attachment?: ArtifactPreview | null;
  }
): Promise<RoomDiscussionRecord> {
  if (!ndk.signer) {
    throw new Error('Connect a signer before starting a discussion.');
  }

  const title = input.title.trim();
  if (!title) {
    throw new Error('Give your discussion a title.');
  }

  const body = (input.body ?? '').trim();
  const summary = (input.summary ?? '').trim();
  const slug = generateDiscussionSlug();

  const event = new NDKEvent(ndk);
  event.kind = ROOM_DISCUSSION_KIND;
  event.content = body;
  event.tags = [
    ['h', input.groupId],
    ['d', slug],
    ['title', title],
    [DISCUSSION_MARKER_TAG[0], DISCUSSION_MARKER_TAG[1]]
  ];

  if (summary) {
    event.tags.push(['summary', summary]);
  }

  const attachment = input.attachment;
  if (attachment) {
    event.tags.push(['source', attachment.source]);

    if (attachment.referenceTagName === 'i') {
      if (attachment.url) {
        event.tags.push(['i', attachment.referenceTagValue, attachment.url]);
      } else {
        event.tags.push(['i', attachment.referenceTagValue]);
      }
      if (attachment.referenceKind) {
        event.tags.push(['k', attachment.referenceKind]);
      }
    } else {
      event.tags.push([attachment.referenceTagName, attachment.referenceTagValue]);
    }

    if (attachment.url) event.tags.push(['r', attachment.url]);
    if (attachment.image) event.tags.push(['image', attachment.image]);
    if (attachment.author) event.tags.push(['author', attachment.author]);
    if (attachment.title) event.tags.push(['attachment_title', attachment.title]);
    if (attachment.description) event.tags.push(['attachment_summary', attachment.description]);
    if (attachment.id) event.tags.push(['attachment_id', attachment.id]);
  }

  await event.sign();
  await event.publish(buildRoomRelaySet(ndk));

  return discussionFromEvent(event);
}

function generateDiscussionSlug(): string {
  const crypto = globalThis.crypto;
  if (crypto?.randomUUID) {
    return `d${crypto.randomUUID().replace(/-/g, '')}`;
  }
  const values = new Uint32Array(4);
  crypto?.getRandomValues?.(values);
  return `d${Array.from(values).map((v) => v.toString(36)).join('')}`;
}
