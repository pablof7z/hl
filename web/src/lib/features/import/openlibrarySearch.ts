/**
 * Open Library title+author → ISBN best-effort lookup.
 *
 * Uses the public Search API:
 *   https://openlibrary.org/search.json?title=<t>&author=<a>&limit=1
 *
 * Returns the first hit's ISBN-13 (preferred), title, author, and
 * cover URL. Caches in-memory by `title|author` so the same book in
 * 200 highlights only fetches once. Throttles concurrent requests with
 * a 200-300ms gap so we don't slam Open Library when resolving 50+ books.
 *
 * Best-effort. Returns `null` on no match, network failure, or timeout.
 */

export type OpenLibraryMatch = {
  isbn13?: string;
  title?: string;
  author?: string;
  coverUrl?: string;
};

const REQUEST_TIMEOUT_MS = 8000;
const MIN_REQUEST_GAP_MS = 250;

const cache = new Map<string, OpenLibraryMatch | null>();
const inflight = new Map<string, Promise<OpenLibraryMatch | null>>();

let lastRequestAt = 0;

function cacheKey(title: string, author: string | undefined): string {
  return `${title.toLowerCase().trim()}|${(author ?? '').toLowerCase().trim()}`;
}

/**
 * Look up a book by title (+optional author). Returns a match or null.
 * Resolved values are cached for the session.
 */
export async function searchOpenLibrary(
  title: string,
  author?: string
): Promise<OpenLibraryMatch | null> {
  const trimmedTitle = title.trim();
  if (!trimmedTitle) return null;

  const trimmedAuthor = author?.trim();
  const key = cacheKey(trimmedTitle, trimmedAuthor);

  if (cache.has(key)) {
    return cache.get(key) ?? null;
  }
  const existing = inflight.get(key);
  if (existing) return existing;

  const promise = throttledFetch(trimmedTitle, trimmedAuthor)
    .then((result) => {
      cache.set(key, result);
      inflight.delete(key);
      return result;
    })
    .catch((err) => {
      // Treat any unexpected error as no match — never let it crash callers.
      console.warn('[openlibrarySearch] failed', err);
      cache.set(key, null);
      inflight.delete(key);
      return null;
    });

  inflight.set(key, promise);
  return promise;
}

async function throttledFetch(
  title: string,
  author: string | undefined
): Promise<OpenLibraryMatch | null> {
  const now = Date.now();
  const wait = Math.max(0, lastRequestAt + MIN_REQUEST_GAP_MS - now);
  if (wait > 0) {
    await new Promise<void>((resolve) => setTimeout(resolve, wait));
  }
  lastRequestAt = Date.now();

  const params = new URLSearchParams({
    title,
    limit: '1',
    // Open Library's default search response omits `isbn` and trims fields
    // aggressively. Request the ones we need explicitly.
    fields: 'title,author_name,isbn,cover_i'
  });
  if (author) params.set('author', author);
  const url = `https://openlibrary.org/search.json?${params.toString()}`;

  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), REQUEST_TIMEOUT_MS);
  try {
    const res = await fetch(url, {
      signal: controller.signal,
      headers: { accept: 'application/json' }
    });
    if (!res.ok) return null;
    const body = (await res.json()) as {
      docs?: Array<{
        title?: string;
        author_name?: string[];
        isbn?: string[];
        cover_i?: number;
      }>;
    };
    const first = body.docs?.[0];
    if (!first) return null;

    const isbn13 = pickIsbn13(first.isbn);
    const result: OpenLibraryMatch = {
      isbn13,
      title: typeof first.title === 'string' ? first.title.trim() : undefined,
      author:
        Array.isArray(first.author_name) && first.author_name.length > 0
          ? first.author_name.join(', ')
          : undefined,
      coverUrl:
        typeof first.cover_i === 'number'
          ? `https://covers.openlibrary.org/b/id/${first.cover_i}-M.jpg`
          : undefined
    };
    // If there's literally nothing useful, treat as no-match.
    if (!result.isbn13 && !result.title && !result.author && !result.coverUrl) {
      return null;
    }
    return result;
  } catch {
    return null;
  } finally {
    clearTimeout(timer);
  }
}

function pickIsbn13(isbns: string[] | undefined): string | undefined {
  if (!Array.isArray(isbns) || isbns.length === 0) return undefined;
  const cleaned = isbns
    .map((value) => (typeof value === 'string' ? value.replace(/[\s-]/g, '') : ''))
    .filter(Boolean);

  const thirteen = cleaned.find((value) => /^\d{13}$/.test(value));
  if (thirteen) return thirteen;

  // Fall back to ISBN-10 → 13 conversion.
  const ten = cleaned.find((value) => /^\d{9}[\dXx]$/.test(value));
  if (ten) return convertIsbn10To13(ten);
  return undefined;
}

function convertIsbn10To13(isbn10: string): string {
  const prefix = `978${isbn10.slice(0, 9)}`;
  let sum = 0;
  for (let i = 0; i < 12; i++) {
    const d = Number(prefix[i]);
    sum += i % 2 === 0 ? d : d * 3;
  }
  const check = (10 - (sum % 10)) % 10;
  return `${prefix}${check}`;
}
