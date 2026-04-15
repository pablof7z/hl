import NDK, {
  NDKEvent,
  NDKKind,
  type NDKEvent as NDKEventType,
  type NDKFilter
} from '@nostr-dev-kit/ndk';
import { buildCommunityRelaySet } from '$lib/ndk/groups';

export const HIGHLIGHTER_COMMENT_KIND = NDKKind.GenericReply as number;

export type ArtifactRootContext = {
  type: 'artifact';
  artifactAddress: string;
  artifactKind: string;
};

export type ShareThreadRootContext = {
  type: 'share-thread';
  shareThreadEventId: string;
};

export type HighlightRootContext = {
  type: 'highlight';
  highlightEventId: string;
};

export type DiscussionRootContext =
  | ArtifactRootContext
  | ShareThreadRootContext
  | HighlightRootContext;

export type CommentRecord = {
  eventId: string;
  pubkey: string;
  content: string;
  createdAt: number | null;
  parentEventId: string;
  rootContext: DiscussionRootContext;
  rawEvent: NDKEventType;
};

export type CommentThread = {
  comment: CommentRecord;
  replies: CommentThread[];
};

export function commentFromEvent(event: NDKEventType): CommentRecord {
  const parentEventId = resolveParentEventId(event);
  const rootContext = resolveRootContext(event);

  return {
    eventId: event.id,
    pubkey: event.pubkey,
    content: event.content?.trim() ?? '',
    createdAt: event.created_at ?? null,
    parentEventId,
    rootContext,
    rawEvent: event
  };
}

function resolveParentEventId(event: NDKEventType): string {
  const lowercaseE = event.getMatchingTags('e');
  for (const tag of lowercaseE) {
    const marker = tag[3];
    if (!marker || marker === 'reply') {
      return tag[1]?.trim() ?? '';
    }
  }
  return '';
}

function resolveRootContext(event: NDKEventType): DiscussionRootContext {
  const uppercaseA = event.getMatchingTags('A');
  if (uppercaseA.length > 0) {
    const address = uppercaseA[0][1]?.trim() ?? '';
    const uppercaseK = event.tagValue('K')?.trim() ?? '';
    return { type: 'artifact', artifactAddress: address, artifactKind: uppercaseK };
  }

  const uppercaseE = event.getMatchingTags('E');
  if (uppercaseE.length > 0) {
    const rootEventId = uppercaseE[0][1]?.trim() ?? '';
    const rootKind = event.tagValue('K')?.trim() ?? '';
    if (rootKind === '9802') {
      return { type: 'highlight', highlightEventId: rootEventId };
    }
    return { type: 'share-thread', shareThreadEventId: rootEventId };
  }

  return { type: 'share-thread', shareThreadEventId: '' };
}

export function buildCommentTree(comments: CommentRecord[]): CommentThread[] {
  const rootIds = new Set<string>();

  for (const comment of comments) {
    if (comment.rootContext.type === 'artifact') {
      rootIds.add('');
    } else if (comment.rootContext.type === 'share-thread') {
      rootIds.add(comment.rootContext.shareThreadEventId);
    } else {
      rootIds.add(comment.rootContext.highlightEventId);
    }
  }

  const byId = new Map<string, CommentThread>();
  for (const comment of comments) {
    byId.set(comment.eventId, { comment, replies: [] });
  }

  const roots: CommentThread[] = [];

  for (const comment of comments) {
    const node = byId.get(comment.eventId)!;
    const parentId = comment.parentEventId;

    if (!parentId || rootIds.has(parentId) || !byId.has(parentId)) {
      roots.push(node);
    } else {
      byId.get(parentId)!.replies.push(node);
    }
  }

  const sortByTime = (a: CommentThread, b: CommentThread) =>
    (a.comment.createdAt ?? 0) - (b.comment.createdAt ?? 0);

  function sortTree(threads: CommentThread[]) {
    threads.sort(sortByTime);
    for (const thread of threads) {
      sortTree(thread.replies);
    }
  }

  sortTree(roots);
  return roots;
}

export function artifactDiscussionFilter(
  groupId: string,
  artifactAddress: string
): NDKFilter {
  return {
    kinds: [HIGHLIGHTER_COMMENT_KIND],
    '#A': [artifactAddress],
    '#h': [groupId],
    limit: 200
  } as NDKFilter;
}

export function shareThreadDiscussionFilter(
  groupId: string,
  shareThreadEventId: string
): NDKFilter {
  return {
    kinds: [HIGHLIGHTER_COMMENT_KIND],
    '#E': [shareThreadEventId],
    '#h': [groupId],
    limit: 200
  } as NDKFilter;
}

export function highlightDiscussionFilter(
  groupId: string,
  highlightEventId: string
): NDKFilter {
  return {
    kinds: [HIGHLIGHTER_COMMENT_KIND],
    '#E': [highlightEventId],
    '#h': [groupId],
    limit: 200
  } as NDKFilter;
}

export function discussionFilterForContext(
  groupId: string,
  rootContext: DiscussionRootContext
): NDKFilter {
  switch (rootContext.type) {
    case 'artifact':
      return artifactDiscussionFilter(groupId, rootContext.artifactAddress);
    case 'share-thread':
      return shareThreadDiscussionFilter(groupId, rootContext.shareThreadEventId);
    case 'highlight':
      return highlightDiscussionFilter(groupId, rootContext.highlightEventId);
  }
}

export async function publishComment(
  ndkInstance: NDK,
  input: {
    groupId: string;
    rootContext: DiscussionRootContext;
    parentComment?: CommentRecord;
    content: string;
  }
): Promise<CommentRecord> {
  if (!ndkInstance.signer) {
    throw new Error('Connect a signer before posting comments.');
  }

  const content = input.content.trim();
  if (!content) {
    throw new Error('Comment cannot be empty.');
  }

  const rootEvent = buildRootEventForReply(ndkInstance, input.rootContext);
  const relaySet = buildCommunityRelaySet(ndkInstance);

  let commentEvent: NDKEvent;

  if (input.parentComment) {
    const parentEvent = new NDKEvent(ndkInstance, input.parentComment.rawEvent);
    commentEvent = parentEvent.reply(true);
  } else {
    commentEvent = rootEvent.reply(true);
  }

  commentEvent.content = content;

  const hasHTag = commentEvent.tags.some((tag) => tag[0] === 'h');
  if (!hasHTag) {
    commentEvent.tags.push(['h', input.groupId]);
  }

  await commentEvent.sign();
  await commentEvent.publish(relaySet);

  return commentFromEvent(commentEvent);
}

function buildRootEventForReply(
  ndkInstance: NDK,
  rootContext: DiscussionRootContext
): NDKEvent {
  const event = new NDKEvent(ndkInstance);

  switch (rootContext.type) {
    case 'artifact':
      event.kind = Number(rootContext.artifactKind) || NDKKind.Thread;
      event.tags = [['d', rootContext.artifactAddress.split(':').pop() ?? '']];
      break;
    case 'share-thread':
      event.kind = NDKKind.Thread;
      event.id = rootContext.shareThreadEventId;
      break;
    case 'highlight':
      event.kind = NDKKind.Highlight;
      event.id = rootContext.highlightEventId;
      break;
  }

  return event;
}

export function relativeTime(timestamp: number): string {
  const now = Math.floor(Date.now() / 1000);
  const diff = now - timestamp;

  if (diff < 60) return 'just now';
  if (diff < 3600) return `${Math.floor(diff / 60)}m`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h`;
  if (diff < 604800) return `${Math.floor(diff / 86400)}d`;

  return new Intl.DateTimeFormat('en', { month: 'short', day: 'numeric' }).format(
    new Date(timestamp * 1000)
  );
}
