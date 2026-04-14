import type { ArtifactPreview } from '$lib/ndk/artifacts';

const DB_NAME = 'highlighter-vault';
const DB_VERSION = 1;
const FOR_LATER_STORE = 'for-later';

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

export async function listForLaterArtifacts(): Promise<ForLaterItem[]> {
  const db = await openVaultDb();

  try {
    const transaction = db.transaction(FOR_LATER_STORE, 'readonly');
    const items = await requestToPromise<ForLaterItem[]>(transaction.objectStore(FOR_LATER_STORE).getAll());
    await waitForTransaction(transaction);
    return items.map(normalizeForLaterItem).toSorted((left, right) => right.savedAt - left.savedAt);
  } finally {
    db.close();
  }
}

export async function getForLaterArtifact(id: string): Promise<ForLaterItem | undefined> {
  const normalizedId = cleanText(id);
  if (!normalizedId) return undefined;

  const db = await openVaultDb();

  try {
    const transaction = db.transaction(FOR_LATER_STORE, 'readonly');
    const item = await requestToPromise<ForLaterItem | undefined>(
      transaction.objectStore(FOR_LATER_STORE).get(normalizedId)
    );
    await waitForTransaction(transaction);
    return item ? normalizeForLaterItem(item) : undefined;
  } finally {
    db.close();
  }
}

export async function saveForLaterArtifact(input: {
  artifact: ArtifactPreview;
  teaser?: string;
  communityIds?: string[];
  sharedRoutes?: ForLaterSharedRoute[];
}): Promise<{ item: ForLaterItem; existing: boolean }> {
  const artifact = pickArtifactPreviewFields(input.artifact);
  const db = await openVaultDb();

  try {
    const transaction = db.transaction(FOR_LATER_STORE, 'readwrite');
    const store = transaction.objectStore(FOR_LATER_STORE);
    const existing = await requestToPromise<ForLaterItem | undefined>(store.get(artifact.id));
    const item = buildForLaterItem(
      {
        artifact,
        teaser: input.teaser,
        communityIds: input.communityIds,
        sharedRoutes: input.sharedRoutes
      },
      existing ? normalizeForLaterItem(existing) : undefined
    );

    await requestToPromise(store.put(item));
    await waitForTransaction(transaction);
    return { item, existing: Boolean(existing) };
  } finally {
    db.close();
  }
}

export async function updateForLaterArtifact(
  id: string,
  patch: {
    teaser?: string;
    communityIds?: string[];
    sharedRoutes?: ForLaterSharedRoute[];
  }
): Promise<ForLaterItem | undefined> {
  const existing = await getForLaterArtifact(id);
  if (!existing) {
    return undefined;
  }

  return (
    await saveForLaterArtifact({
      artifact: previewFromForLaterItem(existing),
      teaser: patch.teaser ?? existing.teaser,
      communityIds: patch.communityIds ?? existing.communityIds,
      sharedRoutes: patch.sharedRoutes ?? existing.sharedRoutes
    })
  ).item;
}

export async function removeForLaterArtifact(id: string): Promise<void> {
  const normalizedId = cleanText(id);
  if (!normalizedId) return;

  const db = await openVaultDb();

  try {
    const transaction = db.transaction(FOR_LATER_STORE, 'readwrite');
    transaction.objectStore(FOR_LATER_STORE).delete(normalizedId);
    await waitForTransaction(transaction);
  } finally {
    db.close();
  }
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

async function openVaultDb(): Promise<IDBDatabase> {
  if (typeof indexedDB === 'undefined') {
    throw new Error('For Later storage is only available in a browser session.');
  }

  return await new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, DB_VERSION);

    request.onupgradeneeded = () => {
      const database = request.result;

      if (!database.objectStoreNames.contains(FOR_LATER_STORE)) {
        database.createObjectStore(FOR_LATER_STORE, { keyPath: 'id' });
      }
    };

    request.onsuccess = () => resolve(request.result);
    request.onerror = () =>
      reject(request.error ?? new Error('Could not open the For Later database.'));
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
