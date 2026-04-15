import changelog from '$lib/changelog.json';

export function load() {
  return { commits: changelog as { hash: string; shortHash: string; message: string; date: string }[] };
}
