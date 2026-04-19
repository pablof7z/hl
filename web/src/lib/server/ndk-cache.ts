import type { NDKCacheAdapter } from '@nostr-dev-kit/ndk';
import NDKCacheUpstashAdapter from '@nostr-dev-kit/cache-upstash';

const SSR_CACHE_NAMESPACE = 'highlighter:ssr:v1';
const SSR_CACHE_TTL_SECONDS = 60 * 60;

let cacheAdapter: NDKCacheAdapter | undefined;
let warnedMissingCredentials = false;

type UpstashCredentials = {
  url: string;
  token: string;
};

export function getServerCacheAdapter(): NDKCacheAdapter | undefined {
  const credentials = getUpstashCredentials();

  if (!credentials) {
    if (!warnedMissingCredentials && process.env.NODE_ENV !== 'development') {
      warnedMissingCredentials = true;
      console.warn(
        'NDK SSR cache is disabled. Set UPSTASH_REDIS_REST_URL/UPSTASH_REDIS_REST_TOKEN or KV_REST_API_URL/KV_REST_API_TOKEN.'
      );
    }

    return undefined;
  }

  cacheAdapter ??= new NDKCacheUpstashAdapter({
    url: credentials.url,
    token: credentials.token,
    namespace: process.env.NDK_SSR_CACHE_NAMESPACE || SSR_CACHE_NAMESPACE,
    expirationTime:
      Number.parseInt(process.env.NDK_SSR_CACHE_TTL_SECONDS || '', 10) || SSR_CACHE_TTL_SECONDS
  });

  return cacheAdapter;
}

function getUpstashCredentials(): UpstashCredentials | undefined {
  const url = process.env.UPSTASH_REDIS_REST_URL || process.env.KV_REST_API_URL;
  const token = process.env.UPSTASH_REDIS_REST_TOKEN || process.env.KV_REST_API_TOKEN;

  if (!url || !token) return undefined;

  return { url, token };
}
