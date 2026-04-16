export const APP_NAME = 'Highlighter';
export const APP_TAGLINE = 'A calm place to read, highlight, and discuss great writing.';

const HIGHLIGHTER_RELAY = 'wss://relay-highlighter.f7z.io';

const FALLBACK_RELAYS = [
  HIGHLIGHTER_RELAY,
  'wss://relay.damus.io',
  'wss://purplepag.es',
  'wss://relay.primal.net'
];

export const DEFAULT_RELAYS = parseRelayList(
  import.meta.env.PUBLIC_HIGHLIGHTER_RELAY as string | undefined,
  FALLBACK_RELAYS
);

function parseRelayList(value: string | undefined, fallback: string[]): string[] {
  if (!value) return fallback;

  const parsed = value
    .split(',')
    .map((relay) => relay.trim())
    .filter(Boolean);

  return parsed.length > 0 ? parsed : fallback;
}
