/**
 * Format a Unix timestamp (seconds) as a compact relative-time string.
 * Examples: "just now", "5m", "2h", "3d", "Apr 12"
 */
export function relativeTime(timestamp: number | undefined | null): string {
  if (!timestamp) return '';
  const now = Math.floor(Date.now() / 1000);
  const diff = now - timestamp;

  if (diff < 60) return 'just now';
  if (diff < 3600) return `${Math.floor(diff / 60)}m`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h`;
  if (diff < 604800) return `${Math.floor(diff / 86400)}d`;

  return new Intl.DateTimeFormat('en', { month: 'short', day: 'numeric' }).format(
    new Date(timestamp * 1000)
  );
}
