import { json, type RequestHandler } from '@sveltejs/kit';
import type { ArtifactSource } from '$lib/ndk/artifacts';
import {
  ArtifactPreviewInputError,
  previewArtifactReference
} from '$lib/server/artifact-preview';

export const POST: RequestHandler = async ({ request, url }) => {
  let body: unknown;

  try {
    body = await request.json();
  } catch {
    return json({ error: 'Invalid JSON.' }, { status: 400 });
  }

  const rawReference =
    typeof body === 'object' && body && 'reference' in body
      ? String((body as { reference: unknown }).reference ?? '')
      : typeof body === 'object' && body && 'url' in body
        ? String((body as { url: unknown }).url ?? '')
        : '';
  const requestedSource =
    typeof body === 'object' && body && 'source' in body
      ? parseSource(String((body as { source: unknown }).source ?? ''))
      : undefined;

  if (!rawReference.trim()) {
    return json({ error: 'Paste a URL or a Nostr article reference.' }, { status: 400 });
  }

  try {
    return json(
      await previewArtifactReference({
        reference: rawReference,
        origin: url.origin,
        source: requestedSource
      })
    );
  } catch (error) {
    const message =
      error instanceof Error ? error.message : 'Could not preview that reference right now.';
    const status = error instanceof ArtifactPreviewInputError ? 400 : 502;
    return json({ error: message }, { status });
  }
};

function parseSource(value: string): ArtifactSource | undefined {
  if (
    value === 'article' ||
    value === 'book' ||
    value === 'podcast' ||
    value === 'video' ||
    value === 'paper' ||
    value === 'web'
  ) {
    return value;
  }

  return undefined;
}
