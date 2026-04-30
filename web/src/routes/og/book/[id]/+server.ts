import type { RequestHandler } from './$types';
import { fetchHighlightsForIsbn } from '$lib/server/nostr';
import { lookupIsbn } from '$lib/server/openlibrary';
import { renderBookOgImage } from '$lib/server/og';

const CACHE_CONTROL = 'public, max-age=300, s-maxage=86400, stale-while-revalidate=604800';

/// `/og/book/<id>` — `<id>` is a 10- or 13-digit ISBN. We accept either
/// form and let `lookupIsbn` canonicalise to ISBN-13 before rendering.
export const GET: RequestHandler = async ({ params, url }) => {
  const isbnRaw = (params.id ?? '').trim();
  if (!/^\d{10}$|^\d{13}$/.test(isbnRaw)) {
    return Response.redirect(new URL('/og-default.png', url), 307);
  }

  try {
    const book = await lookupIsbn(isbnRaw);
    const isbn13 = book?.isbn13 ?? (isbnRaw.length === 13 ? isbnRaw : `978${isbnRaw.slice(0, 9)}`);
    const highlights = await fetchHighlightsForIsbn(isbn13).catch(() => []);

    const image = await renderBookOgImage({
      isbn13,
      title: book?.title ?? `ISBN ${isbn13}`,
      author: book?.author ?? '',
      description: book?.description,
      highlightCount: highlights.length
    });

    return new Response(new Uint8Array(image), {
      headers: {
        'cache-control': CACHE_CONTROL,
        'content-type': 'image/png'
      }
    });
  } catch (error) {
    console.warn('Failed to render book OG image', error);
    return Response.redirect(new URL('/og-default.png', url), 307);
  }
};
