// @ts-nocheck
import type { PageServerLoad } from './$types';

export const load = async ({ setHeaders }: Parameters<PageServerLoad>[0]) => {
  setHeaders({
    'cache-control': 'public, max-age=60, s-maxage=300, stale-while-revalidate=3600'
  });
};
