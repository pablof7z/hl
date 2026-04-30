import type { RequestHandler } from './$types';
import { fetchHighlightWithAuthor } from '$lib/server/nostr';
import { renderHighlightOgImage } from '$lib/server/og';

const CACHE_CONTROL = 'public, max-age=300, s-maxage=86400, stale-while-revalidate=604800';

export const GET: RequestHandler = async ({ params, url }) => {
  try {
    const { event, author, profile, sourceTitle, source } = await fetchHighlightWithAuthor(
      params.nevent
    );
    const image = await renderHighlightOgImage({
      event: event?.rawEvent(),
      authorPubkey: author?.pubkey,
      profile,
      sourceTitle: sourceTitle ?? source?.title
    });

    return new Response(new Uint8Array(image), {
      headers: {
        'cache-control': CACHE_CONTROL,
        'content-type': 'image/png'
      }
    });
  } catch (error) {
    console.warn('Failed to render highlight OG image', error);
    return Response.redirect(new URL('/og-default.png', url), 307);
  }
};
