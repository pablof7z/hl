/// Open Library ISBN → book metadata. Free, public-domain, no API key.
/// Mirrors the Rust core's `isbn_lookup` (app/core/src/isbn_lookup.rs)
/// so SSR for highlights with `i isbn:…` tags can render a real book
/// card (title + author + cover) instead of a generic placeholder.

const OPEN_LIBRARY_TIMEOUT_MS = 8000;
// Some hosts (notably Vercel's serverless runtime egress) get rejected
// or rate-limited without a UA. Identify ourselves explicitly.
const USER_AGENT = 'Highlighter/0.1 (+https://beta.highlighter.com)';

async function fetchOpenLibraryJson(url: string, signal: AbortSignal): Promise<unknown | undefined> {
  const res = await fetch(url, {
    signal,
    headers: { accept: 'application/json', 'user-agent': USER_AGENT },
    redirect: 'follow'
  });
  if (!res.ok) return undefined;
  return res.json();
}

export type BookMetadata = {
  isbn13: string;
  title: string;
  author: string;
  coverUrl: string;
};

export async function lookupIsbn(rawIsbn: string): Promise<BookMetadata | undefined> {
  const isbn13 = normalizeIsbn(rawIsbn);
  if (!isbn13) return undefined;

  // Cover URL is always available from the ISBN-keyed endpoint, so this
  // gives us a working card even when the book record itself is absent.
  const fallback: BookMetadata = {
    isbn13,
    title: '',
    author: '',
    coverUrl: `https://covers.openlibrary.org/b/isbn/${isbn13}-L.jpg`
  };

  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), OPEN_LIBRARY_TIMEOUT_MS);
  try {
    const body = (await fetchOpenLibraryJson(
      `https://openlibrary.org/isbn/${isbn13}.json`,
      controller.signal
    )) as Record<string, unknown> | undefined;
    if (!body) {
      console.warn('[lookupIsbn] empty body for', isbn13);
      return fallback;
    }

    const title = typeof body.title === 'string' ? body.title.trim() : '';
    const coverIds = Array.isArray((body as { covers?: unknown[] }).covers)
      ? ((body as { covers: unknown[] }).covers.find((id) => typeof id === 'number') as
          | number
          | undefined)
      : undefined;
    const coverUrl = coverIds
      ? `https://covers.openlibrary.org/b/id/${coverIds}-L.jpg`
      : fallback.coverUrl;

    const authorRefs = Array.isArray((body as { authors?: unknown[] }).authors)
      ? ((body as { authors: unknown[] }).authors as Array<{ key?: string }>).map(
          (entry) => entry.key
        ).filter((k): k is string => typeof k === 'string')
      : [];
    const author = await resolveAuthors(authorRefs);

    return { isbn13, title: title || fallback.title, author, coverUrl };
  } catch {
    return fallback;
  } finally {
    clearTimeout(timer);
  }
}

/// Resolve up to two author refs (`/authors/OLxxxA`) to display names.
/// Best-effort — bad refs are silently dropped.
async function resolveAuthors(refs: string[]): Promise<string> {
  if (refs.length === 0) return '';
  const sliced = refs.slice(0, 2);
  const names = await Promise.all(
    sliced.map(async (ref) => {
      const trimmed = ref.replace(/^\/+/, '').trim();
      if (!trimmed) return '';
      const controller = new AbortController();
      const timer = setTimeout(() => controller.abort(), OPEN_LIBRARY_TIMEOUT_MS);
      try {
        const body = (await fetchOpenLibraryJson(
          `https://openlibrary.org/${trimmed}.json`,
          controller.signal
        )) as { name?: string } | undefined;
        return typeof body?.name === 'string' ? body.name.trim() : '';
      } catch {
        return '';
      } finally {
        clearTimeout(timer);
      }
    })
  );
  return names.filter(Boolean).join(', ');
}

/// Strip dashes / whitespace, validate, and canonicalise to ISBN-13.
function normalizeIsbn(raw: string): string | undefined {
  const digits = raw.replace(/[\s-]/g, '');

  if (/^\d{13}$/.test(digits)) return digits;

  if (/^\d{9}[\dXx]$/.test(digits)) {
    const prefix = `978${digits.slice(0, 9)}`;
    let sum = 0;
    for (let i = 0; i < 12; i++) {
      const d = Number(prefix[i]);
      sum += i % 2 === 0 ? d : d * 3;
    }
    const check = (10 - (sum % 10)) % 10;
    return `${prefix}${check}`;
  }

  return undefined;
}

/// Extract the first `url …` value from a NIP-92 `imeta` tag on the
/// event. Tag shape: `["imeta", "url <url>", "m <mime>", …]`. Returns
/// `undefined` when the event has no imeta or no url.
export function imetaImageUrl(tags: ReadonlyArray<string[]> | undefined): string | undefined {
  if (!tags) return undefined;
  for (const tag of tags) {
    if (tag[0] !== 'imeta') continue;
    for (let i = 1; i < tag.length; i++) {
      const part = tag[i];
      if (typeof part !== 'string') continue;
      if (part.startsWith('url ')) {
        const url = part.slice(4).trim();
        if (url) return url;
      }
    }
  }
  return undefined;
}
