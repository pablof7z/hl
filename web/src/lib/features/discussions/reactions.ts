import { NDKEvent, NDKKind, type NDKEvent as NDKEventType } from '@nostr-dev-kit/ndk';
import type NDK from '@nostr-dev-kit/ndk';

/// NIP-25 reactions + NIP-09 deletions, scoped to a set of comment ids.
/// One subscription per thread, indexed per-comment. The caller (e.g. a
/// HighlightComments view) fans out the per-comment slices to each
/// CommentCard rather than every card spinning up its own subscription.

export type CommentReactions = {
  /// Pubkey set of users who have a non-deleted "+" reaction.
  likers: Set<string>;
  /// `{ reactionEventId }` for the current user, when present. Used by
  /// the unlike path to publish a NIP-09 deletion against the right id.
  myReactionEventId?: string;
  likeCount: number;
  iLiked: boolean;
};

/// Build the comment-id → reactions map from a flat list of kind:7
/// events + kind:5 deletions. Deletions remove the targeted reaction
/// from the tally before counts/sets are computed.
export function buildReactionMap(
  reactionEvents: NDKEventType[],
  deletionEvents: NDKEventType[],
  myPubkey: string | undefined
): Map<string, CommentReactions> {
  const deletedIds = new Set<string>();
  for (const deletion of deletionEvents) {
    for (const tag of deletion.getMatchingTags('e')) {
      const id = (tag[1] ?? '').trim();
      if (id) deletedIds.add(id);
    }
  }

  const out = new Map<string, CommentReactions>();
  for (const event of reactionEvents) {
    if (deletedIds.has(event.id)) continue;
    if (event.kind !== NDKKind.Reaction) continue;

    // NIP-25: a `-` reaction is a downvote, anything else (including
    // `+` which is the most common) is an upvote / "like".
    const content = (event.content ?? '').trim();
    if (content === '-') continue;

    const targetTag = event.getMatchingTags('e')[0];
    const commentId = (targetTag?.[1] ?? '').trim();
    if (!commentId) continue;

    const entry = out.get(commentId) ?? {
      likers: new Set<string>(),
      likeCount: 0,
      iLiked: false,
      myReactionEventId: undefined as string | undefined
    };
    entry.likers.add(event.pubkey);
    if (myPubkey && event.pubkey === myPubkey) {
      entry.iLiked = true;
      entry.myReactionEventId = event.id;
    }
    entry.likeCount = entry.likers.size;
    out.set(commentId, entry);
  }

  return out;
}

/// Publish a "+" reaction for the given comment. Returns the reaction
/// event id so the caller can stash it on the optimistic state and
/// later target it for unlike.
export async function publishLike(
  ndk: NDK,
  comment: { eventId: string; pubkey: string }
): Promise<string> {
  if (!ndk.signer) throw new Error('Sign in to react to comments.');
  const event = new NDKEvent(ndk);
  event.kind = NDKKind.Reaction;
  event.content = '+';
  event.tags = [
    ['e', comment.eventId],
    ['p', comment.pubkey],
    ['k', '1111']
  ];
  await event.sign();
  await event.publish();
  return event.id;
}

/// Publish a NIP-09 deletion targeting a previously-published reaction.
export async function publishUnlike(ndk: NDK, reactionEventId: string): Promise<void> {
  if (!ndk.signer) throw new Error('Sign in to react to comments.');
  const event = new NDKEvent(ndk);
  event.kind = NDKKind.EventDeletion;
  event.content = '';
  event.tags = [['e', reactionEventId]];
  await event.sign();
  await event.publish();
}
