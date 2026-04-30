/**
 * Kindle `My Clippings.txt` parser.
 *
 * Format (English / default locale):
 *
 *   <Title> (<Author>)
 *   - Your Highlight on Location 1234-1245 | Added on Monday, March 1, 2021 12:00:00 AM
 *
 *   <quote text — possibly multiple lines>
 *   ==========
 *
 * Variants we handle:
 *   - "Your Highlight on Page 12" — kept (locationLabel becomes "Page 12")
 *   - "Your Note on …"            — skipped (counted as note)
 *   - "Your Bookmark on …"        — skipped (counted as bookmark)
 *   - Title with no author parens
 *   - Author "Lastname, Firstname" (comma in author kept verbatim)
 *   - BOM at start of file (stripped)
 *
 * Pure function, no I/O. Safe for use in browser or server.
 */

export type ParsedClipping = {
  /** Stable hash of `title|author|locationLabel|quote`. */
  id: string;
  title: string;
  author?: string;
  kind: 'highlight';
  /** Human label like "Location 1234-1245" or "Page 12". */
  locationLabel: string;
  addedAt: Date;
  /** Cleaned quote: collapsed internal whitespace, stripped trailing whitespace. */
  quote: string;
};

export type ParseSummary = {
  entries: ParsedClipping[];
  skipped: {
    notes: number;
    bookmarks: number;
    malformed: number;
  };
};

const ENTRY_DELIMITER = /\r?\n==========\r?\n/;

/**
 * Parse the contents of a Kindle `My Clippings.txt` file.
 *
 * Never throws on malformed entries — they are counted in
 * `skipped.malformed` and otherwise ignored.
 */
export function parseKindleClippings(raw: string): ParseSummary {
  const summary: ParseSummary = {
    entries: [],
    skipped: { notes: 0, bookmarks: 0, malformed: 0 }
  };

  if (!raw) return summary;

  // Strip UTF-8 BOM if present.
  let text = raw.charCodeAt(0) === 0xfeff ? raw.slice(1) : raw;

  // Some Kindle exports leave a final delimiter without trailing newline,
  // which the regex won't match. Normalise tail so the split picks it up.
  text = text.replace(/\r?\n==========\s*$/, '\n==========\n');

  const blocks = text.split(ENTRY_DELIMITER);
  for (const rawBlock of blocks) {
    const block = rawBlock.replace(/^\s+|\s+$/g, '');
    if (!block) continue;

    const lines = block.split(/\r?\n/);
    if (lines.length < 2) {
      summary.skipped.malformed += 1;
      continue;
    }

    const headerLine = lines[0]?.trim() ?? '';
    const metaLine = lines[1]?.trim() ?? '';
    if (!headerLine || !metaLine) {
      summary.skipped.malformed += 1;
      continue;
    }

    const { title, author } = parseHeaderLine(headerLine);
    if (!title) {
      summary.skipped.malformed += 1;
      continue;
    }

    const meta = parseMetaLine(metaLine);
    if (!meta) {
      summary.skipped.malformed += 1;
      continue;
    }

    if (meta.kind === 'note') {
      summary.skipped.notes += 1;
      continue;
    }
    if (meta.kind === 'bookmark') {
      summary.skipped.bookmarks += 1;
      continue;
    }

    // Quote body is everything from line 2 onward (after a typical blank line).
    const quoteRaw = lines.slice(2).join('\n');
    const quote = cleanQuote(quoteRaw);
    if (!quote) {
      summary.skipped.malformed += 1;
      continue;
    }

    const entry: ParsedClipping = {
      id: stableHash([title, author ?? '', meta.locationLabel, quote].join('|')),
      title,
      author,
      kind: 'highlight',
      locationLabel: meta.locationLabel,
      addedAt: meta.addedAt,
      quote
    };
    summary.entries.push(entry);
  }

  return summary;
}

// ─── Header line ──────────────────────────────────────────────────────────────
// Matches "Title (Author)" or just "Title". Author may itself contain
// commas (e.g. "Eco, Umberto") so we capture greedily up to the LAST `(`.

function parseHeaderLine(line: string): { title: string; author?: string } {
  const trimmed = line.replace(/^﻿/, '').trim();
  const lastOpen = trimmed.lastIndexOf('(');
  const lastClose = trimmed.lastIndexOf(')');

  if (lastOpen > 0 && lastClose === trimmed.length - 1 && lastClose > lastOpen) {
    const title = trimmed.slice(0, lastOpen).trim();
    const author = trimmed.slice(lastOpen + 1, lastClose).trim();
    return { title, author: author || undefined };
  }

  return { title: trimmed };
}

// ─── Meta line ────────────────────────────────────────────────────────────────
// Examples:
//   - Your Highlight on Location 1234-1245 | Added on Monday, March 1, 2021 12:00:00 AM
//   - Your Highlight on page 12 | Added on Sunday, June 2, 2024 1:00:00 AM
//   - Your Note on Location 245 | Added on …
//   - Your Bookmark on Location 99 | Added on …

type MetaInfo = {
  kind: 'highlight' | 'note' | 'bookmark';
  locationLabel: string;
  addedAt: Date;
};

function parseMetaLine(line: string): MetaInfo | undefined {
  const lower = line.toLowerCase();

  let kind: MetaInfo['kind'];
  if (lower.includes('your highlight')) kind = 'highlight';
  else if (lower.includes('your note')) kind = 'note';
  else if (lower.includes('your bookmark')) kind = 'bookmark';
  else return undefined;

  // Split on " | Added on " — robust against odd spacing.
  const splitMatch = line.split(/\s\|\s*Added on\s+/i);
  const left = splitMatch[0] ?? '';
  const right = splitMatch[1] ?? '';

  const locationLabel = extractLocationLabel(left);
  const addedAt = parseAddedDate(right);

  return {
    kind,
    locationLabel,
    addedAt
  };
}

/** Capture the "Location 1234-1245" / "Page 12" piece. */
function extractLocationLabel(left: string): string {
  // Strip the leading "- Your Highlight on " (or note/bookmark) prefix.
  const stripped = left.replace(/^[-\s]*Your\s+\w+\s+on\s+/i, '').trim();
  if (!stripped) return '';

  // Capitalise the kind word ("location" → "Location", "page" → "Page").
  return stripped.replace(/^([a-z])/, (m) => m.toUpperCase());
}

function parseAddedDate(right: string): Date {
  if (!right) return new Date(0);
  // "Monday, March 1, 2021 12:00:00 AM" — Date.parse handles this on V8.
  const cleaned = right.trim().replace(/\s+/g, ' ');
  const ms = Date.parse(cleaned);
  if (Number.isFinite(ms)) return new Date(ms);

  // Fallback: drop the leading weekday and try again.
  const noWeekday = cleaned.replace(/^\w+,\s*/, '');
  const ms2 = Date.parse(noWeekday);
  if (Number.isFinite(ms2)) return new Date(ms2);

  return new Date(0);
}

// ─── Quote cleaning ───────────────────────────────────────────────────────────

function cleanQuote(raw: string): string {
  if (!raw) return '';
  // Replace any whitespace run (including newlines) with a single space,
  // then trim ends. Kindle wraps long quotes; rejoining is what users want.
  const collapsed = raw.replace(/\s+/g, ' ').trim();
  return collapsed;
}

// ─── Stable hash (FNV-1a 32-bit, hex-encoded) ─────────────────────────────────
// Deterministic across runs; non-cryptographic; small bundle footprint.
// Sufficient for client-side dedup of clipping entries.

function stableHash(input: string): string {
  let hash = 0x811c9dc5;
  for (let i = 0; i < input.length; i++) {
    hash ^= input.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return hash.toString(16).padStart(8, '0');
}
