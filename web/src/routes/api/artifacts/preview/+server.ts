import { json, type RequestHandler } from '@sveltejs/kit';
import { nip19 } from '@nostr-dev-kit/ndk';
import {
  authorLabel,
  buildArtifactPreview,
  buildNostrArticleArtifactPreview,
  detectArtifactSource,
  normalizeArtifactUrl
} from '$lib/ndk/artifacts';
import { fetchNoteWithAuthor } from '$lib/server/nostr';

const FETCH_TIMEOUT_MS = 8000;
const MAX_HTML_CHARS = 250_000;

export const POST: RequestHandler = async ({ request, url }) => {
  let body: unknown;

  try {
    body = await request.json();
  } catch {
    return json({ error: 'Invalid JSON.' }, { status: 400 });
  }

  const rawReference =
    typeof body === 'object' && body && 'reference' in body
      ? String((body as { reference: unknown }).reference ?? '')
      : typeof body === 'object' && body && 'url' in body
        ? String((body as { url: unknown }).url ?? '')
        : '';
  const requestedSource =
    typeof body === 'object' && body && 'source' in body
      ? parseSource(String((body as { source: unknown }).source ?? ''))
      : undefined;

  if (!rawReference.trim()) {
    return json({ error: 'Paste a URL or a Nostr article reference.' }, { status: 400 });
  }

  try {
    const nostrIdentifier = extractNostrIdentifier(rawReference);
    if (nostrIdentifier) {
      const preview = await previewNostrArticle(nostrIdentifier, url.origin);

      return json({
        ...preview,
        source: requestedSource ?? preview.source
      });
    }

    const normalizedUrl = normalizeArtifactUrl(rawReference);
    if (!normalizedUrl) {
      return json({ error: 'Enter a valid http(s) URL or a Nostr article link.' }, { status: 400 });
    }

    const response = await fetch(normalizedUrl, {
      headers: {
        accept: 'text/html,application/xhtml+xml',
        'user-agent': 'HighlighterBot/0.2 (+https://highlighter.f7z.io)'
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
      return json(
        buildArtifactPreview({
          url: responseUrl,
          source: requestedSource ?? detectArtifactSource(responseUrl)
        })
      );
    }

    const html = (await response.text()).slice(0, MAX_HTML_CHARS);
    const title =
      metaContent(html, 'property', 'og:title') ||
      metaContent(html, 'name', 'twitter:title') ||
      textContent(html, 'title');
    const description =
      metaContent(html, 'property', 'og:description') ||
      metaContent(html, 'name', 'description') ||
      metaContent(html, 'name', 'twitter:description');
    const image =
      resolveUrl(
        metaContent(html, 'property', 'og:image') || metaContent(html, 'name', 'twitter:image'),
        responseUrl
      ) ?? '';
    const author =
      metaContent(html, 'name', 'author') ||
      metaContent(html, 'property', 'article:author') ||
      metaContent(html, 'property', 'og:article:author');
    const canonicalUrl =
      resolveUrl(linkHref(html, 'canonical'), responseUrl) ||
      resolveUrl(metaContent(html, 'property', 'og:url'), responseUrl) ||
      responseUrl;
    const ogType = metaContent(html, 'property', 'og:type') || '';
    const entity = detectCatalogEntity(html, canonicalUrl);
    const source = requestedSource ?? inferSourceFromEntity(entity.kind, canonicalUrl, ogType);

    return json(
      buildArtifactPreview({
        url: canonicalUrl,
        title,
        author,
        image,
        description,
        source,
        catalogId: entity.id,
        catalogKind: entity.kind,
        referenceTagName: 'i',
        referenceTagValue: entity.id,
        referenceKind: entity.kind,
        highlightTagName: 'r',
        highlightTagValue: canonicalUrl
      })
    );
  } catch (error) {
    console.warn('Artifact preview fetch failed:', error);

    const normalizedUrl = normalizeArtifactUrl(rawReference);

    try {
      if (normalizedUrl) {
        return json(
          buildArtifactPreview({
            url: normalizedUrl,
            source: requestedSource ?? detectArtifactSource(normalizedUrl)
          })
        );
      }

      return json({ error: 'Could not preview that reference right now.' }, { status: 502 });
    } catch {
      return json({ error: 'Could not preview that reference right now.' }, { status: 502 });
    }
  }
};

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

function detectCatalogEntity(html: string, canonicalUrl: string): { id: string; kind: string } {
  const isbn = extractIsbn(html, canonicalUrl);
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

function inferSourceFromEntity(entityKind: string, url: string, hint: string): 'article' | 'book' | 'podcast' | 'video' | 'paper' | 'web' {
  if (entityKind === 'isbn') return 'book';
  if (entityKind === 'doi') return 'paper';
  if (entityKind.startsWith('podcast:')) return 'podcast';
  return detectArtifactSource(url, hint);
}

function parseSource(value: string): 'article' | 'book' | 'podcast' | 'video' | 'paper' | 'web' | undefined {
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

function metaContent(html: string, attribute: 'name' | 'property' | 'itemprop', value: string): string {
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
  return match?.[1] ? decodeHtml(match[1].replace(/\s+/g, ' ').trim()) : '';
}

function extractIsbn(html: string, url: string): string {
  const candidates = [
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
    .replace(/&gt;/g, '>');
}

function escapeRegex(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}
