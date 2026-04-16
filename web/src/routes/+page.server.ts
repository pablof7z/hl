import type { NostrEvent } from '@nostr-dev-kit/ndk';
import type { PageServerLoad } from './$types';
import { buildHomeSeo } from '$lib/seo';
import { fetchFrontPageArticles, fetchProfilesByPubkeys } from '$lib/server/nostr';

export const load: PageServerLoad = async ({ setHeaders, url }) => {
  setHeaders({
    'cache-control': 'public, max-age=60, s-maxage=300, stale-while-revalidate=3600'
  });

  try {
    const articles = await fetchFrontPageArticles(12);
    const profiles = await fetchProfilesByPubkeys(articles.map((event) => event.pubkey));

    return {
      articles: articles.map((event) => event.rawEvent() as NostrEvent),
      profiles,
      seo: buildHomeSeo(url)
    };
  } catch (error) {
    console.warn('Home SSR load failed', error);

    return {
      articles: [],
      profiles: {},
      seo: buildHomeSeo(url)
    };
  }
};
