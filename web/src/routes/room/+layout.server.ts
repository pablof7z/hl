import type { LayoutServerLoad } from './$types';

export const load: LayoutServerLoad = () => {
  // Default to enabled; set PUBLIC_ROOM_UI_ENABLED=false to gate the Room UI.
  const rawFlag = import.meta.env.PUBLIC_ROOM_UI_ENABLED as string | undefined;
  const isRoomEnabled = rawFlag !== 'false';
  return { isRoomEnabled };
};
