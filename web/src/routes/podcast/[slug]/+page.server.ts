import type { NostrEvent } from '@nostr-dev-kit/ndk';
import { error, redirect } from '@sveltejs/kit';
import type { PageServerLoad } from './$types';
import { fetchProfilesByPubkeys } from '$lib/server/nostr';
import { fetchPodcastDetailById } from '$lib/server/podcast-highlights';
import { buildMissingSeo, buildPodcastSeo } from '$lib/seo';
import { slugify, withSlug } from '$lib/utils/slug';

/// Trailing 12 hex chars in the slug = the podcast id (sha256 of the
/// canonical `i podcast:[item:]guid:<guid>` tag value, truncated). Slug
/// in front is decorative.
function extractPodcastId(slug: string): string | undefined {
  const match = /(?:^|-)([0-9a-f]{12})$/i.exec(slug.trim());
  return match?.[1]?.toLowerCase();
}

export const load: PageServerLoad = async ({ params, setHeaders, url }) => {
  setHeaders({
    'cache-control': 'public, max-age=60, s-maxage=300, stale-while-revalidate=3600'
  });

  const id = extractPodcastId(params.slug);
  if (!id) {
    throw error(404, 'Not a valid podcast route');
  }

  const detail = await fetchPodcastDetailById(id);

  if (!detail) {
    return {
      missing: true,
      id,
      seo: buildMissingSeo(url, 'Podcast episode not found')
    };
  }

  // Canonical slug = `<title-slug>-<id>`. If the URL is stale, 301.
  const canonicalSlug = withSlug(
    slugify(detail.episodeTitle || detail.showTitle || ''),
    detail.components.id
  );
  if (params.slug !== canonicalSlug) {
    throw redirect(301, `/podcast/${canonicalSlug}${url.search}`);
  }

  const highlightAuthors = [...new Set(detail.highlights.map((event) => event.pubkey))];
  const profiles = highlightAuthors.length > 0
    ? await fetchProfilesByPubkeys(highlightAuthors).catch(() => ({}))
    : {};

  return {
    missing: false,
    id: detail.components.id,
    podcast: {
      id: detail.components.id,
      iTagValue: detail.components.iTagValue,
      guid: detail.components.guid,
      scope: detail.components.scope,
      episodeTitle: detail.episodeTitle ?? '',
      showTitle: detail.showTitle ?? '',
      imageUrl: detail.imageUrl ?? '',
      audioUrl: detail.audioUrl ?? '',
      durationSeconds: detail.durationSeconds ?? null
    },
    highlights: detail.highlights.map((event) => event.rawEvent() as NostrEvent),
    profiles,
    seo: buildPodcastSeo({
      url,
      id: detail.components.id,
      episodeTitle: detail.episodeTitle ?? '',
      showTitle: detail.showTitle ?? '',
      highlightCount: detail.highlights.length
    })
  };
};
