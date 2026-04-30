import type { NostrEvent } from '@nostr-dev-kit/ndk';
import { error, redirect } from '@sveltejs/kit';
import type { PageServerLoad } from './$types';
import { fetchHighlightsForIsbn, fetchProfilesByPubkeys } from '$lib/server/nostr';
import { lookupIsbn } from '$lib/server/openlibrary';
import { buildBookSeo, buildMissingSeo } from '$lib/seo';
import { slugify, withSlug } from '$lib/utils/slug';

/// Parse the trailing ISBN out of the route slug. The slug is purely
/// decorative — anything in front of the trailing 10- or 13-digit run
/// is ignored. Examples that all yield the same ISBN:
///   - `9780399178740`
///   - `the-name-of-the-rose-9780399178740`
///   - `whatever-9780399178740`
function extractIsbn(slug: string): string | undefined {
  const match = /(?:^|-)(\d{13}|\d{10})$/.exec(slug.trim());
  return match?.[1];
}

export const load: PageServerLoad = async ({ params, setHeaders, url }) => {
  setHeaders({
    'cache-control': 'public, max-age=60, s-maxage=300, stale-while-revalidate=3600'
  });

  const isbn = extractIsbn(params.slug);
  if (!isbn) {
    throw error(404, 'Not a valid book route');
  }

  let book = await lookupIsbn(isbn);
  // Guard against the lookup failing softly. We still render the page
  // with whatever ISBN-derived info we have.
  if (!book) {
    book = {
      isbn13: isbn.length === 13 ? isbn : `978${isbn.slice(0, 9)}`,
      title: '',
      author: '',
      coverUrl: `https://covers.openlibrary.org/b/isbn/${isbn}-L.jpg`,
      description: undefined,
      subjects: []
    };
  }

  // Canonical slug = `<title-slug>-<isbn13>`. If the URL doesn't match,
  // 301 to canonical so we don't fragment SEO juice across stale slugs.
  const canonicalSlug = withSlug(slugify(book.title), book.isbn13);
  if (canonicalSlug && params.slug !== canonicalSlug) {
    throw redirect(301, `/book/${canonicalSlug}${url.search}`);
  }

  const highlights = await fetchHighlightsForIsbn(book.isbn13);
  const highlightAuthors = [...new Set(highlights.map((event) => event.pubkey))];
  const profiles = highlightAuthors.length > 0
    ? await fetchProfilesByPubkeys(highlightAuthors).catch(() => ({}))
    : {};

  const safeTitle = book.title || `ISBN ${book.isbn13}`;

  return {
    missing: false,
    book: {
      isbn13: book.isbn13,
      title: book.title,
      author: book.author,
      coverUrl: book.coverUrl,
      description: book.description,
      subjects: book.subjects ?? []
    },
    highlights: highlights.map((event) => event.rawEvent() as NostrEvent),
    profiles,
    seo: buildBookSeo({
      url,
      isbn13: book.isbn13,
      title: safeTitle,
      author: book.author,
      description: book.description,
      highlightCount: highlights.length
    })
  };
};

export function _missingFallback(url: URL) {
  return { missing: true, seo: buildMissingSeo(url, 'Book not found') };
}
