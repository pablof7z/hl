export const APP_NAME = 'Highlighter';
export const APP_TAGLINE =
  'Nostr-native reading communities built around sources, highlights, and discussion.';

const FALLBACK_PUBLIC_RELAYS = [
  'wss://relay.damus.io',
  'wss://purplepag.es',
  'wss://relay.primal.net'
];

export const HIGHLIGHTER_RELAY_URL = parseRelayUrl(
  import.meta.env.PUBLIC_HIGHLIGHTER_RELAY_URL as string | undefined,
  'wss://relay.highlighter.com'
);

export const DEFAULT_RELAYS = uniqueRelayList([
  HIGHLIGHTER_RELAY_URL,
  ...parseRelayList(
    import.meta.env.PUBLIC_NOSTR_RELAYS as string | undefined,
    FALLBACK_PUBLIC_RELAYS
  )
]);

export const GROUP_RELAY_URLS = [HIGHLIGHTER_RELAY_URL];

function parseRelayList(value: string | undefined, fallback: string[]): string[] {
  if (!value) return fallback;

  return uniqueRelayList(
    value
      .split(',')
      .map((relay) => relay.trim())
      .filter(Boolean)
  );
}

function parseRelayUrl(value: string | undefined, fallback: string): string {
  return parseRelayList(value, [fallback])[0] ?? fallback;
}

function uniqueRelayList(relays: readonly string[]): string[] {
  return [...new Set(relays.map((relay) => relay.trim()).filter(Boolean))];
}
