import type { RequestHandler } from './$types';
import { fetchPodcastDetailById } from '$lib/server/podcast-highlights';
import { renderPodcastOgImage } from '$lib/server/og';

const CACHE_CONTROL = 'public, max-age=300, s-maxage=86400, stale-while-revalidate=604800';

/// `/og/podcast/<id>` — `<id>` is the 12-char hex hash of the canonical
/// `i podcast:[item:]guid:<guid>` tag value. We accept the bare id only
/// (slug is purely decorative on the page route, not the OG route).
export const GET: RequestHandler = async ({ params, url }) => {
  const id = (params.id ?? '').trim().toLowerCase();
  if (!/^[0-9a-f]{12}$/.test(id)) {
    return Response.redirect(new URL('/og-default.png', url), 307);
  }

  try {
    const detail = await fetchPodcastDetailById(id);
    const image = await renderPodcastOgImage({
      id,
      episodeTitle: detail?.episodeTitle ?? '',
      showTitle: detail?.showTitle ?? '',
      highlightCount: detail?.highlights.length ?? 0
    });

    return new Response(new Uint8Array(image), {
      headers: {
        'cache-control': CACHE_CONTROL,
        'content-type': 'image/png'
      }
    });
  } catch (error) {
    console.warn('Failed to render podcast OG image', error);
    return Response.redirect(new URL('/og-default.png', url), 307);
  }
};
