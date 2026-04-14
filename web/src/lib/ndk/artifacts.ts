import NDK, { NDKEvent, type NDKEvent as NDKEventType, type NDKKind } from '@nostr-dev-kit/ndk';
import { buildCommunityRelaySet } from '$lib/ndk/groups';

export const HIGHLIGHTER_ARTIFACT_KIND = 30403 as NDKKind;

export type ArtifactSource = 'article' | 'book' | 'podcast' | 'video' | 'paper' | 'web';

export type ArtifactPreview = {
  id: string;
  url: string;
  title: string;
  author: string;
  image: string;
  description: string;
  source: ArtifactSource;
  domain: string;
};

export type ArtifactRecord = ArtifactPreview & {
  groupId: string;
  address: string;
  eventId: string;
  pubkey: string;
  createdAt: number | null;
  note: string;
};

const TRACKING_PARAMS = new Set([
  'fbclid',
  'gclid',
  'mc_cid',
  'mc_eid',
  'ref',
  'ref_src',
  'ref_url'
]);

export function normalizeArtifactUrl(value: string): string | null {
  if (!value.trim()) return null;

  try {
    const url = new URL(value.trim());
    if (url.protocol !== 'http:' && url.protocol !== 'https:') {
      return null;
    }

    url.hash = '';
    url.username = '';
    url.password = '';
    url.hostname = url.hostname.toLowerCase();

    if ((url.protocol === 'http:' && url.port === '80') || (url.protocol === 'https:' && url.port === '443')) {
      url.port = '';
    }

    if (url.pathname !== '/') {
      url.pathname = url.pathname.replace(/\/+$/, '') || '/';
    }

    const entries = Array.from(url.searchParams.entries())
      .filter(([key]) => !key.toLowerCase().startsWith('utm_') && !TRACKING_PARAMS.has(key.toLowerCase()))
      .sort(([left], [right]) => left.localeCompare(right));

    url.search = '';
    for (const [key, val] of entries) {
      url.searchParams.append(key, val);
    }

    return url.toString();
  } catch {
    return null;
  }
}

export function artifactIdFromUrl(url: string): string {
  const normalized = normalizeArtifactUrl(url);
  if (!normalized) {
    throw new Error('Enter a valid URL.');
  }

  return `u${fnv1a(normalized).toString(36)}`;
}

export function detectArtifactSource(url: string, hint?: string): ArtifactSource {
  const normalized = normalizeArtifactUrl(url);
  if (!normalized) return 'web';

  const hostname = new URL(normalized).hostname.replace(/^www\./, '');
  const lowerHint = hint?.trim().toLowerCase() ?? '';

  if (
    lowerHint.includes('video') ||
    hostname.includes('youtube.com') ||
    hostname.includes('youtu.be') ||
    hostname.includes('vimeo.com') ||
    hostname.includes('tiktok.com')
  ) {
    return 'video';
  }

  if (
    lowerHint.includes('audio') ||
    hostname.includes('spotify.com') ||
    hostname.includes('podcasts.apple.com') ||
    hostname.includes('overcast.fm')
  ) {
    return 'podcast';
  }

  if (
    hostname.includes('arxiv.org') ||
    hostname.includes('ssrn.com') ||
    hostname.includes('researchgate.net') ||
    hostname.includes('doi.org')
  ) {
    return 'paper';
  }

  if (
    hostname.includes('goodreads.com') ||
    hostname.includes('openlibrary.org') ||
    hostname.includes('bookshop.org')
  ) {
    return 'book';
  }

  return lowerHint === 'website' ? 'web' : 'article';
}

export function buildArtifactPreview(input: {
  url: string;
  title?: string;
  author?: string;
  image?: string;
  description?: string;
  source?: ArtifactSource;
}): ArtifactPreview {
  const normalizedUrl = normalizeArtifactUrl(input.url);
  if (!normalizedUrl) {
    throw new Error('Enter a valid URL.');
  }

  const domain = domainLabel(normalizedUrl);
  const title = cleanText(input.title) || fallbackTitle(normalizedUrl);

  return {
    id: artifactIdFromUrl(normalizedUrl),
    url: normalizedUrl,
    title,
    author: cleanText(input.author),
    image: cleanText(input.image),
    description: cleanText(input.description),
    source: input.source ?? detectArtifactSource(normalizedUrl),
    domain
  };
}

export function artifactFromEvent(event: NDKEventType): ArtifactRecord {
  const url = cleanText(event.tagValue('url'));
  const normalizedUrl = normalizeArtifactUrl(url) ?? url;
  const preview = buildArtifactPreview({
    url: normalizedUrl,
    title: event.tagValue('title'),
    author: event.tagValue('author'),
    image: event.tagValue('image'),
    description: event.content,
    source: parseSource(event.tagValue('source'))
  });

  return {
    ...preview,
    id: cleanText(event.tagValue('d')) || preview.id,
    groupId: cleanText(event.tagValue('h')),
    address: event.tagId(),
    eventId: event.id,
    pubkey: event.pubkey,
    createdAt: event.created_at ?? null,
    note: cleanText(event.content)
  };
}

export function artifactPath(groupId: string, artifactId: string): string {
  return `/community/${encodeURIComponent(groupId)}/content/${encodeURIComponent(artifactId)}`;
}

export async function fetchArtifactsByAddresses(
  ndk: NDK,
  addresses: string[]
): Promise<Map<string, ArtifactRecord>> {
  const parsedAddresses = uniqueValues(addresses)
    .map((address) => parseAddress(address))
    .filter((address): address is { kind: number; pubkey: string; identifier: string } => Boolean(address));

  if (parsedAddresses.length === 0) {
    return new Map();
  }

  const relaySet = buildCommunityRelaySet(ndk);
  const events = Array.from(
    (await ndk.fetchEvents(
      {
        kinds: [HIGHLIGHTER_ARTIFACT_KIND],
        authors: uniqueValues(parsedAddresses.map((address) => address.pubkey)),
        '#d': uniqueValues(parsedAddresses.map((address) => address.identifier)),
        limit: Math.max(parsedAddresses.length * 4, 32)
      },
      { closeOnEose: true },
      relaySet
    )) ?? []
  );
  const requestedAddresses = new Set(parsedAddresses.map((address) => `${address.kind}:${address.pubkey}:${address.identifier}`));

  return new Map(
    events
      .filter((event) => requestedAddresses.has(event.tagId()))
      .map((event) => {
        const artifact = artifactFromEvent(event);
        return [artifact.address, artifact] as const;
      })
  );
}

export async function findExistingArtifact(
  ndk: NDK,
  groupId: string,
  artifactId: string
): Promise<ArtifactRecord | undefined> {
  const relaySet = buildCommunityRelaySet(ndk);
  const events = Array.from(
    (await ndk.fetchEvents(
      {
        kinds: [HIGHLIGHTER_ARTIFACT_KIND],
        '#h': [groupId],
        '#d': [artifactId],
        limit: 10
      },
      { closeOnEose: true },
      relaySet
    )) ?? []
  ).sort((left, right) => (right.created_at ?? 0) - (left.created_at ?? 0));

  const existing = events[0];
  return existing ? artifactFromEvent(existing) : undefined;
}

export async function publishArtifact(
  ndk: NDK,
  input: {
    groupId: string;
    preview: ArtifactPreview;
    note?: string;
  }
): Promise<{ artifact: ArtifactRecord; existing: boolean }> {
  if (!ndk.signer) {
    throw new Error('Connect a signer before sharing artifacts.');
  }

  const existing = await findExistingArtifact(ndk, input.groupId, input.preview.id);
  if (existing) {
    return { artifact: existing, existing: true };
  }

  const relaySet = buildCommunityRelaySet(ndk);
  const event = new NDKEvent(ndk);
  event.kind = HIGHLIGHTER_ARTIFACT_KIND;
  event.content = cleanText(input.note);
  event.tags = [
    ['h', input.groupId],
    ['d', input.preview.id],
    ['title', input.preview.title],
    ['source', input.preview.source],
    ['url', input.preview.url]
  ];

  if (input.preview.author) {
    event.tags.push(['author', input.preview.author]);
  }

  if (input.preview.image) {
    event.tags.push(['image', input.preview.image]);
  }

  await event.sign();
  await event.publish(relaySet);

  return { artifact: artifactFromEvent(event), existing: false };
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

function fallbackTitle(url: string): string {
  try {
    const parsed = new URL(url);
    const pathname = parsed.pathname.replace(/\/+$/, '');
    const lastSegment = pathname.split('/').filter(Boolean).at(-1);

    if (lastSegment) {
      return titleCase(lastSegment.replace(/[-_]+/g, ' '));
    }

    return domainLabel(url);
  } catch {
    return 'Untitled artifact';
  }
}

function domainLabel(url: string): string {
  try {
    return new URL(url).hostname.replace(/^www\./, '');
  } catch {
    return url;
  }
}

function titleCase(value: string): string {
  return value
    .split(/\s+/)
    .filter(Boolean)
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(' ');
}

function cleanText(value: string | undefined): string {
  return value?.trim() ?? '';
}

function uniqueValues(values: string[]): string[] {
  return [...new Set(values.map((value) => value.trim()).filter(Boolean))];
}

function parseAddress(address: string): { kind: number; pubkey: string; identifier: string } | undefined {
  const [kindValue, pubkey, ...rest] = address.trim().split(':');
  const kind = Number(kindValue);
  const identifier = rest.join(':').trim();

  if (!Number.isInteger(kind) || !pubkey?.trim() || !identifier) {
    return undefined;
  }

  return {
    kind,
    pubkey: pubkey.trim(),
    identifier
  };
}

function fnv1a(value: string): number {
  let hash = 0x811c9dc5;

  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index);
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }

  return hash >>> 0;
}
