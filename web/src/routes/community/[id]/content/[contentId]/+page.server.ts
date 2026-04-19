import { redirect } from '@sveltejs/kit';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = ({ params, url }) => {
  const slug = encodeURIComponent(params.id);
  const search = url.search;
  redirect(301, `/room/${slug}${search}`);
};
