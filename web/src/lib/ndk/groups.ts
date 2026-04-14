import NDK, {
  NDKEvent,
  NDKKind,
  NDKRelaySet,
  NDKSimpleGroup,
  NDKSimpleGroupMetadata,
  type NDKEvent as NDKEventType
} from '@nostr-dev-kit/ndk';
import { GROUP_RELAY_URLS, HIGHLIGHTER_RELAY_URL } from '$lib/ndk/config';

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

export async function createCommunity(
  ndk: NDK,
  input: CreateCommunityInput
): Promise<{ id: string }> {
  const normalized = normalizeCreateCommunityInput(input);

  if (!normalized.name) {
    throw new Error('Enter a community name.');
  }

  if (!isValidCommunityId(normalized.id)) {
    throw new Error('Community URL must use 3-48 lowercase letters, numbers, and hyphens.');
  }

  if (!ndk.signer) {
    throw new Error('Connect a signer before creating a community.');
  }

  const relaySet = buildCommunityRelaySet(ndk);
  const group = new NDKSimpleGroup(ndk, relaySet, normalized.id);

  try {
    await group.createGroup();
  } catch (error) {
    throw new Error(describePublishError(error, 'Could not create the community.'));
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
        'The community was created, but its metadata did not sync cleanly. Retry with a new URL only if this one does not appear on the relay.'
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
