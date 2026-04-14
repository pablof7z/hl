import { json, type RequestHandler } from '@sveltejs/kit';
import { buildArtifactPreview, detectArtifactSource, normalizeArtifactUrl } from '$lib/ndk/artifacts';

const FETCH_TIMEOUT_MS = 8000;
const MAX_HTML_CHARS = 250_000;

export const POST: RequestHandler = async ({ request }) => {
  let body: unknown;

  try {
    body = await request.json();
  } catch {
    return json({ error: 'Invalid JSON.' }, { status: 400 });
  }

  const rawUrl = typeof body === 'object' && body && 'url' in body ? String((body as { url: unknown }).url ?? '') : '';
  const normalizedUrl = normalizeArtifactUrl(rawUrl);

  if (!normalizedUrl) {
    return json({ error: 'Enter a valid http or https URL.' }, { status: 400 });
  }

  try {
    const response = await fetch(normalizedUrl, {
      headers: {
        accept: 'text/html,application/xhtml+xml',
        'user-agent': 'HighlighterBot/0.1 (+https://highlighter.f7z.io)'
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
      return json(buildArtifactPreview({ url: responseUrl }));
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

    return json(
      buildArtifactPreview({
        url: canonicalUrl,
        title,
        author,
        image,
        description,
        source: detectArtifactSource(canonicalUrl, ogType)
      })
    );
  } catch (error) {
    console.warn('Artifact preview fetch failed:', error);

    try {
      return json(buildArtifactPreview({ url: normalizedUrl }));
    } catch {
      return json({ error: 'Could not preview that URL right now.' }, { status: 502 });
    }
  }
};

function metaContent(html: string, attribute: 'name' | 'property', value: string): string {
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
