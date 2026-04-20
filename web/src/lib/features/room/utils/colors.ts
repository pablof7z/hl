const TINTS = [
  'var(--h-amber)',
  'var(--h-sage)',
  'var(--h-blue)',
  'var(--h-rose)',
  'var(--h-lilac)',
  'var(--h-amber-l)'
] as const;

export function memberTint(colorIndex: number): string {
  return TINTS[(((colorIndex - 1) % 6) + 6) % 6];
}
