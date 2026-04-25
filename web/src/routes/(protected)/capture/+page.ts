import { redirect } from '@sveltejs/kit';
import { browser } from '$app/environment';

export const load = () => {
  // SSR has no session — let the layout handle the redirect.
  // On the client the layout's $effect handles it too, but returning
  // early here avoids a flash of the page before the $effect fires.
  if (!browser) return {};

  // Intentionally no async fetch: the layout already guards the route.
  return {};
};
