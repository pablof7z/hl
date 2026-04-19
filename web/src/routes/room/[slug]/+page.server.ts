import type { PageServerLoad } from './$types';

export const load: PageServerLoad = ({ url }) => {
  const featureFlag = url.searchParams.get('feature');
  const isRoomEnabled = featureFlag === 'room';
  return { isRoomEnabled };
};
