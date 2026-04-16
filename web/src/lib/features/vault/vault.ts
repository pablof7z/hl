import type { ArtifactPreview } from '$lib/ndk/artifacts';
import {
  buildArtifactPreview,
  buildFallbackNostrUrl,
  fetchArtifactsByHighlightReferenceKeys,
  normalizeArtifactUrl
} from '$lib/ndk/artifacts';
import { ensureClientNdk, ndk } from '$lib/ndk/client';
import {
  BOOKMARK_LIST_KIND,
  ensureList,
  fetchLatestUserList,
  type ListTag
} from '$lib/ndk/lists';

type BookmarkTagName = 'a' | 'e' | 'r';

export type ForLaterItem = ArtifactPreview & {
  bookmarkTagName: BookmarkTagName;
  bookmarkTagValue: string;
  bookmarkKey: string;
};

type BookmarkTarget = string | Pick<
  ArtifactPreview,
  'id' | 'url' | 'referenceTagName' | 'referenceTagValue' | 'highlightTagName' | 'highlightTagValue'
>;

export async function listForLaterArtifacts(): Promise<ForLaterItem[]> {
  const { list } = await loadBookmarkList();
  return await resolveBookmarkTags(bookmarkTags(list.tags));
}

export async function getForLaterArtifact(target: BookmarkTarget): Promise<ForLaterItem | undefined> {
  const { list } = await loadBookmarkList();
  const tags = bookmarkTags(list.tags);
  const key = targetKey(target);

  if (!key) return undefined;

  const directTag = tags.find((tag) => bookmarkKey(tag) === key);
  if (directTag) {
    return (await resolveBookmarkTags([directTag]))[0];
  }

  if (typeof target !== 'string') {
    return undefined;
  }

  return (await resolveBookmarkTags(tags)).find(
    (item) => item.id === key || item.referenceKey === key || item.highlightReferenceKey === key
  );
}

export async function saveForLaterArtifact(input: {
  artifact: ArtifactPreview;
}): Promise<{ item: ForLaterItem; existing: boolean }> {
  const { list } = await loadBookmarkList();
  const existingTags = bookmarkTags(list.tags);
  const nextTag = bookmarkTagForArtifact(input.artifact);
  const key = bookmarkKey(nextTag);
  const existing = existingTags.some((tag) => bookmarkKey(tag) === key);

  if (!existing) {
    await publishBookmarkTags(list, [nextTag, ...existingTags]);
  }

  return {
    item: decorateBookmarkPreview(input.artifact, nextTag),
    existing
  };
}

export async function removeForLaterArtifact(target: BookmarkTarget): Promise<void> {
  const { list } = await loadBookmarkList();
  const tags = bookmarkTags(list.tags);
  const key = targetKey(target);
  if (!key) return;

  let nextTags = tags.filter((tag) => bookmarkKey(tag) !== key);

  if (nextTags.length === tags.length && typeof target === 'string') {
    const items = await resolveBookmarkTags(tags);
    const matchingKeys = new Set(
      items
        .filter((item) => item.id === key || item.referenceKey === key || item.highlightReferenceKey === key)
        .map((item) => item.bookmarkKey)
    );
    nextTags = tags.filter((tag) => !matchingKeys.has(bookmarkKey(tag)));
  }

  if (nextTags.length !== tags.length) {
    await publishBookmarkTags(list, nextTags);
  }
}

async function loadBookmarkList(): Promise<{
  list: ReturnType<typeof ensureList>;
}> {
  await ensureClientNdk();

  const currentUser = ndk.$currentUser;
  if (!currentUser) {
    throw new Error('Sign in to manage your For Later bookmarks.');
  }
  if (!ndk.signer) {
    throw new Error('Use a signing session to manage your For Later bookmarks.');
  }

  const event = await fetchLatestUserList(ndk, BOOKMARK_LIST_KIND, currentUser.pubkey);
  return { list: ensureList(ndk, BOOKMARK_LIST_KIND, event) };
}

async function publishBookmarkTags(
  list: ReturnType<typeof ensureList>,
  tags: ListTag[]
): Promise<void> {
  list.tags = uniqueBookmarkTags(tags);
  list.content = '';
  list.created_at = Math.floor(Date.now() / 1e3);
  await list.publishReplaceable();
}

async function resolveBookmarkTags(tags: ListTag[]): Promise<ForLaterItem[]> {
  const uniqueTags = uniqueBookmarkTags(tags);
  const referenceKeys = uniqueTags.map(bookmarkKey).filter(Boolean);
  const resolvedArtifacts = await fetchArtifactsByHighlightReferenceKeys(ndk, referenceKeys);

  return await Promise.all(
    uniqueTags.map(async (tag) => {
      const resolved = resolvedArtifacts.get(bookmarkKey(tag));
      if (resolved) {
        return decorateBookmarkPreview(resolved, tag);
      }

      return decorateBookmarkPreview(await fallbackPreviewForTag(tag), tag);
    })
  );
}

async function fallbackPreviewForTag(tag: ListTag): Promise<ArtifactPreview> {
  const tagName = tag[0] as BookmarkTagName;
  const value = cleanText(tag[1]);

  if (tagName === 'r') {
    const normalizedUrl = normalizeArtifactUrl(value);
    if (normalizedUrl) {
      return await previewUrl(normalizedUrl).catch(() =>
        buildArtifactPreview({ url: normalizedUrl })
      );
    }
  }

  if (tagName === 'a') {
    return buildArtifactPreview({
      url: buildFallbackNostrUrl(value),
      title: 'Nostr article',
      source: 'article',
      domain: 'nostr',
      catalogId: value,
      catalogKind: 'nostr',
      referenceTagName: 'a',
      referenceTagValue: value,
      highlightTagName: 'a',
      highlightTagValue: value
    });
  }

  return buildArtifactPreview({
    url: 'https://beta.highlighter.com/',
    title: tagName === 'e' ? 'Nostr event' : value,
    source: 'article',
    domain: 'nostr',
    catalogId: value,
    catalogKind: tagName === 'e' ? 'nostr:event' : 'nostr',
    referenceTagName: tagName === 'e' ? 'e' : 'i',
    referenceTagValue: value,
    highlightTagName: tagName === 'e' ? 'e' : 'r',
    highlightTagValue: value
  });
}

async function previewUrl(url: string): Promise<ArtifactPreview> {
  const response = await fetch('/api/artifacts/preview', {
    method: 'POST',
    headers: {
      'content-type': 'application/json'
    },
    body: JSON.stringify({ reference: url, source: 'article' })
  });

  const body = (await response.json()) as ArtifactPreview | { error?: string };
  if (!response.ok) {
    throw new Error('error' in body && body.error ? body.error : 'Could not preview that bookmark.');
  }

  return body as ArtifactPreview;
}

function bookmarkTagForArtifact(
  artifact: Pick<
    ArtifactPreview,
    'url' | 'referenceTagName' | 'referenceTagValue' | 'highlightTagName' | 'highlightTagValue'
  >
): ListTag {
  if (artifact.referenceTagName === 'a' || artifact.referenceTagName === 'e') {
    return [artifact.referenceTagName, artifact.referenceTagValue];
  }

  if (artifact.highlightTagName === 'a' || artifact.highlightTagName === 'e') {
    return [artifact.highlightTagName, artifact.highlightTagValue];
  }

  const url = normalizeArtifactUrl(artifact.url || artifact.highlightTagValue || artifact.referenceTagValue);
  if (!url) {
    throw new Error('For Later bookmarks need a Nostr address, event id, or URL.');
  }

  return ['r', url];
}

function decorateBookmarkPreview(preview: ArtifactPreview, tag: ListTag): ForLaterItem {
  const bookmarkTagName = tag[0] as BookmarkTagName;
  const bookmarkTagValue = cleanText(tag[1]);

  return {
    ...preview,
    bookmarkTagName,
    bookmarkTagValue,
    bookmarkKey: bookmarkKey(tag)
  };
}

function bookmarkTags(tags: ListTag[]): ListTag[] {
  return tags.filter((tag) => isBookmarkTagName(tag[0]) && cleanText(tag[1]));
}

function uniqueBookmarkTags(tags: ListTag[]): ListTag[] {
  const seen = new Set<string>();
  const uniqueTags: ListTag[] = [];

  for (const tag of bookmarkTags(tags)) {
    const key = bookmarkKey(tag);
    if (seen.has(key)) continue;
    seen.add(key);
    uniqueTags.push([tag[0], cleanText(tag[1])]);
  }

  return uniqueTags;
}

function targetKey(target: BookmarkTarget): string {
  if (typeof target === 'string') {
    return cleanText(target);
  }

  return bookmarkKey(bookmarkTagForArtifact(target));
}

function bookmarkKey(tag: ListTag): string {
  const tagName = cleanText(tag[0]);
  const value = cleanText(tag[1]);
  return tagName && value ? `${tagName}:${value}` : '';
}

function isBookmarkTagName(value: string | undefined): value is BookmarkTagName {
  return value === 'a' || value === 'e' || value === 'r';
}

function cleanText(value: string | null | undefined): string {
  return typeof value === 'string' ? value.trim() : '';
}
