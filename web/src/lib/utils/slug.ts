/// Build a URL-safe kebab slug from arbitrary text. Strips diacritics,
/// non-alphanumerics, collapses whitespace, lowercases, and truncates so
/// the slug component of a public URL stays readable.
export function slugify(value: string | null | undefined, maxLength = 80): string {
  if (typeof value !== 'string') return '';

  const normalized = value
    .normalize('NFKD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/['']/g, '')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');

  if (!normalized) return '';
  if (normalized.length <= maxLength) return normalized;

  // Try to break at a word boundary near the limit so we don't end on a
  // partial word — fall back to a hard cut if the limit lands inside a
  // long unbroken run.
  const slice = normalized.slice(0, maxLength);
  const lastDash = slice.lastIndexOf('-');
  if (lastDash > maxLength * 0.6) {
    return slice.slice(0, lastDash);
  }
  return slice;
}

/// Combine a slug + identifier as `<slug>-<id>` for SEO-friendly URLs,
/// degrading gracefully to bare `<id>` when no slug is available.
export function withSlug(slug: string, id: string): string {
  const safeSlug = slugify(slug);
  return safeSlug ? `${safeSlug}-${id}` : id;
}
