/**
 * Bulk publisher for Kindle clipping imports.
 *
 * Each entry → one kind:9802 (NDKHighlight) event. Throttled to ~5/sec
 * so we don't slam relays. Yields per-entry status as it progresses,
 * letting the UI render a live progress bar.
 */

import type NDK from '@nostr-dev-kit/ndk';
import { NDKHighlight, NDKRelaySet } from '@nostr-dev-kit/ndk';
import type { ParsedClipping } from './kindleClippings';
import type { OpenLibraryMatch } from './openlibrarySearch';
import { resolveUserHighlightRelayUrls } from '$lib/ndk/highlights';

export type ResolvedClipping = {
  parsed: ParsedClipping;
  match: OpenLibraryMatch | null;
};

export type PublishStatus =
  | { id: string; state: 'queued' }
  | { id: string; state: 'publishing' }
  | { id: string; state: 'published'; eventId: string }
  | { id: string; state: 'failed'; error: string };

export type PublishOptions = {
  /** Maximum events per second. Defaults to 5. */
  rateLimit?: number;
  /** Optional abort signal — cancels the remaining queue. */
  signal?: AbortSignal;
};

/**
 * Publishes the supplied entries one by one, yielding a status update
 * for each transition (queued → publishing → published/failed).
 *
 * The caller drives consumption (e.g. with a `for await` loop) and
 * decides how to render progress. The generator returns nothing on
 * completion.
 */
export async function* publishKindleHighlights(
  ndk: NDK,
  entries: ResolvedClipping[],
  options: PublishOptions = {}
): AsyncGenerator<PublishStatus, void, void> {
  if (!ndk.signer) {
    throw new Error('Connect a signer before publishing highlights.');
  }
  if (entries.length === 0) return;

  const rateLimit = Math.max(1, options.rateLimit ?? 5);
  const minGapMs = Math.ceil(1000 / rateLimit);

  const currentUser = ndk.activeUser ?? (await ndk.signer.user());
  const relayUrls = await resolveUserHighlightRelayUrls(ndk, currentUser.pubkey);
  const relaySet = NDKRelaySet.fromRelayUrls(relayUrls, ndk);

  // Initial: emit a queued state for everything so the UI can render
  // the full list of pending items at zero progress.
  for (const entry of entries) {
    yield { id: entry.parsed.id, state: 'queued' };
  }

  let lastPublishAt = 0;
  for (const entry of entries) {
    if (options.signal?.aborted) {
      yield {
        id: entry.parsed.id,
        state: 'failed',
        error: 'Cancelled before publish.'
      };
      continue;
    }

    const now = Date.now();
    const wait = Math.max(0, lastPublishAt + minGapMs - now);
    if (wait > 0) {
      await new Promise<void>((resolve) => setTimeout(resolve, wait));
    }
    lastPublishAt = Date.now();

    yield { id: entry.parsed.id, state: 'publishing' };

    try {
      const event = buildHighlightEvent(ndk, entry);
      await event.sign();
      await event.publish(relaySet);
      yield {
        id: entry.parsed.id,
        state: 'published',
        eventId: event.id
      };
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Unknown error';
      yield { id: entry.parsed.id, state: 'failed', error: message };
    }
  }
}

/**
 * Build a kind:9802 event for a single resolved clipping.
 *
 * Tag schema:
 *   - content: cleaned quote
 *   - ["i", "isbn:<isbn13>"]      when an ISBN match was found
 *   - ["title", title]            (always, helps SSR / readers without ISBN)
 *   - ["author", author]          when present (always, kept even with ISBN)
 *   - ["alt", "<NIP-31 fallback>"]
 *   - ["client", "highlighter-web", "kindle-import"]
 *
 * Decision: we always include `title` and `author` tags — even when
 * an ISBN is resolved — because they preserve the user's literal
 * Kindle metadata (which may differ from Open Library's canonical
 * casing). For unmatched books they're the only attribution we have.
 */
export function buildHighlightEvent(ndk: NDK, entry: ResolvedClipping): NDKHighlight {
  const event = new NDKHighlight(ndk);
  event.content = entry.parsed.quote;

  const tags: string[][] = [];

  if (entry.match?.isbn13) {
    tags.push(['i', `isbn:${entry.match.isbn13}`]);
  }

  if (entry.parsed.title) {
    tags.push(['title', entry.parsed.title]);
  }
  if (entry.parsed.author) {
    tags.push(['author', entry.parsed.author]);
  }

  tags.push(['alt', buildAltTag(entry.parsed)]);
  tags.push(['client', 'highlighter-web', 'kindle-import']);

  event.tags = tags;
  return event;
}

function buildAltTag(parsed: ParsedClipping): string {
  const author = parsed.author ? ` by ${parsed.author}` : '';
  return `A Kindle highlight from ${parsed.title}${author}`;
}
