import { cleanText } from './format';

/**
 * NIP-11 relay information document.
 * Fields beyond the spec (`software`, `version`, `fees`, ...) are passed through verbatim.
 */
export type Nip11Document = {
  name?: string;
  description?: string;
  icon?: string;
  banner?: string;
  pubkey?: string;
  contact?: string;
  software?: string;
  version?: string;
  supported_nips?: number[];
  limitation?: Record<string, unknown>;
  fees?: Record<string, unknown>;
  posting_policy?: string;
  privacy_policy?: string;
  terms_of_service?: string;
};

export type Nip11ProbeResult = {
  url: string;
  document?: Nip11Document;
  error?: string;
};

const PROBE_TIMEOUT_MS = 6_000;

export function relayHttpUrl(relayUrl: string): string | undefined {
  const cleaned = cleanText(relayUrl);
  if (!cleaned) return undefined;

  if (/^wss:\/\//i.test(cleaned)) return cleaned.replace(/^wss:\/\//i, 'https://');
  if (/^ws:\/\//i.test(cleaned)) return cleaned.replace(/^ws:\/\//i, 'http://');
  if (/^https?:\/\//i.test(cleaned)) return cleaned;

  return `https://${cleaned}`;
}

export async function probeRelayNip11(
  relayUrl: string,
  options: { signal?: AbortSignal; timeoutMs?: number } = {}
): Promise<Nip11ProbeResult> {
  const cleaned = cleanText(relayUrl);
  const httpUrl = relayHttpUrl(cleaned);

  if (!httpUrl) {
    return { url: cleaned, error: 'Invalid relay URL' };
  }

  const timeoutMs = options.timeoutMs ?? PROBE_TIMEOUT_MS;
  const controller = new AbortController();
  const externalSignal = options.signal;
  if (externalSignal) {
    if (externalSignal.aborted) controller.abort(externalSignal.reason);
    else externalSignal.addEventListener('abort', () => controller.abort(externalSignal.reason));
  }

  const timer = setTimeout(() => controller.abort(new Error('NIP-11 probe timeout')), timeoutMs);

  try {
    const response = await fetch(httpUrl, {
      headers: { Accept: 'application/nostr+json' },
      signal: controller.signal,
      redirect: 'follow'
    });

    if (!response.ok) {
      return { url: cleaned, error: `HTTP ${response.status}` };
    }

    const document = (await response.json()) as Nip11Document;
    return { url: cleaned, document };
  } catch (error) {
    return {
      url: cleaned,
      error: error instanceof Error ? error.message : 'Probe failed'
    };
  } finally {
    clearTimeout(timer);
  }
}

export function nip11SupportsNip(document: Nip11Document | undefined, nip: number): boolean {
  return document?.supported_nips?.includes(nip) ?? false;
}

const curatorPubkeyCache = new Map<string, string>();

/**
 * Fetch the operator pubkey advertised in a relay's NIP-11 document.
 * Result is cached in-memory for the duration of the server process.
 * Returns an empty string on failure so callers can treat it as "unknown".
 */
export async function fetchRelayCuratorPubkey(relayUrl: string): Promise<string> {
  const cleaned = cleanText(relayUrl);
  if (!cleaned) return '';

  const cached = curatorPubkeyCache.get(cleaned);
  if (cached !== undefined) return cached;

  const result = await probeRelayNip11(cleaned, { timeoutMs: 5_000 });
  const pubkey = cleanText(result.document?.pubkey ?? '');
  curatorPubkeyCache.set(cleaned, pubkey);
  return pubkey;
}
