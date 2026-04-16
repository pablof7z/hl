// @ts-nocheck
import type { PageServerLoad } from './$types';
import { buildAboutSeo } from '$lib/seo';

export const load = ({ url }: Parameters<PageServerLoad>[0]) => {
  return {
    seo: buildAboutSeo(url)
  };
};
