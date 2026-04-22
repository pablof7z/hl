import NDK, {
  NDKEvent,
  NDKKind,
  NDKRelaySet,
  nip19,
  type NDKEvent as NDKEventType,
  type NostrEvent
} from '@nostr-dev-kit/ndk';
import { articleImageUrl, articleSummary, articleTitle, displayName, shortPubkey } from '$lib/ndk/format';
import { DEFAULT_RELAYS } from '$lib/ndk/config';
import { buildCommunityRelaySet } from '$lib/ndk/groups';

export const HIGHLIGHTER_ARTIFACT_SHARE_KIND = NDKKind.Thread as NDKKind;

export type ArtifactSource = 'article' | 'book' | 'podcast' | 'video' | 'paper' | 'web';
export type ArtifactReferenceTagName = 'a' | 'e' | 'i';
export type ArtifactHighlightTagName = 'a' | 'e' | 'r';

export type ArtifactPreview = {
  id: string;
  url: string;
  title: string;
  author: string;
  image: string;
  description: string;
  source: ArtifactSource;
  domain: string;
  catalogId: string;
  catalogKind: string;
  podcastGuid: string;
  podcastShowTitle: string;
  audioUrl: string;
  audioPreviewUrl: string;
  transcriptUrl: string;
  feedUrl: string;
  publishedAt: string;
  durationSeconds: number | null;
  referenceTagName: ArtifactReferenceTagName;
  referenceTagValue: string;
  referenceKind: string;
  referenceKey: string;
  highlightTagName: ArtifactHighlightTagName;
  highlightTagValue: string;
  highlightReferenceKey: string;
};

export type ArtifactRecord = ArtifactPreview & {
  groupId: string;
  shareEventId: string;
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

export function artifactIdFromReferenceKey(referenceKey: string): string {
  const normalized = cleanText(referenceKey);
  if (!normalized) {
    throw new Error('Artifact references need a stable key.');
  }

  return `c${fnv1a(normalized).toString(36)}`;
}

export function artifactPath(groupId: string, artifactId: string): string {
  return `/r/${encodeURIComponent(groupId)}/e/${encodeURIComponent(artifactId)}`;
}

export function artifactHighlightReferenceKey(
  artifact:
    | Pick<ArtifactPreview, 'highlightTagName' | 'highlightTagValue'>
    | Pick<ArtifactRecord, 'highlightTagName' | 'highlightTagValue'>
): string {
  return referenceKeyForTag(artifact.highlightTagName, artifact.highlightTagValue);
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
    lowerHint.includes('podcast') ||
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
  domain?: string;
  catalogId?: string;
  catalogKind?: string;
  podcastGuid?: string;
  podcastShowTitle?: string;
  audioUrl?: string;
  audioPreviewUrl?: string;
  transcriptUrl?: string;
  feedUrl?: string;
  publishedAt?: string;
  durationSeconds?: number | string | null;
  referenceTagName?: ArtifactReferenceTagName;
  referenceTagValue?: string;
  referenceKind?: string;
  highlightTagName?: ArtifactHighlightTagName;
  highlightTagValue?: string;
}): ArtifactPreview {
  const normalizedUrl = normalizeArtifactUrl(input.url);
  if (!normalizedUrl) {
    throw new Error('Enter a valid URL.');
  }

  const referenceTagName = input.referenceTagName ?? 'i';
  const referenceTagValue = cleanText(input.referenceTagValue) || cleanText(input.catalogId) || normalizedUrl;
  const referenceKind = cleanText(input.referenceKind) || cleanText(input.catalogKind) || 'web';
  const referenceKey = referenceKeyForTag(referenceTagName, referenceTagValue);
  const highlightTagName = input.highlightTagName ?? 'r';
  const highlightTagValue = cleanText(input.highlightTagValue) || normalizedUrl;
  const highlightReferenceKey = referenceKeyForTag(highlightTagName, highlightTagValue);
  const domain = cleanText(input.domain) || domainLabel(normalizedUrl);
  const title = cleanText(input.title) || fallbackTitle(normalizedUrl);

  return {
    id: artifactIdFromReferenceKey(referenceKey),
    url: normalizedUrl,
    title,
    author: cleanText(input.author),
    image: cleanText(input.image),
    description: cleanText(input.description),
    source: input.source ?? detectArtifactSource(normalizedUrl),
    domain,
    catalogId: cleanText(input.catalogId) || referenceTagValue,
    catalogKind: cleanText(input.catalogKind) || referenceKind,
    podcastGuid: cleanText(input.podcastGuid) || podcastGuidFromCatalogValue(referenceTagValue),
    podcastShowTitle: cleanText(input.podcastShowTitle),
    audioUrl: cleanText(input.audioUrl),
    audioPreviewUrl: cleanText(input.audioPreviewUrl),
    transcriptUrl: cleanText(input.transcriptUrl),
    feedUrl: cleanText(input.feedUrl),
    publishedAt: cleanText(input.publishedAt),
    durationSeconds: sanitizeDurationSeconds(input.durationSeconds),
    referenceTagName,
    referenceTagValue,
    referenceKind,
    referenceKey,
    highlightTagName,
    highlightTagValue,
    highlightReferenceKey
  };
}

export function buildNostrArticleArtifactPreview(input: {
  event: NostrEvent;
  canonicalUrl: string;
  authorName?: string;
}): ArtifactPreview {
  const address = eventAddress(input.event);
  if (!address) {
    throw new Error('Only addressable Nostr articles can be shared into communities right now.');
  }

  const normalizedUrl = normalizeArtifactUrl(input.canonicalUrl);
  if (!normalizedUrl) {
    throw new Error('Nostr article preview requires a valid canonical URL.');
  }

  const referenceKey = referenceKeyForTag('a', address);
  const kindLabel = String(input.event.kind ?? 30023);

  return {
    id: artifactIdFromReferenceKey(referenceKey),
    url: normalizedUrl,
    title: articleTitle(input.event),
    author: cleanText(input.authorName),
    image: cleanText(articleImageUrl(input.event)),
    description: articleSummary(input.event),
    source: 'article',
    domain: 'nostr',
    catalogId: address,
    catalogKind: `nostr:${kindLabel}`,
    podcastGuid: '',
    podcastShowTitle: '',
    audioUrl: '',
    audioPreviewUrl: '',
    transcriptUrl: '',
    feedUrl: '',
    publishedAt: '',
    durationSeconds: null,
    referenceTagName: 'a',
    referenceTagValue: address,
    referenceKind: kindLabel,
    referenceKey,
    highlightTagName: 'a',
    highlightTagValue: address,
    highlightReferenceKey: referenceKey
  };
}

export function artifactFromEvent(event: NDKEventType): ArtifactRecord {
  const aTag = firstTagValue(event, 'a');
  const eTag = firstTagValue(event, 'e');
  const iTag = event.getMatchingTags('i')[0];
  const rTag = firstTagValue(event, 'r');
  const url = normalizeArtifactUrl(rTag || iTag?.[2] || '') ?? '';
  const primaryReference = resolvePrimaryReference({
    aTag,
    eTag,
    iTagValue: cleanText(iTag?.[1]),
    kTag: cleanText(event.tagValue('k')),
    url
  });
  const preview =
    primaryReference.mode === 'nostr-article'
      ? buildArtifactPreview({
          url: url || buildFallbackNostrUrl(primaryReference.value),
          title: event.tagValue('title'),
          author: event.tagValue('author'),
          image: event.tagValue('image'),
          description: event.tagValue('summary'),
          source: parseSource(event.tagValue('source')) ?? 'article',
          domain: 'nostr',
          podcastGuid: event.tagValue('podcast_guid'),
          podcastShowTitle: event.tagValue('podcast_show_title'),
          audioUrl: event.tagValue('audio'),
          audioPreviewUrl: event.tagValue('audio_preview'),
          transcriptUrl: event.tagValue('transcript'),
          feedUrl: event.tagValue('feed'),
          publishedAt: event.tagValue('published_at'),
          durationSeconds: event.tagValue('duration'),
          catalogId: primaryReference.value,
          catalogKind: `nostr:${primaryReference.kind}`,
          referenceTagName: 'a',
          referenceTagValue: primaryReference.value,
          referenceKind: primaryReference.kind,
          highlightTagName: 'a',
          highlightTagValue: primaryReference.value
        })
      : buildArtifactPreview({
          url: url || normalizeArtifactUrl(primaryReference.value) || buildFallbackUrl(primaryReference.value),
          title: event.tagValue('title'),
          author: event.tagValue('author'),
          image: event.tagValue('image'),
          description: event.tagValue('summary'),
          source: parseSource(event.tagValue('source')),
          podcastGuid: event.tagValue('podcast_guid'),
          podcastShowTitle: event.tagValue('podcast_show_title'),
          audioUrl: event.tagValue('audio'),
          audioPreviewUrl: event.tagValue('audio_preview'),
          transcriptUrl: event.tagValue('transcript'),
          feedUrl: event.tagValue('feed'),
          publishedAt: event.tagValue('published_at'),
          durationSeconds: event.tagValue('duration'),
          catalogId: primaryReference.catalogId,
          catalogKind: primaryReference.catalogKind,
          referenceTagName: primaryReference.referenceTagName,
          referenceTagValue: primaryReference.value,
          referenceKind: primaryReference.referenceKind,
          highlightTagName: primaryReference.highlightTagName,
          highlightTagValue: primaryReference.highlightTagValue
        });

  return {
    ...preview,
    id: cleanText(event.tagValue('d')) || preview.id,
    groupId: cleanText(event.tagValue('h')),
    shareEventId: event.id,
    pubkey: event.pubkey,
    createdAt: event.created_at ?? null,
    note: cleanText(event.content)
  };
}

export async function fetchArtifactsByHighlightReferenceKeys(
  ndk: NDK,
  referenceKeys: string[]
): Promise<Map<string, ArtifactRecord>> {
  const parsed = uniqueValues(referenceKeys)
    .map(parseReferenceKey)
    .filter((candidate): candidate is { tagName: 'a' | 'e' | 'r'; value: string } => Boolean(candidate));

  if (parsed.length === 0) {
    return new Map();
  }

  const filters = buildShareLookupFilters(parsed, Math.max(parsed.length * 4, 32));
  if (filters.length === 0) {
    return new Map();
  }

  const relaySet = buildCommunityRelaySet(ndk);
  const events = Array.from((await ndk.fetchEvents(filters, { closeOnEose: true }, relaySet)) ?? []).sort(
    (left, right) => (right.created_at ?? 0) - (left.created_at ?? 0)
  );

  const artifacts = new Map<string, ArtifactRecord>();

  for (const event of events) {
    const artifact = artifactFromEvent(event);
    if (!artifact.highlightReferenceKey || artifacts.has(artifact.highlightReferenceKey)) {
      continue;
    }

    artifacts.set(artifact.highlightReferenceKey, artifact);
  }

  const unresolvedNostrArticleAddresses = uniqueValues(
    parsed
      .filter(({ tagName, value }) => {
        if (tagName !== 'a') return false;

        const parsedAddress = parseAddress(value);
        return parsedAddress?.kind === 30023 && !artifacts.has(referenceKeyForTag('a', value));
      })
      .map(({ value }) => value)
  );

  if (unresolvedNostrArticleAddresses.length > 0) {
    const resolvedArticles = await fetchNostrArticleArtifacts(ndk, unresolvedNostrArticleAddresses);

    for (const [referenceKey, artifact] of resolvedArticles) {
      if (!artifacts.has(referenceKey)) {
        artifacts.set(referenceKey, artifact);
      }
    }
  }

  return artifacts;
}

export async function fetchArtifactSharesForGroup(
  ndk: NDK,
  groupId: string,
  limit = 32
): Promise<ArtifactRecord[]> {
  const relaySet = buildCommunityRelaySet(ndk);
  const events = Array.from(
    (await ndk.fetchEvents(
      {
        kinds: [HIGHLIGHTER_ARTIFACT_SHARE_KIND],
        '#h': [groupId],
        limit
      },
      { closeOnEose: true },
      relaySet
    )) ?? []
  ).sort((left, right) => (right.created_at ?? 0) - (left.created_at ?? 0));

  const latestById = new Map<string, ArtifactRecord>();

  for (const event of events) {
    const artifact = artifactFromEvent(event);
    if (!artifact.id || latestById.has(artifact.id)) continue;
    latestById.set(artifact.id, artifact);
  }

  return [...latestById.values()];
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
        kinds: [HIGHLIGHTER_ARTIFACT_SHARE_KIND],
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
    throw new Error('Connect a signer before sharing content.');
  }

  const existing = await findExistingArtifact(ndk, input.groupId, input.preview.id);
  const relaySet = buildCommunityRelaySet(ndk);

  if (existing) {
    const mergedPreview = mergeArtifactPreview(existing, input.preview);
    if (!artifactPreviewShouldRefresh(existing, mergedPreview)) {
      return { artifact: existing, existing: true };
    }

    const event = buildArtifactShareEvent(ndk, {
      groupId: input.groupId,
      preview: mergedPreview,
      note: existing.note || input.note
    });

    await event.sign();
    await event.publish(relaySet);

    return { artifact: artifactFromEvent(event), existing: true };
  }

  const event = buildArtifactShareEvent(ndk, input);

  await event.sign();
  await event.publish(relaySet);

  return { artifact: artifactFromEvent(event), existing: false };
}

function buildArtifactShareEvent(
  ndk: NDK,
  input: {
    groupId: string;
    preview: ArtifactPreview;
    note?: string;
  }
): NDKEvent {
  const event = new NDKEvent(ndk);
  event.kind = HIGHLIGHTER_ARTIFACT_SHARE_KIND;
  event.content = cleanText(input.note);
  event.tags = [
    ['h', input.groupId],
    ['d', input.preview.id],
    ['title', input.preview.title],
    ['source', input.preview.source]
  ];

  if (input.preview.referenceTagName === 'i') {
    if (input.preview.url) {
      event.tags.push(['i', input.preview.referenceTagValue, input.preview.url]);
    } else {
      event.tags.push(['i', input.preview.referenceTagValue]);
    }

    if (input.preview.referenceKind) {
      event.tags.push(['k', input.preview.referenceKind]);
    }
  } else {
    event.tags.push([input.preview.referenceTagName, input.preview.referenceTagValue]);
  }

  if (input.preview.url) {
    event.tags.push(['r', input.preview.url]);
  }

  if (input.preview.author) {
    event.tags.push(['author', input.preview.author]);
  }

  if (input.preview.image) {
    event.tags.push(['image', input.preview.image]);
  }

  if (input.preview.description) {
    event.tags.push(['summary', input.preview.description]);
  }

  if (input.preview.podcastGuid) {
    event.tags.push(['podcast_guid', input.preview.podcastGuid]);
  }

  if (input.preview.podcastShowTitle) {
    event.tags.push(['podcast_show_title', input.preview.podcastShowTitle]);
  }

  if (input.preview.audioUrl) {
    event.tags.push(['audio', input.preview.audioUrl]);
  }

  if (input.preview.audioPreviewUrl) {
    event.tags.push(['audio_preview', input.preview.audioPreviewUrl]);
  }

  if (input.preview.transcriptUrl) {
    event.tags.push(['transcript', input.preview.transcriptUrl]);
  }

  if (input.preview.feedUrl) {
    event.tags.push(['feed', input.preview.feedUrl]);
  }

  if (input.preview.publishedAt) {
    event.tags.push(['published_at', input.preview.publishedAt]);
  }

  if (typeof input.preview.durationSeconds === 'number' && Number.isFinite(input.preview.durationSeconds)) {
    event.tags.push(['duration', String(Math.max(0, Math.round(input.preview.durationSeconds)))]);
  }

  return event;
}

function artifactPreviewShouldRefresh(existing: ArtifactRecord, preview: ArtifactPreview): boolean {
  return (
    cleanText(existing.title) !== cleanText(preview.title) ||
    cleanText(existing.author) !== cleanText(preview.author) ||
    cleanText(existing.image) !== cleanText(preview.image) ||
    cleanText(existing.description) !== cleanText(preview.description) ||
    cleanText(existing.podcastGuid) !== cleanText(preview.podcastGuid) ||
    cleanText(existing.podcastShowTitle) !== cleanText(preview.podcastShowTitle) ||
    cleanText(existing.audioUrl) !== cleanText(preview.audioUrl) ||
    cleanText(existing.audioPreviewUrl) !== cleanText(preview.audioPreviewUrl) ||
    cleanText(existing.transcriptUrl) !== cleanText(preview.transcriptUrl) ||
    cleanText(existing.feedUrl) !== cleanText(preview.feedUrl) ||
    cleanText(existing.publishedAt) !== cleanText(preview.publishedAt) ||
    sanitizeDurationSeconds(existing.durationSeconds) !== sanitizeDurationSeconds(preview.durationSeconds)
  );
}

function mergeArtifactPreview(existing: ArtifactRecord, preview: ArtifactPreview): ArtifactPreview {
  return {
    ...preview,
    title: choosePreferredArtifactText(existing.title, preview.title, existing.domain),
    author: choosePreferredArtifactText(existing.author, preview.author, ''),
    image: choosePreferredArtifactImage(existing.image, preview.image, existing.domain),
    description: choosePreferredArtifactText(existing.description, preview.description, existing.domain),
    podcastGuid: choosePreferredArtifactValue(existing.podcastGuid, preview.podcastGuid),
    podcastShowTitle: choosePreferredArtifactText(existing.podcastShowTitle, preview.podcastShowTitle, ''),
    audioUrl: choosePreferredArtifactValue(existing.audioUrl, preview.audioUrl),
    audioPreviewUrl: choosePreferredArtifactValue(existing.audioPreviewUrl, preview.audioPreviewUrl),
    transcriptUrl: choosePreferredArtifactValue(existing.transcriptUrl, preview.transcriptUrl),
    feedUrl: choosePreferredArtifactValue(existing.feedUrl, preview.feedUrl),
    publishedAt: choosePreferredArtifactValue(existing.publishedAt, preview.publishedAt),
    durationSeconds: choosePreferredDurationSeconds(existing.durationSeconds, preview.durationSeconds)
  };
}

function choosePreferredArtifactValue(existingValue: string, nextValue: string): string {
  const current = cleanText(existingValue);
  const next = cleanText(nextValue);
  if (!next) return current;
  if (!current) return next;
  return current;
}

function choosePreferredDurationSeconds(
  existingValue: number | null,
  nextValue: number | null
): number | null {
  const current = sanitizeDurationSeconds(existingValue);
  const next = sanitizeDurationSeconds(nextValue);
  if (next == null) return current;
  if (current == null) return next;
  return current;
}

function choosePreferredArtifactText(existingValue: string, nextValue: string, domain: string): string {
  const current = cleanText(existingValue);
  const next = cleanText(nextValue);

  if (!next) return current;
  if (!current) return next;
  if (current === next) return current;
  if (isLikelyGenericArtifactText(current, domain) && !isLikelyGenericArtifactText(next, domain)) {
    return next;
  }

  return current;
}

function choosePreferredArtifactImage(existingValue: string, nextValue: string, domain: string): string {
  const current = cleanText(existingValue);
  const next = cleanText(nextValue);

  if (!next) return current;
  if (!current) return next;
  if (current === next) return current;
  if (isLikelyGenericArtifactImage(current, domain) && !isLikelyGenericArtifactImage(next, domain)) {
    return next;
  }

  return current;
}

function isLikelyGenericArtifactText(value: string, domain: string): boolean {
  const normalizedValue = comparisonText(value);
  const normalizedDomain = comparisonText(domain);

  if (!normalizedValue) return true;
  if (normalizedDomain && normalizedValue === normalizedDomain) return true;
  if (normalizedDomain) {
    const siteLabel = normalizedDomain.split(' ')[0];
    if (siteLabel && normalizedValue === siteLabel) return true;
  }

  return false;
}

function isLikelyGenericArtifactImage(value: string, domain: string): boolean {
  if (!value) return true;

  try {
    const parsed = new URL(value);
    const pathname = parsed.pathname.toLowerCase();
    const filename = pathname.split('/').at(-1) ?? '';
    const siteToken = comparisonText(domain).replace(/\s+/g, '');

    if (/favicon|apple-touch|sprite|logo|icon/.test(pathname)) {
      return true;
    }

    if (/share-icons|social-preview|previewdoh/.test(pathname)) {
      return true;
    }

    if (siteToken && new RegExp(`^${siteToken}(?:[-_.]|$)`, 'i').test(filename)) {
      return true;
    }

    return false;
  } catch {
    return false;
  }
}

function comparisonText(value: string): string {
  return cleanText(value)
    .toLowerCase()
    .replace(/^www\./, '')
    .replace(/[^a-z0-9]+/g, ' ')
    .trim();
}

export function buildFallbackNostrUrl(address: string): string {
  const naddr = naddrFromAddress(address);
  if (!naddr) return 'https://beta.highlighter.com/';
  return `https://beta.highlighter.com/note/${naddr}`;
}

export function naddrFromAddress(address: string): string | undefined {
  const parsed = parseNostrAddress(address);
  if (!parsed) return undefined;

  return nip19.naddrEncode({
    kind: parsed.kind,
    pubkey: parsed.pubkey,
    identifier: parsed.identifier
  });
}

export function parseNostrAddress(address: string): { kind: number; pubkey: string; identifier: string } | undefined {
  return parseAddress(address);
}

function firstTagValue(event: NDKEventType, tagName: string): string {
  return cleanText(event.getMatchingTags(tagName)[0]?.[1] ?? event.tagValue(tagName));
}

function resolvePrimaryReference(input: {
  aTag: string;
  eTag: string;
  iTagValue: string;
  kTag: string;
  url: string;
}):
  | {
      mode: 'nostr-article';
      value: string;
      kind: string;
    }
  | {
      mode: 'generic';
      value: string;
      catalogId: string;
      catalogKind: string;
      referenceTagName: ArtifactReferenceTagName;
      referenceKind: string;
      highlightTagName: ArtifactHighlightTagName;
      highlightTagValue: string;
    } {
  if (input.aTag) {
    const parsed = parseAddress(input.aTag);
    if (parsed?.kind === 30023) {
      return {
        mode: 'nostr-article',
        value: input.aTag,
        kind: String(parsed.kind)
      };
    }

    return {
      mode: 'generic',
      value: input.aTag,
      catalogId: input.aTag,
      catalogKind: parsed ? `nostr:${parsed.kind}` : 'nostr',
      referenceTagName: 'a',
      referenceKind: parsed ? String(parsed.kind) : '',
      highlightTagName: 'a',
      highlightTagValue: input.aTag
    };
  }

  if (input.eTag) {
    return {
      mode: 'generic',
      value: input.eTag,
      catalogId: input.eTag,
      catalogKind: 'nostr:event',
      referenceTagName: 'e',
      referenceKind: cleanText(input.kTag),
      highlightTagName: 'e',
      highlightTagValue: input.eTag
    };
  }

  if (input.iTagValue) {
    return {
      mode: 'generic',
      value: input.iTagValue,
      catalogId: input.iTagValue,
      catalogKind: cleanText(input.kTag) || inferCatalogKindFromValue(input.iTagValue),
      referenceTagName: 'i',
      referenceKind: cleanText(input.kTag) || inferCatalogKindFromValue(input.iTagValue),
      highlightTagName: 'r',
      highlightTagValue: input.url || normalizeArtifactUrl(input.iTagValue) || ''
    };
  }

  return {
    mode: 'generic',
    value: input.url,
    catalogId: input.url,
    catalogKind: 'web',
    referenceTagName: 'i',
    referenceKind: 'web',
    highlightTagName: 'r',
    highlightTagValue: input.url
  };
}

function buildShareLookupFilters(
  references: Array<{ tagName: 'a' | 'e' | 'r'; value: string }>,
  limit: number
): Array<Record<`#${string}`, string[]> & { kinds: number[]; limit: number }> {
  const addresses = uniqueValues(
    references.filter((reference) => reference.tagName === 'a').map((reference) => reference.value)
  );
  const eventIds = uniqueValues(
    references.filter((reference) => reference.tagName === 'e').map((reference) => reference.value)
  );
  const urls = uniqueValues(
    references.filter((reference) => reference.tagName === 'r').map((reference) => reference.value)
  );
  const filters: Array<Record<`#${string}`, string[]> & { kinds: number[]; limit: number }> = [];

  if (addresses.length > 0) {
    filters.push({ kinds: [HIGHLIGHTER_ARTIFACT_SHARE_KIND], '#a': addresses, limit });
  }

  if (eventIds.length > 0) {
    filters.push({ kinds: [HIGHLIGHTER_ARTIFACT_SHARE_KIND], '#e': eventIds, limit });
  }

  if (urls.length > 0) {
    filters.push({ kinds: [HIGHLIGHTER_ARTIFACT_SHARE_KIND], '#r': urls, limit });
  }

  return filters;
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

function inferCatalogKindFromValue(value: string): string {
  if (value.startsWith('isbn:')) return 'isbn';
  if (value.startsWith('doi:')) return 'doi';
  if (value.startsWith('podcast:guid:')) return 'podcast:guid';
  if (value.startsWith('podcast:item:guid:')) return 'podcast:item:guid';
  if (value.startsWith('podcast:publisher:guid:')) return 'podcast:publisher:guid';
  if (value.startsWith('spotify:episode:')) return 'spotify:episode';
  if (value.startsWith('apple:podcast-episode:')) return 'apple:podcast-episode';
  if (value.startsWith('overcast:episode:')) return 'overcast:episode';
  return 'web';
}

async function fetchNostrArticleArtifacts(
  ndk: NDK,
  addresses: string[]
): Promise<Map<string, ArtifactRecord>> {
  const parsedAddresses = uniqueValues(addresses)
    .map((address) => {
      const parsed = parseAddress(address);
      if (!parsed || parsed.kind !== 30023) return undefined;

      return { address, ...parsed };
    })
    .filter(
      (
        candidate
      ): candidate is {
        address: string;
        kind: number;
        pubkey: string;
        identifier: string;
      } => Boolean(candidate)
    );

  if (parsedAddresses.length === 0) {
    return new Map();
  }

  const identifiersByPubkey = new Map<string, Set<string>>();

  for (const { pubkey, identifier } of parsedAddresses) {
    const identifiers = identifiersByPubkey.get(pubkey) ?? new Set<string>();
    identifiers.add(identifier);
    identifiersByPubkey.set(pubkey, identifiers);
  }

  const filters = [...identifiersByPubkey.entries()].map(([pubkey, identifiers]) => ({
    kinds: [30023],
    authors: [pubkey],
    '#d': [...identifiers],
    limit: Math.max(identifiers.size * 2, 8)
  }));

  if (filters.length === 0) {
    return new Map();
  }

  const relaySet = NDKRelaySet.fromRelayUrls(DEFAULT_RELAYS, ndk);
  const articleEvents = Array.from(
    (await ndk.fetchEvents(filters, { closeOnEose: true }, relaySet)) ?? []
  ).sort((left, right) => (right.created_at ?? 0) - (left.created_at ?? 0));

  const expectedAddresses = new Set(parsedAddresses.map(({ address }) => address));
  const artifacts = new Map<string, ArtifactRecord>();

  for (const event of articleEvents) {
    const rawEvent = event.rawEvent();
    const address = eventAddress(rawEvent);

    if (!address || !expectedAddresses.has(address)) {
      continue;
    }

    const referenceKey = referenceKeyForTag('a', address);
    if (artifacts.has(referenceKey)) {
      continue;
    }

    const preview = buildNostrArticleArtifactPreview({
      event: rawEvent,
      canonicalUrl: buildFallbackNostrUrl(address)
    });

    artifacts.set(referenceKey, {
      ...preview,
      groupId: '',
      shareEventId: '',
      pubkey: event.pubkey,
      createdAt: event.created_at ?? null,
      note: ''
    });
  }

  return artifacts;
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
    return 'Untitled source';
  }
}

function podcastGuidFromCatalogValue(value: string): string {
  const normalized = cleanText(value);
  if (normalized.startsWith('podcast:guid:')) {
    return normalized.slice('podcast:guid:'.length);
  }

  return '';
}

function sanitizeDurationSeconds(value: number | string | null | undefined): number | null {
  if (typeof value === 'number') {
    return Number.isFinite(value) && value >= 0 ? Math.round(value) : null;
  }

  if (typeof value === 'string') {
    const parsed = Number(value.trim());
    return Number.isFinite(parsed) && parsed >= 0 ? Math.round(parsed) : null;
  }

  return null;
}

function domainLabel(url: string): string {
  try {
    return new URL(url).hostname.replace(/^www\./, '');
  } catch {
    return url;
  }
}

function buildFallbackUrl(value: string): string {
  const normalized = normalizeArtifactUrl(value);
  if (normalized) return normalized;
  return 'https://beta.highlighter.com/';
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

function eventAddress(event: Pick<NostrEvent, 'kind' | 'pubkey' | 'tags'>): string | undefined {
  const identifier = cleanText(event.tags.find((tag) => tag[0] === 'd')?.[1]);
  const kind = Number(event.kind ?? 0);

  if (!Number.isInteger(kind) || !event.pubkey || !identifier) {
    return undefined;
  }

  return `${kind}:${event.pubkey}:${identifier}`;
}

function parseReferenceKey(referenceKey: string): { tagName: 'a' | 'e' | 'r'; value: string } | undefined {
  const trimmed = referenceKey.trim();
  const separator = trimmed.indexOf(':');
  if (separator <= 0) return undefined;

  const tagName = trimmed.slice(0, separator);
  const value = trimmed.slice(separator + 1).trim();

  if ((tagName === 'a' || tagName === 'e' || tagName === 'r') && value) {
    return { tagName, value };
  }

  return undefined;
}

function referenceKeyForTag(tagName: string, value: string): string {
  const normalizedValue = cleanText(value);
  return normalizedValue ? `${tagName}:${normalizedValue}` : '';
}

function fnv1a(value: string): number {
  let hash = 0x811c9dc5;

  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index);
    hash = Math.imul(hash, 0x01000193);
  }

  return hash >>> 0;
}

export function authorLabel(
  profile: { displayName?: string; display_name?: string; name?: string; username?: string } | undefined,
  pubkey: string
): string {
  return displayName(profile, shortPubkey(pubkey));
}
