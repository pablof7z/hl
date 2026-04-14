import { nip19 } from '@nostr-dev-kit/ndk';
import {
  authorLabel,
  buildArtifactPreview,
  buildNostrArticleArtifactPreview,
  detectArtifactSource,
  normalizeArtifactUrl,
  type ArtifactPreview,
  type ArtifactSource
} from '$lib/ndk/artifacts';
import { fetchNoteWithAuthor } from '$lib/server/nostr';

const FETCH_TIMEOUT_MS = 8000;
const MAX_HTML_CHARS = 250_000;

type SchemaNode = Record<string, unknown>;

type TextCandidate = {
  value: string;
  allowGeneric?: boolean;
};

type ImageCandidate = {
  value: string;
  allowGeneric?: boolean;
};

type SelectionContext = {
  siteName: string;
  responseUrl: string;
};

type SchemaMetadata = {
  title: string;
  description: string;
  image: string;
  author: string;
  isbn: string;
};

type ArtifactHtmlMetadata = SchemaMetadata & {
  canonicalUrl: string;
  ogType: string;
};

export class ArtifactPreviewInputError extends Error {}

export async function previewArtifactReference(input: {
  reference: string;
  origin: string;
  source?: ArtifactSource;
}): Promise<ArtifactPreview> {
  const rawReference = input.reference.trim();
  if (!rawReference) {
    throw new ArtifactPreviewInputError('Paste a URL or a Nostr article reference.');
  }

  try {
    const nostrIdentifier = extractNostrIdentifier(rawReference);
    if (nostrIdentifier) {
      const preview = await previewNostrArticle(nostrIdentifier, input.origin);
      return {
        ...preview,
        source: input.source ?? preview.source
      };
    }

    const normalizedUrl = normalizeArtifactUrl(rawReference);
    if (!normalizedUrl) {
      throw new ArtifactPreviewInputError('Enter a valid http(s) URL or a Nostr article link.');
    }

    const response = await fetch(normalizedUrl, {
      headers: {
        accept: 'text/html,application/xhtml+xml',
        'user-agent': 'HighlighterBot/0.2 (+https://beta.highlighter.com)'
      },
      redirect: 'follow',
      signal: AbortSignal.timeout(FETCH_TIMEOUT_MS)
    });

    const responseUrl = normalizeArtifactUrl(response.url) ?? normalizedUrl;
    const contentType = response.headers.get('content-type') ?? '';

    if (!response.ok) {
      throw new Error(`Source responded with ${response.status}.`);
    }

    if (!contentType.includes('text/html')) {
      return buildArtifactPreview({
        url: responseUrl,
        source: input.source ?? detectArtifactSource(responseUrl)
      });
    }

    const html = (await response.text()).slice(0, MAX_HTML_CHARS);
    const metadata = extractArtifactHtmlMetadata(html, responseUrl);
    const entity = detectCatalogEntity(html, metadata.canonicalUrl, metadata.isbn);
    const source = input.source ?? inferSourceFromEntity(entity.kind, metadata.canonicalUrl, metadata.ogType);

    return buildArtifactPreview({
      url: metadata.canonicalUrl,
      title: metadata.title,
      author: metadata.author,
      image: metadata.image,
      description: metadata.description,
      source,
      catalogId: entity.id,
      catalogKind: entity.kind,
      referenceTagName: 'i',
      referenceTagValue: entity.id,
      referenceKind: entity.kind,
      highlightTagName: 'r',
      highlightTagValue: metadata.canonicalUrl
    });
  } catch (error) {
    console.warn('Artifact preview fetch failed:', error);

    if (error instanceof ArtifactPreviewInputError) {
      throw error;
    }

    const normalizedUrl = normalizeArtifactUrl(rawReference);
    if (!normalizedUrl) {
      throw new Error('Could not preview that reference right now.');
    }

    return buildArtifactPreview({
      url: normalizedUrl,
      source: input.source ?? detectArtifactSource(normalizedUrl)
    });
  }
}

async function previewNostrArticle(identifier: string, origin: string) {
  const { event, profile } = await fetchNoteWithAuthor(identifier);
  if (!event) {
    throw new Error('Could not resolve that Nostr article.');
  }

  if (event.kind !== 30023) {
    throw new Error('Only long-form Nostr articles can be shared from Nostr references right now.');
  }

  return buildNostrArticleArtifactPreview({
    event: event.rawEvent(),
    canonicalUrl: noteUrlForEvent(origin, identifier, event.rawEvent()),
    authorName: authorLabel(profile, event.pubkey)
  });
}

function extractArtifactHtmlMetadata(html: string, responseUrl: string): ArtifactHtmlMetadata {
  const siteName =
    metaContent(html, 'property', 'og:site_name') ||
    metaContent(html, 'property', 'og:sitename') ||
    metaContent(html, 'name', 'application-name');
  const ogType = metaContent(html, 'property', 'og:type') || '';
  const context = { siteName, responseUrl };
  const schema = extractSchemaMetadata(html);
  const canonicalUrl =
    resolveUrl(linkHref(html, 'canonical'), responseUrl) ||
    resolveUrl(metaContent(html, 'property', 'og:url'), responseUrl) ||
    responseUrl;

  return {
    title: pickBestText(
      [
        { value: elementTextById(html, 'productTitle') },
        { value: schema.title },
        { value: metaContent(html, 'property', 'og:title') },
        { value: metaContent(html, 'name', 'twitter:title') },
        { value: metaContent(html, 'name', 'title') },
        { value: textContent(html, 'title') }
      ],
      context
    ),
    description: pickBestText(
      [
        { value: schema.description },
        { value: metaContent(html, 'property', 'og:description') },
        { value: metaContent(html, 'name', 'description') },
        { value: metaContent(html, 'name', 'twitter:description') }
      ],
      context
    ),
    image: pickBestImage(
      [
        {
          value:
            resolveUrl(attributeValueById(html, 'landingImage', 'data-old-hires'), responseUrl) ?? ''
        },
        { value: resolveUrl(schema.image, responseUrl) ?? '' },
        {
          value:
            resolveUrl(metaContent(html, 'property', 'og:image'), responseUrl) ??
            resolveUrl(metaContent(html, 'name', 'twitter:image'), responseUrl) ??
            ''
        }
      ],
      context
    ),
    author: pickBestText(
      [
        { value: schema.author, allowGeneric: true },
        { value: bylineAuthor(html), allowGeneric: true },
        { value: metaContent(html, 'name', 'author'), allowGeneric: true },
        { value: metaContent(html, 'property', 'article:author'), allowGeneric: true },
        { value: metaContent(html, 'property', 'og:article:author'), allowGeneric: true }
      ],
      context
    ),
    isbn: schema.isbn,
    canonicalUrl,
    ogType
  };
}

function extractSchemaMetadata(html: string): SchemaMetadata {
  const nodes = parseJsonLdNodes(html).sort((left, right) => schemaNodeScore(right) - schemaNodeScore(left));

  return {
    title: firstText(nodes.map(readSchemaTitle)),
    description: firstText(nodes.map(readSchemaDescription)),
    image: firstText(nodes.map(readSchemaImage)),
    author: firstText(nodes.map(readSchemaAuthor)),
    isbn: firstText(nodes.map(readSchemaIsbn))
  };
}

function parseJsonLdNodes(html: string): SchemaNode[] {
  const matches = html.matchAll(
    /<script[^>]*type=["']application\/ld\+json["'][^>]*>([\s\S]*?)<\/script>/gi
  );
  const nodes: SchemaNode[] = [];

  for (const match of matches) {
    const source = cleanJsonLdSource(match[1] ?? '');
    if (!source) continue;

    try {
      nodes.push(...flattenJsonLd(JSON.parse(source)));
    } catch {
      continue;
    }
  }

  return nodes;
}

function cleanJsonLdSource(value: string): string {
  return value.replace(/^\s*<!--/, '').replace(/-->\s*$/, '').trim();
}

function flattenJsonLd(value: unknown): SchemaNode[] {
  if (!value) return [];

  if (Array.isArray(value)) {
    return value.flatMap((item) => flattenJsonLd(item));
  }

  if (typeof value !== 'object') {
    return [];
  }

  const node = value as SchemaNode;
  const graph = flattenJsonLd(node['@graph']);
  const mainEntity = flattenJsonLd(node.mainEntity);

  return [node, ...graph, ...mainEntity];
}

function schemaNodeScore(node: SchemaNode): number {
  const types = schemaTypes(node);
  let score = 0;

  if (types.includes('book')) score += 60;
  if (types.includes('product')) score += 55;
  if (types.includes('scholarlyarticle')) score += 50;
  if (types.includes('newsarticle')) score += 48;
  if (types.includes('article')) score += 46;
  if (types.includes('podcastepisode') || types.includes('podcastseries')) score += 42;
  if (types.includes('videoobject')) score += 40;
  if (types.includes('webpage') || types.includes('website')) score -= 10;
  if (readSchemaTitle(node)) score += 8;
  if (readSchemaImage(node)) score += 6;
  if (readSchemaAuthor(node)) score += 3;

  return score;
}

function schemaTypes(node: SchemaNode): string[] {
  const value = node['@type'];
  const items = Array.isArray(value) ? value : [value];

  return items
    .map((item) => textValue(item).toLowerCase())
    .filter(Boolean)
    .map((item) => item.split('/').at(-1) ?? item);
}

function readSchemaTitle(node: SchemaNode): string {
  return firstText([textValue(node.name), textValue(node.headline), textValue(node.title)]);
}

function readSchemaDescription(node: SchemaNode): string {
  return firstText([textValue(node.description), textValue(node.abstract)]);
}

function readSchemaImage(node: SchemaNode): string {
  return firstText([
    urlValue(node.image),
    urlValue(node.thumbnailUrl),
    urlValue(node.primaryImageOfPage)
  ]);
}

function readSchemaAuthor(node: SchemaNode): string {
  return firstText([authorValue(node.author), authorValue(node.creator)]);
}

function readSchemaIsbn(node: SchemaNode): string {
  return textValue(node.isbn);
}

function urlValue(value: unknown): string {
  if (!value) return '';

  if (typeof value === 'string') {
    return normalizeWhitespace(decodeHtml(value));
  }

  if (Array.isArray(value)) {
    return firstText(value.map((item) => urlValue(item)));
  }

  if (typeof value !== 'object') {
    return '';
  }

  const record = value as SchemaNode;
  return firstText([
    textValue(record.url),
    textValue(record.contentUrl),
    textValue(record['@id'])
  ]);
}

function authorValue(value: unknown): string {
  if (!value) return '';

  if (typeof value === 'string') {
    return normalizeWhitespace(decodeHtml(value));
  }

  if (Array.isArray(value)) {
    return firstText(value.map((item) => authorValue(item)));
  }

  if (typeof value !== 'object') {
    return '';
  }

  const record = value as SchemaNode;
  return firstText([
    textValue(record.name),
    textValue(record.alternateName),
    textValue(record['@id'])
  ]);
}

function textValue(value: unknown): string {
  if (typeof value === 'string') {
    return normalizeWhitespace(decodeHtml(value));
  }

  if (typeof value === 'number') {
    return String(value);
  }

  return '';
}

function bylineAuthor(html: string): string {
  const patterns = [
    /<a[^>]*href=["'][^"']*dp_byline[^"']*["'][^>]*>([\s\S]*?)<\/a>/i,
    /<a[^>]*href=["'][^"']*\/e\/[^"']*["'][^>]*>([\s\S]*?)<\/a>/i,
    /<a[^>]*class=["'][^"']*contributorNameID[^"']*["'][^>]*>([\s\S]*?)<\/a>/i
  ];

  for (const pattern of patterns) {
    const match = pattern.exec(html);
    const value = normalizeWhitespace(stripHtml(match?.[1] ?? ''));
    if (value) {
      return decodeHtml(value);
    }
  }

  return '';
}

function elementTextById(html: string, id: string): string {
  const escaped = escapeRegex(id);
  const match = new RegExp(`<[^>]*id=["']${escaped}["'][^>]*>([\\s\\S]*?)</[^>]+>`, 'i').exec(html);
  return normalizeWhitespace(stripHtml(match?.[1] ?? ''));
}

function attributeValueById(html: string, id: string, attribute: string): string {
  const escapedId = escapeRegex(id);
  const escapedAttribute = escapeRegex(attribute);
  const patterns = [
    new RegExp(
      `<[^>]*id=["']${escapedId}["'][^>]*${escapedAttribute}=["']([^"']+)["'][^>]*>`,
      'i'
    ),
    new RegExp(
      `<[^>]*${escapedAttribute}=["']([^"']+)["'][^>]*id=["']${escapedId}["'][^>]*>`,
      'i'
    )
  ];

  for (const pattern of patterns) {
    const match = pattern.exec(html);
    if (match?.[1]) {
      return decodeHtml(match[1]);
    }
  }

  return '';
}

function pickBestText(candidates: TextCandidate[], context: SelectionContext): string {
  const cleaned = candidates
    .map((candidate) => ({
      value: normalizeWhitespace(candidate.value),
      allowGeneric: candidate.allowGeneric ?? false
    }))
    .filter((candidate) => Boolean(candidate.value));

  for (const candidate of cleaned) {
    if (candidate.allowGeneric || !isLikelyGenericText(candidate.value, context)) {
      return candidate.value;
    }
  }

  return cleaned[0]?.value ?? '';
}

function pickBestImage(candidates: ImageCandidate[], context: SelectionContext): string {
  const cleaned = candidates
    .map((candidate) => ({
      value: normalizeWhitespace(candidate.value),
      allowGeneric: candidate.allowGeneric ?? false
    }))
    .filter((candidate) => Boolean(candidate.value));

  for (const candidate of cleaned) {
    if (candidate.allowGeneric || !isLikelyGenericImage(candidate.value, context)) {
      return candidate.value;
    }
  }

  return cleaned[0]?.value ?? '';
}

function isLikelyGenericText(value: string, context: SelectionContext): boolean {
  const normalizedValue = comparisonText(value);
  const normalizedSiteName = comparisonText(context.siteName);
  const normalizedHost = comparisonText(hostnameLabel(context.responseUrl));

  if (!normalizedValue) return true;
  if (normalizedSiteName && normalizedValue === normalizedSiteName) return true;
  if (normalizedHost && normalizedValue === normalizedHost) return true;

  return false;
}

function isLikelyGenericImage(value: string, context: SelectionContext): boolean {
  try {
    const parsed = new URL(value, context.responseUrl);
    const pathname = parsed.pathname.toLowerCase();
    const filename = pathname.split('/').at(-1) ?? '';
    const siteToken = comparisonText(context.siteName).replace(/\s+/g, '');

    if (/favicon|apple-touch|sprite|logo|icon/.test(pathname)) {
      return true;
    }

    if (/share-icons|social-preview|previewdoh/.test(pathname)) {
      return true;
    }

    if (siteToken && new RegExp(`^${escapeRegex(siteToken)}(?:[-_.]|$)`, 'i').test(filename)) {
      return true;
    }

    return false;
  } catch {
    return false;
  }
}

function firstText(values: string[]): string {
  for (const value of values) {
    const normalized = normalizeWhitespace(value);
    if (normalized) return normalized;
  }

  return '';
}

function normalizeWhitespace(value: string): string {
  return value.replace(/\s+/g, ' ').trim();
}

function stripHtml(value: string): string {
  return decodeHtml(value.replace(/<[^>]+>/g, ' '));
}

function comparisonText(value: string): string {
  return normalizeWhitespace(value)
    .toLowerCase()
    .replace(/^www\./, '')
    .replace(/[^a-z0-9]+/g, ' ')
    .trim();
}

function hostnameLabel(url: string): string {
  try {
    return new URL(url).hostname.replace(/^www\./, '');
  } catch {
    return url;
  }
}

function noteUrlForEvent(
  origin: string,
  identifier: string,
  event: { kind?: number; pubkey: string; tags: string[][] }
): string {
  const trimmedIdentifier = identifier.trim();
  if (/^(naddr1|nevent1|note1)/i.test(trimmedIdentifier)) {
    return `${origin}/note/${trimmedIdentifier}`;
  }

  const dTag = event.tags.find((tag) => tag[0] === 'd')?.[1]?.trim();
  if (event.kind && dTag) {
    const naddr = nip19.naddrEncode({
      kind: event.kind,
      pubkey: event.pubkey,
      identifier: dTag
    });

    return `${origin}/note/${naddr}`;
  }

  return `${origin}/note/${trimmedIdentifier}`;
}

function extractNostrIdentifier(value: string): string | null {
  const trimmed = value.trim();
  if (!trimmed) return null;

  const nostrPrefix = trimmed.replace(/^nostr:/i, '');
  if (/^(naddr1|nevent1|note1)/i.test(nostrPrefix)) {
    return nostrPrefix;
  }

  const normalizedUrl = normalizeArtifactUrl(trimmed);
  if (!normalizedUrl) {
    return null;
  }

  try {
    const parsed = new URL(normalizedUrl);
    const parts = parsed.pathname.split('/').filter(Boolean);
    if (parts[0] === 'note' && parts[1]) {
      return parts[1];
    }
  } catch {
    return null;
  }

  return null;
}

function detectCatalogEntity(
  html: string,
  canonicalUrl: string,
  schemaIsbn: string
): { id: string; kind: string } {
  const isbn = extractIsbn(html, canonicalUrl, schemaIsbn);
  if (isbn) {
    return {
      id: `isbn:${isbn}`,
      kind: 'isbn'
    };
  }

  const doi = extractDoi(html, canonicalUrl);
  if (doi) {
    return {
      id: `doi:${doi}`,
      kind: 'doi'
    };
  }

  const podcastGuid = extractPodcastGuid(html);
  if (podcastGuid) {
    return {
      id: `podcast:guid:${podcastGuid}`,
      kind: 'podcast:guid'
    };
  }

  return {
    id: canonicalUrl,
    kind: 'web'
  };
}

function inferSourceFromEntity(
  entityKind: string,
  url: string,
  hint: string
): 'article' | 'book' | 'podcast' | 'video' | 'paper' | 'web' {
  if (entityKind === 'isbn') return 'book';
  if (entityKind === 'doi') return 'paper';
  if (entityKind.startsWith('podcast:')) return 'podcast';
  return detectArtifactSource(url, hint);
}

function metaContent(
  html: string,
  attribute: 'name' | 'property' | 'itemprop',
  value: string
): string {
  const escaped = escapeRegex(value);
  const patterns = [
    new RegExp(`<meta[^>]*${attribute}=["']${escaped}["'][^>]*content=["']([^"']+)["'][^>]*>`, 'i'),
    new RegExp(`<meta[^>]*content=["']([^"']+)["'][^>]*${attribute}=["']${escaped}["'][^>]*>`, 'i')
  ];

  for (const pattern of patterns) {
    const match = pattern.exec(html);
    if (match?.[1]) {
      return decodeHtml(match[1]);
    }
  }

  return '';
}

function linkHref(html: string, rel: string): string {
  const escaped = escapeRegex(rel);
  const patterns = [
    new RegExp(`<link[^>]*rel=["'][^"']*${escaped}[^"']*["'][^>]*href=["']([^"']+)["'][^>]*>`, 'i'),
    new RegExp(`<link[^>]*href=["']([^"']+)["'][^>]*rel=["'][^"']*${escaped}[^"']*["'][^>]*>`, 'i')
  ];

  for (const pattern of patterns) {
    const match = pattern.exec(html);
    if (match?.[1]) {
      return decodeHtml(match[1]);
    }
  }

  return '';
}

function textContent(html: string, tag: string): string {
  const match = new RegExp(`<${tag}[^>]*>([\\s\\S]*?)</${tag}>`, 'i').exec(html);
  return match?.[1] ? normalizeWhitespace(stripHtml(match[1])) : '';
}

function extractIsbn(html: string, url: string, schemaIsbn: string): string {
  const candidates = [
    schemaIsbn,
    metaContent(html, 'itemprop', 'isbn'),
    metaContent(html, 'name', 'isbn'),
    textContent(html, 'body'),
    url
  ];

  for (const candidate of candidates) {
    const match = /(97[89][0-9]{10}|[0-9]{9}[0-9xX])/i.exec(candidate.replace(/[\s-]+/g, ''));
    if (!match?.[1]) continue;

    return match[1].toUpperCase();
  }

  return '';
}

function extractDoi(html: string, url: string): string {
  const candidates = [
    metaContent(html, 'name', 'citation_doi'),
    metaContent(html, 'property', 'og:doi'),
    url,
    textContent(html, 'body')
  ];

  for (const candidate of candidates) {
    const match = /(10\.\d{4,9}\/[-._;()/:a-z0-9]+)/i.exec(candidate);
    if (!match?.[1]) continue;

    return match[1].toLowerCase();
  }

  return '';
}

function extractPodcastGuid(html: string): string {
  const candidates = [
    metaContent(html, 'name', 'podcast:guid'),
    metaContent(html, 'property', 'podcast:guid'),
    metaContent(html, 'name', 'itunes:new-feed-url'),
    textContent(html, 'body')
  ];

  for (const candidate of candidates) {
    const match =
      /podcast(?::item)?:guid:([a-z0-9-]{8,})/i.exec(candidate) ||
      /\b([0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})\b/i.exec(candidate);

    if (!match?.[1]) continue;
    return match[1];
  }

  return '';
}

function resolveUrl(value: string, base: string): string | null {
  if (!value.trim()) return null;

  try {
    return new URL(value, base).toString();
  } catch {
    return null;
  }
}

function decodeHtml(value: string): string {
  return value
    .replace(/&amp;/g, '&')
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&uuml;/g, 'ü')
    .replace(/&Uuml;/g, 'Ü')
    .replace(/&ouml;/g, 'ö')
    .replace(/&Ouml;/g, 'Ö')
    .replace(/&auml;/g, 'ä')
    .replace(/&Auml;/g, 'Ä')
    .replace(/&nbsp;/g, ' ');
}

function escapeRegex(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}
