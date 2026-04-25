import NDK, { NDKEvent, NDKKind, type NDKEvent as NDKEventType, type NDKFilter } from '@nostr-dev-kit/ndk';
import { buildRoomRelaySet } from '$lib/ndk/groups';

export type ChatMessage = {
  eventId: string;
  pubkey: string;
  content: string;
  createdAt: number;
  parentEventId: string | null;
  rawEvent: NDKEventType;
};

export function chatFilter(groupId: string): NDKFilter {
  return { kinds: [NDKKind.GroupChat], '#h': [groupId], limit: 100 } as NDKFilter;
}

export function messageFromEvent(event: NDKEventType): ChatMessage {
  const eTags = event.getMatchingTags('e');
  let parentEventId: string | null = null;

  for (const tag of eTags) {
    const marker = tag[3];
    if (marker === 'reply' || marker === 'root') {
      parentEventId = tag[1]?.trim() || null;
      break;
    }
  }

  if (!parentEventId && eTags.length > 0) {
    parentEventId = eTags[0][1]?.trim() || null;
  }

  return {
    eventId: event.id,
    pubkey: event.pubkey,
    content: event.content?.trim() ?? '',
    createdAt: event.created_at ?? 0,
    parentEventId,
    rawEvent: event
  };
}

export async function publishChatMessage(
  ndk: NDK,
  input: { groupId: string; content: string; replyTo?: ChatMessage }
): Promise<ChatMessage> {
  if (!ndk.signer) throw new Error('Connect a signer before sending messages.');

  const content = input.content.trim();
  if (!content) throw new Error('Message cannot be empty.');

  const event = new NDKEvent(ndk);
  event.kind = NDKKind.GroupChat;
  event.content = content;
  event.tags.push(['h', input.groupId]);

  if (input.replyTo) {
    // Build NIP-10 tags manually — NDKEvent.reply() produces kind:1111 for non-kind:1 events,
    // which is wrong for NIP-29 group chat (kind:9).
    event.tags.push(['e', input.replyTo.eventId, '', 'reply']);
    event.tags.push(['p', input.replyTo.pubkey]);
  }

  await event.sign();
  await event.publish(buildRoomRelaySet(ndk));

  return messageFromEvent(event);
}
