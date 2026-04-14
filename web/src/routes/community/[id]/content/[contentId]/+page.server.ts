import type { PageServerLoad } from './$types';
import { fetchArtifactForGroup } from '$lib/server/artifacts';
import { fetchCommunityById } from '$lib/server/communities';

export const load: PageServerLoad = async ({ params, setHeaders }) => {
  setHeaders({
    'cache-control': 'public, max-age=30, s-maxage=120, stale-while-revalidate=600'
  });

  const [community, artifact] = await Promise.all([
    fetchCommunityById(params.id),
    fetchArtifactForGroup(params.id, params.contentId)
  ]);

  return {
    community,
    artifact,
    groupId: params.id,
    contentId: params.contentId,
    missing: !community || !artifact
  };
};
