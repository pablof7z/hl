import { browser } from '$app/environment';

/**
 * Auth-guarded surface — the `(protected)` layout already redirects
 * unauthenticated visitors to `/discover`. This load is intentionally
 * empty: SSR has no session, and the layout's $effect handles the
 * client-side redirect once sessions are restored.
 */
export const load = () => {
  if (!browser) return {};
  return {};
};
