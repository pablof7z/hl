import NDK, {
  NDKEvent,
  NDKKind,
  NDKRelaySet,
  NDKSimpleGroup,
  NDKSimpleGroupMetadata,
  type NDKEvent as NDKEventType
} from '@nostr-dev-kit/ndk';
import { GROUP_RELAY_URLS, HIGHLIGHTER_RELAY_URL } from '$lib/ndk/config';

// NIP-29 kind for admin-minted invite codes. NDK doesn't expose a constant
// (it has GroupAdminCreateGroup=9007 and GroupAdminRequestJoin=9021 but no 9009).
const KIND_GROUP_ADMIN_CREATE_INVITE = 9009;

// Maximum number of `code` tags the relay accepts on a single kind:9009 event.
// See relay29/nip29 moderation_actions.go — it rejects >10 codes per event.
export const MAX_CODES_PER_INVITE_EVENT = 10;

export type CommunityAccess = 'open' | 'closed';
export type CommunityVisibility = 'public' | 'private';

export type CreateCommunityInput = {
  id: string;
  name: string;
  about?: string;
  picture?: string;
  access: CommunityAccess;
  visibility: CommunityVisibility;
};

export type CommunitySummary = {
  id: string;
  name: string;
  about: string;
  picture: string;
  access: CommunityAccess;
  visibility: CommunityVisibility;
  adminPubkeys: string[];
  memberCount: number | null;
  relayUrl: string;
  metadataEventId: string;
  createdAt: number | null;
};

const COMMUNITY_ID_PATTERN = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;

export function slugifyCommunityId(value: string): string {
  return value
    .trim()
    .toLowerCase()
    .normalize('NFKD')
    .replace(/[^\x00-\x7F]/g, '')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .replace(/-{2,}/g, '-')
    .slice(0, 48);
}

export function isValidCommunityId(value: string): boolean {
  return value.length >= 3 && value.length <= 48 && COMMUNITY_ID_PATTERN.test(value);
}

export function buildCommunityRelaySet(ndk: NDK): NDKRelaySet {
  return NDKRelaySet.fromRelayUrls(GROUP_RELAY_URLS, ndk);
}

export function buildCommunitySummary(
  metadataEvent: NDKEventType,
  options?: {
    adminEvent?: NDKEventType | null;
    memberEvent?: NDKEventType | null;
    relayUrl?: string;
  }
): CommunitySummary {
  const metadata = NDKSimpleGroupMetadata.from(metadataEvent);
  const id = metadata.tagValue('d')?.trim();

  if (!id) {
    throw new Error('Group metadata missing d tag.');
  }

  const visibility = metadata.scope ?? 'public';
  const access = visibility === 'private' ? 'closed' : (metadata.access ?? 'open');
  const adminPubkeys = uniqueValues(
    options?.adminEvent?.getMatchingTags('p').map((tag) => tag[1]).filter(Boolean) ?? []
  );
  const memberPubkeys = uniqueValues(
    options?.memberEvent?.getMatchingTags('p').map((tag) => tag[1]).filter(Boolean) ?? []
  );

  return {
    id,
    name: cleanText(metadata.name) || id,
    about: cleanText(metadata.about),
    picture: cleanText(metadata.picture),
    access,
    visibility,
    adminPubkeys,
    memberCount: options?.memberEvent ? memberPubkeys.length : visibility === 'private' ? null : 0,
    relayUrl: options?.relayUrl ?? HIGHLIGHTER_RELAY_URL,
    metadataEventId: metadata.id,
    createdAt: metadata.created_at ?? null
  };
}

export function groupIdFromEvent(event: Pick<NDKEventType, 'tagValue'>): string {
  return event.tagValue('d')?.trim() || event.tagValue('h')?.trim() || '';
}

export function buildJoinedCommunities(
  currentPubkey: string,
  metadataEvents: NDKEventType[],
  membershipEvents: NDKEventType[]
): CommunitySummary[] {
  if (!currentPubkey.trim()) {
    return [];
  }

  const metadataByGroupId = latestEventsByGroupId(metadataEvents);
  const adminByGroupId = latestEventsByGroupId(
    membershipEvents.filter((event) => event.kind === NDKKind.GroupAdmins)
  );
  const memberByGroupId = latestEventsByGroupId(
    membershipEvents.filter((event) => event.kind === NDKKind.GroupMembers)
  );
  const joined: CommunitySummary[] = [];

  for (const [groupId, metadataEvent] of metadataByGroupId) {
    const adminEvent = adminByGroupId.get(groupId);
    const memberEvent = memberByGroupId.get(groupId);
    const isAdmin = includesPubkey(adminEvent, currentPubkey);
    const isMember = includesPubkey(memberEvent, currentPubkey);

    if (!isAdmin && !isMember) {
      continue;
    }

    try {
      joined.push(
        buildCommunitySummary(metadataEvent, {
          adminEvent,
          memberEvent
        })
      );
    } catch {
      continue;
    }
  }

  return joined.toSorted((left, right) => left.name.localeCompare(right.name));
}

export async function requestToJoinCommunity(
  ndk: NDK,
  groupId: string,
  message?: string
): Promise<void> {
  const normalizedGroupId = groupId.trim();

  if (!normalizedGroupId) {
    throw new Error('Missing room id.');
  }

  if (!ndk.signer) {
    throw new Error('Connect a signer before joining a room.');
  }

  const user = await ndk.signer.user();
  if (!user?.pubkey) {
    throw new Error('Could not resolve the current account.');
  }

  const group = new NDKSimpleGroup(ndk, buildCommunityRelaySet(ndk), normalizedGroupId);

  try {
    await group.requestToJoin(user.pubkey, message);
  } catch (error) {
    throw new Error(describePublishError(error, 'Could not send the join request.'));
  }
}

const INVITE_CODE_ALPHABET =
  'ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnpqrstuvwxyz23456789';

// 24 chars from a 56-glyph alphabet is ~139 bits of entropy. Omits look-alikes
// (0/O, 1/I/l) so codes survive being dictated verbally.
export function generateInviteCode(length = 24): string {
  const chars = INVITE_CODE_ALPHABET;
  const values = new Uint32Array(length);
  const crypto = globalThis.crypto;
  if (!crypto?.getRandomValues) {
    throw new Error('Secure random not available in this environment.');
  }
  crypto.getRandomValues(values);
  let out = '';
  for (let i = 0; i < length; i++) {
    out += chars[values[i] % chars.length];
  }
  return out;
}

export type CreateInviteCodesResult = {
  codes: string[];
  eventId: string;
};

/**
 * Mint one or more invite codes for a closed group by publishing kind:9009.
 * Relay29 consumes codes on use (single-use), so creators mint more as needed.
 * The relay caps each kind:9009 at 10 codes; this helper fans out if needed.
 */
export async function createInviteCodes(
  ndk: NDK,
  groupId: string,
  opts: { count?: number } = {}
): Promise<CreateInviteCodesResult[]> {
  const normalizedGroupId = groupId.trim();
  if (!normalizedGroupId) throw new Error('Missing room id.');
  if (!ndk.signer) throw new Error('Connect a signer before creating invites.');

  const count = Math.max(1, Math.min(opts.count ?? 1, 100));
  const relaySet = buildCommunityRelaySet(ndk);
  const results: CreateInviteCodesResult[] = [];

  for (let i = 0; i < count; i += MAX_CODES_PER_INVITE_EVENT) {
    const batchSize = Math.min(MAX_CODES_PER_INVITE_EVENT, count - i);
    const codes = Array.from({ length: batchSize }, () => generateInviteCode());

    const event = new NDKEvent(ndk);
    event.kind = KIND_GROUP_ADMIN_CREATE_INVITE;
    event.tags.push(['h', normalizedGroupId]);
    for (const code of codes) {
      event.tags.push(['code', code]);
    }

    try {
      await event.sign();
      await event.publish(relaySet);
    } catch (error) {
      throw new Error(describePublishError(error, 'Could not create invite codes.'));
    }

    results.push({ codes, eventId: event.id });
  }

  return results;
}

/**
 * Consume an invite code by publishing a kind:9021 join request carrying the
 * `code` tag. The relay validates the code against the group's live pool and,
 * on success, auto-adds the requester as a member and removes the code.
 */
export async function acceptInviteCode(
  ndk: NDK,
  groupId: string,
  code: string
): Promise<void> {
  const normalizedGroupId = groupId.trim();
  const normalizedCode = code.trim();
  if (!normalizedGroupId) throw new Error('Missing room id.');
  if (!normalizedCode) throw new Error('Missing invite code.');
  if (!ndk.signer) throw new Error('Connect a signer before accepting.');

  const event = new NDKEvent(ndk);
  event.kind = NDKKind.GroupAdminRequestJoin;
  event.tags.push(['h', normalizedGroupId], ['code', normalizedCode]);

  try {
    await event.sign();
    await event.publish(buildCommunityRelaySet(ndk));
  } catch (error) {
    throw new Error(describePublishError(error, 'Could not accept the invitation.'));
  }
}

export async function createCommunity(
  ndk: NDK,
  input: CreateCommunityInput
): Promise<{ id: string }> {
  const normalized = normalizeCreateCommunityInput(input);

  if (!normalized.name) {
    throw new Error('Enter a room name.');
  }

  if (!isValidCommunityId(normalized.id)) {
    throw new Error('Room URL must use 3-48 lowercase letters, numbers, and hyphens.');
  }

  if (!ndk.signer) {
    throw new Error('Connect a signer before creating a room.');
  }

  const relaySet = buildCommunityRelaySet(ndk);
  const group = new NDKSimpleGroup(ndk, relaySet, normalized.id);

  try {
    await group.createGroup();
  } catch (error) {
    throw new Error(describePublishError(error, 'Could not create the room.'));
  }

  const metadataEvent = new NDKEvent(ndk);
  metadataEvent.kind = NDKKind.GroupAdminEditMetadata;
  metadataEvent.tags.push(['h', normalized.id], ['name', normalized.name]);

  if (normalized.about) {
    metadataEvent.tags.push(['about', normalized.about]);
  }

  if (normalized.picture) {
    metadataEvent.tags.push(['picture', normalized.picture]);
  }

  metadataEvent.tags.push([normalized.visibility, '']);
  metadataEvent.tags.push([normalized.access, '']);

  try {
    await metadataEvent.sign();
    await metadataEvent.publish(relaySet);
  } catch (error) {
    throw new Error(
      describePublishError(
        error,
        'The room was created, but its metadata did not sync cleanly. Retry with a new URL only if this one does not appear on the relay.'
      )
    );
  }

  return { id: normalized.id };
}

function normalizeCreateCommunityInput(input: CreateCommunityInput): CreateCommunityInput {
  const visibility = input.visibility === 'private' ? 'private' : 'public';
  const access = visibility === 'private' ? 'closed' : input.access === 'closed' ? 'closed' : 'open';

  return {
    id: slugifyCommunityId(input.id),
    name: cleanText(input.name),
    about: cleanText(input.about),
    picture: cleanText(input.picture),
    access,
    visibility
  };
}

function cleanText(value: string | undefined): string {
  return value?.trim() ?? '';
}

function uniqueValues(values: string[]): string[] {
  return [...new Set(values)];
}

function latestEventsByGroupId(events: NDKEventType[]): Map<string, NDKEventType> {
  const latest = new Map<string, NDKEventType>();

  for (const event of events) {
    const groupId = groupIdFromEvent(event);
    if (!groupId) continue;

    const existing = latest.get(groupId);
    if (!existing || (event.created_at ?? 0) > (existing.created_at ?? 0)) {
      latest.set(groupId, event);
    }
  }

  return latest;
}

function includesPubkey(event: NDKEventType | undefined, pubkey: string): boolean {
  if (!event || !pubkey) {
    return false;
  }

  return event.getMatchingTags('p').some((tag) => tag[1] === pubkey);
}

export type EditRoomMetadataInput = {
  name: string;
  about?: string;
  picture?: string;
  visibility: CommunityVisibility;
  access: CommunityAccess;
};

export async function editRoomMetadata(
  ndk: NDK,
  groupId: string,
  input: EditRoomMetadataInput
): Promise<void> {
  const normalizedGroupId = groupId.trim();
  if (!normalizedGroupId) throw new Error('Missing room id.');
  if (!ndk.signer) throw new Error('Connect a signer before editing room settings.');

  const visibility = input.visibility === 'private' ? 'private' : 'public';
  const access = visibility === 'private' ? 'closed' : input.access === 'closed' ? 'closed' : 'open';

  const event = new NDKEvent(ndk);
  event.kind = NDKKind.GroupAdminEditMetadata;
  event.tags.push(['h', normalizedGroupId]);

  const name = cleanText(input.name);
  if (name) event.tags.push(['name', name]);

  const about = cleanText(input.about);
  if (about) event.tags.push(['about', about]);

  const picture = cleanText(input.picture);
  if (picture) event.tags.push(['picture', picture]);

  event.tags.push([visibility, '']);
  event.tags.push([access, '']);

  try {
    await event.sign();
    await event.publish(buildCommunityRelaySet(ndk));
  } catch (error) {
    throw new Error(describePublishError(error, 'Could not update room settings.'));
  }
}

export async function addRoomMember(
  ndk: NDK,
  groupId: string,
  pubkey: string,
  role?: 'admin'
): Promise<void> {
  const normalizedGroupId = groupId.trim();
  const normalizedPubkey = pubkey.trim();
  if (!normalizedGroupId) throw new Error('Missing room id.');
  if (!normalizedPubkey) throw new Error('Missing pubkey.');
  if (!ndk.signer) throw new Error('Connect a signer before managing room members.');

  const event = new NDKEvent(ndk);
  event.kind = NDKKind.GroupAdminAddUser;
  event.tags.push(['h', normalizedGroupId], ['p', normalizedPubkey]);
  if (role === 'admin') event.tags.push(['role', 'admin']);

  try {
    await event.sign();
    await event.publish(buildCommunityRelaySet(ndk));
  } catch (error) {
    throw new Error(describePublishError(error, 'Could not add member to room.'));
  }
}

export async function removeRoomMember(
  ndk: NDK,
  groupId: string,
  pubkey: string
): Promise<void> {
  const normalizedGroupId = groupId.trim();
  const normalizedPubkey = pubkey.trim();
  if (!normalizedGroupId) throw new Error('Missing room id.');
  if (!normalizedPubkey) throw new Error('Missing pubkey.');
  if (!ndk.signer) throw new Error('Connect a signer before managing room members.');

  const event = new NDKEvent(ndk);
  event.kind = NDKKind.GroupAdminRemoveUser;
  event.tags.push(['h', normalizedGroupId], ['p', normalizedPubkey]);

  try {
    await event.sign();
    await event.publish(buildCommunityRelaySet(ndk));
  } catch (error) {
    throw new Error(describePublishError(error, 'Could not remove member from room.'));
  }
}

function describePublishError(error: unknown, fallback: string): string {
  if (error && typeof error === 'object' && 'relayErrors' in error) {
    const relayErrors = error.relayErrors;
    if (typeof relayErrors === 'string' && relayErrors.trim()) {
      return relayErrors;
    }
  }

  if (error instanceof Error && error.message.trim()) {
    return error.message;
  }

  return fallback;
}
