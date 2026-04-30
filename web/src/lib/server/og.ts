import { access, writeFile } from 'node:fs/promises';
import type { NDKUserProfile, NostrEvent } from '@nostr-dev-kit/ndk';
import sharp from 'sharp';
import { APP_NAME } from '$lib/ndk/config';
import interFontDataUrl from './fonts/InterVariable.ttf?inline';
import {
  articlePublishedAt,
  articleReadTimeMinutes,
  articleSummary,
  cleanText,
  displayName,
  formatDisplayDate,
  noteExcerpt,
  noteTitle
} from '$lib/ndk/format';

const WIDTH = 1200;
const HEIGHT = 630;
const FONT_PATH = '/tmp/ndk-og-inter.ttf';

type OgRenderArgs = {
  event?: NostrEvent;
  authorPubkey?: string;
  profile?: NDKUserProfile;
};

type Palette = {
  paper: string;
  panel: string;
  ink: string;
  muted: string;
  accent: string;
  accentSoft: string;
  border: string;
  badgeText: string;
};

const PALETTES: Palette[] = [
  {
    paper: '#f6efe6',
    panel: '#fffaf3',
    ink: '#24180f',
    muted: '#695747',
    accent: '#d05a2d',
    accentSoft: '#f0c7b5',
    border: '#e8d8cb',
    badgeText: '#fff8f2'
  },
  {
    paper: '#f0f3ed',
    panel: '#fbfdf8',
    ink: '#182217',
    muted: '#556454',
    accent: '#4c8b5b',
    accentSoft: '#c9ddcd',
    border: '#d7e2d6',
    badgeText: '#f8fff9'
  },
  {
    paper: '#f2f0eb',
    panel: '#fcfbf7',
    ink: '#171c23',
    muted: '#58616d',
    accent: '#4d7899',
    accentSoft: '#c8d9e6',
    border: '#dde3e8',
    badgeText: '#f6fbff'
  },
  {
    paper: '#f7efe9',
    panel: '#fffaf5',
    ink: '#241514',
    muted: '#6b4f4b',
    accent: '#b34a4a',
    accentSoft: '#edc7c2',
    border: '#ecd8d5',
    badgeText: '#fff8f8'
  }
];

export async function renderNoteOgImage(args: OgRenderArgs): Promise<Buffer> {
  const fontfile = await ensureInterFontFile();
  const event = args.event;
  const isArticle = event?.kind === 30023;
  const title = event ? noteTitle(event) : 'Note unavailable';
  const summary = event
    ? previewSnippet(
        isArticle ? articleSummary(event, 240) : noteExcerpt(event.content, 240),
        'A note shared over Nostr.'
      )
    : 'The requested post is not available right now.';
  const author = displayName(args.profile, 'Author');
  const published = event ? formatDisplayDate(articlePublishedAt(event)) : 'Unavailable';
  const metaBits = [author || 'Unknown author', published];

  if (event && isArticle) {
    metaBits.push(`${articleReadTimeMinutes(event.content)} min read`);
  }

  const palette = paletteFor(`${args.authorPubkey ?? ''}${title}`);
  const badge = isArticle ? 'ARTICLE' : 'NOTE';
  const titleLines = wrapText(title, 28, 3);
  const summaryLines = wrapText(summary, 58, 4);
  const titleBlockHeight = titleLines.length * 66;
  const summaryY = 258 + titleBlockHeight;
  const footerY = 548;
  const base = sharp(
    Buffer.from(`
      <svg width="${WIDTH}" height="${HEIGHT}" viewBox="0 0 ${WIDTH} ${HEIGHT}" fill="none" xmlns="http://www.w3.org/2000/svg">
        <rect width="${WIDTH}" height="${HEIGHT}" fill="${palette.paper}" />
        <circle cx="1054" cy="118" r="188" fill="${palette.accentSoft}" opacity="0.82" />
        <circle cx="1138" cy="586" r="176" fill="${palette.accentSoft}" opacity="0.42" />
        <circle cx="948" cy="508" r="96" fill="${palette.accentSoft}" opacity="0.22" />
        <rect x="44" y="40" width="1112" height="550" rx="34" fill="${palette.panel}" stroke="${palette.border}" stroke-width="2" />
        <rect x="44" y="40" width="18" height="550" rx="9" fill="${palette.accent}" />
        <rect x="956" y="70" width="150" height="46" rx="23" fill="${palette.accent}" />
        <rect x="108" y="${footerY - 36}" width="80" height="80" rx="40" fill="${palette.accent}" />
      </svg>
    `)
  );

  const overlays = await Promise.all([
    createTextOverlay({
      text: APP_NAME.toUpperCase(),
      color: palette.accent,
      font: 'Inter OG Bold 20',
      fontfile,
      left: 108,
      top: 72
    }),
    createTextOverlay({
      text: cleanText(metaBits.join(' • ')).toUpperCase(),
      color: palette.muted,
      font: 'Inter OG 16',
      fontfile,
      left: 108,
      top: 106
    }),
    createTextOverlay({
      text: badge,
      color: palette.badgeText,
      font: 'Inter OG Bold 20',
      fontfile,
      width: 150,
      align: 'center',
      left: 956,
      top: 79
    }),
    createTextOverlay({
      text: titleLines.join('\n'),
      color: palette.ink,
      font: 'Inter OG Bold 58',
      fontfile,
      width: 900,
      spacing: 6,
      left: 108,
      top: 168
    }),
    createTextOverlay({
      text: summaryLines.join('\n'),
      color: palette.muted,
      font: 'Inter OG 28',
      fontfile,
      width: 820,
      spacing: 4,
      left: 108,
      top: summaryY
    }),
    createTextOverlay({
      text: authorInitials(author),
      color: palette.badgeText,
      font: 'Inter OG Bold 30',
      fontfile,
      width: 80,
      align: 'center',
      left: 108,
      top: footerY - 10
    }),
    createTextOverlay({
      text: author || 'Unknown author',
      color: palette.ink,
      font: 'Inter OG Bold 28',
      fontfile,
      width: 560,
      left: 214,
      top: footerY - 4
    }),
    createTextOverlay({
      text: isArticle ? 'Long-form post on Nostr' : 'Nostr note preview',
      color: palette.muted,
      font: 'Inter OG 22',
      fontfile,
      width: 440,
      left: 214,
      top: footerY + 36
    })
  ]);

  return await base.composite(overlays).png().toBuffer();
}

type HighlightOgArgs = {
  event?: NostrEvent;
  authorPubkey?: string;
  profile?: NDKUserProfile;
  sourceTitle?: string;
};

/// Quote-centric OG card for `/highlight/<nevent>`. The quote is the
/// hero — set in oversized Inter, framed by a giant curly quote glyph
/// in the accent colour, with a thin accent rail down the left edge to
/// echo the iOS treatment. Author + source attribution sit in the
/// footer. Palette is selected from `PALETTES` based on a hash of the
/// content so cards have variety without per-render config.
export async function renderHighlightOgImage(args: HighlightOgArgs): Promise<Buffer> {
  const fontfile = await ensureInterFontFile();
  const event = args.event;
  const rawQuote = event ? cleanText(event.content ?? '') : '';
  const quote = rawQuote || 'Highlight unavailable';
  const author = displayName(args.profile, 'A reader');
  const sourceLine = args.sourceTitle
    ? `from ${args.sourceTitle}`
    : 'highlighted on Nostr';

  // Wrap so the quote dominates the canvas. We size between 4 and 6
  // lines depending on length to keep the typographic colour even.
  const palette = paletteFor(`${args.authorPubkey ?? ''}${quote}`);
  const { lines, fontSize, lineHeight } = layoutQuote(quote);
  const quoteBlockHeight = lines.length * lineHeight;
  const quoteTop = Math.round((HEIGHT - quoteBlockHeight) / 2) - 30;
  const footerY = 540;

  const base = sharp(
    Buffer.from(`
      <svg width="${WIDTH}" height="${HEIGHT}" viewBox="0 0 ${WIDTH} ${HEIGHT}" fill="none" xmlns="http://www.w3.org/2000/svg">
        <rect width="${WIDTH}" height="${HEIGHT}" fill="${palette.paper}" />
        <circle cx="1080" cy="92" r="220" fill="${palette.accentSoft}" opacity="0.65" />
        <circle cx="92" cy="538" r="180" fill="${palette.accentSoft}" opacity="0.40" />
        <rect x="44" y="40" width="1112" height="550" rx="28" fill="${palette.panel}" stroke="${palette.border}" stroke-width="2" />
        <rect x="44" y="40" width="14" height="550" rx="7" fill="${palette.accent}" />
        <rect x="956" y="70" width="170" height="46" rx="23" fill="${palette.accent}" />
        <rect x="108" y="${footerY - 12}" width="68" height="68" rx="34" fill="${palette.accent}" />
        <text x="100" y="200" fill="${palette.accent}" font-family="Georgia, 'Times New Roman', serif" font-size="240" font-weight="700" opacity="0.18">“</text>
        <text x="1080" y="${footerY - 30}" fill="${palette.accent}" font-family="Georgia, 'Times New Roman', serif" font-size="240" font-weight="700" opacity="0.18" text-anchor="end">”</text>
      </svg>
    `)
  );

  const overlays = await Promise.all([
    createTextOverlay({
      text: APP_NAME.toUpperCase(),
      color: palette.accent,
      font: 'Inter OG Bold 20',
      fontfile,
      left: 108,
      top: 72
    }),
    createTextOverlay({
      text: 'HIGHLIGHT',
      color: palette.badgeText,
      font: 'Inter OG Bold 20',
      fontfile,
      width: 170,
      align: 'center',
      left: 956,
      top: 79
    }),
    createTextOverlay({
      text: lines.join('\n'),
      color: palette.ink,
      font: `Inter OG Bold ${fontSize}`,
      fontfile,
      width: 940,
      spacing: 6,
      left: 130,
      top: quoteTop
    }),
    createTextOverlay({
      text: authorInitials(author),
      color: palette.badgeText,
      font: 'Inter OG Bold 26',
      fontfile,
      width: 68,
      align: 'center',
      left: 108,
      top: footerY + 6
    }),
    createTextOverlay({
      text: cleanText(author),
      color: palette.ink,
      font: 'Inter OG Bold 26',
      fontfile,
      width: 700,
      left: 196,
      top: footerY
    }),
    createTextOverlay({
      text: cleanText(sourceLine),
      color: palette.muted,
      font: 'Inter OG 20',
      fontfile,
      width: 700,
      left: 196,
      top: footerY + 36
    })
  ]);

  return await base.composite(overlays).png().toBuffer();
}

/// Pick a font size + line cap so the quote breathes. Short quotes get
/// the loudest treatment; longer ones step down so we never overflow
/// the panel.
function layoutQuote(quote: string): { lines: string[]; fontSize: number; lineHeight: number } {
  const text = cleanText(quote);
  const length = text.length;

  // Three tiers — tuned by eye against typical kind:9802 quote lengths.
  let fontSize: number;
  let maxChars: number;
  let maxLines: number;
  if (length <= 80) {
    fontSize = 64;
    maxChars = 22;
    maxLines = 4;
  } else if (length <= 160) {
    fontSize = 52;
    maxChars = 28;
    maxLines = 5;
  } else {
    fontSize = 42;
    maxChars = 36;
    maxLines = 6;
  }

  const lines = wrapText(text, maxChars, maxLines);
  const lineHeight = Math.round(fontSize * 1.18);
  return { lines, fontSize, lineHeight };
}

async function createTextOverlay(args: {
  text: string;
  color: string;
  font: string;
  fontfile: string;
  left: number;
  top: number;
  width?: number;
  align?: 'left' | 'centre' | 'center' | 'right';
  spacing?: number;
}): Promise<{ input: Buffer; left: number; top: number }> {
  const input = await sharp({
    text: {
      text: `<span foreground="${args.color}">${escapeXml(args.text)}</span>`,
      font: args.font,
      fontfile: args.fontfile,
      width: args.width,
      align: args.align,
      spacing: args.spacing,
      rgba: true,
      wrap: 'none'
    }
  })
    .png()
    .toBuffer();

  return {
    input,
    left: args.left,
    top: args.top
  };
}

async function ensureInterFontFile(): Promise<string> {
  try {
    await access(FONT_PATH);
    return FONT_PATH;
  } catch {
    const [, encoded] = interFontDataUrl.split(',', 2);
    await writeFile(FONT_PATH, Buffer.from(encoded ?? '', 'base64'));
    return FONT_PATH;
  }
}

function wrapText(input: string, maxChars: number, maxLines: number): string[] {
  const words = cleanText(input).split(' ').filter(Boolean);
  if (words.length === 0) return [''];

  const lines: string[] = [];
  let current = '';

  for (const word of words) {
    const next = current ? `${current} ${word}` : word;

    if (next.length <= maxChars) {
      current = next;
      continue;
    }

    if (current) {
      lines.push(current);
    }

    current = word;

    if (lines.length === maxLines - 1) {
      break;
    }
  }

  if (lines.length < maxLines && current) {
    lines.push(current);
  }

  const consumed = lines.join(' ');
  if (consumed.length < cleanText(input).length && lines.length > 0) {
    lines[lines.length - 1] = truncateLine(lines[lines.length - 1], Math.max(8, maxChars - 1));
  }

  return lines.slice(0, maxLines);
}

function authorInitials(name: string): string {
  const parts = cleanText(name)
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2);

  if (parts.length === 0) return 'N';

  return parts.map((part) => part[0]?.toUpperCase() ?? '').join('').slice(0, 2);
}

function paletteFor(seed: string): Palette {
  // `| 0` forces 32-bit integer math so long seeds (full highlight quotes)
  // don't overflow to Infinity and produce a NaN index.
  const hash = [...seed].reduce((total, char) => (total * 33 + char.charCodeAt(0)) | 0, 5381);
  return PALETTES[Math.abs(hash) % PALETTES.length];
}

function previewSnippet(value: string, fallback: string): string {
  const sanitized = cleanText(
    value
      .replace(/\(\s*https?:\/\/[^)]+\)/g, ' ')
      .replace(/https?:\/\/\S+/g, ' ')
      .replace(/\(\s*\)/g, ' ')
  );
  return sanitized || fallback;
}

function escapeXml(value: string): string {
  return value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#39;');
}

function truncateLine(value: string, maxLength: number): string {
  if (value.length <= maxLength) return value;
  return `${value.slice(0, Math.max(0, maxLength - 3)).trim()}...`;
}
