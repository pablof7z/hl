import type { PageServerLoad } from './$types';
import { buildHomeSeo } from '$lib/seo';

export const load: PageServerLoad = async ({ setHeaders, url }) => {
  setHeaders({
    'cache-control': 'public, max-age=60, s-maxage=300, stale-while-revalidate=3600'
  });

  return {
    seo: buildHomeSeo(url)
  };
};
