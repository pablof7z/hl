import type { LayoutServerLoad } from './$types';

export const load: LayoutServerLoad = ({ url }) => {
  // Enable via ?feature=room query param OR unless env var is explicitly 'false'.
  const enabledByQuery = url.searchParams.get('feature') === 'room';
  const rawFlag = process.env.PUBLIC_ROOM_UI_ENABLED;
  const enabledByEnv = rawFlag !== 'false';

  return { isRoomEnabled: enabledByEnv || enabledByQuery };
};
