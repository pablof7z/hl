import type { NostrEvent } from '@nostr-dev-kit/ndk';
import type { PageServerLoad } from './$types';
import { fetchRecentArticlesByAuthor, fetchUserWithProfile } from '$lib/server/nostr';
import { buildMissingSeo, buildProfileSeo } from '$lib/seo';

export const load: PageServerLoad = async ({ params, setHeaders, url }) => {
  setHeaders({
    'cache-control': 'public, max-age=60, s-maxage=300, stale-while-revalidate=3600'
  });

  try {
    const { user, profile } = await fetchUserWithProfile(params.identifier);

    if (!user) {
      return {
        missing: true,
        identifier: params.identifier,
        seo: buildMissingSeo(url, 'Profile not found')
      };
    }

    const articles = await fetchRecentArticlesByAuthor(user.pubkey, 12);

    return {
      missing: false,
      identifier: params.identifier,
      pubkey: user.pubkey,
      npub: user.npub,
      profile,
      seedArticles: articles.map((event) => event.rawEvent() as NostrEvent),
      seo: buildProfileSeo({
        url,
        pubkey: user.pubkey,
        profile
      })
    };
  } catch (error) {
    console.warn('Profile SSR load failed', error);

    return {
      missing: true,
      identifier: params.identifier,
      seo: buildMissingSeo(url, 'Profile unavailable')
    };
  }
};
