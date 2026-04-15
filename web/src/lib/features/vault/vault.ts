import type { ArtifactPreview } from '$lib/ndk/artifacts';
import { ensureClientNdk, ndk } from '$lib/ndk/client';
import {
  BOOKMARK_LIST_KIND,
  decryptPrivateListTags,
  ensureList,
  fetchLatestUserList,
  publishPrivateListTags,
  type ListTag
} from '$lib/ndk/lists';

const LEGACY_DB_NAME = 'highlighter-vault';
const LEGACY_DB_VERSION = 1;
const LEGACY_FOR_LATER_STORE = 'for-later';
const LEGACY_MIGRATION_KEY = 'highlighter:for-later:nip51-migrated:v1';
const FOR_LATER_TAG = 'hl-for-later';

const ARTIFACT_PREVIEW_KEYS = [
  'id',
  'url',
  'title',
  'author',
  'image',
  'description',
  'source',
  'domain',
  'catalogId',
  'catalogKind',
  'podcastGuid',
  'podcastShowTitle',
  'audioUrl',
  'audioPreviewUrl',
  'transcriptUrl',
  'feedUrl',
  'publishedAt',
  'durationSeconds',
  'referenceTagName',
  'referenceTagValue',
  'referenceKind',
  'referenceKey',
  'highlightTagName',
  'highlightTagValue',
  'highlightReferenceKey'
] as const satisfies ReadonlyArray<keyof ArtifactPreview>;

type ArtifactPreviewKey = (typeof ARTIFACT_PREVIEW_KEYS)[number];
type ArtifactPreviewFields = Pick<ArtifactPreview, ArtifactPreviewKey>;

export type ForLaterSharedRoute = {
  groupId: string;
  artifactId: string;
};

export type ForLaterItem = ArtifactPreviewFields & {
  savedAt: number;
  teaser: string;
  communityIds: string[];
  sharedRoutes: ForLaterSharedRoute[];
};

export type ForLaterStatus =
  | {
      tone: 'ready';
      label: 'Ready to share';
    }
  | {
      tone: 'needs-teaser';
      label: 'Needs teaser';
    }
  | {
      tone: 'already-shared';
      label: string;
    };

type ForLaterPayload = {
  version: 1;
  item: ForLaterItem;
};

export async function listForLaterArtifacts(): Promise<ForLaterItem[]> {
  const { items } = await loadForLaterState();
  return items.toSorted((left, right) => right.savedAt - left.savedAt);
}

export async function getForLaterArtifact(id: string): Promise<ForLaterItem | undefined> {
  const normalizedId = cleanText(id);
  if (!normalizedId) return undefined;

  const { items } = await loadForLaterState();
  return items.find((item) => item.id === normalizedId);
}

export async function saveForLaterArtifact(input: {
  artifact: ArtifactPreview;
  teaser?: string;
  communityIds?: string[];
  sharedRoutes?: ForLaterSharedRoute[];
}): Promise<{ item: ForLaterItem; existing: boolean }> {
  const artifact = pickArtifactPreviewFields(input.artifact);
  const { list, items } = await loadForLaterState();
  const existing = items.find((item) => item.id === artifact.id);
  const item = buildForLaterItem(
    {
      artifact,
      teaser: input.teaser,
      communityIds: input.communityIds,
      sharedRoutes: input.sharedRoutes
    },
    existing
  );

  const nextItems = existing
    ? items.map((candidate) => (candidate.id === item.id ? item : candidate))
    : [...items, item];

  await writeForLaterItems(list, nextItems);
  return { item, existing: Boolean(existing) };
}

export async function updateForLaterArtifact(
  id: string,
  patch: {
    teaser?: string;
    communityIds?: string[];
    sharedRoutes?: ForLaterSharedRoute[];
  }
): Promise<ForLaterItem | undefined> {
  const normalizedId = cleanText(id);
  if (!normalizedId) return undefined;

  const { list, items } = await loadForLaterState();
  const existing = items.find((item) => item.id === normalizedId);
  if (!existing) {
    return undefined;
  }

  const updated = buildForLaterItem(
    {
      artifact: previewFromForLaterItem(existing),
      teaser: patch.teaser ?? existing.teaser,
      communityIds: patch.communityIds ?? existing.communityIds,
      sharedRoutes: patch.sharedRoutes ?? existing.sharedRoutes
    },
    existing
  );

  await writeForLaterItems(
    list,
    items.map((item) => (item.id === normalizedId ? updated : item))
  );

  return updated;
}

export async function removeForLaterArtifact(id: string): Promise<void> {
  const normalizedId = cleanText(id);
  if (!normalizedId) return;

  const { list, items } = await loadForLaterState();
  if (!items.some((item) => item.id === normalizedId)) {
    return;
  }

  await writeForLaterItems(
    list,
    items.filter((item) => item.id !== normalizedId)
  );
}

export function previewFromForLaterItem(item: ForLaterItem): ArtifactPreview {
  return pickArtifactPreviewFields(item);
}

export function forLaterStatus(item: Pick<ForLaterItem, 'teaser' | 'communityIds'>): ForLaterStatus {
  if (item.communityIds.length > 0) {
    return {
      tone: 'already-shared',
      label: `Already in ${item.communityIds.length} communit${item.communityIds.length === 1 ? 'y' : 'ies'}`
    };
  }

  if (!cleanText(item.teaser)) {
    return {
      tone: 'needs-teaser',
      label: 'Needs teaser'
    };
  }

  return {
    tone: 'ready',
    label: 'Ready to share'
  };
}

async function loadForLaterState(): Promise<{
  list: ReturnType<typeof ensureList>;
  items: ForLaterItem[];
}> {
  await ensureClientNdk();

  const currentUser = ndk.$currentUser;
  if (!currentUser) {
    throw new Error('Sign in to manage your private For Later list.');
  }
  if (!ndk.signer) {
    throw new Error('Use a signing session to manage your private For Later list.');
  }

  const event = await fetchLatestUserList(ndk, BOOKMARK_LIST_KIND, currentUser.pubkey);
  const list = ensureList(ndk, BOOKMARK_LIST_KIND, event);
  let items = decodeForLaterItems(await decryptPrivateListTags(list));

  if (shouldAttemptLegacyMigration()) {
    try {
      items = await migrateLegacyItems(list, items);
    } catch {
      // Keep the remote list usable even if local migration is unavailable.
    }
  }

  return { list, items };
}

async function migrateLegacyItems(
  list: ReturnType<typeof ensureList>,
  items: ForLaterItem[]
): Promise<ForLaterItem[]> {
  const legacyItems = await listLegacyForLaterArtifacts();
  markLegacyMigrationChecked();

  if (legacyItems.length === 0) {
    return items;
  }

  const merged = mergeLegacyForLaterItems(items, legacyItems);
  if (merged.length === items.length) {
    return items;
  }

  await writeForLaterItems(list, merged);
  return merged;
}

function mergeLegacyForLaterItems(currentItems: ForLaterItem[], legacyItems: ForLaterItem[]): ForLaterItem[] {
  const seen = new Set(currentItems.map((item) => item.id));
  const merged = [...currentItems];

  for (const item of legacyItems.toSorted((left, right) => left.savedAt - right.savedAt)) {
    if (!seen.has(item.id)) {
      merged.push(item);
      seen.add(item.id);
    }
  }

  return merged.toSorted((left, right) => left.savedAt - right.savedAt);
}

async function writeForLaterItems(
  list: ReturnType<typeof ensureList>,
  items: ForLaterItem[]
): Promise<void> {
  const encryptedTags = await decryptPrivateListTags(list);
  const preservedTags = encryptedTags.filter((tag) => tag[0] !== FOR_LATER_TAG);
  const nextTags = [...preservedTags, ...encodeForLaterItems(items)];

  await publishPrivateListTags(list, nextTags);
}

function encodeForLaterItems(items: ForLaterItem[]): ListTag[] {
  return items
    .toSorted((left, right) => left.savedAt - right.savedAt)
    .map((item) => {
      const normalizedItem = normalizeForLaterItem(item);
      const payload: ForLaterPayload = {
        version: 1,
        item: normalizedItem
      };

      return [FOR_LATER_TAG, normalizedItem.id, JSON.stringify(payload)];
    });
}

function decodeForLaterItems(tags: ListTag[]): ForLaterItem[] {
  const seen = new Set<string>();
  const items: ForLaterItem[] = [];

  for (const tag of tags) {
    const item = decodeForLaterTag(tag);
    if (!item || seen.has(item.id)) continue;
    seen.add(item.id);
    items.push(item);
  }

  return items.toSorted((left, right) => left.savedAt - right.savedAt);
}

function decodeForLaterTag(tag: ListTag): ForLaterItem | undefined {
  if (tag[0] !== FOR_LATER_TAG) {
    return undefined;
  }

  const normalizedId = cleanText(tag[1]);
  const payload = cleanText(tag[2]);
  if (!normalizedId || !payload) {
    return undefined;
  }

  try {
    const parsed = JSON.parse(payload) as Partial<ForLaterPayload> | undefined;
    const rawItem = parsed?.item;
    if (!rawItem || parsed?.version !== 1) {
      return undefined;
    }

    return normalizeForLaterItem({
      ...(rawItem as ForLaterItem),
      id: normalizedId
    });
  } catch {
    return undefined;
  }
}

function buildForLaterItem(
  input: {
    artifact: ArtifactPreviewFields;
    teaser?: string;
    communityIds?: string[];
    sharedRoutes?: ForLaterSharedRoute[];
  },
  existing?: ForLaterItem
): ForLaterItem {
  return {
    ...input.artifact,
    savedAt: existing?.savedAt ?? Date.now(),
    teaser: cleanText(input.teaser ?? existing?.teaser),
    communityIds: uniqueValues([...(existing?.communityIds ?? []), ...(input.communityIds ?? [])]),
    sharedRoutes: uniqueRoutes([...(existing?.sharedRoutes ?? []), ...(input.sharedRoutes ?? [])])
  };
}

function normalizeForLaterItem(item: ForLaterItem): ForLaterItem {
  return {
    ...pickArtifactPreviewFields(item),
    savedAt: Number.isFinite(item.savedAt) && item.savedAt > 0 ? item.savedAt : Date.now(),
    teaser: cleanText(item.teaser),
    communityIds: uniqueValues(item.communityIds),
    sharedRoutes: uniqueRoutes(item.sharedRoutes)
  };
}

function pickArtifactPreviewFields(value: ArtifactPreviewFields): ArtifactPreviewFields {
  return {
    id: cleanText(value.id),
    url: cleanText(value.url),
    title: cleanText(value.title),
    author: cleanText(value.author),
    image: cleanText(value.image),
    description: cleanText(value.description),
    source: value.source,
    domain: cleanText(value.domain),
    catalogId: cleanText(value.catalogId),
    catalogKind: cleanText(value.catalogKind),
    podcastGuid: cleanText(value.podcastGuid),
    podcastShowTitle: cleanText(value.podcastShowTitle),
    audioUrl: cleanText(value.audioUrl),
    audioPreviewUrl: cleanText(value.audioPreviewUrl),
    transcriptUrl: cleanText(value.transcriptUrl),
    feedUrl: cleanText(value.feedUrl),
    publishedAt: cleanText(value.publishedAt),
    durationSeconds:
      typeof value.durationSeconds === 'number' && Number.isFinite(value.durationSeconds)
        ? Math.max(0, Math.round(value.durationSeconds))
        : null,
    referenceTagName: value.referenceTagName,
    referenceTagValue: cleanText(value.referenceTagValue),
    referenceKind: cleanText(value.referenceKind),
    referenceKey: cleanText(value.referenceKey),
    highlightTagName: value.highlightTagName,
    highlightTagValue: cleanText(value.highlightTagValue),
    highlightReferenceKey: cleanText(value.highlightReferenceKey)
  };
}

function uniqueValues(values: string[] | undefined): string[] {
  return [...new Set((values ?? []).map(cleanText).filter(Boolean))];
}

function uniqueRoutes(routes: ForLaterSharedRoute[] | undefined): ForLaterSharedRoute[] {
  const seen = new Set<string>();

  return (routes ?? [])
    .map((route) => ({
      groupId: cleanText(route.groupId),
      artifactId: cleanText(route.artifactId)
    }))
    .filter((route) => {
      if (!route.groupId || !route.artifactId) {
        return false;
      }

      const key = `${route.groupId}:${route.artifactId}`;
      if (seen.has(key)) {
        return false;
      }

      seen.add(key);
      return true;
    });
}

function cleanText(value: string | null | undefined): string {
  return typeof value === 'string' ? value.trim() : '';
}

function shouldAttemptLegacyMigration(): boolean {
  return (
    typeof window !== 'undefined' &&
    typeof localStorage !== 'undefined' &&
    localStorage.getItem(LEGACY_MIGRATION_KEY) !== '1'
  );
}

function markLegacyMigrationChecked(): void {
  if (typeof window === 'undefined' || typeof localStorage === 'undefined') {
    return;
  }

  localStorage.setItem(LEGACY_MIGRATION_KEY, '1');
}

async function listLegacyForLaterArtifacts(): Promise<ForLaterItem[]> {
  if (typeof indexedDB === 'undefined') {
    return [];
  }

  const db = await openLegacyVaultDb();

  try {
    const transaction = db.transaction(LEGACY_FOR_LATER_STORE, 'readonly');
    const items = await requestToPromise<ForLaterItem[]>(transaction.objectStore(LEGACY_FOR_LATER_STORE).getAll());
    await waitForTransaction(transaction);
    return items.map(normalizeForLaterItem).toSorted((left, right) => right.savedAt - left.savedAt);
  } finally {
    db.close();
  }
}

async function openLegacyVaultDb(): Promise<IDBDatabase> {
  return await new Promise((resolve, reject) => {
    const request = indexedDB.open(LEGACY_DB_NAME, LEGACY_DB_VERSION);

    request.onupgradeneeded = () => {
      const database = request.result;

      if (!database.objectStoreNames.contains(LEGACY_FOR_LATER_STORE)) {
        database.createObjectStore(LEGACY_FOR_LATER_STORE, { keyPath: 'id' });
      }
    };

    request.onsuccess = () => resolve(request.result);
    request.onerror = () =>
      reject(request.error ?? new Error('Could not open the legacy For Later database.'));
  });
}

async function waitForTransaction(transaction: IDBTransaction): Promise<void> {
  await new Promise<void>((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onabort = () =>
      reject(transaction.error ?? new Error('For Later storage transaction was aborted.'));
    transaction.onerror = () =>
      reject(transaction.error ?? new Error('For Later storage transaction failed.'));
  });
}

async function requestToPromise<T>(request: IDBRequest<T>): Promise<T> {
  return await new Promise<T>((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () =>
      reject(request.error ?? new Error('IndexedDB request failed.'));
  });
}
