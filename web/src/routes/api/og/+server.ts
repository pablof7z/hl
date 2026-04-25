import { json, error } from '@sveltejs/kit';
import type { RequestHandler } from './$types';

const MAX_BYTES = 1_048_576; // 1 MB
const FETCH_TIMEOUT_MS = 5_000;

export type OgMeta = {
  title: string;
  description: string;
  image: string;
  siteName: string;
  byline: string;
  canonicalUrl: string;
};

export const GET: RequestHandler = async ({ url }) => {
  const rawUrl = url.searchParams.get('url') ?? '';

  let target: URL;
  try {
    target = new URL(rawUrl);
  } catch {
    throw error(400, 'Invalid URL');
  }

  if (target.protocol !== 'http:' && target.protocol !== 'https:') {
    throw error(400, 'Only HTTP and HTTPS URLs are supported');
  }

  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), FETCH_TIMEOUT_MS);

  let html: string;
  let finalUrl: string;

  try {
    const response = await fetch(target.toString(), {
      signal: controller.signal,
      redirect: 'follow',
      headers: {
        'user-agent': 'Highlighter/1.0 (link-preview; +https://highlighter.com)',
        accept: 'text/html,application/xhtml+xml'
      }
    });

    finalUrl = response.url || target.toString();

    const contentType = response.headers.get('content-type') ?? '';
    if (!contentType.includes('text/html') && !contentType.includes('application/xhtml')) {
      throw error(422, 'URL does not return an HTML page');
    }

    const reader = response.body?.getReader();
    if (!reader) throw error(502, 'Could not read response body');

    const chunks: Uint8Array[] = [];
    let totalBytes = 0;

    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      totalBytes += value.byteLength;
      if (totalBytes > MAX_BYTES) {
        reader.cancel();
        break;
      }
      chunks.push(value);
    }

    html = new TextDecoder().decode(
      chunks.reduce((acc, chunk) => {
        const merged = new Uint8Array(acc.byteLength + chunk.byteLength);
        merged.set(acc);
        merged.set(chunk, acc.byteLength);
        return merged;
      }, new Uint8Array(0))
    );
  } catch (fetchError) {
    if (fetchError && typeof fetchError === 'object' && 'status' in fetchError) {
      throw fetchError;
    }
    throw error(502, 'Could not fetch the URL');
  } finally {
    clearTimeout(timer);
  }

  const meta = parseOgMeta(html, finalUrl);

  return json(meta, {
    headers: {
      'cache-control': 'public, max-age=300, s-maxage=3600, stale-while-revalidate=86400'
    }
  });
};

function parseOgMeta(html: string, finalUrl: string): OgMeta {
  // Only scan the <head> section for performance
  const headMatch = html.match(/<head[\s>][\s\S]*?<\/head>/i);
  const head = headMatch ? headMatch[0] : html.slice(0, 8192);

  function metaContent(property: string): string {
    // match property="…" or name="…" with content="…"
    const re = new RegExp(
      `<meta[^>]+(?:property|name)=["']${escapeRegex(property)}["'][^>]*content=["']([^"']*?)["']`,
      'i'
    );
    const match = head.match(re);
    if (match) return decode(match[1].trim());

    // also try reversed attribute order: content="…" property="…"
    const re2 = new RegExp(
      `<meta[^>]+content=["']([^"']*?)["'][^>]*(?:property|name)=["']${escapeRegex(property)}["']`,
      'i'
    );
    const match2 = head.match(re2);
    return match2 ? decode(match2[1].trim()) : '';
  }

  function linkHref(rel: string): string {
    const re = new RegExp(
      `<link[^>]+rel=["']${escapeRegex(rel)}["'][^>]*href=["']([^"']*?)["']`,
      'i'
    );
    const match = head.match(re);
    if (match) return decode(match[1].trim());

    const re2 = new RegExp(
      `<link[^>]+href=["']([^"']*?)["'][^>]*rel=["']${escapeRegex(rel)}["']`,
      'i'
    );
    const match2 = head.match(re2);
    return match2 ? decode(match2[1].trim()) : '';
  }

  const title =
    metaContent('og:title') ||
    metaContent('twitter:title') ||
    (head.match(/<title[^>]*>([^<]*)<\/title>/i)?.[1]?.trim() ?? '');

  const description =
    metaContent('og:description') ||
    metaContent('twitter:description') ||
    metaContent('description') ||
    firstParagraph(html);

  const image =
    metaContent('og:image') ||
    metaContent('og:image:url') ||
    metaContent('twitter:image') ||
    metaContent('twitter:image:src');

  const siteName = metaContent('og:site_name') || metaContent('application-name');

  const byline =
    metaContent('author') ||
    metaContent('article:author') ||
    metaContent('twitter:creator');

  const canonicalUrl =
    linkHref('canonical') ||
    metaContent('og:url') ||
    finalUrl;

  return {
    title: decode(title),
    description: decode(description).slice(0, 500),
    image: resolveUrl(image, finalUrl),
    siteName: decode(siteName),
    byline: decode(byline),
    canonicalUrl: canonicalUrl || finalUrl
  };
}

function firstParagraph(html: string): string {
  // scan body for first non-empty <p>
  const bodyMatch = html.match(/<body[\s>][\s\S]*/i);
  const body = bodyMatch ? bodyMatch[0] : html;
  const matches = body.matchAll(/<p(?:\s[^>]*)?>([\s\S]*?)<\/p>/gi);

  for (const match of matches) {
    const text = stripTags(match[1]).trim();
    if (text.length > 40) return text.slice(0, 300);
  }

  return '';
}

function stripTags(html: string): string {
  return html.replace(/<[^>]*>/g, ' ').replace(/\s+/g, ' ').trim();
}

function resolveUrl(maybeUrl: string, base: string): string {
  if (!maybeUrl) return '';
  try {
    return new URL(maybeUrl, base).toString();
  } catch {
    return maybeUrl;
  }
}

function escapeRegex(value: string): string {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function decode(value: string): string {
  return value
    .replace(/&amp;/gi, '&')
    .replace(/&lt;/gi, '<')
    .replace(/&gt;/gi, '>')
    .replace(/&quot;/gi, '"')
    .replace(/&#039;/gi, "'")
    .replace(/&#(\d+);/gi, (_, code) => String.fromCodePoint(Number(code)))
    .replace(/&apos;/gi, "'");
}
